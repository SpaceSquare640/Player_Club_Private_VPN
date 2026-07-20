//! Forward Error Correction (Phase D).
//!
//! FEC adds redundancy to the encrypted data plane so a few lost packets can be
//! reconstructed without retransmission — steadier latency for game traffic.
//!
//! **D.2** ships Reed-Solomon erasure coding ([`rs`]): `r` parity shards per
//! group of `k`, recovering **any `r` losses** at `r/k` overhead. It supersedes
//! D.1's single-parity XOR code, which could only ever rebuild one packet and
//! gave up on the burst losses typical of real networks — `RS(k, 1)` is
//! equivalent to that XOR parity, so the old behaviour remains available simply
//! by configuring `r = 1`.
//!
//! FEC runs **over the inner IP plaintext**: parity is computed before each
//! packet is sealed, and the receiver decrypts each packet before recovering on
//! plaintext. The C4 single-session guarantee and the anti-replay window are
//! therefore untouched, and a reconstructed packet needs no per-packet nonce.

pub mod rs;


pub use rs::{RsDecoder, RsEncoder, RsParity};
