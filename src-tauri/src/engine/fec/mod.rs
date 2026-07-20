//! Forward Error Correction (Phase D).
//!
//! FEC adds redundancy to the encrypted data plane so a few lost packets can be
//! reconstructed without retransmission — steadier latency for game traffic.
//!
//! **D.1** ships a single-parity XOR erasure code ([`xor`]): one parity packet
//! per group of `k`, recovering one loss per group at ~`1/k` overhead. It runs
//! **over the inner IP plaintext** (parity is computed before each packet is
//! sealed; the receiver decrypts each packet, then recovers on plaintext), so
//! the C4 single-session guarantee and anti-replay window are untouched.
//!
//! **D.2** will introduce Reed-Solomon (multiple parity, multiple losses per
//! group) behind the same call sites; the XOR types here are the first
//! implementation, not the abstraction boundary.

pub mod xor;

pub use xor::{Parity, XorDecoder, XorEncoder};
