//! IPC command handlers and the event bridge.

pub mod connection_cmds;
pub mod engine_cmds;
pub mod events;
pub mod identity_cmds;
pub mod mesh_cmds;
pub mod privilege_cmds;
pub mod relay_cmds;
pub mod signaling_cmds;
pub mod telemetry_cmds;

pub use connection_cmds::{connect_peer, disconnect_peer, update_connection_settings};
pub use engine_cmds::{get_status, start_engine, stop_engine};
pub use identity_cmds::get_identity;
pub use mesh_cmds::{create_network, get_network_statuses, join_network, leave_network};
pub use privilege_cmds::{get_privilege_status, request_elevation};
pub use relay_cmds::{get_relay_status, start_relay, stop_relay};
pub use signaling_cmds::{accept_answer, accept_offer, create_offer, get_connection};
pub use telemetry_cmds::{get_packet_log, get_snapshot};
