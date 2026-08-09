//! The relay: a dumb rendezvous/byte-splicer with zero knowledge of what's
//! flowing through it (WebSocket, Noise, anything). See the module doc
//! comment in `mod.rs` for the four-step handshake this implements.

use std::collections::HashMap;
use std::io;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use rand::RngCore;
use tokio::io::AsyncReadExt;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, oneshot, watch};

use super::protocol::{read_client_frame, write_server_frame, ClientFrame, ServerFrame};

/// How long a joiner's `CONNECT` waits for the host to answer with a matching
/// `ACCEPT` before giving up — generous, since the host's own reaction is
/// just "read a line, dial back out," not a real network round trip on top
/// of the one already happening.
const ACCEPT_TIMEOUT: Duration = Duration::from_secs(15);

fn new_session_id() -> String {
    let mut bytes = [0u8; 8];
    rand::thread_rng().fill_bytes(&mut bytes);
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

struct RegisteredHost {
    /// Delivers `NEW_MEMBER` frames to the task driving this host's control
    /// connection's write half.
    notify_tx: mpsc::UnboundedSender<ServerFrame>,
}

#[derive(Default)]
struct SharedState {
    hosts: Mutex<HashMap<String, RegisteredHost>>,
    /// A joiner's `CONNECT` is waiting here under its session id until the
    /// matching `ACCEPT` connection arrives and sends its stream through.
    pending_sessions: Mutex<HashMap<String, oneshot::Sender<TcpStream>>>,
}

/// A running relay. Dropping this (or calling [`shutdown`]) stops accepting
/// new connections; already-spliced pairs keep running until either side
/// closes on its own.
///
/// [`shutdown`]: RelayServer::shutdown
pub struct RelayServer {
    local_addr: SocketAddr,
    state: Arc<SharedState>,
    cancel: watch::Sender<bool>,
    accept_task: tokio::task::JoinHandle<()>,
}

impl RelayServer {
    pub async fn start(bind_addr: SocketAddr) -> io::Result<Self> {
        let listener = TcpListener::bind(bind_addr).await?;
        let local_addr = listener.local_addr()?;
        let state = Arc::new(SharedState::default());
        let (cancel, cancel_rx) = watch::channel(false);
        let accept_task = tokio::spawn(accept_loop(listener, state.clone(), cancel_rx));
        Ok(Self { local_addr, state, cancel, accept_task })
    }

    pub fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }

    /// Network names currently registered (i.e. with a live host control
    /// connection) — display-only, e.g. for an in-app "who's using this
    /// relay right now" status readout. No further detail (member counts,
    /// addresses) is tracked here; the relay deliberately knows nothing
    /// about what's happening inside a spliced connection.
    pub fn registered_network_names(&self) -> Vec<String> {
        self.state.hosts.lock().expect("relay hosts lock poisoned").keys().cloned().collect()
    }

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
                let Ok((stream, _addr)) = accepted else { continue };
                let state = state.clone();
                tokio::spawn(async move {
                    let _ = handle_connection(stream, state).await;
                });
            }
        }
    }
}

async fn handle_connection(mut stream: TcpStream, state: Arc<SharedState>) -> io::Result<()> {
    let Some(frame) = read_client_frame(&mut stream).await? else { return Ok(()) };
    match frame {
        ClientFrame::Register { network_name } => handle_register(stream, network_name, state).await,
        ClientFrame::Connect { network_name } => handle_connect(stream, network_name, state).await,
        ClientFrame::Accept { session_id } => handle_accept(stream, session_id, state).await,
    }
}

