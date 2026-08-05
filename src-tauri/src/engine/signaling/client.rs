//! Networked signaling client (G.2): joins a [`SignalingServer`]-hosted
//! virtual network and tracks its member roster in the background.
//!
//! Deliberately does nothing else yet — this is roster tracking only.
//! Automatically relaying offer/answer through this connection so joining a
//! network establishes P2P links without manual paste is Phase G.3's job.
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

/// A roster change, forwarded live as the background reader task observes
/// `ServerMessage` broadcasts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MemberEvent {
    Joined(MemberInfo),
    Left { pubkey: String },
}

type Socket = WebSocketStream<MaybeTlsStream<TcpStream>>;

/// A live membership in a networked-signaling network. Dropping this (or
/// calling [`disconnect`]) closes the connection.
///
/// [`disconnect`]: SignalingClient::disconnect
#[derive(Debug)]
pub struct SignalingClient {
    members: Arc<Mutex<HashMap<String, MemberInfo>>>,
    cancel: watch::Sender<bool>,
    reader_task: tokio::task::JoinHandle<()>,
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
        let (cancel, cancel_rx) = watch::channel(false);
        let reader_task = tokio::spawn(read_loop(ws_rx, members.clone(), event_tx, cancel_rx));

        Ok((Self { members, cancel, reader_task }, event_rx))
    }

    /// Snapshot of the current roster (excludes this client itself, which the
    /// host never echoes back as a member event).
    pub fn members(&self) -> Vec<MemberInfo> {
        self.members.lock().expect("signaling client members lock poisoned").values().cloned().collect()
    }

    /// Closes the connection and stops the background reader task.
    pub async fn disconnect(self) {
        let _ = self.cancel.send(true);
        let _ = self.reader_task.await;
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
}
