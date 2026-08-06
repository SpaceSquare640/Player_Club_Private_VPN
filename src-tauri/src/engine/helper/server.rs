//! The elevated side of the helper protocol: reads [`HelperRequest`] lines,
//! dispatches each to a [`HelperDispatcher`], writes back a
//! [`HelperResponse`] line. Generic over the transport (`AsyncRead`/
//! `AsyncWrite`) so it can be driven by a real named pipe (a later step —
//! see the `helper` module doc comment) or, in tests, an in-memory
//! `tokio::io::duplex` — no real elevation or Windows API involved in
//! exercising this loop at all.
#![allow(dead_code)]

use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader};

use super::dispatch::HelperDispatcher;
use super::protocol::{decode_request, encode_response, HelperRequest, HelperResponse, HELPER_PROTOCOL_VERSION};

/// Runs until the stream closes or a `Shutdown` request is processed.
/// Returns `Ok(())` in both cases — neither is an error from the server
/// loop's own point of view; the caller decides what "the helper exited"
/// means for its own lifecycle.
pub async fn run<R, W, D>(reader: R, mut writer: W, mut dispatcher: D) -> std::io::Result<()>
where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin,
    D: HelperDispatcher,
{
    let mut lines = BufReader::new(reader).lines();
    while let Some(line) = lines.next_line().await? {
        let request = match decode_request(&line) {
            Ok(r) => r,
            Err(e) => {
                send(&mut writer, &HelperResponse::Error { message: e.to_string() }).await?;
                continue;
            }
        };

        if request.version() != HELPER_PROTOCOL_VERSION {
            send(&mut writer, &HelperResponse::UnsupportedVersion { supported: HELPER_PROTOCOL_VERSION }).await?;
            continue;
        }

        let is_shutdown = matches!(request, HelperRequest::Shutdown { .. });
        let response = dispatch(&mut dispatcher, request);
        send(&mut writer, &response).await?;
        if is_shutdown {
            break;
        }
    }
    Ok(())
}

fn dispatch(dispatcher: &mut impl HelperDispatcher, request: HelperRequest) -> HelperResponse {
    let outcome = match request {
        HelperRequest::CreateAdapter { name, virtual_ip, prefix_len, .. } => {
            dispatcher.create_adapter(&name, virtual_ip, prefix_len)
        }
        HelperRequest::ConfigureNetworkIntegration { adapter_name, .. } => {
            dispatcher.configure_network_integration(&adapter_name)
        }
        HelperRequest::RemoveNetworkIntegration { adapter_name, .. } => {
            dispatcher.remove_network_integration(&adapter_name)
        }
        HelperRequest::AddExtraRoutes { adapter_name, routes, .. } => {
            dispatcher.add_extra_routes(&adapter_name, &routes)
        }
        HelperRequest::RemoveExtraRoutes { adapter_name, routes, .. } => {
            dispatcher.remove_extra_routes(&adapter_name, &routes)
        }
        HelperRequest::Shutdown { .. } => Ok(()),
    };
    match outcome {
        Ok(()) => HelperResponse::Ok,
        Err(message) => HelperResponse::Error { message },
    }
}

