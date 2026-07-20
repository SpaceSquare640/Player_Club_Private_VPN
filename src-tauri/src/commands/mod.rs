//! IPC command handlers and the event bridge.

pub mod connection_cmds;
pub mod engine_cmds;
pub mod events;
pub mod identity_cmds;
pub mod privilege_cmds;
pub mod signaling_cmds;
pub mod telemetry_cmds;

pub use connection_cmds::{connect_peer, disconnect_peer};
pub use engine_cmds::{get_status, start_engine, stop_engine};
pub use identity_cmds::get_identity;
pub use privilege_cmds::{get_privilege_status, request_elevation};
pub use signaling_cmds::{accept_answer, accept_offer, create_offer, get_connection};
pub use telemetry_cmds::{get_packet_log, get_snapshot};
