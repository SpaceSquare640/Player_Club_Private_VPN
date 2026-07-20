//! Peer-connection commands (C4) — drive the hole-punch handshake + data link.

use std::sync::Arc;

use tauri::{AppHandle, State};

use crate::engine::connection::ConnectionManager;
use crate::engine::crypto::Identity;
use crate::engine::telemetry::TelemetrySink;

use super::events::TauriSink;

/// Connect to the negotiated peer. Spawns the C4 pipeline (fan-out handshake →
/// encrypted keepalive); progress is reported via `engine://state` /
/// `telemetry://stats` and the `get_connection` snapshot's `link` field.
#[tauri::command]
pub fn connect_peer(
    app: AppHandle,
    identity: State<'_, Arc<Identity>>,
    manager: State<'_, ConnectionManager>,
) -> Result<(), String> {
    let sink: Box<dyn TelemetrySink> = Box::new(TauriSink::new(app));
    manager.connect(identity.inner().clone(), sink)
}

/// Disconnect the live peer link (idempotent).
#[tauri::command]
pub fn disconnect_peer(manager: State<'_, ConnectionManager>) -> Result<(), String> {
    manager.disconnect();
    Ok(())
}
