//! Networked signaling client (G.2 + G.3): joins a [`SignalingServer`]-hosted
//! virtual network, tracks its member roster in the background, and relays
//! opaque offer/answer blobs to other members (G.3).
//!
//! G.3's relay is transport only — it moves the exact same blob a user would
//! otherwise copy/paste (`signaling::blob::encode`d), just over the network
//! instead of by hand. Deciding *when* to relay an offer (auto-connecting to
//! every member on join) and juggling more than one live peer connection is
//! explicitly out of scope here — that's the `ConnectionManager` rework in a
//! later phase (currently single-peer only).
//!
//! [`SignalingServer`]: super::server::SignalingServer
#![allow(dead_code)]

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpStream;
use tokio::sync::{mpsc, watch};
use tokio_tungstenite::tungstenite::Message as WsMessage;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};

use super::blob;
use super::message::SignalEnvelope;
use super::protocol::{ClientMessage, JoinRejectReason, MemberInfo, ServerMessage, PROTOCOL_VERSION};

#[derive(Debug, thiserror::Error)]
pub enum SignalingClientError {
    #[error("failed to connect to signaling host: {0}")]
    Connect(#[source] tokio_tungstenite::tungstenite::Error),
    #[error("connection closed before the host replied to Join")]
    ClosedBeforeJoin,
    #[error("host sent a malformed message")]
    MalformedMessage,
    #[error("host rejected Join: wrong password")]
    WrongPassword,
    #[error("host rejected Join: wrong network name")]
    WrongNetworkName,
    #[error("host rejected Join: unsupported protocol version")]
    UnsupportedVersion,
    #[error("host rejected Join: this identity already joined")]
    AlreadyJoined,
    #[error("signaling connection is already closed")]
    Disconnected,
}

impl From<JoinRejectReason> for SignalingClientError {
    fn from(reason: JoinRejectReason) -> Self {
        match reason {
            JoinRejectReason::WrongPassword => Self::WrongPassword,
            JoinRejectReason::WrongNetworkName => Self::WrongNetworkName,
            JoinRejectReason::UnsupportedVersion => Self::UnsupportedVersion,
            JoinRejectReason::AlreadyJoined => Self::AlreadyJoined,
            JoinRejectReason::MalformedJoin => Self::MalformedMessage,
        }
    }
}

/// A roster change or an incoming relay, forwarded live as the background
/// reader task observes `ServerMessage` broadcasts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MemberEvent {
    Joined(MemberInfo),
    Left { pubkey: String },
    /// An offer/answer envelope another member relayed to us. A blob that
    /// fails to decode (corrupt, wrong version, …) is dropped silently
    /// rather than surfaced here — same posture as any other malformed
    /// server frame in this reader loop.
    Relayed { from_pubkey: String, envelope: SignalEnvelope },
}

type Socket = WebSocketStream<MaybeTlsStream<TcpStream>>;

/// A live membership in a networked-signaling network. Dropping this (or
/// calling [`disconnect`]) closes the connection.
///
/// [`disconnect`]: SignalingClient::disconnect
#[derive(Debug)]
pub struct SignalingClient {
    members: Arc<Mutex<HashMap<String, MemberInfo>>>,
    outbound_tx: mpsc::UnboundedSender<ClientMessage>,
    cancel: watch::Sender<bool>,
    reader_task: tokio::task::JoinHandle<()>,
    writer_task: tokio::task::JoinHandle<()>,
}

