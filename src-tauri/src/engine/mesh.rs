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
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

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
    /// `roster` is supplied by the caller (rather than allocated here) so a
    /// reconnect can hand in the *same* `Arc` a fresh orchestrator instance
    /// replaces — `MeshSession::start`'s retry loop depends on this: the
    /// `ActiveMesh`/`statuses()` plumbing holds one roster handle for the
    /// lifetime of a network, surviving any number of reconnects underneath it.
    pub fn new(
        manager: Arc<ConnectionManager>,
        identity: Arc<Identity>,
        settings: ConnectionSettings,
        socket: UdpTransport,
        local_candidates: Vec<Candidate>,
        roster: Arc<std::sync::Mutex<Vec<MemberInfo>>>,
    ) -> Self {
        Self {
            manager,
            identity,
            settings,
            socket,
            local_candidates,
            pending_offers: HashMap::new(),
            seen_members: std::collections::HashSet::new(),
            roster,
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
                        self.settings.clone(),
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
                        self.settings.clone(),
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

/// Identifies one membership among possibly several concurrently-active
/// networks on the same [`MeshSession`]. Opaque — callers should treat it as
/// an id to pass back to [`MeshSession::leave`], not parse it.
pub type NetworkId = String;

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct NetworkStatus {
    pub id: NetworkId,
    pub network_name: String,
    pub is_host: bool,
    /// The host's address, in `ip:port` form — what a joiner needs to type
    /// in. Meaningful whether or not this node is the host: everyone in the
    /// network already knows it (it's how they connected).
    pub host_addr: String,
    /// Free-form label set at creation/join time (e.g. `"minecraft"`) —
    /// display metadata only, never inspected by any connection logic.
    /// Deliberately a plain string rather than an enum: new games get a new
    /// tag value, not a code change here.
    pub game_tag: Option<String>,
    pub members: Vec<MemberSnapshot>,
    /// `true` while a previously-established connection is being retried
    /// after an unexpected drop (see `MeshSession::start`'s retry loop) —
    /// `false` both before the first connection succeeds (that failure is
    /// returned directly from `create`/`join`, never surfaced as this) and
    /// once reconnected.
    pub reconnecting: bool,
}

struct ActiveMesh {
    cancel: watch::Sender<bool>,
    task: tokio::task::JoinHandle<()>,
    roster: Arc<Mutex<Vec<MemberInfo>>>,
    network_name: String,
    is_host: bool,
    host_addr: SocketAddr,
    game_tag: Option<String>,
    reconnecting: Arc<std::sync::atomic::AtomicBool>,
}

/// Resolve an unspecified bind IP (`0.0.0.0`) to this machine's primary
/// local IPv4 address, for display/sharing purposes — see the comment at
/// `MeshSession::create`'s call site. Falls back to loopback if discovery
/// fails, which at least keeps same-machine testing working. A concrete
/// (already-specific) IP is returned unchanged.
fn advertisable_addr(addr: SocketAddr) -> SocketAddr {
    if !addr.ip().is_unspecified() {
        return addr;
    }
    let ip = super::nat::candidate::primary_local_ipv4().unwrap_or(Ipv4Addr::LOCALHOST);
    SocketAddr::new(IpAddr::V4(ip), addr.port())
}

/// Zero or more concurrent networked-signaling memberships (Phase G.4+).
/// Owns the lifecycle Tauri commands drive: `create`/`join` each start a
/// [`SignalingClient`] + [`MeshOrchestrator`] pair (and, for `create`, a
/// [`SignalingServer`]) in their own background task and register it under a
/// freshly generated [`NetworkId`]; `leave` tears down just the one
/// identified by that id; `statuses` reads the live roster and each member's
/// `ConnectionManager` link state for every active network, without needing
/// to reach into any background task at all.
///
/// Known, accepted limitation: [`ConnectionManager`]'s peer map is keyed only
/// by remote pubkey (`PeerKey`), shared across every network this session is
/// a member of. If the same remote peer were ever a member of two of this
/// node's networks at once, their `PeerLink`/link-state would collide. Real
/// usage has distinct membership per network, so this isn't hit in practice
/// and isn't addressed here.
#[derive(Default)]
pub struct MeshSession {
    active: Mutex<HashMap<NetworkId, ActiveMesh>>,
}

/// Backoff for `run_mesh_task`'s reconnect loop — the exact shape of the
/// prior Python project's `Client_App.py` `control_loop` (`retry_delay`
/// starting at 2s, doubling, capped at 30s), since matching that project's
/// reconnect feel is the explicit point of this behavior.
const INITIAL_RETRY_DELAY: Duration = Duration::from_secs(2);
const MAX_RETRY_DELAY: Duration = Duration::from_secs(30);

/// One connection attempt — direct or via relay depending on `relay_addr` —
/// shared by the initial (synchronous, failure-propagating) connect in
/// `MeshSession::start` and every retry `run_mesh_task` makes afterward.
async fn connect_once(
    host_addr: SocketAddr,
    network_name: &str,
    password: &str,
    identity: &Identity,
    relay_addr: Option<SocketAddr>,
) -> Result<(SignalingClient, mpsc::UnboundedReceiver<MemberEvent>), String> {
    let result = if relay_addr.is_some() {
        SignalingClient::join_via_relay(
            host_addr,
            network_name.to_string(),
            password,
            identity.public_b64(),
            identity.peer_address(),
        )
        .await
    } else {
        SignalingClient::join(
            host_addr,
            network_name.to_string(),
            password,
            identity.public_b64(),
            identity.peer_address(),
        )
        .await
    };
    result.map_err(|e| e.to_string())
}

/// Drives one network's whole background lifecycle, across any number of
/// reconnects: runs `MeshOrchestrator` on the already-connected `client`
/// until it exits, and — unless that exit was `cancel` firing (an explicit
/// `MeshSession::leave`) — clears stale membership and retries `connect_once`
/// with capped exponential backoff (see `INITIAL_RETRY_DELAY`/`MAX_RETRY_DELAY`)
/// until it reconnects or is cancelled while waiting. `roster` is the one
/// `Arc` `ActiveMesh`/`statuses()` holds for this network's whole lifetime —
/// every fresh `MeshOrchestrator` built here reuses it rather than starting
/// a new one, so a reconnect is invisible to anything reading status.
#[allow(clippy::too_many_arguments)]
async fn run_mesh_task(
    mut client: SignalingClient,
    mut events: mpsc::UnboundedReceiver<MemberEvent>,
    manager: Arc<ConnectionManager>,
    identity: Arc<Identity>,
    settings: ConnectionSettings,
    socket: UdpTransport,
    candidates: Vec<Candidate>,
    roster: Arc<Mutex<Vec<MemberInfo>>>,
    sink_factory: impl Fn() -> Box<dyn TelemetrySink> + Send + Sync + 'static,
    mut cancel_rx: watch::Receiver<bool>,
    server: Option<SignalingServer>,
    host_addr: SocketAddr,
    network_name: String,
    password: String,
    relay_addr: Option<SocketAddr>,
    reconnecting: Arc<AtomicBool>,
) {
    loop {
        let mut orch =
            MeshOrchestrator::new(manager.clone(), identity.clone(), settings.clone(), socket.clone(), candidates.clone(), roster.clone());
        orch.run(&client, events, &sink_factory, cancel_rx.clone()).await;
        client.disconnect().await;

        // Always tear down this session's peer links when its signaling
        // session ends — explicit `leave()` or an unexpected drop alike.
        // Without this, a peer's `PeerLink` can linger `Connected` in
        // `ConnectionManager` past the point its signaling roster says
        // they're gone, which blocks `connect_to` from ever establishing a
        // fresh link to that same peer again (its guard rejects a second
        // `connect_to` while an existing entry still reads Connecting/Connected).
        let stale_peers: Vec<PeerKey> =
            roster.lock().expect("mesh roster lock poisoned").drain(..).map(|m| m.pubkey).collect();
        for peer in stale_peers {
            manager.disconnect_peer(&peer);
        }

        if *cancel_rx.borrow() {
            break;
        }

        // Unexpected disconnect, not a `leave()` — retry.
        reconnecting.store(true, Ordering::Relaxed);

        let mut delay = INITIAL_RETRY_DELAY;
        let reconnected = 'retry: loop {
            tokio::select! {
                _ = cancel_rx.changed() => break 'retry None,
                result = connect_once(host_addr, &network_name, &password, &identity, relay_addr) => {
                    match result {
                        Ok(pair) => break 'retry Some(pair),
                        Err(_) => {
                            tokio::select! {
                                _ = cancel_rx.changed() => break 'retry None,
                                _ = tokio::time::sleep(delay) => {}
                            }
                            delay = (delay * 2).min(MAX_RETRY_DELAY);
                        }
                    }
                }
            }
        };

        match reconnected {
            Some((new_client, new_events)) => {
                client = new_client;
                events = new_events;
                reconnecting.store(false, Ordering::Relaxed);
            }
            None => break,
        }
    }

    if let Some(server) = server {
        server.shutdown().await;
    }
}

impl MeshSession {
    /// Starts hosting a new network. With `relay_addr: None`, binds a
    /// `SignalingServer` directly on `bind_addr` (port `0` picks an
    /// ephemeral port — the actual bound address is returned so the UI can
    /// show it to share) — reachable only from wherever `bind_addr` actually
    /// is (same LAN, or a manually port-forwarded address). With
    /// `relay_addr: Some(_)`, `bind_addr` is ignored entirely and the
    /// network instead registers on a
    /// [`RelayServer`](super::relay::RelayServer) at that address (see
    /// `SignalingServer::start_via_relay`) — reachable across the internet
    /// by anyone who can reach the relay, without any port forwarding of
    /// their own. Either way this joins the network as its first member and
    /// starts auto-connecting. Can be called any number of times to host (or
    /// join, via [`Self::join`]) several networks at once — each gets its
    /// own freshly generated [`NetworkId`].
    #[allow(clippy::too_many_arguments)]
    pub async fn create(
        &self,
        bind_addr: SocketAddr,
        network_name: String,
        password: String,
        game_tag: Option<String>,
        settings: ConnectionSettings,
        identity: Arc<Identity>,
        manager: Arc<ConnectionManager>,
        sink_factory: impl Fn() -> Box<dyn TelemetrySink> + Send + Sync + 'static,
        relay_addr: Option<SocketAddr>,
    ) -> Result<(NetworkId, SocketAddr), String> {
        let (server, host_addr) = match relay_addr {
            Some(relay_addr) => {
                let server = SignalingServer::start_via_relay(relay_addr, network_name.clone(), &password)
                    .await
                    .map_err(|e| e.to_string())?;
                let host_addr = server.local_addr(); // always `relay_addr` itself — see `start_via_relay`'s doc comment
                (server, host_addr)
            }
            None => {
                let server = SignalingServer::start(bind_addr, network_name.clone(), &password)
                    .await
                    .map_err(|e| e.to_string())?;
                // `local_addr()` echoes back the literal bind IP — with the
                // UI's default `0.0.0.0`, that's `TcpListener::local_addr`'s
                // unspecified address, not a real interface address. This
                // used to be resolved only for the *returned/displayed*
                // value while the self-join below still connected to the
                // raw `0.0.0.0:port`, on the assumption that `connect()` to
                // `0.0.0.0` gets silently treated as loopback. It does not
                // on Windows: that connect fails outright with
                // WSAEADDRNOTAVAIL ("os error 10049"), which is exactly the
                // error a user hit hosting a Virtual Network with the
                // default bind address. Resolve once, use everywhere: the
                // server itself still listens on every interface
                // (`bind_addr` is untouched), only what we
                // connect-to-ourselves-with and advertise changes.
                let host_addr = advertisable_addr(server.local_addr());
                (server, host_addr)
            }
        };
        let id = self
            .start(
                host_addr,
                network_name,
                password,
                game_tag,
                settings,
                identity,
                manager,
                sink_factory,
                Some(server),
                relay_addr,
            )
            .await?;
        Ok((id, host_addr))
    }

    /// Joins an existing network. With `relay_addr: None`, dials `host_addr`
    /// directly (must be reachable — same LAN, or manually port-forwarded).
    /// With `relay_addr: Some(_)`, `host_addr` is ignored and this instead
    /// connects out to that relay and requests `network_name` (see
    /// `SignalingClient::join_via_relay`) — the same relay address the host
    /// used with `Self::create`. Can be called any number of times,
    /// including while already hosting or having joined other networks —
    /// see [`Self::create`].
    #[allow(clippy::too_many_arguments)]
    pub async fn join(
        &self,
        host_addr: SocketAddr,
        network_name: String,
        password: String,
        game_tag: Option<String>,
        settings: ConnectionSettings,
        identity: Arc<Identity>,
        manager: Arc<ConnectionManager>,
        sink_factory: impl Fn() -> Box<dyn TelemetrySink> + Send + Sync + 'static,
        relay_addr: Option<SocketAddr>,
    ) -> Result<NetworkId, String> {
        // Stored/displayed as whichever address is actually meaningful: the
        // relay's, when relaying (nothing reachable to show for `host_addr`
        // itself in that mode), otherwise the direct address as given.
        let stored_host_addr = relay_addr.unwrap_or(host_addr);
        self.start(
            stored_host_addr,
            network_name,
            password,
            game_tag,
            settings,
            identity,
            manager,
            sink_factory,
            None,
            relay_addr,
        )
        .await
    }

    #[allow(clippy::too_many_arguments)]
    async fn start(
        &self,
        host_addr: SocketAddr,
        network_name: String,
        password: String,
        game_tag: Option<String>,
        settings: ConnectionSettings,
        identity: Arc<Identity>,
        manager: Arc<ConnectionManager>,
        sink_factory: impl Fn() -> Box<dyn TelemetrySink> + Send + Sync + 'static,
        server: Option<SignalingServer>,
        relay_addr: Option<SocketAddr>,
    ) -> Result<NetworkId, String> {
        manager.ensure_socket(STUN_SERVER).await.map_err(|e| e.to_string())?;
        // The first attempt is synchronous and its failure is returned
        // directly — a wrong password/network name, or an address that's
        // simply unreachable, rejects the `create`/`join` call immediately,
        // exactly as before this method grew a retry loop. Auto-reconnect
        // (in `run_mesh_task`) only ever kicks in after a connection that
        // *did* succeed later drops — at that point the credentials are
        // already known good, so blindly retrying makes sense in a way it
        // wouldn't here.
        let (client, events) = connect_once(host_addr, &network_name, &password, &identity, relay_addr).await?;

        let socket = manager.socket().ok_or("no socket after ensure_socket")?;
        let candidates = manager.local_candidates();
        let roster: Arc<Mutex<Vec<MemberInfo>>> = Arc::new(Mutex::new(Vec::new()));
        let is_host = server.is_some();
        let reconnecting = Arc::new(AtomicBool::new(false));

        let (cancel, cancel_rx) = watch::channel(false);
        let task = tokio::spawn(run_mesh_task(
            client,
            events,
            manager,
            identity,
            settings,
            socket,
            candidates,
            roster.clone(),
            sink_factory,
            cancel_rx,
            server,
            host_addr,
            network_name.clone(),
            password,
            relay_addr,
            reconnecting.clone(),
        ));

        let id = new_sid();
        self.active.lock().expect("mesh session lock poisoned").insert(
            id.clone(),
            ActiveMesh { cancel, task, roster, network_name, is_host, host_addr, game_tag, reconnecting },
        );
        Ok(id)
    }

    /// Leaves the network identified by `id` (idempotent: a no-op if not a
    /// member of it, e.g. already left or never joined). Cancels that
    /// network's orchestrator, disconnects its signaling client, and — if
    /// this node was its host — shuts down its signaling server, all inside
    /// the same background task `start` spawned. Other active networks on
    /// this session are untouched.
    pub async fn leave(&self, id: &NetworkId) {
        let active = self.active.lock().expect("mesh session lock poisoned").remove(id);
        if let Some(active) = active {
            let _ = active.cancel.send(true);
            let _ = active.task.await;
        }
    }

    /// The status of every currently active network. Every member's link
    /// state is read live from `manager` — the roster itself says nothing
    /// about connection progress, `ConnectionManager` does.
    pub fn statuses(&self, manager: &ConnectionManager) -> Vec<NetworkStatus> {
        self.active
            .lock()
            .expect("mesh session lock poisoned")
            .iter()
            .map(|(id, active)| {
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
                NetworkStatus {
                    id: id.clone(),
                    network_name: active.network_name.clone(),
                    is_host: active.is_host,
                    host_addr: active.host_addr.to_string(),
                    game_tag: active.game_tag.clone(),
                    members,
                    reconnecting: active.reconnecting.load(std::sync::atomic::Ordering::Relaxed),
                }
            })
            .collect()
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
            MeshOrchestrator::new(manager_a.clone(), id_a.clone(), ConnectionSettings::default(), socket_a, vec![cand_a], Arc::new(Mutex::new(Vec::new())));
        let mut orch_b =
            MeshOrchestrator::new(manager_b.clone(), id_b.clone(), ConnectionSettings::default(), socket_b, vec![cand_b], Arc::new(Mutex::new(Vec::new())));

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
        let mut orch = MeshOrchestrator::new(manager, id, ConnectionSettings::default(), socket, vec![cand], Arc::new(Mutex::new(Vec::new())));
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
        let mut orch = MeshOrchestrator::new(manager, id, ConnectionSettings::default(), socket, vec![cand], Arc::new(Mutex::new(Vec::new())));
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
    async fn mesh_session_statuses_is_empty_when_not_in_any_network() {
        let session = MeshSession::default();
        let manager = ConnectionManager::default();
        assert!(session.statuses(&manager).is_empty());
    }

    #[tokio::test]
    async fn mesh_session_create_then_status_reports_self_as_host_with_no_members() {
        let session = MeshSession::default();
        let manager = Arc::new(ConnectionManager::default());
        let identity = Arc::new(Identity::generate().unwrap());

        let (id, host_addr) = session
            .create(
                "127.0.0.1:0".parse().unwrap(),
                "party".to_string(),
                "secret".to_string(),
                None,
                ConnectionSettings::default(),
                identity,
                manager.clone(),
                || Box::new(NullSink),
                None,
            )
            .await
            .unwrap();
        assert_ne!(host_addr.port(), 0); // an ephemeral port was actually assigned

        let statuses = session.statuses(&manager);
        assert_eq!(statuses.len(), 1);
        let status = &statuses[0];
        assert_eq!(status.id, id);
        assert_eq!(status.network_name, "party");
        assert!(status.is_host);
        assert_eq!(status.host_addr, host_addr.to_string());
        assert_eq!(status.members.len(), 0); // only member so far is ourselves, never listed

        session.leave(&id).await;
        assert!(session.statuses(&manager).is_empty());
    }

    /// Regression test for a real user-hit bug: `create()` with the UI's
    /// actual default bind address (`0.0.0.0:0`, not `127.0.0.1:0` like the
    /// test above) failed outright on Windows with "os error 10049"
    /// (WSAEADDRNOTAVAIL) — the self-join step connected to the literal
    /// unspecified address instead of a resolved concrete one. Every other
    /// test in this file uses `127.0.0.1` and would never have caught this.
    #[tokio::test]
    async fn mesh_session_create_succeeds_with_the_uis_actual_default_bind_address() {
        let session = MeshSession::default();
        let manager = Arc::new(ConnectionManager::default());
        let identity = Arc::new(Identity::generate().unwrap());

        let (id, host_addr) = session
            .create(
                "0.0.0.0:0".parse().unwrap(),
                "party".to_string(),
                "secret".to_string(),
                None,
                ConnectionSettings::default(),
                identity,
                manager.clone(),
                || Box::new(NullSink),
                None,
            )
            .await
            .unwrap();

        assert_ne!(host_addr.ip(), Ipv4Addr::UNSPECIFIED, "must not advertise/self-join 0.0.0.0");
        assert!(!session.statuses(&manager).is_empty(), "self-join must have actually succeeded");

        session.leave(&id).await;
    }

    #[tokio::test]
    async fn mesh_session_create_carries_the_game_tag_and_settings_into_the_orchestrator() {
        let session = MeshSession::default();
        let manager = Arc::new(ConnectionManager::default());
        let identity = Arc::new(Identity::generate().unwrap());
        let settings = ConnectionSettings {
            forward_broadcast: true,
            forward_multicast: true,
            fec_parity_shards: 2,
            extra_routes: Vec::new(),
        };

        let (id, _) = session
            .create(
                "127.0.0.1:0".parse().unwrap(),
                "party".to_string(),
                "secret".to_string(),
                Some("minecraft".to_string()),
                settings,
                identity,
                manager.clone(),
                || Box::new(NullSink),
                None,
            )
            .await
            .unwrap();

        let statuses = session.statuses(&manager);
        assert_eq!(statuses[0].game_tag, Some("minecraft".to_string()));

        session.leave(&id).await;
    }

    #[tokio::test]
    async fn mesh_session_status_has_no_game_tag_when_not_supplied() {
        let session = MeshSession::default();
        let manager = Arc::new(ConnectionManager::default());
        let identity = Arc::new(Identity::generate().unwrap());

        let (id, _) = session
            .create(
                "127.0.0.1:0".parse().unwrap(),
                "party".to_string(),
                "secret".to_string(),
                None,
                ConnectionSettings::default(),
                identity,
                manager.clone(),
                || Box::new(NullSink),
                None,
            )
            .await
            .unwrap();

        assert_eq!(session.statuses(&manager)[0].game_tag, None);

        session.leave(&id).await;
    }

    /// The regression this guards: two networks hosted/joined on the same
    /// `MeshSession` must coexist — `create`/`join` no longer reject a
    /// second membership, `statuses()` reports both, and leaving one leaves
    /// the other completely untouched.
    #[tokio::test]
    async fn mesh_session_supports_two_simultaneous_networks() {
        let session = MeshSession::default();
        let manager = Arc::new(ConnectionManager::default());
        let identity = Arc::new(Identity::generate().unwrap());

        let (id_a, _) = session
            .create(
                "127.0.0.1:0".parse().unwrap(),
                "party-a".to_string(),
                "secret".to_string(),
                None,
                ConnectionSettings::default(),
                identity.clone(),
                manager.clone(),
                || Box::new(NullSink),
                None,
            )
            .await
            .unwrap();

        let (id_b, _) = session
            .create(
                "127.0.0.1:0".parse().unwrap(),
                "party-b".to_string(),
                "secret".to_string(),
                None,
                ConnectionSettings::default(),
                identity,
                manager.clone(),
                || Box::new(NullSink),
                None,
            )
            .await
            .unwrap();

        assert_ne!(id_a, id_b);
        let statuses = session.statuses(&manager);
        assert_eq!(statuses.len(), 2);
        assert!(statuses.iter().any(|s| s.id == id_a && s.network_name == "party-a"));
        assert!(statuses.iter().any(|s| s.id == id_b && s.network_name == "party-b"));

        session.leave(&id_a).await;
        let statuses = session.statuses(&manager);
        assert_eq!(statuses.len(), 1);
        assert_eq!(statuses[0].id, id_b);

        session.leave(&id_b).await;
        assert!(session.statuses(&manager).is_empty());
    }

    /// The end-to-end proof at the Tauri-facing layer: one `MeshSession`
    /// hosts, another joins it, and — through nothing but `create`/`join` —
    /// both `statuses()` calls eventually show the other as `Connected`.
    #[tokio::test]
    async fn two_mesh_sessions_create_and_join_reach_connected() {
        let session_a = MeshSession::default();
        let session_b = MeshSession::default();
        let manager_a = Arc::new(ConnectionManager::default());
        let manager_b = Arc::new(ConnectionManager::default());
        let identity_a = Arc::new(Identity::generate().unwrap());
        let identity_b = Arc::new(Identity::generate().unwrap());

        let (_, host_addr) = session_a
            .create(
                "127.0.0.1:0".parse().unwrap(),
                "party".to_string(),
                "secret".to_string(),
                None,
                ConnectionSettings::default(),
                identity_a,
                manager_a.clone(),
                || Box::new(NullSink),
                None,
            )
            .await
            .unwrap();

        session_b
            .join(
                host_addr,
                "party".to_string(),
                "secret".to_string(),
                None,
                ConnectionSettings::default(),
                identity_b,
                manager_b.clone(),
                || Box::new(NullSink),
                None,
            )
            .await
            .unwrap();

        until(15, "both sides show Connected in status()", move || {
            let sa = session_a.statuses(&manager_a);
            let sb = session_b.statuses(&manager_b);
            let a_sees_b = sa.iter().any(|s| s.members.iter().any(|m| m.link == LinkState::Connected));
            let b_sees_a = sb.iter().any(|s| s.members.iter().any(|m| m.link == LinkState::Connected));
            a_sees_b && b_sees_a
        })
        .await;
    }

    /// The behavior the whole retry loop exists for: a joiner whose host
    /// disconnects it unexpectedly (here, by the host leaving — which shuts
    /// down its `SignalingServer` and severs every member's connection,
    /// exactly like a real network blip would from the joiner's point of
    /// view) shows `reconnecting: true`, and `leave()` on the joiner's side
    /// returns promptly rather than waiting out the backoff — proving
    /// `cancel` actually interrupts the retry loop's sleep, not just the
    /// connected state.
    #[tokio::test]
    async fn joiner_shows_reconnecting_after_the_host_disappears_and_leave_is_prompt() {
        let session_a = MeshSession::default();
        let session_b = MeshSession::default();
        let manager_a = Arc::new(ConnectionManager::default());
        let manager_b = Arc::new(ConnectionManager::default());
        let identity_a = Arc::new(Identity::generate().unwrap());
        let identity_b = Arc::new(Identity::generate().unwrap());

        let (host_id, host_addr) = session_a
            .create(
                "127.0.0.1:0".parse().unwrap(),
                "party".to_string(),
                "secret".to_string(),
                None,
                ConnectionSettings::default(),
                identity_a,
                manager_a.clone(),
                || Box::new(NullSink),
                None,
            )
            .await
            .unwrap();

        let join_id = session_b
            .join(
                host_addr,
                "party".to_string(),
                "secret".to_string(),
                None,
                ConnectionSettings::default(),
                identity_b,
                manager_b.clone(),
                || Box::new(NullSink),
                None,
            )
            .await
            .unwrap();

        until(15, "joiner shows Connected before disconnecting the host", || {
            session_b.statuses(&manager_b).iter().any(|s| s.members.iter().any(|m| m.link == LinkState::Connected))
        })
        .await;

        session_a.leave(&host_id).await; // shuts down the host's server out from under session_b

        until(5, "joiner shows reconnecting after the host disappears", || {
            session_b.statuses(&manager_b).iter().any(|s| s.reconnecting)
        })
        .await;

        // Bounded well under INITIAL_RETRY_DELAY's 2s-and-doubling backoff —
        // if leave() actually waited out a sleep instead of racing `cancel`
        // against it, this would time out.
        tokio::time::timeout(Duration::from_millis(500), session_b.leave(&join_id))
            .await
            .expect("leave() should cancel the retry loop promptly, not wait out the backoff");
        assert!(session_b.statuses(&manager_b).is_empty());
    }

    /// Proves the retry loop doesn't just detect a drop but actually
    /// recovers from one: the joiner's host disappears, then a new host
    /// comes back up on the exact same address shortly after — the joiner
    /// should reconnect on its own, ending with both sides `Connected`
    /// again, with no `leave`/re-`join` on the joiner's side at all.
    #[tokio::test]
    async fn joiner_reconnects_once_the_host_comes_back_on_the_same_address() {
        let session_a = MeshSession::default();
        let session_b = MeshSession::default();
        let manager_a = Arc::new(ConnectionManager::default());
        let manager_b = Arc::new(ConnectionManager::default());
        let identity_a = Arc::new(Identity::generate().unwrap());
        let identity_b = Arc::new(Identity::generate().unwrap());

        let (host_id, host_addr) = session_a
            .create(
                "127.0.0.1:0".parse().unwrap(),
                "party".to_string(),
                "secret".to_string(),
                None,
                ConnectionSettings::default(),
                identity_a.clone(),
                manager_a.clone(),
                || Box::new(NullSink),
                None,
            )
            .await
            .unwrap();

        session_b
            .join(
                host_addr,
                "party".to_string(),
                "secret".to_string(),
                None,
                ConnectionSettings::default(),
                identity_b,
                manager_b.clone(),
                || Box::new(NullSink),
                None,
            )
            .await
            .unwrap();

        until(15, "joiner shows Connected before disconnecting the host", || {
            session_b.statuses(&manager_b).iter().any(|s| s.members.iter().any(|m| m.link == LinkState::Connected))
        })
        .await;

        session_a.leave(&host_id).await;

        until(5, "joiner shows reconnecting after the host disappears", || {
            session_b.statuses(&manager_b).iter().any(|s| s.reconnecting)
        })
        .await;

        // Bring the host back on the exact same port the joiner is retrying
        // against — the retry loop must find it without any help from us.
        session_a
            .create(
                host_addr,
                "party".to_string(),
                "secret".to_string(),
                None,
                ConnectionSettings::default(),
                identity_a,
                manager_a.clone(),
                || Box::new(NullSink),
                None,
            )
            .await
            .unwrap();

        until(15, "joiner reconnects and both sides show Connected again", || {
            let sa = session_a.statuses(&manager_a);
            let sb = session_b.statuses(&manager_b);
            let a_sees_b = sa.iter().any(|s| s.members.iter().any(|m| m.link == LinkState::Connected));
            let b_sees_a = sb.iter().any(|s| !s.reconnecting && s.members.iter().any(|m| m.link == LinkState::Connected));
            a_sees_b && b_sees_a
        })
        .await;
    }

    /// The reason this whole module exists: proves `create`/`join` reach
    /// `Connected` through a [`RelayServer`](super::super::relay::RelayServer)
    /// exactly like `two_mesh_sessions_create_and_join_reach_connected` does
    /// directly — i.e. this is that same test, with `relay_addr: Some(_)`
    /// instead of `None`, and neither side able to dial the other's `host_addr`
    /// (they never even see one — see `create`/`join`'s doc comments on what
    /// `relay_addr: Some(_)` does to that parameter).
    #[tokio::test]
    async fn two_mesh_sessions_create_and_join_reach_connected_through_a_relay() {
        use crate::engine::relay::RelayServer;

        let relay = RelayServer::start("127.0.0.1:0".parse().unwrap()).await.unwrap();
        let relay_addr = relay.local_addr();

        let session_a = MeshSession::default();
        let session_b = MeshSession::default();
        let manager_a = Arc::new(ConnectionManager::default());
        let manager_b = Arc::new(ConnectionManager::default());
        let identity_a = Arc::new(Identity::generate().unwrap());
        let identity_b = Arc::new(Identity::generate().unwrap());

        session_a
            .create(
                "127.0.0.1:0".parse().unwrap(), // ignored — relay_addr wins
                "party".to_string(),
                "secret".to_string(),
                None,
                ConnectionSettings::default(),
                identity_a,
                manager_a.clone(),
                || Box::new(NullSink),
                Some(relay_addr),
            )
            .await
            .unwrap();

        session_b
            .join(
                "127.0.0.1:0".parse().unwrap(), // ignored — relay_addr wins
                "party".to_string(),
                "secret".to_string(),
                None,
                ConnectionSettings::default(),
                identity_b,
                manager_b.clone(),
                || Box::new(NullSink),
                Some(relay_addr),
            )
            .await
            .unwrap();

        until(15, "both sides show Connected in status()", move || {
            let sa = session_a.statuses(&manager_a);
            let sb = session_b.statuses(&manager_b);
            let a_sees_b = sa.iter().any(|s| s.members.iter().any(|m| m.link == LinkState::Connected));
            let b_sees_a = sb.iter().any(|s| s.members.iter().any(|m| m.link == LinkState::Connected));
            a_sees_b && b_sees_a
        })
        .await;

        relay.shutdown().await;
    }

    /// The bug this guards against: hosting with the UI's default
    /// `0.0.0.0:0` bind address must never surface `0.0.0.0` as "the
    /// address to share" — that's not connectable by anyone, including a
    /// second instance on the same machine.
    #[test]
    fn advertisable_addr_resolves_unspecified_ip() {
        let resolved = advertisable_addr("0.0.0.0:51820".parse().unwrap());
        assert_ne!(resolved.ip(), Ipv4Addr::UNSPECIFIED, "must not advertise 0.0.0.0");
        assert_eq!(resolved.port(), 51820, "port must be preserved exactly");
    }

    #[test]
    fn advertisable_addr_leaves_a_concrete_ip_unchanged() {
        let addr: SocketAddr = "192.168.1.42:51820".parse().unwrap();
        assert_eq!(advertisable_addr(addr), addr);
    }
}