/// The host's long-lived control connection: registers under `network_name`
/// (rejecting a name already in use), then forwards every `NEW_MEMBER`
/// notification down the socket until it closes, at which point the host is
/// deregistered so a later `REGISTER` of the same name can succeed again.
async fn handle_register(mut stream: TcpStream, network_name: String, state: Arc<SharedState>) -> io::Result<()> {
    let (notify_tx, mut notify_rx) = mpsc::unbounded_channel::<ServerFrame>();
    let name_in_use = {
        let mut hosts = state.hosts.lock().expect("relay hosts lock poisoned");
        if hosts.contains_key(&network_name) {
            true
        } else {
            hosts.insert(network_name.clone(), RegisteredHost { notify_tx });
            false
        }
    };
    if name_in_use {
        write_server_frame(&mut stream, &ServerFrame::Err { message: "name in use".into() }).await?;
        return Ok(());
    }
    write_server_frame(&mut stream, &ServerFrame::Ok).await?;

    let (mut read_half, mut write_half) = stream.into_split();
    let writer = tokio::spawn(async move {
        while let Some(frame) = notify_rx.recv().await {
            if write_server_frame(&mut write_half, &frame).await.is_err() {
                break;
            }
        }
    });

    // The control connection sends nothing further after REGISTER — this
    // loop's only job is noticing when the host disconnects (deliberately or
    // otherwise), so a stale registration can't wedge the network name
    // forever. Any bytes read are discarded; a well-behaved host sends none.
    let mut sink = [0u8; 64];
    loop {
        match read_half.read(&mut sink).await {
            Ok(0) | Err(_) => break,
            Ok(_) => continue,
        }
    }

    state.hosts.lock().expect("relay hosts lock poisoned").remove(&network_name);
    writer.abort();
    Ok(())
}

/// A joiner's request to reach `network_name`. Rejected immediately if no
/// host is registered under that name; otherwise notifies the host and waits
/// (bounded by [`ACCEPT_TIMEOUT`]) for the matching `ACCEPT`, then splices
/// this connection to the host's.
async fn handle_connect(mut stream: TcpStream, network_name: String, state: Arc<SharedState>) -> io::Result<()> {
    let notify_tx = {
        let hosts = state.hosts.lock().expect("relay hosts lock poisoned");
        hosts.get(&network_name).map(|host| host.notify_tx.clone())
    };
    let Some(notify_tx) = notify_tx else {
        write_server_frame(&mut stream, &ServerFrame::Err { message: "no such network".into() }).await?;
        return Ok(());
    };

    let session_id = new_session_id();
    let (accept_tx, accept_rx) = oneshot::channel();
    state.pending_sessions.lock().expect("relay pending-sessions lock poisoned").insert(session_id.clone(), accept_tx);

    if notify_tx.send(ServerFrame::NewMember { session_id: session_id.clone() }).is_err() {
        // Host's control connection just died between the lookup above and
        // here — treat exactly like "no such network".
        state.pending_sessions.lock().expect("relay pending-sessions lock poisoned").remove(&session_id);
        write_server_frame(&mut stream, &ServerFrame::Err { message: "no such network".into() }).await?;
        return Ok(());
    }

    let host_stream = match tokio::time::timeout(ACCEPT_TIMEOUT, accept_rx).await {
        Ok(Ok(host_stream)) => host_stream,
        _ => {
            state.pending_sessions.lock().expect("relay pending-sessions lock poisoned").remove(&session_id);
            write_server_frame(&mut stream, &ServerFrame::Err { message: "host did not accept in time".into() })
                .await?;
            return Ok(());
        }
    };

    // Pairing succeeded — tell the joiner explicitly before going silent and
    // splicing raw bytes. Without this, a joiner reading its first line to
    // check for `ERR` would instead consume (and lose) the first line of
    // whatever protocol runs over the spliced pipe next, since a successful
    // pairing otherwise produces no relay-generated bytes at all.
    write_server_frame(&mut stream, &ServerFrame::Ok).await?;

    let mut host_stream = host_stream;
    let _ = tokio::io::copy_bidirectional(&mut stream, &mut host_stream).await;
    Ok(())
}

