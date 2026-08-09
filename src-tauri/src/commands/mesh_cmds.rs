//! Virtual-network commands (Phase G.4) — create/join a Hamachi-style
//! signaling network and drive automatic mesh connection (G.1–G.3c).
//!
//! A `MeshSession` supports any number of simultaneously active networks
//! (hosted and/or joined, in any combination) — every command here is keyed
//! by the `NetworkId` returned from `create_network`/`join_network`.

use std::net::SocketAddr;
use std::sync::Arc;

use tauri::{AppHandle, State};

use crate::engine::connection::{ConnectionManager, ConnectionSettings};
use crate::engine::crypto::Identity;
use crate::engine::mesh::{MeshSession, NetworkStatus};
use crate::engine::telemetry::TelemetrySink;

use super::events::TauriSink;

/// Start hosting a new virtual network. `bind_addr` is `ip:port`; port `0`
/// picks an ephemeral port. Returns the new network's id — its bound
/// address is available from `get_network_statuses`'s `hostAddr` field.
///
/// Can be called while already hosting or having joined other networks; each
/// call adds one more simultaneously active network rather than replacing
/// any existing one.
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
    let (id, _host_addr) =
        session.create(bind_addr, network_name, password, game_tag, settings, identity, manager, sink_factory).await?;
    Ok(id)
}

/// Join an existing virtual network hosted at `host_addr` (`ip:port`).
/// Returns the new network's id. Can be called while already a member of
/// other networks — see `create_network`.
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
) -> Result<String, String> {
    let host_addr: SocketAddr = host_addr.parse().map_err(|e| format!("invalid host address: {e}"))?;
    let identity = identity.inner().clone();
    let manager = manager.inner().clone();
    let sink_factory = move || -> Box<dyn TelemetrySink> { Box::new(TauriSink::new(app.clone())) };
    session.join(host_addr, network_name, password, game_tag, settings, identity, manager, sink_factory).await
}

/// Leave the virtual network identified by `network_id` (idempotent — a
/// no-op if not a member of it). Other active networks are unaffected.
#[tauri::command]
pub async fn leave_network(network_id: String, session: State<'_, MeshSession>) -> Result<(), String> {
    session.leave(&network_id).await;
    Ok(())
}

/// The status of every currently active virtual network (empty if none).
#[tauri::command]
pub fn get_network_statuses(
    manager: State<'_, Arc<ConnectionManager>>,
    session: State<'_, MeshSession>,
) -> Vec<NetworkStatus> {
    session.statuses(&manager)
}
