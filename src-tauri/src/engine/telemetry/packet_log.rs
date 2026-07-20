//! Packet-log entry type and a bounded ring buffer that keeps memory flat over
//! long sessions.

use std::collections::VecDeque;

use serde::Serialize;

/// Direction of a logged packet relative to this host.
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Direction {
    Tx,
    Rx,
}

/// One line in the terminal-style packet log.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PacketLogEntry {
    /// Milliseconds since the engine session started.
    pub t_ms: u64,
    pub dir: Direction,
    pub proto: String,
    pub len: u16,
    pub note: String,
}

/// Bounded FIFO of recent packet entries. Oldest entries are dropped once the
/// capacity is reached.
#[derive(Debug)]
pub struct RingBuffer {
    cap: usize,
    inner: VecDeque<PacketLogEntry>,
}

impl RingBuffer {
    pub fn new(cap: usize) -> Self {
        Self {
            cap,
            inner: VecDeque::with_capacity(cap),
        }
    }

    pub fn push(&mut self, entry: PacketLogEntry) {
        if self.inner.len() == self.cap {
            self.inner.pop_front();
        }
        self.inner.push_back(entry);
    }

    pub fn extend<I: IntoIterator<Item = PacketLogEntry>>(&mut self, entries: I) {
        for e in entries {
            self.push(e);
        }
    }

    pub fn snapshot(&self) -> Vec<PacketLogEntry> {
        self.inner.iter().cloned().collect()
    }

    pub fn clear(&mut self) {
        self.inner.clear();
    }
}
