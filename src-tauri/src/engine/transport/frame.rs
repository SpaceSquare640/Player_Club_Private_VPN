//! Wire framing + the demultiplexer that lets STUN and our own frames coexist
//! on one socket.
//!
//! Disambiguation rules:
//!   1. STUN is identified by its magic cookie (`0x2112A442` at byte offset 4) —
//!      a signature our frames never carry.
//!   2. Our frames carry a 1-byte [`FrameKind`] tag, inspected only after the
//!      STUN check fails.

/// STUN magic cookie (RFC 5389), big-endian at datagram bytes [4..8].
pub const STUN_MAGIC_COOKIE: u32 = 0x2112_A442;

/// First-byte tag for our protocol frames.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameKind {
    Ping = 1,
    Pong = 2,
    Handshake = 3, // Noise IK messages (C2 crypto, C4 fan-out)
    Data = 4,      // encrypted session frames (C4 keepalive, C5 data plane)
}

impl FrameKind {
    fn from_tag(tag: u8) -> Option<FrameKind> {
        match tag {
            1 => Some(FrameKind::Ping),
            2 => Some(FrameKind::Pong),
            3 => Some(FrameKind::Handshake),
            4 => Some(FrameKind::Data),
            _ => None,
        }
    }
}

/// Result of classifying an inbound datagram.
#[derive(Debug)]
pub enum Inbound<'a> {
    /// A STUN message. The bytes feed the response correlator in C4; the C1
    /// probe handles STUN inline during gathering, so they are unused for now.
    Stun(#[allow(dead_code)] &'a [u8]),
    /// A protocol frame and its payload (the bytes after the 1-byte tag).
    Frame(FrameKind, &'a [u8]),
    Unknown,
}

/// Classify a raw datagram: STUN (by cookie) first, then our frame tag.
pub fn classify(datagram: &[u8]) -> Inbound<'_> {
    if is_stun(datagram) {
        return Inbound::Stun(datagram);
    }
    match datagram.first().and_then(|t| FrameKind::from_tag(*t)) {
        Some(kind) => Inbound::Frame(kind, &datagram[1..]),
        None => Inbound::Unknown,
    }
}

fn is_stun(d: &[u8]) -> bool {
    d.len() >= 20
        && (d[0] >> 6) == 0 // STUN's two most-significant bits are zero
        && u32::from_be_bytes([d[4], d[5], d[6], d[7]]) == STUN_MAGIC_COOKIE
}

/// Encode a Ping/Pong frame: `[tag:1][seq:8 BE]`.
pub fn encode_ping(seq: u64) -> [u8; 9] {
    encode_seq_frame(FrameKind::Ping, seq)
}

pub fn encode_pong(seq: u64) -> [u8; 9] {
    encode_seq_frame(FrameKind::Pong, seq)
}

fn encode_seq_frame(kind: FrameKind, seq: u64) -> [u8; 9] {
    let mut b = [0u8; 9];
    b[0] = kind as u8;
    b[1..].copy_from_slice(&seq.to_be_bytes());
    b
}

/// Decode the sequence number from a Ping/Pong payload (the bytes after the tag).
pub fn decode_seq(payload: &[u8]) -> Option<u64> {
    if payload.len() < 8 {
        return None;
    }
    let bytes: [u8; 8] = payload[..8].try_into().ok()?;
    Some(u64::from_be_bytes(bytes))
}

/// Encode a Handshake frame: `[tag][noise message…]`.
pub fn encode_handshake(msg: &[u8]) -> Vec<u8> {
    let mut v = Vec::with_capacity(1 + msg.len());
    v.push(FrameKind::Handshake as u8);
    v.extend_from_slice(msg);
    v
}

/// Encode a Data frame: `[tag][counter:8 BE][ciphertext…]`.
pub fn encode_data(counter: u64, ciphertext: &[u8]) -> Vec<u8> {
    let mut v = Vec::with_capacity(9 + ciphertext.len());
    v.push(FrameKind::Data as u8);
    v.extend_from_slice(&counter.to_be_bytes());
    v.extend_from_slice(ciphertext);
    v
}

/// Split a Data payload (the bytes after the tag) into `(counter, ciphertext)`.
pub fn decode_data(payload: &[u8]) -> Option<(u64, &[u8])> {
    if payload.len() < 8 {
        return None;
    }
    let counter = u64::from_be_bytes(payload[..8].try_into().ok()?);
    Some((counter, &payload[8..]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_stun_by_cookie() {
        let mut d = [0u8; 20];
        d[0] = 0x01; // binding response high byte (MSBs still zero)
        d[4..8].copy_from_slice(&STUN_MAGIC_COOKIE.to_be_bytes());
        assert!(matches!(classify(&d), Inbound::Stun(_)));
    }

    #[test]
    fn classifies_ping_and_pong() {
        let ping = encode_ping(42);
        match classify(&ping) {
            Inbound::Frame(FrameKind::Ping, payload) => assert_eq!(decode_seq(payload), Some(42)),
            other => panic!("expected ping, got {other:?}"),
        }
        let pong = encode_pong(7);
        match classify(&pong) {
            Inbound::Frame(FrameKind::Pong, payload) => assert_eq!(decode_seq(payload), Some(7)),
            other => panic!("expected pong, got {other:?}"),
        }
    }

    #[test]
    fn unknown_tag_is_unknown() {
        assert!(matches!(classify(&[0x7f, 0, 0]), Inbound::Unknown));
        assert!(matches!(classify(&[]), Inbound::Unknown));
    }
}
