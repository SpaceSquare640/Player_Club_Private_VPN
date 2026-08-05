//! Virtual-network commands (Phase G.4) — create/join a Hamachi-style
//! signaling network and drive automatic mesh connection (G.1–G.3c).

use std::net::SocketAddr;
use std::sync::Arc;

use tauri::{AppHandle, State};

use crate::engine::connection::{ConnectionManager, ConnectionSettings};
use crate::engine::crypto::Identity;
use crate::engine::mesh::{MeshSession, NetworkStatus};
use crate::engine::telemetry::TelemetrySink;

use super::events::TauriSink;

/// Start hosting a new virtual network. `bind_addr` is `ip:port`; port `0`
/// picks an ephemeral port. Returns the actual bound address (`ip:port`) so
/// the UI can show it to others to join.
///
/// `game_tag` is display metadata only (e.g. `"minecraft"`) — never
/// inspected by connection logic. `settings` (split-tunnel forwarding + FEC
/// redundancy) applies to every auto-connected peer in this network, same as
/// the manual-signaling `connect_peer`'s `settings` argument.
#[tauri::command]
pub async fn create_network(
    bind_addr: String,
    network_name: String,
    password: String,
    game_tag: Option<String>,
    settings: ConnectionSettings,
    app: AppHandle,
    identity: State<'_, Arc<Identity>>,
    manager: State<'_, Arc<ConnectionManager>>,
    session: State<'_, MeshSession>,
) -> Result<String, String> {
    let bind_addr: SocketAddr = bind_addr.parse().map_err(|e| format!("invalid bind address: {e}"))?;
    let identity = identity.inner().clone();
    let manager = manager.inner().clone();
    let sink_factory = move || -> Box<dyn TelemetrySink> { Box::new(TauriSink::new(app.clone())) };
    let addr =
        session.create(bind_addr, network_name, password, game_tag, settings, identity, manager, sink_factory).await?;
    Ok(addr.to_string())
}

/// Join an existing virtual network hosted at `host_addr` (`ip:port`).
#[tauri::command]
pub async fn join_network(
    host_addr: String,
    network_name: String,
    password: String,
    game_tag: Option<String>,
    settings: ConnectionSettings,
    app: AppHandle,
    identity: State<'_, Arc<Identity>>,
    manager: State<'_, Arc<ConnectionManager>>,
    session: State<'_, MeshSession>,
) -> Result<(), String> {
    let host_addr: SocketAddr = host_addr.parse().map_err(|e| format!("invalid host address: {e}"))?;
    let identity = identity.inner().clone();
    let manager = manager.inner().clone();
    let sink_factory = move || -> Box<dyn TelemetrySink> { Box::new(TauriSink::new(app.clone())) };
    session.join(host_addr, network_name, password, game_tag, settings, identity, manager, sink_factory).await
}

/// Leave the current virtual network (idempotent).
#[tauri::command]
pub async fn leave_network(session: State<'_, MeshSession>) -> Result<(), String> {
    session.leave().await;
    Ok(())
}

/// Current virtual-network status: `None` if not in one.
#[tauri::command]
pub fn get_network_status(
    manager: State<'_, Arc<ConnectionManager>>,
    session: State<'_, MeshSession>,
) -> Option<NetworkStatus> {
    session.status(&manager)
}
