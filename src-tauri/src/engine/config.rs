//! Configuration supplied by the UI when starting the engine.

use std::time::Duration;

use serde::{Deserialize, Serialize};

use super::tun::TunConfig;

/// Telemetry "shape" presets so the UI can stress-test different link
/// conditions without any real networking.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SimProfile {
    /// Low, steady latency — a healthy LAN-grade link.
    #[default]
    Stable,
    /// Elevated jitter and occasional loss — a congested link.
    Congested,
    /// High loss spikes — a stress profile for the loss/FEC UI.
    Lossy,
}

/// Parameters for an engine session. All fields have defaults so the UI can
/// call `start_engine` with a partial (or empty) object.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EngineConfig {
    /// Stats emission rate in Hz (clamped to [1, 20] at use).
    #[serde(default = "default_tick_hz")]
    pub tick_hz: u8,
    /// Telemetry profile.
    #[serde(default)]
    pub sim_profile: SimProfile,
    /// Optional PRNG seed. `Some(_)` → reproducible run (UI stress testing);
    /// `None` → seed from OS entropy for organic variation.
    #[serde(default)]
    pub seed: Option<u64>,
    /// Use the real Wintun adapter (Expert mode) instead of the simulator.
    /// Requires elevation; gated by the controller.
    #[serde(default)]
    pub use_real_tun: bool,
    /// Virtual adapter parameters (used only when `use_real_tun`).
    #[serde(default)]
    pub tun: TunConfig,
    /// Run the Phase-C1 transport probe (STUN + Ping/Pong RTT) instead of the
    /// simulator. Ignored when `use_real_tun` is set.
    #[serde(default)]
    pub transport_probe: bool,
    /// STUN server for reflexive-address discovery.
    #[serde(default = "default_stun_server")]
    pub stun_server: String,
    /// Local UDP port to bind (0 = ephemeral).
    #[serde(default)]
    pub bind_port: u16,
    /// Optional `host:port` ping target; `None` → loopback self-test.
    #[serde(default)]
    pub probe_target: Option<String>,
}

fn default_stun_server() -> String {
    "stun.l.google.com:19302".to_string()
}

fn default_tick_hz() -> u8 {
    4
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            tick_hz: default_tick_hz(),
            sim_profile: SimProfile::default(),
            seed: None,
            use_real_tun: false,
            tun: TunConfig::default(),
            transport_probe: false,
            stun_server: default_stun_server(),
            bind_port: 0,
            probe_target: None,
        }
    }
}

impl EngineConfig {
    /// Per-tick interval derived from `tick_hz`, clamped to a sane range.
    pub fn tick_interval(&self) -> Duration {
        let hz = self.tick_hz.clamp(1, 20) as u64;
        Duration::from_millis(1000 / hz)
    }
}
