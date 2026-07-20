//! Telemetry stats DTO pushed to the UI on `telemetry://stats`.

use serde::Serialize;

use crate::engine::state::EngineState;

/// A single telemetry sample. Field names are camelCase on the wire to match
/// the TypeScript consumer.
///
/// `Default` is derived so producers can fill only the fields they actually
/// measure (`..Default::default()`); otherwise every new field forces an edit at
/// each of the construction sites across the engine.
#[derive(Debug, Clone, Default, Serialize)]
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
    /// Packets rebuilt by FEC since the link came up — **cumulative**, not a
    /// rate. "This connection has recovered N packets" is the useful framing;
    /// a per-second figure would round to zero on a healthy link.
    pub fec_recovered: u32,
    /// Packets the split-tunnel policy refused, in either direction, since the
    /// link came up. Also cumulative.
    pub policy_blocked: u32,
}

impl TelemetrySnapshot {
    /// The zeroed snapshot shown while the engine is Idle.
    pub fn idle() -> Self {
        Self::default()
    }
}
