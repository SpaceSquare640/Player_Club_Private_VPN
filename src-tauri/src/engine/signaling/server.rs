//! Networked signaling server (G.1): the "host" side of a Hamachi-style
//! virtual network. Listens for WebSocket connections, gatekeeps them by
//! network name + password, and maintains/broadcasts the member roster.
//!
//! Deliberately does nothing else yet — relaying offer/answer/ICE between
//! members (so joining a network auto-establishes P2P links) is Phase G.3.
//! This phase only makes "who's on the network" a solved problem.
//!
//! Not yet wired into any Tauri command (that's G.2/G.4), so the public API
//! is exercised only by this module's own tests for now — hence the blanket
//! allow, rather than papering over each item individually.
#![allow(dead_code)]

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use sha2::{Digest, Sha256};
use socket2::{Domain, Socket, Type};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, watch};
use tokio_tungstenite::tungstenite::Message as WsMessage;

use crate::engine::relay::protocol::{read_server_frame, write_client_frame, ClientFrame as RelayClientFrame, ServerFrame as RelayServerFrame};
use super::protocol::{ClientMessage, JoinRejectReason, MemberInfo, ServerMessage, PROTOCOL_VERSION};

#[derive(Debug, thiserror::Error)]
pub enum SignalingError {
    #[error("failed to bind signaling server: {0}")]
    Bind(#[source] std::io::Error),
    #[error("failed to reach relay: {0}")]
    RelayConnect(#[source] std::io::Error),
    #[error("relay rejected registration: {0}")]
    RelayRejected(String),
    #[error("relay sent an unexpected reply to REGISTER")]
    RelayMalformedReply,
}

/// Same reasoning as `signaling::client`'s `CONNECT_TIMEOUT`: fail fast on an
/// unreachable relay instead of waiting out the OS's own TCP connect timeout.
const RELAY_CONNECT_TIMEOUT: Duration = Duration::from_secs(8);

/// Hex SHA-256 digest of a network password. The server only ever sees and
/// stores this — never the plaintext password — matching the boundary
/// `ClientMessage::Join` already draws on the wire.
pub fn hash_password(password: &str) -> String {
    let digest = Sha256::digest(password.as_bytes());
    hex_encode(&digest)
}

fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Binds a TCP listener with `SO_REUSEADDR` set — plain `TcpListener::bind`
/// doesn't set it, and on Windows a listening socket can fail to rebind a
/// port immediately after a previous listener on it closed (lingering
/// per-connection state, not the listener itself, but Windows is stricter
/// about it than Linux/macOS). This matters concretely for this app:
/// recreating a network — including auto-reconnect's host-side self-join
/// after a drop — can land on the exact same address moments after leaving
/// it, and that rebind needs to just work.
fn bind_reuseaddr(addr: SocketAddr) -> std::io::Result<TcpListener> {
    let socket = Socket::new(Domain::for_address(addr), Type::STREAM, None)?;
    socket.set_reuse_address(true)?;
    socket.bind(&addr.into())?;
    socket.listen(1024)?;
    socket.set_nonblocking(true)?;
    TcpListener::from_std(socket.into())
}

struct MemberHandle {
    fingerprint: String,
    tx: mpsc::UnboundedSender<ServerMessage>,
}

struct SharedState {
    network_name: String,
    password_hash: String,
    members: Mutex<HashMap<String, MemberHandle>>,
}

/// A running signaling server. Dropping this (or calling [`shutdown`]) stops
/// accepting new connections and closes every open one.
///
/// [`shutdown`]: SignalingServer::shutdown
pub struct SignalingServer {
    local_addr: SocketAddr,
    state: Arc<SharedState>,
    cancel: watch::Sender<bool>,
    accept_task: tokio::task::JoinHandle<()>,
}

impl SignalingServer {
    /// Binds `bind_addr` and starts accepting connections in the background.
    /// `password` is hashed immediately; the plaintext is not retained.
    pub async fn start(
        bind_addr: SocketAddr,
        network_name: impl Into<String>,
        password: &str,
    ) -> Result<Self, SignalingError> {
        let listener = bind_reuseaddr(bind_addr).map_err(SignalingError::Bind)?;
        let local_addr = listener.local_addr().map_err(SignalingError::Bind)?;

        let state = Arc::new(SharedState {
            network_name: network_name.into(),
            password_hash: hash_password(password),
            members: Mutex::new(HashMap::new()),
        });

        let (cancel, cancel_rx) = watch::channel(false);
        let accept_task = tokio::spawn(accept_loop(listener, state.clone(), cancel_rx));

        Ok(Self { local_addr, state, cancel, accept_task })
    }

    /// Like [`start`](Self::start), but reachable across the internet
    /// without any port forwarding: instead of binding a local listener,
    /// this registers `network_name` on a [`RelayServer`](crate::engine::relay::RelayServer)
    /// at `relay_addr` and accepts every member the relay pairs us with from
    /// there. Everything above this point — the WebSocket accept handshake,
    /// roster tracking, relay-of-offers (a different, unrelated meaning of
    /// "relay" — see `mesh.rs`) — is the exact same `handle_connection` the
    /// direct-bind path uses; only how connections *arrive* differs.
    ///
    /// [`local_addr`](Self::local_addr) reports `relay_addr` itself (there's
    /// no local bind address meaningful to show) — a joiner only needs the
    /// network name plus this same relay address to reach it.
    pub async fn start_via_relay(
        relay_addr: SocketAddr,
        network_name: impl Into<String>,
        password: &str,
    ) -> Result<Self, SignalingError> {
        let network_name = network_name.into();
        let mut control = tokio::time::timeout(RELAY_CONNECT_TIMEOUT, TcpStream::connect(relay_addr))
            .await
            .map_err(|_| SignalingError::RelayConnect(std::io::Error::new(std::io::ErrorKind::TimedOut, "connect timed out")))?
            .map_err(SignalingError::RelayConnect)?;
        write_client_frame(&mut control, &RelayClientFrame::Register { network_name: network_name.clone() })
            .await
            .map_err(SignalingError::RelayConnect)?;

        match read_server_frame(&mut control).await.map_err(SignalingError::RelayConnect)? {
            Some(RelayServerFrame::Ok) => {}
            Some(RelayServerFrame::Err { message }) => return Err(SignalingError::RelayRejected(message)),
            _ => return Err(SignalingError::RelayMalformedReply),
        }

        let state = Arc::new(SharedState {
            network_name,
            password_hash: hash_password(password),
            members: Mutex::new(HashMap::new()),
        });

        let (cancel, cancel_rx) = watch::channel(false);
        let accept_task = tokio::spawn(relay_accept_loop(control, relay_addr, state.clone(), cancel_rx));

        Ok(Self { local_addr: relay_addr, state, cancel, accept_task })
    }

    pub fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }

    /// Snapshot of the current roster, in no particular order.
    pub fn members(&self) -> Vec<MemberInfo> {
        self.state
            .members
            .lock()
            .expect("signaling members lock poisoned")
            .iter()
            .map(|(pubkey, h)| MemberInfo { pubkey: pubkey.clone(), fingerprint: h.fingerprint.clone() })
            .collect()
    }

    /// Stops accepting new connections and closes every open one.
    pub async fn shutdown(self) {
        let _ = self.cancel.send(true);
        let _ = self.accept_task.await;
    }
}

async fn accept_loop(listener: TcpListener, state: Arc<SharedState>, mut cancel_rx: watch::Receiver<bool>) {
    loop {
        tokio::select! {
            _ = cancel_rx.changed() => break,
            accepted = listener.accept() => {
                let Ok((stream, _peer_addr)) = accepted else { continue };
                let state = state.clone();
                let member_cancel_rx = cancel_rx.clone();
                tokio::spawn(async move {
                    let _ = handle_connection(stream, state, member_cancel_rx).await;
                });
            }
        }
    }
}

/// The relay-backed counterpart to [`accept_loop`]: instead of `TcpListener::accept()`
/// producing new connections, each `NEW_MEMBER` notice on the relay's control
/// connection does — dial the relay again, `ACCEPT` that session, and hand
/// the resulting stream to the same [`handle_connection`] the direct path uses.
async fn relay_accept_loop(
    mut control: TcpStream,
    relay_addr: SocketAddr,
    state: Arc<SharedState>,
    mut cancel_rx: watch::Receiver<bool>,
) {
    loop {
        tokio::select! {
            _ = cancel_rx.changed() => break,
            frame = read_server_frame(&mut control) => {
                let session_id = match frame {
                    Ok(Some(RelayServerFrame::NewMember { session_id })) => session_id,
                    Ok(Some(_)) => continue, // not expected post-registration; ignored, not fatal
                    Ok(None) | Err(_) => break, // control connection closed — the relay considers us gone
                };
                let Ok(mut data) = TcpStream::connect(relay_addr).await else { continue };
                if write_client_frame(&mut data, &RelayClientFrame::Accept { session_id }).await.is_err() {
                    continue;
                }
                let state = state.clone();
                let member_cancel_rx = cancel_rx.clone();
                tokio::spawn(async move {
                    let _ = handle_connection(data, state, member_cancel_rx).await;
                });
            }
        }
    }
}

async fn handle_connection(
    stream: TcpStream,
    state: Arc<SharedState>,
    mut cancel_rx: watch::Receiver<bool>,
) -> Result<(), ()> {
    let ws = tokio_tungstenite::accept_async(stream).await.map_err(|_| ())?;
    let (mut ws_tx, mut ws_rx) = ws.split();

    // The first frame must be a well-formed Join, or the connection is
    // rejected and dropped — no partial membership state.
    let join_frame = ws_rx.next().await.ok_or(())?.map_err(|_| ())?;
    let ClientMessage::Join { v, network_name, password_hash, pubkey, fingerprint } =
        parse_client_message(&join_frame).ok_or(())?
    else {
        return Ok(());
    };

    let reject = |reason: JoinRejectReason| -> Option<ServerMessage> { Some(ServerMessage::JoinRejected { reason }) };
    let rejection = if v != PROTOCOL_VERSION {
        reject(JoinRejectReason::UnsupportedVersion)
    } else if network_name != state.network_name {
        reject(JoinRejectReason::WrongNetworkName)
    } else if password_hash != state.password_hash {
        reject(JoinRejectReason::WrongPassword)
    } else if state.members.lock().expect("signaling members lock poisoned").contains_key(&pubkey) {
        reject(JoinRejectReason::AlreadyJoined)
    } else {
        None
    };

    if let Some(msg) = rejection {
        let _ = send(&mut ws_tx, &msg).await;
        return Ok(());
    }

    let (tx, mut rx) = mpsc::unbounded_channel::<ServerMessage>();
    let existing_members = {
        let mut members = state.members.lock().expect("signaling members lock poisoned");
        let snapshot: Vec<MemberInfo> = members
            .iter()
            .map(|(pk, h)| MemberInfo { pubkey: pk.clone(), fingerprint: h.fingerprint.clone() })
            .collect();
        members.insert(pubkey.clone(), MemberHandle { fingerprint: fingerprint.clone(), tx });
        snapshot
    };

    send(&mut ws_tx, &ServerMessage::JoinAccepted { members: existing_members }).await.map_err(|_| ())?;
    broadcast(&state, &pubkey, &ServerMessage::MemberJoined(MemberInfo { pubkey: pubkey.clone(), fingerprint }));

    // Keep the connection open: forward outgoing broadcasts/relays to this
    // member, and relay any `Relay` this member sends on to its target.
    loop {
        tokio::select! {
            _ = cancel_rx.changed() => break,
            outgoing = rx.recv() => {
                match outgoing {
                    Some(msg) => { if send(&mut ws_tx, &msg).await.is_err() { break; } }
                    None => break,
                }
            }
            incoming = ws_rx.next() => {
                match incoming {
                    Some(Ok(frame)) => {
                        if let Some(ClientMessage::Relay { to_pubkey, blob }) = parse_client_message(&frame) {
                            relay_to(&state, &to_pubkey, &pubkey, blob);
                        }
                    }
                    _ => break,
                }
            }
        }
    }

    state.members.lock().expect("signaling members lock poisoned").remove(&pubkey);
    broadcast(&state, &pubkey, &ServerMessage::MemberLeft { pubkey: pubkey.clone() });
    Ok(())
}

fn parse_client_message(frame: &WsMessage) -> Option<ClientMessage> {
    let text = frame.to_text().ok()?;
    serde_json::from_str(text).ok()
}

async fn send(
    ws_tx: &mut futures_util::stream::SplitSink<
        tokio_tungstenite::WebSocketStream<TcpStream>,
        WsMessage,
    >,
    msg: &ServerMessage,
) -> Result<(), ()> {
    let text = serde_json::to_string(msg).map_err(|_| ())?;
    ws_tx.send(WsMessage::Text(text.into())).await.map_err(|_| ())
}

/// Sends `msg` to every member except `exclude_pubkey` (typically the member
/// that just triggered the event being broadcast).
fn broadcast(state: &SharedState, exclude_pubkey: &str, msg: &ServerMessage) {
    let members = state.members.lock().expect("signaling members lock poisoned");
    for (pubkey, handle) in members.iter() {
        if pubkey != exclude_pubkey {
            let _ = handle.tx.send(msg.clone());
        }
    }
}

/// Forwards a `Relay`'s blob to `to_pubkey` as `Relayed`. A no-op if
/// `to_pubkey` isn't currently a member — an expected race (they may have
/// just left), not treated as an error.
fn relay_to(state: &SharedState, to_pubkey: &str, from_pubkey: &str, blob: String) {
    let members = state.members.lock().expect("signaling members lock poisoned");
    if let Some(handle) = members.get(to_pubkey) {
        let _ = handle.tx.send(ServerMessage::Relayed { from_pubkey: from_pubkey.to_string(), blob });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::signaling::protocol::ClientMessage;
    use tokio_tungstenite::connect_async;

    type TestSocket = tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<TcpStream>,
    >;

    async fn connect(addr: SocketAddr) -> TestSocket {
        let (ws, _resp) = connect_async(format!("ws://{addr}")).await.expect("connect");
        ws
    }

    async fn join(
        ws: &mut TestSocket,
        network_name: &str,
        password: &str,
        pubkey: &str,
        fingerprint: &str,
    ) {
        let msg = ClientMessage::Join {
            v: PROTOCOL_VERSION,
            network_name: network_name.to_string(),
            password_hash: hash_password(password),
            pubkey: pubkey.to_string(),
            fingerprint: fingerprint.to_string(),
        };
        ws.send(WsMessage::Text(serde_json::to_string(&msg).unwrap().into()))
            .await
            .expect("send join");
    }

    async fn recv(ws: &mut TestSocket) -> ServerMessage {
        let frame = ws.next().await.expect("stream ended").expect("ws error");
        serde_json::from_str(frame.to_text().unwrap()).expect("malformed server message")
    }

    #[tokio::test]
    async fn accepts_a_correct_join_with_an_empty_roster() {
        let server = SignalingServer::start("127.0.0.1:0".parse().unwrap(), "party", "secret")
            .await
            .unwrap();

        let mut ws = connect(server.local_addr()).await;
        join(&mut ws, "party", "secret", "pkA", "PC-AAAA-AAAA-AAAA-AAAA").await;

        assert_eq!(recv(&mut ws).await, ServerMessage::JoinAccepted { members: vec![] });
        server.shutdown().await;
    }

    #[tokio::test]
    async fn rejects_the_wrong_password() {
        let server = SignalingServer::start("127.0.0.1:0".parse().unwrap(), "party", "secret")
            .await
            .unwrap();

        let mut ws = connect(server.local_addr()).await;
        join(&mut ws, "party", "not-the-password", "pkA", "PC-AAAA-AAAA-AAAA-AAAA").await;

        assert_eq!(
            recv(&mut ws).await,
            ServerMessage::JoinRejected { reason: JoinRejectReason::WrongPassword }
        );
        server.shutdown().await;
    }

    #[tokio::test]
    async fn rejects_the_wrong_network_name() {
        let server = SignalingServer::start("127.0.0.1:0".parse().unwrap(), "party", "secret")
            .await
            .unwrap();

        let mut ws = connect(server.local_addr()).await;
        join(&mut ws, "not-the-network", "secret", "pkA", "PC-AAAA-AAAA-AAAA-AAAA").await;

        assert_eq!(
            recv(&mut ws).await,
            ServerMessage::JoinRejected { reason: JoinRejectReason::WrongNetworkName }
        );
        server.shutdown().await;
    }

    #[tokio::test]
    async fn a_second_member_sees_the_first_and_the_first_is_notified() {
        let server = SignalingServer::start("127.0.0.1:0".parse().unwrap(), "party", "secret")
            .await
            .unwrap();

        let mut ws_a = connect(server.local_addr()).await;
        join(&mut ws_a, "party", "secret", "pkA", "PC-AAAA-AAAA-AAAA-AAAA").await;
        assert_eq!(recv(&mut ws_a).await, ServerMessage::JoinAccepted { members: vec![] });

        let mut ws_b = connect(server.local_addr()).await;
        join(&mut ws_b, "party", "secret", "pkB", "PC-BBBB-BBBB-BBBB-BBBB").await;

        let b_joined = recv(&mut ws_b).await;
        assert_eq!(
            b_joined,
            ServerMessage::JoinAccepted {
                members: vec![MemberInfo {
                    pubkey: "pkA".to_string(),
                    fingerprint: "PC-AAAA-AAAA-AAAA-AAAA".to_string(),
                }],
            }
        );

        let a_notified = recv(&mut ws_a).await;
        assert_eq!(
            a_notified,
            ServerMessage::MemberJoined(MemberInfo {
                pubkey: "pkB".to_string(),
                fingerprint: "PC-BBBB-BBBB-BBBB-BBBB".to_string(),
            })
        );

        server.shutdown().await;
    }

    #[tokio::test]
    async fn a_second_pubkey_cannot_join_twice() {
        let server = SignalingServer::start("127.0.0.1:0".parse().unwrap(), "party", "secret")
            .await
            .unwrap();

        let mut ws_a = connect(server.local_addr()).await;
        join(&mut ws_a, "party", "secret", "pkA", "PC-AAAA-AAAA-AAAA-AAAA").await;
        assert_eq!(recv(&mut ws_a).await, ServerMessage::JoinAccepted { members: vec![] });

        let mut ws_a2 = connect(server.local_addr()).await;
        join(&mut ws_a2, "party", "secret", "pkA", "PC-AAAA-AAAA-AAAA-AAAA").await;
        assert_eq!(
            recv(&mut ws_a2).await,
            ServerMessage::JoinRejected { reason: JoinRejectReason::AlreadyJoined }
        );

        server.shutdown().await;
    }

    #[tokio::test]
    async fn other_members_are_notified_when_someone_leaves() {
        let server = SignalingServer::start("127.0.0.1:0".parse().unwrap(), "party", "secret")
            .await
            .unwrap();

        let mut ws_a = connect(server.local_addr()).await;
        join(&mut ws_a, "party", "secret", "pkA", "PC-AAAA-AAAA-AAAA-AAAA").await;
        assert_eq!(recv(&mut ws_a).await, ServerMessage::JoinAccepted { members: vec![] });

        let mut ws_b = connect(server.local_addr()).await;
        join(&mut ws_b, "party", "secret", "pkB", "PC-BBBB-BBBB-BBBB-BBBB").await;
        recv(&mut ws_b).await; // JoinAccepted
        recv(&mut ws_a).await; // MemberJoined(B)

        drop(ws_b);

        let a_notified = recv(&mut ws_a).await;
        assert_eq!(a_notified, ServerMessage::MemberLeft { pubkey: "pkB".to_string() });

        server.shutdown().await;
    }

    #[tokio::test]
    async fn members_snapshot_reflects_the_live_roster() {
        let server = SignalingServer::start("127.0.0.1:0".parse().unwrap(), "party", "secret")
            .await
            .unwrap();
        assert_eq!(server.members(), vec![]);

        let mut ws_a = connect(server.local_addr()).await;
        join(&mut ws_a, "party", "secret", "pkA", "PC-AAAA-AAAA-AAAA-AAAA").await;
        recv(&mut ws_a).await; // JoinAccepted

        assert_eq!(
            server.members(),
            vec![MemberInfo { pubkey: "pkA".to_string(), fingerprint: "PC-AAAA-AAAA-AAAA-AAAA".to_string() }],
        );

        server.shutdown().await;
    }

    #[tokio::test]
    async fn relays_a_blob_to_its_target_and_silently_drops_relays_to_unknown_members() {
        let server = SignalingServer::start("127.0.0.1:0".parse().unwrap(), "party", "secret")
            .await
            .unwrap();

        let mut ws_a = connect(server.local_addr()).await;
        join(&mut ws_a, "party", "secret", "pkA", "PC-AAAA-AAAA-AAAA-AAAA").await;
        recv(&mut ws_a).await; // JoinAccepted

        let mut ws_b = connect(server.local_addr()).await;
        join(&mut ws_b, "party", "secret", "pkB", "PC-BBBB-BBBB-BBBB-BBBB").await;
        recv(&mut ws_b).await; // JoinAccepted
        recv(&mut ws_a).await; // MemberJoined(B)

        // Relaying to a member that doesn't exist is a silent no-op — proven
        // by then successfully relaying to a real member on the same socket.
        ws_a.send(WsMessage::Text(
            serde_json::to_string(&ClientMessage::Relay {
                to_pubkey: "does-not-exist".to_string(),
                blob: "irrelevant".to_string(),
            })
            .unwrap()
            .into(),
        ))
        .await
        .unwrap();

        ws_a.send(WsMessage::Text(
            serde_json::to_string(&ClientMessage::Relay {
                to_pubkey: "pkB".to_string(),
                blob: "PCPV1.OFFER.example.deadbeef".to_string(),
            })
            .unwrap()
            .into(),
        ))
        .await
        .unwrap();

        let relayed = recv(&mut ws_b).await;
        assert_eq!(
            relayed,
            ServerMessage::Relayed {
                from_pubkey: "pkA".to_string(),
                blob: "PCPV1.OFFER.example.deadbeef".to_string(),
            }
        );

        server.shutdown().await;
    }

    #[test]
    fn hash_password_is_deterministic_and_not_the_plaintext() {
        let h1 = hash_password("hunter2");
        let h2 = hash_password("hunter2");
        assert_eq!(h1, h2);
        assert_ne!(h1, "hunter2");
        assert_eq!(h1.len(), 64); // hex-encoded SHA-256
    }
}
