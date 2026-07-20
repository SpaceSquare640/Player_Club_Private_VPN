//! Telemetry stats DTO pushed to the UI on `telemetry://stats`.

use serde::Serialize;

use crate::engine::state::EngineState;

/// A single telemetry sample. Field names are camelCase on the wire to match
/// the TypeScript consumer.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TelemetrySnapshot {
    pub state: EngineState,
    /// Round-trip time in milliseconds (the "ping").
    pub rtt_ms: f32,
    pub jitter_ms: f32,
    pub loss_pct: f32,
    pub tx_kbps: f32,
    pub rx_kbps: f32,
    pub peers: u32,
}

impl TelemetrySnapshot {
    /// The zeroed snapshot shown while the engine is Idle.
    pub fn idle() -> Self {
        Self {
            state: EngineState::Idle,
            rtt_ms: 0.0,
            jitter_ms: 0.0,
            loss_pct: 0.0,
            tx_kbps: 0.0,
            rx_kbps: 0.0,
            peers: 0,
        }
    }
}
