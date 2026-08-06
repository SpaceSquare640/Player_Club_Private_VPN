//! The unelevated side of the helper protocol: sends one [`HelperRequest`]
//! at a time and awaits its [`HelperResponse`]. Generic over the transport,
//! same as [`super::server`] — a real named pipe client handle (a later
//! step) or, in tests, an in-memory `tokio::io::duplex` talking directly to
//! [`super::server::run`].
#![allow(dead_code)]

use std::io;

use tokio::io::{split, AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader, ReadHalf, WriteHalf};

use super::protocol::{decode_response, encode_request, HelperRequest, HelperResponse};

pub struct HelperClient<S> {
    reader: BufReader<ReadHalf<S>>,
    writer: WriteHalf<S>,
}

impl<S: AsyncRead + AsyncWrite + Unpin> HelperClient<S> {
    pub fn new(stream: S) -> Self {
        let (read_half, writer) = split(stream);
        Self { reader: BufReader::new(read_half), writer }
    }

    /// Sends `request` and waits for the matching reply. One in-flight
    /// request at a time — this client does not pipeline, matching how
    /// every call site (each a one-shot privileged operation, not a stream
    /// of them) will actually use it.
    pub async fn request(&mut self, request: &HelperRequest) -> io::Result<HelperResponse> {
        let line =
            encode_request(request).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;
        self.writer.write_all(line.as_bytes()).await?;
        self.writer.write_all(b"\n").await?;
        self.writer.flush().await?;

        let mut response_line = String::new();
        let n = self.reader.read_line(&mut response_line).await?;
        if n == 0 {
            return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "helper closed the connection"));
        }
        decode_response(&response_line).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::helper::dispatch::test_support::FakeDispatcher;
    use crate::engine::helper::protocol::HELPER_PROTOCOL_VERSION;

    /// Full round trip against the real server loop — not a hand-rolled
    /// stand-in — proving the client and server actually agree on framing,
    /// not just that each one's own unit tests pass in isolation.
    async fn connected_client() -> (HelperClient<tokio::io::DuplexStream>, FakeDispatcher, tokio::task::JoinHandle<io::Result<()>>)
    {
        let (client_side, server_side) = tokio::io::duplex(4096);
        let dispatcher = FakeDispatcher::default();
        let dispatcher_clone = dispatcher.clone();
        let (server_read, server_write) = split(server_side);
        let handle = tokio::spawn(super::super::server::run(server_read, server_write, dispatcher_clone));
        (HelperClient::new(client_side), dispatcher, handle)
    }

    #[tokio::test]
    async fn request_returns_ok_on_success() {
        let (mut client, dispatcher, handle) = connected_client().await;

        let resp = client
            .request(&HelperRequest::ConfigureNetworkIntegration {
                v: HELPER_PROTOCOL_VERSION,
                adapter_name: "PlayerClubVPN".to_string(),
            })
            .await
            .unwrap();

        assert_eq!(resp, HelperResponse::Ok);
        assert_eq!(dispatcher.calls.lock().unwrap().len(), 1);

        let _ = client.request(&HelperRequest::Shutdown { v: HELPER_PROTOCOL_VERSION }).await;
        let _ = handle.await;
    }

    #[tokio::test]
    async fn request_surfaces_a_dispatcher_error() {
        let (mut client, dispatcher, handle) = connected_client().await;
        *dispatcher.fail_next.lock().unwrap() = Some("adapter busy".to_string());

        let resp = client
            .request(&HelperRequest::RemoveNetworkIntegration {
                v: HELPER_PROTOCOL_VERSION,
                adapter_name: "PlayerClubVPN".to_string(),
            })
            .await
            .unwrap();

        assert_eq!(resp, HelperResponse::Error { message: "adapter busy".to_string() });

        let _ = client.request(&HelperRequest::Shutdown { v: HELPER_PROTOCOL_VERSION }).await;
        let _ = handle.await;
    }

    #[tokio::test]
    async fn several_requests_in_sequence_each_get_their_own_reply() {
        let (mut client, _dispatcher, handle) = connected_client().await;

        for _ in 0..3 {
            let resp = client
                .request(&HelperRequest::ConfigureNetworkIntegration {
                    v: HELPER_PROTOCOL_VERSION,
                    adapter_name: "PlayerClubVPN".to_string(),
                })
                .await
                .unwrap();
            assert_eq!(resp, HelperResponse::Ok);
        }

        let resp = client.request(&HelperRequest::Shutdown { v: HELPER_PROTOCOL_VERSION }).await.unwrap();
        assert_eq!(resp, HelperResponse::Ok);
        let _ = handle.await;
    }

    #[tokio::test]
    async fn request_errors_when_the_helper_closes_the_connection() {
        let (client_side, server_side) = tokio::io::duplex(4096);
        drop(server_side); // simulate the helper process exiting unexpectedly
        let mut client = HelperClient::new(client_side);

        let err = client
            .request(&HelperRequest::ConfigureNetworkIntegration {
                v: HELPER_PROTOCOL_VERSION,
                adapter_name: "PlayerClubVPN".to_string(),
            })
            .await
            .unwrap_err();
        // Which of these fires depends on whether the write or the
        // subsequent read notices the dropped peer first — both are the
        // same "helper process is gone" condition from the caller's view.
        assert!(
            matches!(err.kind(), io::ErrorKind::UnexpectedEof | io::ErrorKind::BrokenPipe),
            "unexpected error kind: {:?}",
            err.kind()
        );
    }
}
