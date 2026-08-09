//! Wire protocol for the relay's control frames. Deliberately not JSON/WebSocket
//! — this sits one layer *below* the signaling protocol (`engine::signaling`),
//! which runs completely unmodified over whatever `TcpStream` this layer hands
//! it, whether that stream is a direct connection or a relay-spliced one.
//!
//! Every frame is a single newline-terminated line of ASCII, kept as simple as
//! possible since this is the one layer that can never assume the peer speaks
//! the same (versioned, evolving) app protocol above it — a relay may outlive
//! several app versions.

use std::io;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

/// Longest control line accepted — generous for a network name, tiny next to
/// what a malicious/broken peer could otherwise make us buffer.
const MAX_LINE_LEN: usize = 512;

/// The first line sent by a connecting party, before anything else happens.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClientFrame {
    /// The host's long-lived control connection for `network_name`.
    Register { network_name: String },
    /// A joiner's request to reach the host registered under `network_name`.
    Connect { network_name: String },
    /// The host's response to a `NEW_MEMBER` notification, completing the
    /// pairing for `session_id`.
    Accept { session_id: String },
}

/// A line sent by the relay back to a connected party.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServerFrame {
    /// Acknowledges a successful `REGISTER`, before any `NEW_MEMBER` ever
    /// arrives on the same connection — lets the host tell "registered,
    /// nothing's happened yet" apart from "rejected" without guessing from
    /// silence.
    Ok,
    /// Sent down the host's control connection when a joiner is waiting.
    NewMember { session_id: String },
    /// Registration/connect rejected; the relay closes the connection right after.
    Err { message: String },
}

impl ClientFrame {
    pub fn encode(&self) -> String {
        match self {
            ClientFrame::Register { network_name } => format!("REGISTER {network_name}\n"),
            ClientFrame::Connect { network_name } => format!("CONNECT {network_name}\n"),
            ClientFrame::Accept { session_id } => format!("ACCEPT {session_id}\n"),
        }
    }

    pub fn parse(line: &str) -> Option<Self> {
        let (verb, rest) = line.trim_end().split_once(' ')?;
        let arg = rest.trim();
        if arg.is_empty() {
            return None;
        }
        match verb {
            "REGISTER" => Some(ClientFrame::Register { network_name: arg.to_string() }),
            "CONNECT" => Some(ClientFrame::Connect { network_name: arg.to_string() }),
            "ACCEPT" => Some(ClientFrame::Accept { session_id: arg.to_string() }),
            _ => None,
        }
    }
}

impl ServerFrame {
    pub fn encode(&self) -> String {
        match self {
            ServerFrame::Ok => "OK\n".to_string(),
            ServerFrame::NewMember { session_id } => format!("NEW_MEMBER {session_id}\n"),
            ServerFrame::Err { message } => format!("ERR {message}\n"),
        }
    }

    pub fn parse(line: &str) -> Option<Self> {
        let line = line.trim_end();
        if line == "OK" {
            return Some(ServerFrame::Ok);
        }
        let (verb, rest) = line.split_once(' ')?;
        let arg = rest.trim();
        match verb {
            "NEW_MEMBER" if !arg.is_empty() => Some(ServerFrame::NewMember { session_id: arg.to_string() }),
            "ERR" => Some(ServerFrame::Err { message: arg.to_string() }),
            _ => None,
        }
    }
}

/// Reads exactly the bytes of one line (up to and including its `\n`, which is
/// consumed but not returned) directly from the socket, one byte at a time —
/// deliberately not a `BufReader`, whose internal buffer could swallow bytes
/// that belong to whatever runs *after* this control line (the WebSocket
/// handshake, once this connection is spliced) rather than to the line
/// itself. Control lines are short and infrequent, so the per-byte read cost
/// is irrelevant next to that correctness requirement.
async fn read_line(stream: &mut TcpStream) -> io::Result<Option<String>> {
    let mut buf = Vec::new();
    let mut byte = [0u8; 1];
    loop {
        if stream.read(&mut byte).await? == 0 {
            return Ok(None); // closed before a newline arrived
        }
        if byte[0] == b'\n' {
            return Ok(Some(String::from_utf8_lossy(&buf).into_owned()));
        }
        buf.push(byte[0]);
        if buf.len() > MAX_LINE_LEN {
            return Ok(None);
        }
    }
}

/// Reads exactly one line and parses it as a [`ClientFrame`], or `None` if the
/// connection closed before a newline arrived or the line was malformed.
pub async fn read_client_frame(stream: &mut TcpStream) -> io::Result<Option<ClientFrame>> {
    let Some(line) = read_line(stream).await? else { return Ok(None) };
    Ok(ClientFrame::parse(&line))
}

/// Reads exactly one line and parses it as a [`ServerFrame`], or `None` if the
/// connection closed before a newline arrived or the line was malformed.
pub async fn read_server_frame(stream: &mut TcpStream) -> io::Result<Option<ServerFrame>> {
    let Some(line) = read_line(stream).await? else { return Ok(None) };
    Ok(ServerFrame::parse(&line))
}

pub async fn write_client_frame(stream: &mut TcpStream, frame: &ClientFrame) -> io::Result<()> {
    stream.write_all(frame.encode().as_bytes()).await
}

/// Generic over the writer so the relay server can use it on a split
/// `OwnedWriteHalf` (its host control connections need read and write
/// running as two independent tasks) as well as on a whole `TcpStream`.
pub async fn write_server_frame(
    writer: &mut (impl AsyncWriteExt + Unpin),
    frame: &ServerFrame,
) -> io::Result<()> {
    writer.write_all(frame.encode().as_bytes()).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_register() {
        let f = ClientFrame::Register { network_name: "party".into() };
        assert_eq!(ClientFrame::parse(f.encode().trim_end()), Some(f));
    }

    #[test]
    fn round_trips_connect() {
        let f = ClientFrame::Connect { network_name: "party".into() };
        assert_eq!(ClientFrame::parse(f.encode().trim_end()), Some(f));
    }

    #[test]
    fn round_trips_accept() {
        let f = ClientFrame::Accept { session_id: "abc123".into() };
        assert_eq!(ClientFrame::parse(f.encode().trim_end()), Some(f));
    }

    #[test]
    fn round_trips_ok() {
        assert_eq!(ServerFrame::parse(ServerFrame::Ok.encode().trim_end()), Some(ServerFrame::Ok));
    }

    #[test]
    fn round_trips_new_member() {
        let f = ServerFrame::NewMember { session_id: "abc123".into() };
        assert_eq!(ServerFrame::parse(f.encode().trim_end()), Some(f));
    }

    #[test]
    fn round_trips_err() {
        let f = ServerFrame::Err { message: "name in use".into() };
        assert_eq!(ServerFrame::parse(f.encode().trim_end()), Some(f));
    }

    #[test]
    fn rejects_malformed_lines() {
        assert_eq!(ClientFrame::parse("GARBAGE"), None);
        assert_eq!(ClientFrame::parse("REGISTER"), None);
        assert_eq!(ClientFrame::parse("REGISTER   "), None);
        assert_eq!(ServerFrame::parse("GARBAGE"), None);
    }

    #[test]
    fn network_names_with_spaces_are_preserved_verbatim() {
        let f = ClientFrame::Register { network_name: "my cool party".into() };
        assert_eq!(ClientFrame::parse(f.encode().trim_end()), Some(f));
    }
}
