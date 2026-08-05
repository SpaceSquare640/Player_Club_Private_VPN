//! Wire protocol for the networked signaling server (G.1). Distinct from
//! `message::SignalEnvelope` — that DTO carries a single offer/answer blob for
//! manual paste-based signaling; this one carries network membership over a
//! live WebSocket connection. Offer/answer relay through this channel is
//! Phase G.3's job, not this one's.
//!
//! Not yet wired into any Tauri command (that's G.2/G.4) — see `server.rs`.
#![allow(dead_code)]

use serde::{Deserialize, Serialize};

/// Current networked-signaling protocol version.
pub const PROTOCOL_VERSION: u32 = 1;

/// A network member as seen by every other member, including themselves.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemberInfo {
    /// Base64 X25519 public key — the authoritative per-connection identity,
    /// matching `Identity::public_b64()`.
    pub pubkey: String,
    /// Short `PC-XXXX-XXXX-XXXX-XXXX` fingerprint, for display only.
    pub fingerprint: String,
}

/// Client → server.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ClientMessage {
    /// Sent once, immediately after the WebSocket connects.
    Join {
        v: u32,
        network_name: String,
        /// SHA-256 hex digest of the network password — the server never
        /// receives or stores the plaintext password.
        password_hash: String,
        pubkey: String,
        fingerprint: String,
    },
    /// Forward an opaque signaling blob (a `blob::encode`d offer/answer
    /// envelope — same format a user would otherwise paste manually) to
    /// another member, addressed by pubkey. The server relays this without
    /// inspecting or validating its contents; only the two endpoints ever
    /// decode it.
    Relay { to_pubkey: String, blob: String },
}

/// Server → client.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ServerMessage {
    /// Reply to a successful `Join` — includes every member already present,
    /// so the new arrival can build its initial roster in one message.
    JoinAccepted { members: Vec<MemberInfo> },
    /// Reply to a rejected `Join`. The connection is closed immediately after.
    JoinRejected { reason: JoinRejectReason },
    /// Broadcast to every other member when someone new joins.
    MemberJoined(MemberInfo),
    /// Broadcast to every other member when someone disconnects.
    MemberLeft { pubkey: String },
    /// A `Relay` forwarded from `from_pubkey`. Silently dropped server-side
    /// (never sent) if `to_pubkey` in the original `Relay` isn't a current
    /// member — a member leaving mid-relay is an expected race, not an error.
    Relayed { from_pubkey: String, blob: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum JoinRejectReason {
    WrongPassword,
    WrongNetworkName,
    UnsupportedVersion,
    AlreadyJoined,
    MalformedJoin,
}
