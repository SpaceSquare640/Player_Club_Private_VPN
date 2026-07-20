//! Privilege/elevation commands (UI → engine).

use tauri::AppHandle;

use crate::engine::tun::{privilege, ElevationStatus};

/// Report whether the process is elevated and can create a real adapter.
#[tauri::command]
pub fn get_privilege_status() -> ElevationStatus {
    privilege::status()
}

/// Relaunch the app elevated (UAC), then exit this instance so the elevated one
/// takes over. Returns an error string if the request fails or is declined.
#[tauri::command]
pub fn request_elevation(app: AppHandle) -> Result<(), String> {
    privilege::relaunch_elevated().map_err(|e| e.to_string())?;
    app.exit(0);
    Ok(())
}