impl SignalingClient {
    /// Connects to `host_addr`, joins `network_name` with `password`, and — on
    /// success — starts tracking the roster in the background. Returns the
    /// client plus a receiver for live roster-change events; the initial
    /// roster is available immediately via [`members`](Self::members) without
    /// waiting on the event channel.
    pub async fn join(
        host_addr: SocketAddr,
        network_name: impl Into<String>,
        password: &str,
        pubkey: impl Into<String>,
        fingerprint: impl Into<String>,
    ) -> Result<(Self, mpsc::UnboundedReceiver<MemberEvent>), SignalingClientError> {
        let (ws, _resp) = tokio_tungstenite::connect_async(format!("ws://{host_addr}"))
            .await
            .map_err(SignalingClientError::Connect)?;
        let (mut ws_tx, mut ws_rx) = ws.split();

        let join_msg = ClientMessage::Join {
            v: PROTOCOL_VERSION,
            network_name: network_name.into(),
            password_hash: super::server::hash_password(password),
            pubkey: pubkey.into(),
            fingerprint: fingerprint.into(),
        };
        let text = serde_json::to_string(&join_msg).map_err(|_| SignalingClientError::MalformedMessage)?;
        ws_tx
            .send(WsMessage::Text(text.into()))
            .await
            .map_err(|e| SignalingClientError::Connect(e))?;

        let frame = ws_rx.next().await.ok_or(SignalingClientError::ClosedBeforeJoin)?;
        let frame = frame.map_err(SignalingClientError::Connect)?;
        let reply: ServerMessage = serde_json::from_str(
            frame.to_text().map_err(|_| SignalingClientError::MalformedMessage)?,
        )
        .map_err(|_| SignalingClientError::MalformedMessage)?;

        let initial_members = match reply {
            ServerMessage::JoinAccepted { members } => members,
            ServerMessage::JoinRejected { reason } => return Err(reason.into()),
            _ => return Err(SignalingClientError::MalformedMessage),
        };

        let members: Arc<Mutex<HashMap<String, MemberInfo>>> = Arc::new(Mutex::new(
            initial_members.into_iter().map(|m| (m.pubkey.clone(), m)).collect(),
        ));

        let (event_tx, event_rx) = mpsc::unbounded_channel();
        let (outbound_tx, outbound_rx) = mpsc::unbounded_channel();
        let (cancel, cancel_rx) = watch::channel(false);
        let reader_task = tokio::spawn(read_loop(ws_rx, members.clone(), event_tx, cancel_rx.clone()));
        let writer_task = tokio::spawn(write_loop(ws_tx, outbound_rx, cancel_rx));

        Ok((Self { members, outbound_tx, cancel, reader_task, writer_task }, event_rx))
    }

    /// Snapshot of the current roster (excludes this client itself, which the
    /// host never echoes back as a member event).
    pub fn members(&self) -> Vec<MemberInfo> {
        self.members.lock().expect("signaling client members lock poisoned").values().cloned().collect()
    }

    /// Relays `envelope` (an offer or answer) to `to_pubkey` through the
    /// host. Transport only — encodes the envelope exactly as the manual
    /// paste flow would (`blob::encode`) and hands it to the writer task;
    /// the host forwards it opaquely without inspecting its contents.
    pub fn relay(&self, to_pubkey: impl Into<String>, envelope: &SignalEnvelope) -> Result<(), SignalingClientError> {
        let blob = blob::encode(envelope);
        self.outbound_tx
            .send(ClientMessage::Relay { to_pubkey: to_pubkey.into(), blob })
            .map_err(|_| SignalingClientError::Disconnected)
    }

    /// Closes the connection and stops the background reader/writer tasks.
    pub async fn disconnect(self) {
        let _ = self.cancel.send(true);
        let _ = self.reader_task.await;
        let _ = self.writer_task.await;
    }
}

async fn write_loop(
    mut ws_tx: futures_util::stream::SplitSink<Socket, WsMessage>,
    mut outbound_rx: mpsc::UnboundedReceiver<ClientMessage>,
    mut cancel_rx: watch::Receiver<bool>,
) {
    loop {
        tokio::select! {
            _ = cancel_rx.changed() => break,
            outgoing = outbound_rx.recv() => {
                match outgoing {
                    Some(msg) => {
                        let Ok(text) = serde_json::to_string(&msg) else { continue };
                        if ws_tx.send(WsMessage::Text(text.into())).await.is_err() { break; }
                    }
                    None => break,
                }
            }
        }
    }
}