async fn send<W: AsyncWrite + Unpin>(writer: &mut W, response: &HelperResponse) -> std::io::Result<()> {
    let line = encode_response(response)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))?;
    writer.write_all(line.as_bytes()).await?;
    writer.write_all(b"\n").await?;
    writer.flush().await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::helper::dispatch::test_support::FakeDispatcher;
    use crate::engine::helper::protocol::{encode_request, RouteSpec};
    use std::net::Ipv4Addr;
    use tokio::io::{split, BufReader as TokioBufReader};

    /// Wires a `run` server to one side of an in-memory duplex pipe and
    /// hands the test the other side, plus the `FakeDispatcher` so it can
    /// assert on what was actually dispatched.
    async fn spawn_server() -> (tokio::io::DuplexStream, FakeDispatcher, tokio::task::JoinHandle<std::io::Result<()>>)
    {
        let (client_side, server_side) = tokio::io::duplex(4096);
        let dispatcher = FakeDispatcher::default();
        let dispatcher_clone = dispatcher.clone();
        let (server_read, server_write) = split(server_side);
        let handle = tokio::spawn(run(server_read, server_write, dispatcher_clone));
        (client_side, dispatcher, handle)
    }

    async fn roundtrip(client_side: &mut tokio::io::DuplexStream, request: &HelperRequest) -> HelperResponse {
        let line = encode_request(request).unwrap();
        client_side.write_all(line.as_bytes()).await.unwrap();
        client_side.write_all(b"\n").await.unwrap();
        client_side.flush().await.unwrap();

        let (read_half, _write_half) = tokio::io::split(&mut *client_side);
        let mut reader = TokioBufReader::new(read_half);
        let mut response_line = String::new();
        reader.read_line(&mut response_line).await.unwrap();
        crate::engine::helper::protocol::decode_response(&response_line).unwrap()
    }

    #[tokio::test]
    async fn dispatches_configure_network_integration_and_replies_ok() {
        let (mut client_side, dispatcher, handle) = spawn_server().await;

        let resp = roundtrip(
            &mut client_side,
            &HelperRequest::ConfigureNetworkIntegration {
                v: HELPER_PROTOCOL_VERSION,
                adapter_name: "PlayerClubVPN".to_string(),
            },
        )
        .await;

        assert_eq!(resp, HelperResponse::Ok);
        assert_eq!(dispatcher.calls.lock().unwrap().as_slice(), ["configure_network_integration(PlayerClubVPN)"]);

        drop(client_side);
        let _ = handle.await;
    }

    #[tokio::test]
    async fn dispatches_add_extra_routes_with_the_right_route_count() {
        let (mut client_side, dispatcher, handle) = spawn_server().await;

        let resp = roundtrip(
            &mut client_side,
            &HelperRequest::AddExtraRoutes {
                v: HELPER_PROTOCOL_VERSION,
                adapter_name: "PlayerClubVPN".to_string(),
                routes: vec![
                    RouteSpec { network: Ipv4Addr::new(192, 168, 50, 0), prefix: 24 },
                    RouteSpec { network: Ipv4Addr::new(10, 0, 5, 0), prefix: 24 },
                ],
            },
        )
        .await;

        assert_eq!(resp, HelperResponse::Ok);
        assert_eq!(dispatcher.calls.lock().unwrap().as_slice(), ["add_extra_routes(PlayerClubVPN, 2)"]);

        drop(client_side);
        let _ = handle.await;
    }

    #[tokio::test]
    async fn a_dispatcher_error_becomes_an_error_response() {
        let (mut client_side, dispatcher, handle) = spawn_server().await;
        *dispatcher.fail_next.lock().unwrap() = Some("access denied".to_string());

        let resp = roundtrip(
            &mut client_side,
            &HelperRequest::RemoveNetworkIntegration {
                v: HELPER_PROTOCOL_VERSION,
                adapter_name: "PlayerClubVPN".to_string(),
            },
        )
        .await;

        assert_eq!(resp, HelperResponse::Error { message: "access denied".to_string() });

        drop(client_side);
        let _ = handle.await;
    }

    #[tokio::test]
    async fn an_unsupported_version_is_reported_without_dispatching() {
        let (mut client_side, dispatcher, handle) = spawn_server().await;

        let resp = roundtrip(
            &mut client_side,
            &HelperRequest::ConfigureNetworkIntegration { v: 999, adapter_name: "PlayerClubVPN".to_string() },
        )
        .await;

        assert_eq!(resp, HelperResponse::UnsupportedVersion { supported: HELPER_PROTOCOL_VERSION });
        assert!(dispatcher.calls.lock().unwrap().is_empty());

        drop(client_side);
        let _ = handle.await;
    }

    #[tokio::test]
    async fn malformed_input_gets_an_error_reply_and_the_loop_continues() {
        let (mut client_side, dispatcher, handle) = spawn_server().await;

        client_side.write_all(b"{not json\n").await.unwrap();
        client_side.flush().await.unwrap();
        let (read_half, _write_half) = tokio::io::split(&mut client_side);
        let mut reader = TokioBufReader::new(read_half);
        let mut line = String::new();
        reader.read_line(&mut line).await.unwrap();
        assert!(matches!(decode_response_or_panic(&line), HelperResponse::Error { .. }));

        // The loop is still alive after a malformed line — prove it by
        // sending a real request next and getting a real reply.
        let resp = roundtrip(
            &mut client_side,
            &HelperRequest::RemoveNetworkIntegration {
                v: HELPER_PROTOCOL_VERSION,
                adapter_name: "PlayerClubVPN".to_string(),
            },
        )
        .await;
        assert_eq!(resp, HelperResponse::Ok);
        assert_eq!(dispatcher.calls.lock().unwrap().len(), 1);

        drop(client_side);
        let _ = handle.await;
    }

    #[tokio::test]
    async fn shutdown_ends_the_server_loop_after_replying() {
        let (mut client_side, _dispatcher, handle) = spawn_server().await;

        let resp = roundtrip(&mut client_side, &HelperRequest::Shutdown { v: HELPER_PROTOCOL_VERSION }).await;
        assert_eq!(resp, HelperResponse::Ok);

        // The server task must actually finish, not hang waiting for more input.
        tokio::time::timeout(std::time::Duration::from_secs(2), handle)
            .await
            .expect("server task did not exit after Shutdown")
            .unwrap()
            .unwrap();
    }

    fn decode_response_or_panic(line: &str) -> HelperResponse {
        crate::engine::helper::protocol::decode_response(line).unwrap()
    }
}
