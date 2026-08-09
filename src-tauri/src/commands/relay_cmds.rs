//! Commands for hosting a relay (`engine::relay`) from within this app
//! itself, rather than requiring the separate standalone `relay` binary.

use tauri::State;

use crate::engine::relay::{RelayHost, RelayHostStatus};

/// Start hosting a relay on `port` (every interface; `0` picks an ephemeral
/// port). Returns the actual bound port. Rejected if already hosting one —
/// `stop_relay` first.
#[tauri::command]
pub async fn start_relay(port: u16, host: State<'_, RelayHost>) -> Result<u16, String> {
    host.start(port).await
}

/// Stop hosting the relay (idempotent).
#[tauri::command]
pub async fn stop_relay(host: State<'_, RelayHost>) -> Result<(), String> {
    host.stop().await;
    Ok(())
}

/// Current relay-hosting status, or `None` if not hosting one.
#[tauri::command]
pub fn get_relay_status(host: State<'_, RelayHost>) -> Option<RelayHostStatus> {
    host.status()
}
