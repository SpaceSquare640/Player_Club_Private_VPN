//! Player Club networking engine.
//!
//! Phase A implements the **telemetry spine**: lifecycle (start/stop), a
//! simulated data path that produces authentic-feeling metrics and packet
//! logs, and a [`telemetry::TelemetrySink`] seam that lets the IPC layer
//! forward telemetry to the UI without the engine depending on Tauri.
//!
//! Future phases add the real data plane under sibling modules: `tun/`,
//! `nat/`, `transport/`, `fec/`, and `split_tunnel/`.

pub mod config;
pub mod connection;
pub mod controller;
pub mod crypto;
pub mod dataplane;
pub mod error;
pub mod fec;
pub mod helper;
pub mod mesh;
pub mod nat;
pub mod notice;
pub mod pipeline;
pub mod signaling;
pub mod split_tunnel;
pub mod state;
pub mod telemetry;
pub mod transport;
pub mod tun;

pub use config::EngineConfig;
pub use controller::EngineController;
pub use error::Result;
pub use state::{EngineState, EngineStatus};
