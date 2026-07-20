//! Engine lifecycle controller — owns the running task and the shared live
//! state. A single instance is stored in Tauri's managed state.

use std::sync::Mutex;

use tauri::async_runtime::{self, JoinHandle};
use tokio::sync::watch;

use super::config::EngineConfig;
use super::error::{EngineError, Result};
use super::notice::EngineNotice;
use super::state::{EngineState, EngineStatus, SharedState};
use super::telemetry::{capture, task, PacketLogEntry, TelemetrySink, TelemetrySnapshot};
use super::transport::probe;
use super::tun::privilege;

/// Handle to the currently running telemetry task.
struct Running {
    shutdown: watch::Sender<bool>,
    handle: JoinHandle<()>,
}

/// Owns the engine lifecycle and the shared state read by the IPC commands.
#[derive(Default)]
pub struct EngineController {
    shared: SharedState,
    running: Mutex<Option<Running>>,
}

impl EngineController {
    /// Start the telemetry loop. Returns [`EngineError::AlreadyRunning`] if a
    /// session is already active.
    pub fn start(&self, cfg: EngineConfig, sink: Box<dyn TelemetrySink>) -> Result<()> {
        let mut guard = self.running.lock().map_err(|_| EngineError::LockPoisoned)?;
        if guard.is_some() {
            return Err(EngineError::AlreadyRunning);
        }

        // Real-adapter mode is gated on elevation. Rather than fail, report a
        // clear state + remediation and let the UI offer to relaunch elevated.
        if cfg.use_real_tun {
            let status = privilege::status();
            if !status.can_create_tun {
                self.shared.set_state(EngineState::NeedsElevation);
                sink.state(EngineState::NeedsElevation);
                let notice = if status.os == "windows" {
                    EngineNotice::needs_elevation()
                } else {
                    EngineNotice::unsupported_platform()
                };
                sink.notice(&notice);
                return Ok(());
            }
        }

        self.shared.mark_started();
        let (shutdown, rx) = watch::channel(false);
        let shared = self.shared.clone();
        let handle = if cfg.use_real_tun {
            async_runtime::spawn(capture::run(cfg, sink, shared, rx))
        } else if cfg.transport_probe {
            async_runtime::spawn(probe::run(cfg, sink, shared, rx))
        } else {
            async_runtime::spawn(task::run(cfg, sink, shared, rx))
        };
        *guard = Some(Running { shutdown, handle });
        Ok(())
    }

    /// Stop the loop and reset live state to Idle. Returns
    /// [`EngineError::NotRunning`] if no session is active.
    pub fn stop(&self) -> Result<()> {
        let mut guard = self.running.lock().map_err(|_| EngineError::LockPoisoned)?;
        let running = guard.take().ok_or(EngineError::NotRunning)?;
        drop(guard);

        // Signal the task to exit, then abort to guarantee prompt teardown.
        let _ = running.shutdown.send(true);
        running.handle.abort();
        self.shared.mark_stopped();
        Ok(())
    }

    pub fn status(&self) -> EngineStatus {
        self.shared.status()
    }

    pub fn snapshot(&self) -> TelemetrySnapshot {
        self.shared.snapshot()
    }

    pub fn packet_log(&self) -> Vec<PacketLogEntry> {
        self.shared.packet_log()
    }
}
