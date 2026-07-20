//! Telemetry pull commands — fallbacks so a late-mounting view paints
//! immediately instead of waiting for the next pushed event.

use tauri::State;

use crate::engine::telemetry::{PacketLogEntry, TelemetrySnapshot};
use crate::engine::EngineController;

/// Latest stats snapshot.
#[tauri::command]
pub fn get_snapshot(controller: State<'_, EngineController>) -> TelemetrySnapshot {
    controller.snapshot()
}

/// Current packet-log ring-buffer contents (oldest first).
#[tauri::command]
pub fn get_packet_log(controller: State<'_, EngineController>) -> Vec<PacketLogEntry> {
    controller.packet_log()
}
