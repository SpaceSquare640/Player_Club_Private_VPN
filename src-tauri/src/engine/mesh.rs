//! Mesh orchestration (Phase G.3c): wires the signaling roster (G.2) and
//! relay (G.3) into the multi-peer `ConnectionManager` (G.3b) so that joining
//! a network establishes P2P links with every other member automatically —
//! no manual offer/answer paste required.
//!
//! This is glue, not new protocol: every offer/answer it sends is the exact
//! same [`SignalEnvelope`] the manual paste flow (`commands/signaling_cmds.rs`)
//! already produces and validates, just carried over [`SignalingClient::relay`]
//! instead of copied by hand.
//!
//! [`MeshSession`] (Phase G.4) is the Tauri-facing lifecycle wrapper: one
//! network membership at a time, created by `commands/mesh_cmds.rs`.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use rand::RngCore;
use serde::Serialize;
use tokio::sync::{mpsc, watch};

use super::connection::{ConnectionManager, ConnectionSettings, LinkState, PeerKey, Role};
use super::crypto::Identity;
use super::nat::candidate::Candidate;
use super::signaling::client::{MemberEvent, SignalingClient};
use super::signaling::message::{SignalEnvelope, SignalKind, WireCandidate, PROTOCOL_VERSION};
use super::signaling::protocol::MemberInfo;
use super::signaling::server::SignalingServer;
use super::telemetry::TelemetrySink;
use super::transport::UdpTransport;

#[derive(Debug, thiserror::Error)]
pub enum MeshError {
    #[error("malformed relayed envelope: {0}")]
    MalformedEnvelope(String),
    #[error("received an answer with no matching pending offer")]
    UnexpectedAnswer,
    #[error("received an answer for the wrong session")]
    SessionMismatch,
    #[error("signaling client is disconnected")]
    SignalingDisconnected,
    #[error("connect_to rejected the relayed peer: {0}")]
    Connect(String),
}

/// Decides, for a pair of members, which one sends the offer — both sides
/// independently compute the same answer from data they both already have
/// (their own and the other's public key), so exactly one offer is ever sent
/// per pair, with no coordination message needed to arbitrate it.
fn should_initiate(our_pubkey_b64: &str, their_pubkey_b64: &str) -> bool {
    our_pubkey_b64 < their_pubkey_b64
}

fn decode_envelope_peer(envelope: &SignalEnvelope) -> Result<(Vec<u8>, Vec<SocketAddr>), MeshError> {
    let public_key = STANDARD
        .decode(envelope.pk.trim())
        .map_err(|e| MeshError::MalformedEnvelope(format!("public key: {e}")))?;
    let mut candidates = Vec::with_capacity(envelope.cands.len());
    for c in &envelope.cands {
        candidates.push(
            c.a.parse()
                .map_err(|_| MeshError::MalformedEnvelope(format!("candidate address '{}'", c.a)))?,
        );
    }
    Ok((public_key, candidates))
}

fn new_sid() -> String {
    let mut bytes = [0u8; 16];
    rand::thread_rng().fill_bytes(&mut bytes);
    STANDARD.encode(bytes)
}

