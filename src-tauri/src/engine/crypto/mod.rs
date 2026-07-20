//! Cryptographic layer: persistent identity, the Noise IK handshake, and the
//! encrypted peer session.

pub mod handshake;
pub mod identity;
pub mod replay;
pub mod session;

pub use identity::Identity;

/// Noise pattern: IK (initiator knows responder static) with X25519 +
/// ChaCha20-Poly1305 + BLAKE2s.
pub(crate) const NOISE_PARAMS: &str = "Noise_IK_25519_ChaChaPoly_BLAKE2s";
