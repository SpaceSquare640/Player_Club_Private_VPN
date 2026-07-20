//! Signaling message DTOs carried inside a blob. Field names are short to keep
//! the pasted blob compact; all values are public (no secrets).

use serde::{Deserialize, Serialize};

/// Current signaling protocol version.
pub const PROTOCOL_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SignalKind {
    Offer,
    Answer,
}

impl SignalKind {
    /// Uppercase label embedded in the blob prefix (`PCPV1.OFFER.…`).
    pub fn label(&self) -> &'static str {
        match self {
            SignalKind::Offer => "OFFER",
            SignalKind::Answer => "ANSWER",
        }
    }
}

/// One candidate endpoint on the wire.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WireCandidate {
    /// Socket address, e.g. `203.0.113.5:51820`.
    pub a: String,
    /// Candidate kind: `host` or `reflexive`.
    pub k: String,
}

/// The full signaling payload (the JSON inside a blob).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignalEnvelope {
    pub v: u32,
    pub kind: SignalKind,
    /// Base64 16-byte session id (shared by the offer and its answer).
    pub sid: String,
    /// Base64 32-byte X25519 public key.
    pub pk: String,
    pub cands: Vec<WireCandidate>,
}