/// Orchestrates automatic peer connections for one member of a network,
/// driven by [`MeshOrchestrator::run`]'s event loop. Everything routes
/// through the [`SignalingClient`] (already connected) and
/// [`ConnectionManager`] (into which every established link is registered)
/// passed in, but the socket and locally-advertised candidates are supplied
/// explicitly at construction rather than pulled from the manager's own
/// `ensure_socket`/STUN bootstrap — the caller (in production: `ensure_socket`
/// then `manager.socket()`/`local_candidates()`; in tests: a directly-bound
/// loopback socket, same as `pipeline.rs`'s own integration tests) decides
/// how those are obtained.
pub struct MeshOrchestrator {
    manager: Arc<ConnectionManager>,
    identity: Arc<Identity>,
    settings: ConnectionSettings,
    socket: UdpTransport,
    local_candidates: Vec<Candidate>,
    /// Session ids of offers we've sent and are still waiting on an answer
    /// for, keyed by the target peer. Cleared once the matching answer
    /// arrives (or the peer leaves).
    pending_offers: HashMap<PeerKey, String>,
    /// Peers already handed to `on_member_present`. Necessary because a
    /// member who joins between `SignalingClient::join` returning and
    /// `run`'s initial-roster read is *both* already reflected in
    /// `client.members()` *and* still sitting as an unconsumed `Joined`
    /// event in the channel (the client updates its roster and pushes the
    /// event from the same code path, atomically) — without this guard,
    /// such a member would be processed twice, sending two different offers
    /// with two different session ids and corrupting `pending_offers`.
    seen_members: std::collections::HashSet<PeerKey>,
    /// Live roster mirror, updated as members are seen/leave. Exposed via
    /// [`roster_handle`](Self::roster_handle) so a caller (Phase G.4's Tauri
    /// command layer) can read the current member list from outside the
    /// task this orchestrator's `run` loop lives in, without needing to hold
    /// the `SignalingClient` itself (which `run` borrows for its duration).
    roster: Arc<std::sync::Mutex<Vec<MemberInfo>>>,
}

impl MeshOrchestrator {
    pub fn new(
        manager: Arc<ConnectionManager>,
        identity: Arc<Identity>,
        settings: ConnectionSettings,
        socket: UdpTransport,
        local_candidates: Vec<Candidate>,
    ) -> Self {
        Self {
            manager,
            identity,
            settings,
            socket,
            local_candidates,
            pending_offers: HashMap::new(),
            seen_members: std::collections::HashSet::new(),
            roster: Arc::new(std::sync::Mutex::new(Vec::new())),
        }
    }

    /// A handle to the live roster mirror — safe to read from outside the
    /// task `run` is driven in.
    pub fn roster_handle(&self) -> Arc<std::sync::Mutex<Vec<MemberInfo>>> {
        self.roster.clone()
    }

    /// Drives every roster/relay event to completion: sends offers to newly
    /// seen members (ourselves initiating per [`should_initiate`]), answers
    /// and connects on an incoming offer, connects on an incoming answer, and
    /// tears down the link when a member leaves. Runs until `cancel` fires or
    /// the event channel closes.
    pub async fn run(
        &mut self,
        client: &SignalingClient,
        mut events: mpsc::UnboundedReceiver<MemberEvent>,
        sink_factory: impl Fn() -> Box<dyn TelemetrySink>,
        mut cancel: watch::Receiver<bool>,
    ) {
        // The initial roster is a burst of "this member exists" — identical
        // in effect to each of them having just sent a `Joined` event.
        for member in client.members() {
            let _ = self.on_member_present(client, &member).await;
        }

        loop {
            tokio::select! {
                _ = cancel.changed() => break,
                event = events.recv() => {
                    let Some(event) = event else { break };
                    let _ = self.on_event(client, event, &sink_factory).await;
                }
            }
        }
    }

    async fn on_event(
        &mut self,
        client: &SignalingClient,
        event: MemberEvent,
        sink_factory: &impl Fn() -> Box<dyn TelemetrySink>,
    ) -> Result<(), MeshError> {
        match event {
            MemberEvent::Joined(member) => self.on_member_present(client, &member).await,
            MemberEvent::Left { pubkey } => {
                self.pending_offers.remove(&pubkey);
                self.seen_members.remove(&pubkey);
                self.roster.lock().expect("mesh roster lock poisoned").retain(|m| m.pubkey != pubkey);
                self.manager.disconnect_peer(&pubkey);
                Ok(())
            }
            MemberEvent::Relayed { from_pubkey, envelope } => {
                self.on_relayed(client, from_pubkey, envelope, sink_factory).await
            }
        }
    }

