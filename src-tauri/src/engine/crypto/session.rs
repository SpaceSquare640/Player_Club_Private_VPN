//! Encrypted peer session: AEAD over the Noise transport keys, with an explicit
//! per-packet counter and sliding-window anti-replay (UDP reorders/loses).
//!
//! `seal`/`open` drive the C4 encrypted keepalive (see `engine::pipeline`) and
//! the C5 data plane.

use std::io;

use snow::TransportState;

use super::replay::ReplayWindow;

/// AEAD overhead added by the ChaChaPoly cipher per message.
const TAG_LEN: usize = 16;

pub struct CryptoSession {
    transport: TransportState,
    replay: ReplayWindow,
}

impl CryptoSession {
    pub fn new(transport: TransportState) -> Self {
        Self {
            transport,
            replay: ReplayWindow::new(),
        }
    }

    /// Encrypt `plaintext`, returning `(counter, ciphertext)`. The caller frames
    /// it via `frame::encode_data(counter, &ciphertext)`.
    pub fn seal(&mut self, plaintext: &[u8]) -> io::Result<(u64, Vec<u8>)> {
        let counter = self.transport.sending_nonce();
        let mut out = vec![0u8; plaintext.len() + TAG_LEN];
        let n = self
            .transport
            .write_message(plaintext, &mut out)
            .map_err(|e| io::Error::other(format!("seal: {e}")))?;
        out.truncate(n);
        Ok((counter, out))
    }

    /// Decrypt a `Data` frame's `(counter, ciphertext)`, enforcing anti-replay.
    /// The replay window is only advanced after the packet authenticates.
    pub fn open(&mut self, counter: u64, ciphertext: &[u8]) -> io::Result<Vec<u8>> {
        if !self.replay.check(counter) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "replay or out-of-window packet rejected",
            ));
        }
        self.transport.set_receiving_nonce(counter);
        let mut out = vec![0u8; ciphertext.len()];
        let n = self
            .transport
            .read_message(ciphertext, &mut out)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, format!("open: {e}")))?;
        out.truncate(n);
        self.replay.accept(counter);
        Ok(out)
    }

    /// The peer's static public key, learned during the handshake.
    pub fn remote_static(&self) -> Option<Vec<u8>> {
        self.transport.get_remote_static().map(|s| s.to_vec())
    }
}
