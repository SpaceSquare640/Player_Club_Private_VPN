//! Connection manager — owns the shared UDP socket and the negotiated peer
//! across signaling (C3) and the eventual handshake/data plane (C4/C5).
//!
//! The socket is bound once and its local candidates are gathered once, so the
//! NAT mapping STUN observed is the exact one peers will punch toward.

use std::io;
use std::net::{Ipv4Addr, SocketAddr};
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use tauri::async_runtime::{self, JoinHandle};
use tokio::sync::watch;

use super::crypto::identity::fingerprint_of;
use super::crypto::Identity;
use super::nat::candidate::{self, Candidate};
use super::pipeline;
use super::telemetry::TelemetrySink;
use super::transport::UdpTransport;
use super::tun::{privilege, TunConfig};

/// The point-to-point virtual LAN a connected pair shares (C5). Each side takes
/// a distinct host address by role so in-subnet traffic routes to the other.
fn dataplane_source_for(role: Role) -> pipeline::DataPlaneSource {
    // A real data plane needs a real adapter → Windows + elevation. Otherwise
    // the link stays control-only (encrypted keepalive).
    if !privilege::status().can_create_tun {
        return pipeline::DataPlaneSource::None;
    }
    let virtual_ip = match role {
        Role::Initiator => Ipv4Addr::new(10, 77, 0, 1),
        Role::Responder => Ipv4Addr::new(10, 77, 0, 2),
        Role::Idle => return pipeline::DataPlaneSource::None,
    };
    pipeline::DataPlaneSource::Adapter(TunConfig {
        virtual_ip,
        ..TunConfig::default()
    })
}

/// User-configurable connection-time settings (Phase B.3). Applied once, at
/// [`ConnectionManager::connect`] — **not** retroactively to an already-live
/// link. Live toggling would need a control channel into the running pipeline
/// task and is deferred (see the [0.15.0] changelog entry).
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionSettings {
    /// Forward broadcast traffic (LAN discovery) into the tunnel.
    #[serde(default = "default_true")]
    pub forward_broadcast: bool,
    /// Forward multicast traffic (LAN discovery) into the tunnel.
    #[serde(default = "default_true")]
    pub forward_multicast: bool,
    /// FEC parity shards per group of 8 data packets — recovers up to this many
    /// losses per group, at a `shards/8` bandwidth cost. `RsEncoder::new` clamps
    /// this to `1..=16` regardless of what is supplied here.
    #[serde(default = "default_fec_parity_shards")]
    pub fec_parity_shards: u8,
}

fn default_true() -> bool {
    true
}

fn default_fec_parity_shards() -> u8 {
    1
}