    /// A member is known to exist (from the initial roster or a live
    /// `Joined` event): if the tie-break says we initiate, send them an
    /// offer and record its session id to match against their answer.
    async fn on_member_present(&mut self, client: &SignalingClient, member: &MemberInfo) -> Result<(), MeshError> {
        if !self.seen_members.insert(member.pubkey.clone()) {
            return Ok(()); // already handled — see `seen_members`'s doc comment
        }
        self.roster.lock().expect("mesh roster lock poisoned").push(member.clone());
        if !should_initiate(&self.identity.public_b64(), &member.pubkey) {
            return Ok(());
        }
        let sid = new_sid();
        let envelope = SignalEnvelope {
            v: PROTOCOL_VERSION,
            kind: SignalKind::Offer,
            sid: sid.clone(),
            pk: self.identity.public_b64(),
            cands: wire_candidates(&self.local_candidates),
        };
        client
            .relay(member.pubkey.clone(), &envelope)
            .map_err(|_| MeshError::SignalingDisconnected)?;
        self.pending_offers.insert(member.pubkey.clone(), sid);
        Ok(())
    }

    async fn on_relayed(
        &mut self,
        client: &SignalingClient,
        from_pubkey: PeerKey,
        envelope: SignalEnvelope,
        sink_factory: &impl Fn() -> Box<dyn TelemetrySink>,
    ) -> Result<(), MeshError> {
        match envelope.kind {
            SignalKind::Offer => {
                let (peer_public, peer_candidates) = decode_envelope_peer(&envelope)?;

                let answer = SignalEnvelope {
                    v: PROTOCOL_VERSION,
                    kind: SignalKind::Answer,
                    sid: envelope.sid,
                    pk: self.identity.public_b64(),
                    cands: wire_candidates(&self.local_candidates),
                };
                client
                    .relay(from_pubkey, &answer)
                    .map_err(|_| MeshError::SignalingDisconnected)?;

                self.manager
                    .connect_to(
                        self.socket.clone(),
                        Role::Responder,
                        peer_public,
                        peer_candidates,
                        self.identity.clone(),
                        sink_factory(),
                        self.settings,
                    )
                    .map_err(MeshError::Connect)?;
                Ok(())
            }
            SignalKind::Answer => {
                let expected_sid = self.pending_offers.remove(&from_pubkey).ok_or(MeshError::UnexpectedAnswer)?;
                if expected_sid != envelope.sid {
                    return Err(MeshError::SessionMismatch);
                }
                let (peer_public, peer_candidates) = decode_envelope_peer(&envelope)?;

                self.manager
                    .connect_to(
                        self.socket.clone(),
                        Role::Initiator,
                        peer_public,
                        peer_candidates,
                        self.identity.clone(),
                        sink_factory(),
                        self.settings,
                    )
                    .map_err(MeshError::Connect)?;
                Ok(())
            }
        }
    }
}

fn wire_candidates(cands: &[super::nat::candidate::Candidate]) -> Vec<WireCandidate> {
    cands.iter().map(|c| WireCandidate { a: c.addr.to_string(), k: c.kind.as_str().to_string() }).collect()
}

