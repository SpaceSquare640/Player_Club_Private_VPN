//! The decoupling seam between the engine and the IPC layer.

use crate::engine::notice::EngineNotice;
use crate::engine::state::EngineState;

use super::metrics::TelemetrySnapshot;
use super::packet_log::PacketLogEntry;

/// The engine emits telemetry through this trait; the IPC layer supplies a
/// concrete implementation (e.g. backed by a Tauri `AppHandle`). Keeping it
/// abstract means the engine never depends on Tauri and can be driven by a test
/// sink in unit tests.
pub trait TelemetrySink: Send + Sync + 'static {
    /// A fresh stats sample (emitted each tick).
    fn stats(&self, snapshot: &TelemetrySnapshot);
    /// A batch of packet-log entries.
    fn packets(&self, batch: &[PacketLogEntry]);
    /// A lifecycle-state transition.
    fn state(&self, state: EngineState);
    /// A structured notice (remediation hint or error).
    fn notice(&self, notice: &EngineNotice);
}
