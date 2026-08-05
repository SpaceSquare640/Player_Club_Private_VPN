//! Connection manager — owns the shared UDP socket and the negotiated peer
//! across signaling (C3) and the eventual handshake/data plane (C4/C5).
//!
//! The socket is bound once and its local candidates are gathered once, so the
//! NAT mapping STUN observed is the exact one peers will punch toward.

use std::collections::HashMap;
use std::io;
use std::net::{Ipv4Addr, SocketAddr};
use std::sync::{Arc, Mutex};

use base64::engine::general_purpose::STANDARD;
use base64::Engine;
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
#[derive(Clone)]
pub struct NegotiatedPeer {
    pub public_key: Vec<u8>,
    pub candidates: Vec<SocketAddr>,
}

/// Identifies a peer link independent of how it was negotiated — the base64
/// encoding of the peer's public key (same string `Identity::public_b64`
/// produces for our own key). Stable across reconnect attempts, unlike a
/// signaling session id, which is per-negotiation.
pub type PeerKey = String;

fn peer_key(public_key: &[u8]) -> PeerKey {
    STANDARD.encode(public_key)
}

/// One peer's live link (Phase G.3b): its lifecycle state, shared with the
/// running pipeline task, and the task's own handle. Entries are removed on
/// disconnect rather than kept around as `Idle` — a missing key and an
/// `Idle` key mean the same thing to every reader here.
struct PeerLink {
    link: Arc<Mutex<LinkState>>,
    active: Option<Active>,
}

#[derive(Default)]
struct Inner {
    socket: Option<UdpTransport>,
    local_candidates: Vec<Candidate>,
    /// The peer currently being negotiated via manual signaling (C3) — set by
    /// `begin_offer`/`begin_answer`/`set_peer`. Distinct from `peers` below:
    /// this is "who am I currently exchanging an offer/answer with," which
    /// remains a single slot because the manual paste UI only ever handles
    /// one negotiation at a time. Once negotiated, the legacy no-argument
    /// `connect`/`disconnect`/`link_state` derive their target peer key from
    /// this slot, so they keep working unchanged even though the underlying
    /// link bookkeeping below is now multi-peer.
    sid: Option<String>,
    role: Role,
    peer: Option<NegotiatedPeer>,
}

/// Shared connection state (managed by Tauri). Tracks a live link per peer
/// (Phase G.3b), keyed by [`PeerKey`], so more than one connection can be
/// `Connecting`/`Connected` at once — a prerequisite for a Hamachi-style
/// virtual network with more than two members.
#[derive(Default)]
pub struct ConnectionManager {
    inner: Mutex<Inner>,
    peers: Mutex<HashMap<PeerKey, PeerLink>>,
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

    /// The pending-negotiation peer's key, if `set_peer` has been called.
    fn current_peer_key(&self) -> Option<PeerKey> {
        self.inner.lock().unwrap().peer.as_ref().map(|p| peer_key(&p.public_key))
    }

    /// The current live-link state of the pending-negotiation peer (legacy
    /// single-peer accessor; see [`current_peer_key`](Self::current_peer_key)).
    /// A peer that was never connected, or has since disconnected, is `Idle`.
    pub fn link_state(&self) -> LinkState {
        match self.current_peer_key() {
            Some(key) => self.link_state_of(&key),
            None => LinkState::Idle,
        }
    }

    /// The live-link state of a specific peer (Phase G.3b). `Idle` for any
    /// key with no tracked entry — a missing entry and an explicit `Idle`
    /// mean the same thing here.
    pub fn link_state_of(&self, key: &PeerKey) -> LinkState {
        self.peers.lock().unwrap().get(key).map(|p| *p.link.lock().unwrap()).unwrap_or_default()
    }

    /// Every peer with a tracked link, and its current state — the basis for
    /// a future "list every connection" snapshot (G.4 UI). Not yet called
    /// from any Tauri command, hence the explicit allow.
    #[allow(dead_code)]
    pub fn peer_link_states(&self) -> Vec<(PeerKey, LinkState)> {
        self.peers.lock().unwrap().iter().map(|(k, v)| (k.clone(), *v.link.lock().unwrap())).collect()
    }