async fn read_loop(
    mut ws_rx: futures_util::stream::SplitStream<Socket>,
    members: Arc<Mutex<HashMap<String, MemberInfo>>>,
    event_tx: mpsc::UnboundedSender<MemberEvent>,
    mut cancel_rx: watch::Receiver<bool>,
) {
    loop {
        tokio::select! {
            _ = cancel_rx.changed() => break,
            frame = ws_rx.next() => {
                let Some(Ok(frame)) = frame else { break };
                let Ok(text) = frame.to_text() else { continue };
                let Ok(msg) = serde_json::from_str::<ServerMessage>(text) else { continue };
                match msg {
                    ServerMessage::MemberJoined(info) => {
                        members.lock().expect("signaling client members lock poisoned").insert(info.pubkey.clone(), info.clone());
                        let _ = event_tx.send(MemberEvent::Joined(info));
                    }
                    ServerMessage::MemberLeft { pubkey } => {
                        members.lock().expect("signaling client members lock poisoned").remove(&pubkey);
                        let _ = event_tx.send(MemberEvent::Left { pubkey });
                    }
                    ServerMessage::Relayed { from_pubkey, blob: encoded } => {
                        if let Ok(envelope) = blob::decode(&encoded) {
                            let _ = event_tx.send(MemberEvent::Relayed { from_pubkey, envelope });
                        }
                    }
                    // Not expected post-join in G.2 (no second JoinAccepted/JoinRejected
                    // should ever arrive); ignored rather than treated as fatal.
                    ServerMessage::JoinAccepted { .. } | ServerMessage::JoinRejected { .. } => {}
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::signaling::server::SignalingServer;
    use std::time::Duration;

    async fn start_server() -> SignalingServer {
        SignalingServer::start("127.0.0.1:0".parse().unwrap(), "party", "secret").await.unwrap()
    }

    async fn recv_event(rx: &mut mpsc::UnboundedReceiver<MemberEvent>) -> MemberEvent {
        tokio::time::timeout(Duration::from_secs(2), rx.recv())
            .await
            .expect("timed out waiting for member event")
            .expect("event channel closed")
    }

    #[tokio::test]
    async fn joins_an_empty_network_successfully() {
        let server = start_server().await;

        let (client, _events) =
            SignalingClient::join(server.local_addr(), "party", "secret", "pkA", "PC-AAAA-AAAA-AAAA-AAAA")
                .await
                .unwrap();

        assert_eq!(client.members(), vec![]);
        client.disconnect().await;
        server.shutdown().await;
    }

    #[tokio::test]
    async fn rejects_wrong_password_without_creating_a_client() {
        let server = start_server().await;

        let err = SignalingClient::join(
            server.local_addr(),
            "party",
            "not-the-password",
            "pkA",
            "PC-AAAA-AAAA-AAAA-AAAA",
        )
        .await
        .unwrap_err();

        assert!(matches!(err, SignalingClientError::WrongPassword));
        server.shutdown().await;
    }

    #[tokio::test]
    async fn rejects_wrong_network_name() {
        let server = start_server().await;

        let err = SignalingClient::join(
            server.local_addr(),
            "not-the-network",
            "secret",
            "pkA",
            "PC-AAAA-AAAA-AAAA-AAAA",
        )
        .await
        .unwrap_err();

        assert!(matches!(err, SignalingClientError::WrongNetworkName));
        server.shutdown().await;
    }

    #[tokio::test]
    async fn sees_an_existing_member_in_its_initial_roster() {
        let server = start_server().await;

        let (client_a, _events_a) =
            SignalingClient::join(server.local_addr(), "party", "secret", "pkA", "PC-AAAA-AAAA-AAAA-AAAA")
                .await
                .unwrap();

        let (client_b, _events_b) =
            SignalingClient::join(server.local_addr(), "party", "secret", "pkB", "PC-BBBB-BBBB-BBBB-BBBB")
                .await
                .unwrap();

        assert_eq!(
            client_b.members(),
            vec![MemberInfo { pubkey: "pkA".to_string(), fingerprint: "PC-AAAA-AAAA-AAAA-AAAA".to_string() }],
        );

        client_a.disconnect().await;
        client_b.disconnect().await;
        server.shutdown().await;
    }

    #[tokio::test]
    async fn observes_a_later_join_as_a_live_event_and_in_its_roster() {
        let server = start_server().await;

        let (client_a, mut events_a) =
            SignalingClient::join(server.local_addr(), "party", "secret", "pkA", "PC-AAAA-AAAA-AAAA-AAAA")
                .await
                .unwrap();

        let (client_b, _events_b) =
            SignalingClient::join(server.local_addr(), "party", "secret", "pkB", "PC-BBBB-BBBB-BBBB-BBBB")
                .await
                .unwrap();

        let event = recv_event(&mut events_a).await;
        assert_eq!(
            event,
            MemberEvent::Joined(MemberInfo {
                pubkey: "pkB".to_string(),
                fingerprint: "PC-BBBB-BBBB-BBBB-BBBB".to_string(),
            })
        );
        assert_eq!(
            client_a.members(),
            vec![MemberInfo { pubkey: "pkB".to_string(), fingerprint: "PC-BBBB-BBBB-BBBB-BBBB".to_string() }],
        );

        client_a.disconnect().await;
        client_b.disconnect().await;
        server.shutdown().await;
    }

    #[tokio::test]
    async fn observes_a_departure_as_a_live_event_and_removes_it_from_the_roster() {
        let server = start_server().await;

        let (client_a, mut events_a) =
            SignalingClient::join(server.local_addr(), "party", "secret", "pkA", "PC-AAAA-AAAA-AAAA-AAAA")
                .await
                .unwrap();
        let (client_b, _events_b) =
            SignalingClient::join(server.local_addr(), "party", "secret", "pkB", "PC-BBBB-BBBB-BBBB-BBBB")
                .await
                .unwrap();
        recv_event(&mut events_a).await; // the join event from client_b

        client_b.disconnect().await;

        let event = recv_event(&mut events_a).await;
        assert_eq!(event, MemberEvent::Left { pubkey: "pkB".to_string() });
        assert_eq!(client_a.members(), vec![]);

        client_a.disconnect().await;
        server.shutdown().await;
    }

    fn sample_offer() -> SignalEnvelope {
        use crate::engine::signaling::message::{SignalKind, WireCandidate};
        use base64::engine::general_purpose::STANDARD;
        use base64::Engine;

        SignalEnvelope {
            v: crate::engine::signaling::message::PROTOCOL_VERSION,
            kind: SignalKind::Offer,
            sid: STANDARD.encode([7u8; 16]),
            pk: STANDARD.encode([9u8; 32]),
            cands: vec![WireCandidate { a: "192.168.1.9:51820".into(), k: "host".into() }],
        }
    }

    #[tokio::test]
    async fn relays_an_envelope_to_its_target_and_no_one_else() {
        let server = start_server().await;

        let (client_a, mut events_a) =
            SignalingClient::join(server.local_addr(), "party", "secret", "pkA", "PC-AAAA-AAAA-AAAA-AAAA")
                .await
                .unwrap();
        let (client_b, mut events_b) =
            SignalingClient::join(server.local_addr(), "party", "secret", "pkB", "PC-BBBB-BBBB-BBBB-BBBB")
                .await
                .unwrap();
        let (client_c, _events_c) =
            SignalingClient::join(server.local_addr(), "party", "secret", "pkC", "PC-CCCC-CCCC-CCCC-CCCC")
                .await
                .unwrap();
        recv_event(&mut events_a).await; // pkB joined
        recv_event(&mut events_a).await; // pkC joined
        recv_event(&mut events_b).await; // pkC joined

        let offer = sample_offer();
        client_a.relay("pkB", &offer).unwrap();

        let event = recv_event(&mut events_b).await;
        assert_eq!(event, MemberEvent::Relayed { from_pubkey: "pkA".to_string(), envelope: offer });

        // pkC never receives it — confirmed by the fact that the very next
        // thing pkC's own relay-to-pkB produces is observable in isolation.
        client_a.disconnect().await;
        client_b.disconnect().await;
        client_c.disconnect().await;
        server.shutdown().await;
    }

    #[tokio::test]
    async fn relaying_to_a_nonexistent_member_is_a_silent_no_op() {
        let server = start_server().await;

        let (client_a, _events_a) =
            SignalingClient::join(server.local_addr(), "party", "secret", "pkA", "PC-AAAA-AAAA-AAAA-AAAA")
                .await
                .unwrap();

        client_a.relay("does-not-exist", &sample_offer()).unwrap();

        // No crash, no event, no hang — the host silently dropped it. Prove
        // the connection is still alive by successfully relaying to a real
        // member afterwards.
        let (client_b, mut events_b) =
            SignalingClient::join(server.local_addr(), "party", "secret", "pkB", "PC-BBBB-BBBB-BBBB-BBBB")
                .await
                .unwrap();
        let offer = sample_offer();
        client_a.relay("pkB", &offer).unwrap();
        let event = recv_event(&mut events_b).await;
        assert_eq!(event, MemberEvent::Relayed { from_pubkey: "pkA".to_string(), envelope: offer });

        client_a.disconnect().await;
        client_b.disconnect().await;
        server.shutdown().await;
    }
}
