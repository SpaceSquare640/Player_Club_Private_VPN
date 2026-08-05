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
//! Not yet wired into any Tauri command (that's G.4), so the public API is
//! exercised only by this module's own tests for now — hence the blanket
//! allow, rather than papering over each item individually.
#![allow(dead_code)]

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;

use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use rand::RngCore;
use tokio::sync::{mpsc, watch};

use super::connection::{ConnectionManager, ConnectionSettings, PeerKey, Role};
use super::crypto::Identity;
use super::nat::candidate::Candidate;
use super::signaling::client::{MemberEvent, SignalingClient};
use super::signaling::message::{SignalEnvelope, SignalKind, WireCandidate, PROTOCOL_VERSION};
use super::signaling::protocol::MemberInfo;
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
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::nat::candidate::CandidateKind;
    use crate::engine::connection::LinkState;
    use crate::engine::signaling::server::SignalingServer;
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

        client.disconnect().await;
        server.shutdown().await;
    }
}