/// The host's response to a `NEW_MEMBER` notification: hands this fresh
/// connection's stream to whichever `handle_connect` task is waiting on
/// `session_id`, which then owns the splice. A session id with no matching
/// pending joiner (already timed out, or bogus) is rejected.
async fn handle_accept(mut stream: TcpStream, session_id: String, state: Arc<SharedState>) -> io::Result<()> {
    let sender = state.pending_sessions.lock().expect("relay pending-sessions lock poisoned").remove(&session_id);
    match sender {
        Some(sender) => {
            // The receiving side may have timed out microseconds ago; a
            // failed send just means the joiner's stream close does the
            // right thing for us without any special-casing here.
            let _ = sender.send(stream);
            Ok(())
        }
        None => write_server_frame(&mut stream, &ServerFrame::Err { message: "unknown session".into() }).await,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::AsyncWriteExt as _;

    async fn connect(addr: SocketAddr) -> TcpStream {
        TcpStream::connect(addr).await.unwrap()
    }

    async fn send_line(stream: &mut TcpStream, line: &str) {
        stream.write_all(format!("{line}\n").as_bytes()).await.unwrap();
    }

    async fn recv_line(stream: &mut TcpStream) -> String {
        let mut buf = Vec::new();
        let mut byte = [0u8; 1];
        loop {
            let n = stream.read(&mut byte).await.unwrap();
            assert_ne!(n, 0, "connection closed before a line arrived");
            if byte[0] == b'\n' {
                return String::from_utf8(buf).unwrap();
            }
            buf.push(byte[0]);
        }
    }

    #[tokio::test]
    async fn connect_with_no_registered_host_is_rejected() {
        let relay = RelayServer::start("127.0.0.1:0".parse().unwrap()).await.unwrap();

        let mut joiner = connect(relay.local_addr()).await;
        send_line(&mut joiner, "CONNECT party").await;

        assert_eq!(recv_line(&mut joiner).await, "ERR no such network");
        relay.shutdown().await;
    }

    #[tokio::test]
    async fn registering_the_same_name_twice_is_rejected() {
        let relay = RelayServer::start("127.0.0.1:0".parse().unwrap()).await.unwrap();

        let mut host_a = connect(relay.local_addr()).await;
        send_line(&mut host_a, "REGISTER party").await;
        assert_eq!(recv_line(&mut host_a).await, "OK");

        let mut host_b = connect(relay.local_addr()).await;
        send_line(&mut host_b, "REGISTER party").await;

        assert_eq!(recv_line(&mut host_b).await, "ERR name in use");
        relay.shutdown().await;
    }

    #[tokio::test]
    async fn connect_pairs_with_the_registered_host_and_splices_bytes_both_ways() {
        let relay = RelayServer::start("127.0.0.1:0".parse().unwrap()).await.unwrap();

        let mut host_ctrl = connect(relay.local_addr()).await;
        send_line(&mut host_ctrl, "REGISTER party").await;
        assert_eq!(recv_line(&mut host_ctrl).await, "OK");

        let mut joiner = connect(relay.local_addr()).await;
        send_line(&mut joiner, "CONNECT party").await;

        let notice = recv_line(&mut host_ctrl).await;
        let session_id = notice.strip_prefix("NEW_MEMBER ").expect("expected a NEW_MEMBER notice").to_string();

        let mut host_data = connect(relay.local_addr()).await;
        send_line(&mut host_data, &format!("ACCEPT {session_id}")).await;

        assert_eq!(recv_line(&mut joiner).await, "OK");

        // From here, the relay is invisible: raw bytes sent on one side must
        // arrive verbatim on the other, in both directions.
        joiner.write_all(b"hello from joiner").await.unwrap();
        let mut buf = [0u8; 17];
        host_data.read_exact(&mut buf).await.unwrap();
        assert_eq!(&buf, b"hello from joiner");

        host_data.write_all(b"hello from host").await.unwrap();
        let mut buf2 = [0u8; 15];
        joiner.read_exact(&mut buf2).await.unwrap();
        assert_eq!(&buf2, b"hello from host");

        relay.shutdown().await;
    }

    #[tokio::test]
    async fn accept_with_an_unknown_session_id_is_rejected() {
        let relay = RelayServer::start("127.0.0.1:0".parse().unwrap()).await.unwrap();

        let mut host_data = connect(relay.local_addr()).await;
        send_line(&mut host_data, "ACCEPT does-not-exist").await;

        assert_eq!(recv_line(&mut host_data).await, "ERR unknown session");
        relay.shutdown().await;
    }

    #[tokio::test]
    async fn a_name_frees_up_again_once_its_host_disconnects() {
        let relay = RelayServer::start("127.0.0.1:0".parse().unwrap()).await.unwrap();

        let host_ctrl = connect(relay.local_addr()).await;
        {
            let mut h = host_ctrl;
            send_line(&mut h, "REGISTER party").await;
            drop(h); // host disconnects
        }

        // Give the relay's accept-loop task a moment to notice the close.
        tokio::time::sleep(Duration::from_millis(100)).await;

        let mut host_b = connect(relay.local_addr()).await;
        send_line(&mut host_b, "REGISTER party").await;
        assert_eq!(recv_line(&mut host_b).await, "OK");

        // No ERR arrives — proven by successfully completing a connect
        // against this second registration.
        let mut joiner = connect(relay.local_addr()).await;
        send_line(&mut joiner, "CONNECT party").await;
        let notice = recv_line(&mut host_b).await;
        assert!(notice.starts_with("NEW_MEMBER "));

        relay.shutdown().await;
    }
}
