//! Ready-to-paste blob codec: `PCPV1.<KIND>.<base64url(json)>.<crc32 hex>`.
//!
//! The CRC32 detects copy/paste corruption *before* any crypto runs — a
//! malformed blob is rejected instantly. Authenticity is NOT provided here
//! (the blob is public): it comes from the out-of-band channel + fingerprint
//! verification + the C4 IK handshake which authorizes only the expected key.

use std::net::SocketAddr;

use base64::engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD};
use base64::Engine;

use super::message::{SignalEnvelope, PROTOCOL_VERSION};

const MAGIC: &str = "PCPV1";

#[derive(Debug)]
pub enum SignalError {
    Format(String),
    Checksum,
    Decode(String),
    Version(u32),
    KindMismatch,
    Invalid(String),
}

impl std::fmt::Display for SignalError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SignalError::Format(s) => write!(f, "malformed blob: {s}"),
            SignalError::Checksum => write!(f, "blob is corrupted or truncated (checksum mismatch)"),
            SignalError::Decode(s) => write!(f, "could not decode blob: {s}"),
            SignalError::Version(v) => write!(f, "unsupported blob version {v}"),
            SignalError::KindMismatch => write!(f, "blob kind does not match its label"),
            SignalError::Invalid(s) => write!(f, "invalid blob content: {s}"),
        }
    }
}

impl std::error::Error for SignalError {}

fn crc(bytes: &[u8]) -> u32 {
    let mut hasher = crc32fast::Hasher::new();
    hasher.update(bytes);
    hasher.finalize()
}

/// Encode an envelope into a single paste-robust line.
pub fn encode(envelope: &SignalEnvelope) -> String {
    let json = serde_json::to_vec(envelope).expect("envelope serialization is infallible");
    let body = URL_SAFE_NO_PAD.encode(&json);
    format!("{MAGIC}.{}.{}.{:08x}", envelope.kind.label(), body, crc(&json))
}

/// Decode and validate a blob. Rejects corruption (CRC), wrong format/version,
/// kind/label mismatch, bad key length, and unparseable candidate addresses.
pub fn decode(blob: &str) -> Result<SignalEnvelope, SignalError> {
    let parts: Vec<&str> = blob.trim().split('.').collect();
    if parts.len() != 4 {
        return Err(SignalError::Format(
            "expected 4 dot-separated sections".into(),
        ));
    }
    if parts[0] != MAGIC {
        return Err(SignalError::Format(format!(
            "unknown blob prefix '{}'",
            parts[0]
        )));
    }
    let label = parts[1];

    let json = URL_SAFE_NO_PAD
        .decode(parts[2])
        .map_err(|e| SignalError::Decode(e.to_string()))?;
    let expected = u32::from_str_radix(parts[3], 16)
        .map_err(|_| SignalError::Format("invalid checksum field".into()))?;
    if crc(&json) != expected {
        return Err(SignalError::Checksum);
    }

    let envelope: SignalEnvelope =
        serde_json::from_slice(&json).map_err(|e| SignalError::Decode(e.to_string()))?;
    if envelope.v != PROTOCOL_VERSION {
        return Err(SignalError::Version(envelope.v));
    }
    if envelope.kind.label() != label {
        return Err(SignalError::KindMismatch);
    }

    // Structural validation so callers get clean, typed data.
    let pk = STANDARD
        .decode(envelope.pk.trim())
        .map_err(|e| SignalError::Invalid(format!("public key: {e}")))?;
    if pk.len() != 32 {
        return Err(SignalError::Invalid("public key length".into()));
    }
    for c in &envelope.cands {
        c.a.parse::<SocketAddr>()
            .map_err(|_| SignalError::Invalid(format!("candidate address '{}'", c.a)))?;
    }

    Ok(envelope)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::signaling::message::{SignalKind, WireCandidate};

    fn sample(kind: SignalKind) -> SignalEnvelope {
        SignalEnvelope {
            v: PROTOCOL_VERSION,
            kind,
            sid: STANDARD.encode([1u8; 16]),
            pk: STANDARD.encode([2u8; 32]),
            cands: vec![
                WireCandidate {
                    a: "192.168.1.5:51820".into(),
                    k: "host".into(),
                },
                WireCandidate {
                    a: "203.0.113.9:40000".into(),
                    k: "reflexive".into(),
                },
            ],
        }
    }

    #[test]
    fn round_trips_offer_and_answer() {
        for kind in [SignalKind::Offer, SignalKind::Answer] {
            let env = sample(kind);
            let blob = encode(&env);
            assert!(blob.starts_with("PCPV1."));
            assert_eq!(decode(&blob).unwrap(), env);
        }
    }

    #[test]
    fn rejects_checksum_corruption() {
        let blob = encode(&sample(SignalKind::Offer));
        let mut chars: Vec<char> = blob.chars().collect();
        let last = chars.len() - 1;
        chars[last] = if chars[last] == '0' { '1' } else { '0' };
        let bad: String = chars.into_iter().collect();
        assert!(matches!(decode(&bad), Err(SignalError::Checksum)));
    }

    #[test]
    fn rejects_truncation_and_bad_prefix() {
        let blob = encode(&sample(SignalKind::Offer));
        assert!(decode(&blob[..blob.len() - 6]).is_err());
        assert!(matches!(
            decode(&blob.replacen("PCPV1", "XXXX1", 1)),
            Err(SignalError::Format(_))
        ));
    }

    #[test]
    fn rejects_kind_label_mismatch() {
        let blob = encode(&sample(SignalKind::Answer));
        let tampered = blob.replacen(".ANSWER.", ".OFFER.", 1);
        assert!(matches!(decode(&tampered), Err(SignalError::KindMismatch)));
    }

    #[test]
    fn rejects_bad_version() {
        let mut env = sample(SignalKind::Offer);
        env.v = 2;
        assert!(matches!(decode(&encode(&env)), Err(SignalError::Version(2))));
    }

    #[test]
    fn rejects_bad_public_key_and_candidate() {
        let mut env = sample(SignalKind::Offer);
        env.pk = STANDARD.encode([2u8; 16]); // wrong length
        assert!(matches!(decode(&encode(&env)), Err(SignalError::Invalid(_))));

        let mut env2 = sample(SignalKind::Offer);
        env2.cands[0].a = "not-an-address".into();
        assert!(matches!(decode(&encode(&env2)), Err(SignalError::Invalid(_))));
    }
}
