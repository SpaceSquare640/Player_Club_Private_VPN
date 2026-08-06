//! Peer-connection commands (C4) — drive the hole-punch handshake + data link.

use std::sync::Arc;

use tauri::{AppHandle, State};

use crate::engine::connection::{ConnectionManager, ConnectionSettings};
use crate::engine::crypto::Identity;
use crate::engine::telemetry::TelemetrySink;

use super::events::TauriSink;

/// Connect to the negotiated peer. Spawns the C4 pipeline (fan-out handshake →
/// encrypted keepalive); progress is reported via `engine://state` /
/// `telemetry://stats` and the `get_connection` snapshot's `link` field.
///
/// `settings` (Phase B.3 — split-tunnel broadcast/multicast forwarding, FEC
/// redundancy) is applied once, at this call. To change the live-toggleable
/// subset on an already-running link, see [`update_connection_settings`].
#[tauri::command]
pub fn connect_peer(
    app: AppHandle,
    identity: State<'_, Arc<Identity>>,
    manager: State<'_, Arc<ConnectionManager>>,
    settings: ConnectionSettings,
) -> Result<(), String> {
    let sink: Box<dyn TelemetrySink> = Box::new(TauriSink::new(app));
    manager.connect(identity.inner().clone(), sink, settings)
}

/// Disconnect the live peer link (idempotent).
#[tauri::command]
pub fn disconnect_peer(manager: State<'_, Arc<ConnectionManager>>) -> Result<(), String> {
    manager.disconnect();
    Ok(())
}

/// Push a settings change into every already-live link (Phase B.4).
///
/// Only the split-tunnel broadcast/multicast toggles take effect
/// mid-session — they are pure local packet filtering, invisible to the
/// peer. FEC redundancy and extra routed networks still apply only at the
/// next Connect: the former is a wire-format agreement with the peer, the
/// latter mutates the OS routing table and needs elevation. Passing them
/// here is harmless — they are simply not applied live — so the frontend can
/// keep sending the whole `ConnectionSettings` object rather than a
/// hand-curated subset that would drift out of sync with it.
///
/// A no-op when nothing is connected.
#[tauri::command]
pub fn update_connection_settings(
    manager: State<'_, Arc<ConnectionManager>>,
    settings: ConnectionSettings,
) -> Result<(), String> {
    manager.update_settings(settings);
    Ok(())
}