    /// Begin connecting to the negotiated peer (C4 hole-punch-as-handshake).
    ///
    /// Legacy single-peer entry point: derives the peer key, role, socket, and
    /// candidates from the pending-negotiation slot (`begin_offer`/
    /// `begin_answer`/`set_peer`) and delegates to [`connect_to`]. Unchanged
    /// behavior and signature from before Phase G.3b — only one session at a
    /// time for *this* peer, but other peers connected via `connect_to` are
    /// unaffected by it.
    ///
    /// [`connect_to`]: ConnectionManager::connect_to
    pub fn connect(
        &self,
        identity: Arc<Identity>,
        sink: Box<dyn TelemetrySink>,
        settings: ConnectionSettings,
    ) -> Result<(), String> {
        let (socket, role, peer_public, peer_candidates) = {
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
        self.connect_to(socket, role, peer_public, peer_candidates, identity, sink, settings)
    }

    /// Begin connecting to `peer_public` at `peer_candidates` (Phase G.3b).
    /// Spawns a single [`pipeline::run`] task and tracks its link state under
    /// [`peer_key(peer_public)`](peer_key), independent of every other peer's
    /// entry — two different peers can be `Connecting`/`Connected`
    /// simultaneously. Rejected only if *this specific* peer already has a
    /// link `Connecting` or `Connected` (one session per peer, not one
    /// session total); a prior `Idle`/`Failed` entry for the same peer is
    /// torn down and replaced. Returns once the task is launched — progress
    /// is reported asynchronously via the telemetry sink and
    /// [`link_state_of`].
    ///
    /// [`link_state_of`]: ConnectionManager::link_state_of
    #[allow(clippy::too_many_arguments)]
    pub fn connect_to(
        &self,
        socket: UdpTransport,
        role: Role,
        peer_public: Vec<u8>,
        mut peer_candidates: Vec<SocketAddr>,
        identity: Arc<Identity>,
        sink: Box<dyn TelemetrySink>,
        settings: ConnectionSettings,
    ) -> Result<(), String> {
        // De-duplicate candidate paths (host and reflexive coincide without NAT).
        peer_candidates.sort();
        peer_candidates.dedup();
        if peer_candidates.is_empty() {
            return Err("peer advertised no candidates to punch".into());
        }

        let key = peer_key(&peer_public);
        let mut peers = self.peers.lock().unwrap();
        if let Some(existing) = peers.get(&key) {
            if matches!(*existing.link.lock().unwrap(), LinkState::Connecting | LinkState::Connected) {
                return Err("a connection to this peer is already active".into());
            }
        }
        // Replace any finished prior task for this peer.
        if let Some(old) = peers.remove(&key) {
            if let Some(a) = old.active {
                let _ = a.cancel.send(true);
                a.handle.abort();
            }
        }

        let (cancel, cancel_rx) = watch::channel(false);
        let link = Arc::new(Mutex::new(LinkState::Connecting));
        let handle = async_runtime::spawn(pipeline::run(
            socket,
            role,
            identity,
            peer_public,
            peer_candidates,
            dataplane_source_for(role),
            settings,
            sink,
            link.clone(),
            cancel_rx,
        ));
        peers.insert(key, PeerLink { link, active: Some(Active { cancel, handle }) });
        Ok(())
    }

    /// Tear down the pending-negotiation peer's live link (legacy single-peer
    /// accessor). Idempotent: a no-op when that peer has no tracked link, or
    /// when nothing has been negotiated at all.
    pub fn disconnect(&self) {
        if let Some(key) = self.current_peer_key() {
            self.disconnect_peer(&key);
        }
    }

    /// Tear down a specific peer's live link (Phase G.3b): cancels its task
    /// and removes its entry (equivalent to `Idle`). Every other peer's link
    /// is untouched. Idempotent: a no-op for a key with no tracked entry.
    pub fn disconnect_peer(&self, key: &PeerKey) {
        if let Some(entry) = self.peers.lock().unwrap().remove(key) {
            if let Some(a) = entry.active {
                let _ = a.cancel.send(true);
                a.handle.abort();
            }
        }
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
    use crate::engine::notice::EngineNotice;
    use crate::engine::state::EngineState;
    use crate::engine::telemetry::{PacketLogEntry, TelemetrySnapshot};
    use std::net::Ipv4Addr;

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

    struct NullSink;
    impl TelemetrySink for NullSink {
        fn stats(&self, _: &TelemetrySnapshot) {}
        fn packets(&self, _: &[PacketLogEntry]) {}
        fn state(&self, _: EngineState) {}
        fn notice(&self, _: &EngineNotice) {}
    }

    /// These G.3b tests exercise `ConnectionManager`'s per-peer bookkeeping,
    /// not the handshake protocol itself (that's `pipeline.rs`'s job, already
    /// covered end-to-end there). An unreachable candidate address is enough
    /// to put a link into `Connecting` and keep it there for the duration of
    /// the test — no real peer needs to answer.
    fn unreachable_candidate() -> SocketAddr {
        // RFC 5737 TEST-NET-1: reserved, never routable.
        SocketAddr::from((Ipv4Addr::new(192, 0, 2, 1), 9))
    }

    async fn bound_socket() -> UdpTransport {
        UdpTransport::bind(0).await.unwrap()
    }

    #[tokio::test]
    async fn two_different_peers_can_be_connecting_at_once() {
        let manager = ConnectionManager::default();
        let id = Arc::new(Identity::generate().unwrap());

        let peer_a = vec![1u8; 32];
        let peer_b = vec![2u8; 32];

        manager
            .connect_to(
                bound_socket().await,
                Role::Initiator,
                peer_a.clone(),
                vec![unreachable_candidate()],
                id.clone(),
                Box::new(NullSink),
                ConnectionSettings::default(),
            )
            .unwrap();
        manager
            .connect_to(
                bound_socket().await,
                Role::Initiator,
                peer_b.clone(),
                vec![unreachable_candidate()],
                id.clone(),
                Box::new(NullSink),
                ConnectionSettings::default(),
            )
            .unwrap();

        assert_eq!(manager.link_state_of(&peer_key(&peer_a)), LinkState::Connecting);
        assert_eq!(manager.link_state_of(&peer_key(&peer_b)), LinkState::Connecting);

        manager.disconnect_peer(&peer_key(&peer_a));
        manager.disconnect_peer(&peer_key(&peer_b));
    }

    #[tokio::test]
    async fn reconnecting_to_an_already_connecting_peer_is_rejected() {
        let manager = ConnectionManager::default();
        let id = Arc::new(Identity::generate().unwrap());
        let peer_a = vec![3u8; 32];

        manager
            .connect_to(
                bound_socket().await,
                Role::Initiator,
                peer_a.clone(),
                vec![unreachable_candidate()],
                id.clone(),
                Box::new(NullSink),
                ConnectionSettings::default(),
            )
            .unwrap();

        let err = manager
            .connect_to(
                bound_socket().await,
                Role::Initiator,
                peer_a.clone(),
                vec![unreachable_candidate()],
                id.clone(),
                Box::new(NullSink),
                ConnectionSettings::default(),
            )
            .unwrap_err();
        assert!(err.contains("already active"));

        manager.disconnect_peer(&peer_key(&peer_a));
    }

    #[tokio::test]
    async fn disconnecting_one_peer_leaves_another_untouched() {
        let manager = ConnectionManager::default();
        let id = Arc::new(Identity::generate().unwrap());
        let peer_a = vec![4u8; 32];
        let peer_b = vec![5u8; 32];

        for peer in [&peer_a, &peer_b] {
            manager
                .connect_to(
                    bound_socket().await,
                    Role::Initiator,
                    peer.clone(),
                    vec![unreachable_candidate()],
                    id.clone(),
                    Box::new(NullSink),
                    ConnectionSettings::default(),
                )
                .unwrap();
        }

        manager.disconnect_peer(&peer_key(&peer_a));

        assert_eq!(manager.link_state_of(&peer_key(&peer_a)), LinkState::Idle);
        assert_eq!(manager.link_state_of(&peer_key(&peer_b)), LinkState::Connecting);

        manager.disconnect_peer(&peer_key(&peer_b));
    }

    #[test]
    fn link_state_of_an_untracked_key_is_idle() {
        let manager = ConnectionManager::default();
        assert_eq!(manager.link_state_of(&peer_key(&[9u8; 32])), LinkState::Idle);
    }

    #[tokio::test]
    async fn peer_link_states_lists_every_tracked_peer() {
        let manager = ConnectionManager::default();
        let id = Arc::new(Identity::generate().unwrap());
        let peer_a = vec![6u8; 32];
        let peer_b = vec![7u8; 32];

        assert_eq!(manager.peer_link_states(), vec![]);

        for peer in [&peer_a, &peer_b] {
            manager
                .connect_to(
                    bound_socket().await,
                    Role::Initiator,
                    peer.clone(),
                    vec![unreachable_candidate()],
                    id.clone(),
                    Box::new(NullSink),
                    ConnectionSettings::default(),
                )
                .unwrap();
        }

        let mut states = manager.peer_link_states();
        states.sort_by(|a, b| a.0.cmp(&b.0));
        let mut expected =
            vec![(peer_key(&peer_a), LinkState::Connecting), (peer_key(&peer_b), LinkState::Connecting)];
        expected.sort_by(|a, b| a.0.cmp(&b.0));
        assert_eq!(states, expected);

        manager.disconnect_peer(&peer_key(&peer_a));
        manager.disconnect_peer(&peer_key(&peer_b));
    }
}
