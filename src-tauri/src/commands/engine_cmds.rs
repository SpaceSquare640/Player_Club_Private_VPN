//! Engine lifecycle commands (UI → engine).

use tauri::{AppHandle, State};

use crate::engine::telemetry::TelemetrySink;
use crate::engine::{EngineConfig, EngineController, EngineState, EngineStatus, Result};

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
///
/// `EngineController::stop` aborts the running task immediately rather than
/// waiting for it to observe its shutdown signal — necessary so Stop is
/// instant even if the task is blocked in an `.await`, but it means the
/// task never reaches its own graceful-exit code, which is what normally
/// pushes the `engine://state: Idle` event the UI relies on to re-enable
/// Start. Without emitting it here too, the frontend's `running`/`state`
/// stay on whatever they were mid-session — the Diagnostics page (and any
/// other view mirroring live engine state) looks permanently stuck after
/// Stop until something else happens to trigger a re-pull, even though the
/// engine itself is correctly idle. Push it explicitly so Stop is visibly
/// complete, not just actually complete.
#[tauri::command]
pub fn stop_engine(app: AppHandle, controller: State<'_, EngineController>) -> Result<()> {
    controller.stop()?;
    TauriSink::new(app).state(EngineState::Idle);
    Ok(())
}

/// High-level status snapshot (pull).
#[tauri::command]
pub fn get_status(controller: State<'_, EngineController>) -> EngineStatus {
    controller.status()
}
