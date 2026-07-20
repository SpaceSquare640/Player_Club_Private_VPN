//! Engine lifecycle state and the shared, thread-safe live state that backs the
//! pull-style commands (`get_status`, `get_snapshot`, `get_packet_log`).

use std::sync::{Arc, Mutex};
use std::time::Instant;

use serde::Serialize;

use super::telemetry::{PacketLogEntry, RingBuffer, TelemetrySnapshot};

/// How many packet-log entries to retain in the ring buffer.
const PACKET_LOG_CAP: usize = 500;

/// Engine lifecycle state. Mirrors the `engine://state` event payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum EngineState {
    Idle,
    Connecting,
    /// Bringing up the real adapter (Wintun create + IP assignment).
    Starting,
    Connected,
    /// A real-adapter session was requested without the required privileges.
    #[serde(rename = "needs-elevation")]
    NeedsElevation,
    /// A fault occurred (e.g. adapter creation failed).
    Error,
}

/// High-level status returned by `get_status`.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EngineStatus {
    pub state: EngineState,
    pub peers: u32,
    pub uptime_s: u64,
}

#[derive(Debug)]
struct Inner {
    state: Mutex<EngineState>,
    snapshot: Mutex<TelemetrySnapshot>,
    packets: Mutex<RingBuffer>,
    started_at: Mutex<Option<Instant>>,
    peers: Mutex<u32>,
}

/// Cheaply cloneable handle to the engine's live state, shared between the
/// async telemetry loop (writer) and the IPC commands (readers).
#[derive(Debug, Clone)]
pub struct SharedState {
    inner: Arc<Inner>,
}

impl Default for SharedState {
    fn default() -> Self {
        Self {
            inner: Arc::new(Inner {
                state: Mutex::new(EngineState::Idle),
                snapshot: Mutex::new(TelemetrySnapshot::idle()),
                packets: Mutex::new(RingBuffer::new(PACKET_LOG_CAP)),
                started_at: Mutex::new(None),
                peers: Mutex::new(0),
            }),
        }
    }
}

impl SharedState {
    /// Record the session start time (drives `uptime_s` / packet timestamps).
    pub fn mark_started(&self) {
        *self.inner.started_at.lock().unwrap() = Some(Instant::now());
    }

    /// Reset all live state back to Idle (called on stop).
    pub fn mark_stopped(&self) {
        *self.inner.started_at.lock().unwrap() = None;
        *self.inner.peers.lock().unwrap() = 0;
        *self.inner.state.lock().unwrap() = EngineState::Idle;
        *self.inner.snapshot.lock().unwrap() = TelemetrySnapshot::idle();
        self.inner.packets.lock().unwrap().clear();
    }

    pub fn set_state(&self, s: EngineState) {
        *self.inner.state.lock().unwrap() = s;
    }

    pub fn state(&self) -> EngineState {
        *self.inner.state.lock().unwrap()
    }

    pub fn set_snapshot(&self, snap: TelemetrySnapshot) {
        *self.inner.peers.lock().unwrap() = snap.peers;
        *self.inner.snapshot.lock().unwrap() = snap;
    }

    pub fn snapshot(&self) -> TelemetrySnapshot {
        self.inner.snapshot.lock().unwrap().clone()
    }

    pub fn push_packets(&self, batch: &[PacketLogEntry]) {
        self.inner
            .packets
            .lock()
            .unwrap()
            .extend(batch.iter().cloned());
    }

    pub fn packet_log(&self) -> Vec<PacketLogEntry> {
        self.inner.packets.lock().unwrap().snapshot()
    }

    /// Milliseconds since the session started (0 when stopped).
    pub fn elapsed_ms(&self) -> u64 {
        self.inner
            .started_at
            .lock()
            .unwrap()
            .map(|t| t.elapsed().as_millis() as u64)
            .unwrap_or(0)
    }

    pub fn status(&self) -> EngineStatus {
        let uptime_s = self
            .inner
            .started_at
            .lock()
            .unwrap()
            .map(|t| t.elapsed().as_secs())
            .unwrap_or(0);
        EngineStatus {
            state: self.state(),
            peers: *self.inner.peers.lock().unwrap(),
            uptime_s,
        }
    }
}
