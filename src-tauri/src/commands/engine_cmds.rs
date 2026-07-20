//! Engine lifecycle commands (UI → engine).

use tauri::{AppHandle, State};

use crate::engine::{EngineConfig, EngineController, EngineStatus, Result};

use super::events::TauriSink;

/// Start the engine telemetry loop with the given configuration.
#[tauri::command]
pub fn start_engine(
    app: AppHandle,
    controller: State<'_, EngineController>,
    config: EngineConfig,
) -> Result<()> {
    controller.start(config, Box::new(TauriSink::new(app)))
}

/// Stop the engine and return to Idle.
#[tauri::command]
pub fn stop_engine(controller: State<'_, EngineController>) -> Result<()> {
    controller.stop()
}

/// High-level status snapshot (pull).
#[tauri::command]
pub fn get_status(controller: State<'_, EngineController>) -> EngineStatus {
    controller.status()
}
