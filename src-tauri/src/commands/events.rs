//! Event-channel names and the Tauri-backed [`TelemetrySink`] implementation.

use serde::Serialize;
use tauri::{AppHandle, Emitter};

use crate::engine::notice::EngineNotice;
use crate::engine::telemetry::{PacketLogEntry, TelemetrySink, TelemetrySnapshot};
use crate::engine::EngineState;

/// Stats stream (engine → UI), ~tick_hz times per second.
pub const EVT_STATS: &str = "telemetry://stats";
/// Packet-log batches (engine → UI).
pub const EVT_PACKET: &str = "telemetry://packet";
/// Lifecycle-state transitions (engine → UI).
pub const EVT_STATE: &str = "engine://state";
/// Structured notices / remediation hints (engine → UI).
pub const EVT_NOTICE: &str = "engine://notice";

#[derive(Clone, Serialize)]
struct StatePayload {
    state: EngineState,
}

/// Concrete sink that forwards telemetry to the webview via the app's event
/// system. Emission errors (e.g. no listener attached yet) are intentionally
/// ignored — telemetry is best-effort and the pull commands cover late mounts.
pub struct TauriSink {
    app: AppHandle,
}

impl TauriSink {
    pub fn new(app: AppHandle) -> Self {
        Self { app }
    }
}

impl TelemetrySink for TauriSink {
    fn stats(&self, snapshot: &TelemetrySnapshot) {
        let _ = self.app.emit(EVT_STATS, snapshot);
    }

    fn packets(&self, batch: &[PacketLogEntry]) {
        let _ = self.app.emit(EVT_PACKET, batch);
    }

    fn state(&self, state: EngineState) {
        let _ = self.app.emit(EVT_STATE, StatePayload { state });
    }

    fn notice(&self, notice: &EngineNotice) {
        let _ = self.app.emit(EVT_NOTICE, notice);
    }
}
