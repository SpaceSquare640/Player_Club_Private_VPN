//! Player Club Private VPN — Rust engine entry point.
//!
//! The networking engine lives under `engine/` (Phase A: telemetry spine,
//! Phase B: TUN/TAP, Phase C: transport/NAT/crypto); IPC handlers and the event
//! bridge live under `commands/`. The `ping` command is a liveness probe.

mod commands;
mod engine;

use std::sync::Arc;

use tauri::Manager;

use commands::{
    accept_answer, accept_offer, connect_peer, create_offer, disconnect_peer, get_connection,
    get_identity, get_packet_log, get_privilege_status, get_snapshot, get_status,
    request_elevation, start_engine, stop_engine,
};
use engine::connection::ConnectionManager;
use engine::crypto::Identity;
use engine::EngineController;

/// Liveness probe invoked from the frontend to confirm the engine is reachable.
#[tauri::command]
fn ping() -> String {
    "pong from the Player Club engine".to_string()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(EngineController::default())
        .manage(ConnectionManager::default())
        .setup(|app| {
            // Load (or generate on first run) the static identity into a
            // per-app config location, then share it via managed state.
            let dir = app.path().app_config_dir()?;
            std::fs::create_dir_all(&dir)?;
            let identity = Identity::load_or_generate(&dir.join("identity.json"))?;
            app.manage(Arc::new(identity));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            ping,
            start_engine,
            stop_engine,
            get_status,
            get_snapshot,
            get_packet_log,
            get_privilege_status,
            request_elevation,
            get_identity,
            create_offer,
            accept_offer,
            accept_answer,
            get_connection,
            connect_peer,
            disconnect_peer
        ])
        .run(tauri::generate_context!())
        .expect("error while running Player Club Private VPN");
}