impl Default for ConnectionSettings {
    /// Matches the values that were hardcoded before Phase B.3 — so a caller
    /// that does not set these gets the exact prior behaviour.
    fn default() -> Self {
        Self {
            forward_broadcast: true,
            forward_multicast: true,
            fec_parity_shards: 1,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Role {
    #[default]
    Idle,
    Initiator,
    Responder,
}

impl Role {
    fn as_str(&self) -> &'static str {
        match self {
            Role::Idle => "idle",
            Role::Initiator => "initiator",
            Role::Responder => "responder",
        }
    }
}

/// Lifecycle of the live peer link (distinct from signaling [`Role`]).
///
/// `Idle` (never connected / disconnected) and `Failed` (handshake timed out)
/// both permit a fresh [`ConnectionManager::connect`]; `Connecting` and
/// `Connected` reject a second attempt so only one session can exist per peer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum LinkState {
    #[default]
    Idle,
    Connecting,
    Connected,
    Failed,
}

/// The running peer-link task and its cancellation handle. Dropping/aborting it
/// (or signalling `cancel`) tears the link down; the single [`CryptoSession`]
/// lives inside the task, so teardown is also session teardown.
struct Active {
    cancel: watch::Sender<bool>,
    handle: JoinHandle<()>,
}

/// The remote peer negotiated via signaling, ready for C4 to connect.
pub struct NegotiatedPeer {
    pub public_key: Vec<u8>,
    pub candidates: Vec<SocketAddr>,
}

#[derive(Default)]
struct Inner {
    socket: Option<UdpTransport>,
    local_candidates: Vec<Candidate>,
    sid: Option<String>,
    role: Role,
    peer: Option<NegotiatedPeer>,
}

/// Shared connection state (managed by Tauri).
#[derive(Default)]
pub struct ConnectionManager {
    inner: Mutex<Inner>,
    /// Live link lifecycle, shared with the running pipeline task (which writes
    /// the Connecting → Connected/Failed/Idle transitions) and read by snapshots.
    link: Arc<Mutex<LinkState>>,
    /// The in-flight/established link task, if any.
    active: Mutex<Option<Active>>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PeerSnapshot {
    pub peer_address: String,
    pub candidate_count: usize,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionSnapshot {
    pub role: String,
    pub local_candidate_count: usize,
    pub link: LinkState,
    pub peer: Option<PeerSnapshot>,
}

impl ConnectionManager {
    /// Bind the shared socket and gather candidates once (idempotent).
    pub async fn ensure_socket(&self, stun_server: &str) -> io::Result<()> {
        if self.inner.lock().unwrap().socket.is_some() {
            return Ok(());
        }
        // Bind + STUN without holding the lock across the await points.
        let transport = UdpTransport::bind(0).await?;
        let cands = candidate::gather(&transport, stun_server).await;

        let mut guard = self.inner.lock().unwrap();
        if guard.socket.is_none() {
            guard.socket = Some(transport);
            guard.local_candidates = cands;
        }
        Ok(())
    }

    pub fn local_candidates(&self) -> Vec<Candidate> {
        self.inner.lock().unwrap().local_candidates.clone()
    }

    /// Record that we created an offer with this session id (we are initiator).
    pub fn begin_offer(&self, sid: String) {
        let mut g = self.inner.lock().unwrap();
        g.sid = Some(sid);
        g.role = Role::Initiator;
        g.peer = None;
    }

    /// Record that we accepted an offer with this session id (we are responder).
    pub fn begin_answer(&self, sid: String) {
        let mut g = self.inner.lock().unwrap();
        g.sid = Some(sid);
        g.role = Role::Responder;
    }

    pub fn set_peer(&self, peer: NegotiatedPeer) {
        self.inner.lock().unwrap().peer = Some(peer);
    }

    /// The session id of our pending offer/answer, if any.
    pub fn session_id(&self) -> Option<String> {
        self.inner.lock().unwrap().sid.clone()
    }

    /// The current live-link state.
    pub fn link_state(&self) -> LinkState {
        *self.link.lock().unwrap()
    }

    /// Begin connecting to the negotiated peer (C4 hole-punch-as-handshake).
    ///
    /// Spawns a single [`pipeline::run`] task for our [`Role`] and hands it a
    /// `watch` cancellation channel. Rejected if a link is already `Connecting`
    /// or `Connected` (one session per peer); a prior `Idle`/`Failed` task is
    /// torn down first. Returns once the task is launched — progress is reported
    /// asynchronously via the telemetry sink and [`link_state`].
    ///
    /// [`link_state`]: ConnectionManager::link_state
    pub fn connect(
        &self,
        identity: Arc<Identity>,
        sink: Box<dyn TelemetrySink>,
        settings: ConnectionSettings,
    ) -> Result<(), String> {
        // One session per peer: refuse if a link is already coming up / live.
        if matches!(self.link_state(), LinkState::Connecting | LinkState::Connected) {
            return Err("a peer connection is already active".into());
        }

        // Snapshot the negotiated context without holding the lock across spawn.
        let (socket, role, peer_public, mut peer_candidates) = {
            let g = self.inner.lock().unwrap();
            let socket = g
                .socket
                .clone()
                .ok_or("no socket — create or accept an offer first")?;
            let role = g.role;
            let peer = g
                .peer
                .as_ref()
                .ok_or("no negotiated peer — exchange an offer/answer first")?;
            (socket, role, peer.public_key.clone(), peer.candidates.clone())
        };
        if role == Role::Idle {
            return Err("no role — create or accept an offer first".into());
        }
        // De-duplicate candidate paths (host and reflexive coincide without NAT).
        peer_candidates.sort();
        peer_candidates.dedup();
        if peer_candidates.is_empty() {
            return Err("peer advertised no candidates to punch".into());
        }

        let (cancel, cancel_rx) = watch::channel(false);
        let link = self.link.clone();

        // Replace any finished prior task, set Connecting, then launch.
        let mut active = self.active.lock().unwrap();
        if let Some(old) = active.take() {
            let _ = old.cancel.send(true);
            old.handle.abort();
        }
        *self.link.lock().unwrap() = LinkState::Connecting;
        let handle = async_runtime::spawn(pipeline::run(
            socket,
            role,
            identity,
            peer_public,
            peer_candidates,
            dataplane_source_for(role),
            settings,
            sink,
            link,
            cancel_rx,
        ));
        *active = Some(Active { cancel, handle });
        Ok(())
    }

    /// Tear down the live peer link (cancel the task, reset to `Idle`).
    /// Idempotent: a no-op when nothing is connected.
    pub fn disconnect(&self) {
        let mut active = self.active.lock().unwrap();
        if let Some(a) = active.take() {
            let _ = a.cancel.send(true);
            a.handle.abort();
        }
        *self.link.lock().unwrap() = LinkState::Idle;
    }

    pub fn snapshot(&self) -> ConnectionSnapshot {
        let link = self.link_state();
        let g = self.inner.lock().unwrap();
        ConnectionSnapshot {
            role: g.role.as_str().to_string(),
            local_candidate_count: g.local_candidates.len(),
            link,
            peer: g.peer.as_ref().map(|p| PeerSnapshot {
                peer_address: fingerprint_of(&p.public_key),
                candidate_count: p.candidates.len(),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A regression guard: before Phase B.3 these values were hardcoded
    /// (`forward_broadcast`/`forward_multicast` always `true` in
    /// `SplitPolicy::from_tun`, `r` always `1` in `pipeline.rs`). A caller that
    /// omits `settings` — or a stale JS blob missing these fields — must
    /// reproduce that exact prior behaviour, not silently change it.
    #[test]
    fn connection_settings_default_matches_pre_b3_hardcoded_behaviour() {
        let s = ConnectionSettings::default();
        assert!(s.forward_broadcast);
        assert!(s.forward_multicast);
        assert_eq!(s.fec_parity_shards, 1);
    }
}