/// STUN server for candidate gathering (matches the C1/manual-signaling
/// default in `commands/signaling_cmds.rs`).
const STUN_SERVER: &str = "stun.l.google.com:19302";

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct MemberSnapshot {
    pub pubkey: String,
    pub fingerprint: String,
    pub link: LinkState,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct NetworkStatus {
    pub network_name: String,
    pub is_host: bool,
    /// The host's address, in `ip:port` form — what a joiner needs to type
    /// in. Meaningful whether or not this node is the host: everyone in the
    /// network already knows it (it's how they connected).
    pub host_addr: String,
    pub members: Vec<MemberSnapshot>,
}

struct ActiveMesh {
    cancel: watch::Sender<bool>,
    task: tokio::task::JoinHandle<()>,
    roster: Arc<Mutex<Vec<MemberInfo>>>,
    network_name: String,
    is_host: bool,
    host_addr: SocketAddr,
}

/// One networked-signaling membership at a time (Phase G.4). Owns the
/// lifecycle Tauri commands drive: `create`/`join` start a
/// [`SignalingClient`] + [`MeshOrchestrator`] pair (and, for `create`, a
/// [`SignalingServer`]) in a background task; `leave` tears it all down;
/// `status` reads the live roster and each member's `ConnectionManager` link
/// state without needing to reach into the background task at all.
#[derive(Default)]
pub struct MeshSession {
    active: Mutex<Option<ActiveMesh>>,
}

impl MeshSession {
    /// Starts hosting a new network: binds a `SignalingServer` on
    /// `bind_addr` (port `0` picks an ephemeral port — the actual bound
    /// address is returned so the UI can show it to share), joins it as the
    /// first member, and starts auto-connecting. Rejected if already in a
    /// network.
    pub async fn create(
        &self,
        bind_addr: SocketAddr,
        network_name: String,
        password: String,
        identity: Arc<Identity>,
        manager: Arc<ConnectionManager>,
        sink_factory: impl Fn() -> Box<dyn TelemetrySink> + Send + Sync + 'static,
    ) -> Result<SocketAddr, String> {
        if self.active.lock().expect("mesh session lock poisoned").is_some() {
            return Err("already in a network — leave it first".into());
        }
        let server = SignalingServer::start(bind_addr, network_name.clone(), &password)
            .await
            .map_err(|e| e.to_string())?;
        let host_addr = server.local_addr();
        self.start(host_addr, network_name, password, identity, manager, sink_factory, Some(server)).await?;
        Ok(host_addr)
    }

    /// Joins an existing network hosted at `host_addr`. Rejected if already
    /// in a network.
    pub async fn join(
        &self,
        host_addr: SocketAddr,
        network_name: String,
        password: String,
        identity: Arc<Identity>,
        manager: Arc<ConnectionManager>,
        sink_factory: impl Fn() -> Box<dyn TelemetrySink> + Send + Sync + 'static,
    ) -> Result<(), String> {
        if self.active.lock().expect("mesh session lock poisoned").is_some() {
            return Err("already in a network — leave it first".into());
        }
        self.start(host_addr, network_name, password, identity, manager, sink_factory, None).await
    }

    #[allow(clippy::too_many_arguments)]
    async fn start(
        &self,
        host_addr: SocketAddr,
        network_name: String,
        password: String,
        identity: Arc<Identity>,
        manager: Arc<ConnectionManager>,
        sink_factory: impl Fn() -> Box<dyn TelemetrySink> + Send + Sync + 'static,
        server: Option<SignalingServer>,
    ) -> Result<(), String> {
        manager.ensure_socket(STUN_SERVER).await.map_err(|e| e.to_string())?;
        let (client, events) = SignalingClient::join(
            host_addr,
            network_name.clone(),
            &password,
            identity.public_b64(),
            identity.peer_address(),
        )
        .await
        .map_err(|e| e.to_string())?;

        let socket = manager.socket().ok_or("no socket after ensure_socket")?;
        let candidates = manager.local_candidates();
        let mut orch = MeshOrchestrator::new(manager, identity, ConnectionSettings::default(), socket, candidates);
        let roster = orch.roster_handle();
        let is_host = server.is_some();

        let (cancel, cancel_rx) = watch::channel(false);
        let task = tokio::spawn(async move {
            orch.run(&client, events, sink_factory, cancel_rx).await;
            client.disconnect().await;
            if let Some(server) = server {
                server.shutdown().await;
            }
        });

        *self.active.lock().expect("mesh session lock poisoned") =
            Some(ActiveMesh { cancel, task, roster, network_name, is_host, host_addr });
        Ok(())
    }

    /// Leaves the current network (idempotent: a no-op if not in one).
    /// Cancels the orchestrator, disconnects the signaling client, and — if
    /// this node was the host — shuts down the signaling server, all inside
    /// the same background task `start` spawned.
    pub async fn leave(&self) {
        let active = self.active.lock().expect("mesh session lock poisoned").take();
        if let Some(active) = active {
            let _ = active.cancel.send(true);
            let _ = active.task.await;
        }
    }

    /// The current network's status, or `None` if not in one. Every
    /// member's link state is read live from `manager` — the roster itself
    /// says nothing about connection progress, `ConnectionManager` does.
    pub fn status(&self, manager: &ConnectionManager) -> Option<NetworkStatus> {
        let active = self.active.lock().expect("mesh session lock poisoned");
        let active = active.as_ref()?;
        let members = active
            .roster
            .lock()
            .expect("mesh roster lock poisoned")
            .iter()
            .map(|m| MemberSnapshot {
                pubkey: m.pubkey.clone(),
                fingerprint: m.fingerprint.clone(),
                link: manager.link_state_of(&m.pubkey),
            })
            .collect();
        Some(NetworkStatus {
            network_name: active.network_name.clone(),
            is_host: active.is_host,
            host_addr: active.host_addr.to_string(),
            members,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::nat::candidate::CandidateKind;
    use crate::engine::state::EngineState;
    use crate::engine::notice::EngineNotice;
    use crate::engine::telemetry::{PacketLogEntry, TelemetrySnapshot};
    use std::net::Ipv4Addr;
    use std::time::Duration;

    struct NullSink;
    impl TelemetrySink for NullSink {
        fn stats(&self, _: &TelemetrySnapshot) {}
        fn packets(&self, _: &[PacketLogEntry]) {}
        fn state(&self, _: EngineState) {}
        fn notice(&self, _: &EngineNotice) {}
    }

    async fn until(secs: u64, label: &str, mut cond: impl FnMut() -> bool) {
        for _ in 0..(secs * 100) {
            if cond() {
                return;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        panic!("timed out waiting for: {label}");
    }

    async fn loopback_socket_and_candidate() -> (UdpTransport, Candidate) {
        let socket = UdpTransport::bind(0).await.unwrap();
        let addr = SocketAddr::from((Ipv4Addr::LOCALHOST, socket.local_addr().unwrap().port()));
        (socket, Candidate { addr, kind: CandidateKind::Host })
    }

    /// The end-to-end proof of Phase G.3c: two members join the same
    /// signaling network and, with zero manual offer/answer paste, both
    /// reach `Connected` — the tie-break picks exactly one initiator, its
    /// offer is relayed and answered automatically, and `connect_to` brings
    /// up the real C4/C5 pipeline on both ends.
    #[tokio::test]
    async fn two_members_auto_connect_with_no_manual_signaling() {
        let server = SignalingServer::start("127.0.0.1:0".parse().unwrap(), "party", "secret")
            .await
            .unwrap();

        let id_a = Arc::new(Identity::generate().unwrap());
        let id_b = Arc::new(Identity::generate().unwrap());
        let (client_a, events_a) = SignalingClient::join(
            server.local_addr(),
            "party",
            "secret",
            id_a.public_b64(),
            id_a.peer_address(),
        )
        .await
        .unwrap();
        let (client_b, events_b) = SignalingClient::join(
            server.local_addr(),
            "party",
            "secret",
            id_b.public_b64(),
            id_b.peer_address(),
        )
        .await
        .unwrap();

        let manager_a = Arc::new(ConnectionManager::default());
        let manager_b = Arc::new(ConnectionManager::default());
        let (socket_a, cand_a) = loopback_socket_and_candidate().await;
        let (socket_b, cand_b) = loopback_socket_and_candidate().await;

        let mut orch_a =
            MeshOrchestrator::new(manager_a.clone(), id_a.clone(), ConnectionSettings::default(), socket_a, vec![cand_a]);
        let mut orch_b =
            MeshOrchestrator::new(manager_b.clone(), id_b.clone(), ConnectionSettings::default(), socket_b, vec![cand_b]);

        let (cancel_a, cancel_rx_a) = watch::channel(false);
        let (cancel_b, cancel_rx_b) = watch::channel(false);
        let task_a = tokio::spawn(async move {
            orch_a.run(&client_a, events_a, || Box::new(NullSink), cancel_rx_a).await;
            client_a.disconnect().await;
        });
        let task_b = tokio::spawn(async move {
            orch_b.run(&client_b, events_b, || Box::new(NullSink), cancel_rx_b).await;
            client_b.disconnect().await;
        });

        let key_a: PeerKey = id_a.public_b64();
        let key_b: PeerKey = id_b.public_b64();
        let (ma, mb) = (manager_a.clone(), manager_b.clone());
        until(15, "both sides Connected", move || {
            ma.link_state_of(&key_b) == LinkState::Connected && mb.link_state_of(&key_a) == LinkState::Connected
        })
        .await;

        let _ = cancel_a.send(true);
        let _ = cancel_b.send(true);
        let _ = task_a.await;
        let _ = task_b.await;
        server.shutdown().await;
    }

    #[test]
    fn should_initiate_picks_exactly_one_side_of_a_pair() {
        assert!(should_initiate("aaa", "bbb"));
        assert!(!should_initiate("bbb", "aaa"));
        assert!(!should_initiate("same", "same")); // never true for a (hypothetical) tie
    }

    /// The regression this guards: a member whose `Joined` event was already
    /// queued by the time `client.members()` is read for the initial-roster
    /// burst must only be handled once — not once from the snapshot and
    /// again from the queued event — or two different offers with two
    /// different session ids get sent, corrupting `pending_offers` (see
    /// `seen_members`'s doc comment for the full mechanism).
    #[tokio::test]
    async fn on_member_present_is_idempotent_per_peer() {
        let server = SignalingServer::start("127.0.0.1:0".parse().unwrap(), "party", "secret")
            .await
            .unwrap();
        let id = Arc::new(Identity::generate().unwrap());
        let (client, _events) =
            SignalingClient::join(server.local_addr(), "party", "secret", id.public_b64(), id.peer_address())
                .await
                .unwrap();

        let manager = Arc::new(ConnectionManager::default());
        let (socket, cand) = loopback_socket_and_candidate().await;
        // 'z' sorts after every character standard base64 ever produces, so
        // this is guaranteed to sort after any real identity's key — meaning
        // `should_initiate` is true and an offer actually gets sent, proving
        // the guard fires before the *send*, not just before some no-op.
        let mut orch = MeshOrchestrator::new(manager, id, ConnectionSettings::default(), socket, vec![cand]);
        let member = MemberInfo { pubkey: "z".repeat(44), fingerprint: "PC-0000-0000-0000-0000".to_string() };

        orch.on_member_present(&client, &member).await.unwrap();
        orch.on_member_present(&client, &member).await.unwrap();

        assert_eq!(orch.pending_offers.len(), 1);
        assert_eq!(orch.roster_handle().lock().unwrap().len(), 1); // not duplicated either

        client.disconnect().await;
        server.shutdown().await;
    }

    #[tokio::test]
    async fn roster_handle_reflects_joins_and_leaves() {
        let server = SignalingServer::start("127.0.0.1:0".parse().unwrap(), "party", "secret")
            .await
            .unwrap();
        let id = Arc::new(Identity::generate().unwrap());
        let (client, _events) =
            SignalingClient::join(server.local_addr(), "party", "secret", id.public_b64(), id.peer_address())
                .await
                .unwrap();

        let manager = Arc::new(ConnectionManager::default());
        let (socket, cand) = loopback_socket_and_candidate().await;
        let mut orch = MeshOrchestrator::new(manager, id, ConnectionSettings::default(), socket, vec![cand]);
        let roster = orch.roster_handle();
        assert_eq!(roster.lock().unwrap().len(), 0);

        let member = MemberInfo { pubkey: "z".repeat(44), fingerprint: "PC-0000-0000-0000-0000".to_string() };
        orch.on_member_present(&client, &member).await.unwrap();
        assert_eq!(roster.lock().unwrap().clone(), vec![member.clone()]);

        orch.on_event(&client, MemberEvent::Left { pubkey: member.pubkey.clone() }, &|| Box::new(NullSink))
            .await
            .unwrap();
        assert_eq!(roster.lock().unwrap().len(), 0);

        client.disconnect().await;
        server.shutdown().await;
    }

    #[tokio::test]
    async fn mesh_session_status_is_none_when_not_in_a_network() {
        let session = MeshSession::default();
        let manager = ConnectionManager::default();
        assert!(session.status(&manager).is_none());
    }

    #[tokio::test]
    async fn mesh_session_create_then_status_reports_self_as_host_with_no_members() {
        let session = MeshSession::default();
        let manager = Arc::new(ConnectionManager::default());
        let identity = Arc::new(Identity::generate().unwrap());

        let host_addr = session
            .create(
                "127.0.0.1:0".parse().unwrap(),
                "party".to_string(),
                "secret".to_string(),
                identity,
                manager.clone(),
                || Box::new(NullSink),
            )
            .await
            .unwrap();
        assert_ne!(host_addr.port(), 0); // an ephemeral port was actually assigned

        let status = session.status(&manager).unwrap();
        assert_eq!(status.network_name, "party");
        assert!(status.is_host);
        assert_eq!(status.host_addr, host_addr.to_string());
        assert_eq!(status.members.len(), 0); // only member so far is ourselves, never listed

        session.leave().await;
        assert!(session.status(&manager).is_none());
    }

    #[tokio::test]
    async fn mesh_session_create_twice_is_rejected() {
        let session = MeshSession::default();
        let manager = Arc::new(ConnectionManager::default());
        let identity = Arc::new(Identity::generate().unwrap());

        session
            .create(
                "127.0.0.1:0".parse().unwrap(),
                "party".to_string(),
                "secret".to_string(),
                identity.clone(),
                manager.clone(),
                || Box::new(NullSink),
            )
            .await
            .unwrap();

        let err = session
            .create("127.0.0.1:0".parse().unwrap(), "other".to_string(), "secret".to_string(), identity, manager, || {
                Box::new(NullSink)
            })
            .await
            .unwrap_err();
        assert!(err.contains("already in a network"));

        session.leave().await;
    }

    /// The end-to-end proof at the Tauri-facing layer: one `MeshSession`
    /// hosts, another joins it, and — through nothing but `create`/`join` —
    /// both `status()` calls eventually show the other as `Connected`.
    #[tokio::test]
    async fn two_mesh_sessions_create_and_join_reach_connected() {
        let session_a = MeshSession::default();
        let session_b = MeshSession::default();
        let manager_a = Arc::new(ConnectionManager::default());
        let manager_b = Arc::new(ConnectionManager::default());
        let identity_a = Arc::new(Identity::generate().unwrap());
        let identity_b = Arc::new(Identity::generate().unwrap());

        let host_addr = session_a
            .create(
                "127.0.0.1:0".parse().unwrap(),
                "party".to_string(),
                "secret".to_string(),
                identity_a,
                manager_a.clone(),
                || Box::new(NullSink),
            )
            .await
            .unwrap();

        session_b
            .join(host_addr, "party".to_string(), "secret".to_string(), identity_b, manager_b.clone(), || {
                Box::new(NullSink)
            })
            .await
            .unwrap();

        until(15, "both sides show Connected in status()", move || {
            let sa = session_a.status(&manager_a);
            let sb = session_b.status(&manager_b);
            let a_sees_b = sa.map(|s| s.members.iter().any(|m| m.link == LinkState::Connected)).unwrap_or(false);
            let b_sees_a = sb.map(|s| s.members.iter().any(|m| m.link == LinkState::Connected)).unwrap_or(false);
            a_sees_b && b_sees_a
        })
        .await;
    }
}
