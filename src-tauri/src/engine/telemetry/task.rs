//! The Phase-A telemetry loop: a brief simulated handshake, then a steady
//! stream of stats + packet batches until asked to stop.

use tokio::sync::watch;
use tokio::time::{self, Duration};

use crate::engine::config::EngineConfig;
use crate::engine::state::{EngineState, SharedState};

use super::simulator::Simulator;
use super::sink::TelemetrySink;

/// Drive the telemetry loop until `shutdown` flips to `true` (or its sender is
/// dropped). Owns the simulator and writes to both the shared state (for pull
/// commands) and the sink (for push events).
pub async fn run(
    cfg: EngineConfig,
    sink: Box<dyn TelemetrySink>,
    shared: SharedState,
    mut shutdown: watch::Receiver<bool>,
) {
    // Simulated NAT/handshake delay — bail out early if stopped during it.
    shared.set_state(EngineState::Connecting);
    sink.state(EngineState::Connecting);
    tokio::select! {
        _ = time::sleep(Duration::from_millis(900)) => {}
        res = shutdown.changed() => {
            if res.is_err() || *shutdown.borrow() {
                shared.set_state(EngineState::Idle);
                sink.state(EngineState::Idle);
                return;
            }
        }
    }

    shared.set_state(EngineState::Connected);
    sink.state(EngineState::Connected);

    let mut sim = Simulator::new(&cfg);
    let mut ticker = time::interval(cfg.tick_interval());

    loop {
        tokio::select! {
            _ = ticker.tick() => {
                let t_ms = shared.elapsed_ms();
                let (snapshot, packets) = sim.tick(t_ms);
                shared.set_snapshot(snapshot.clone());
                shared.push_packets(&packets);
                sink.stats(&snapshot);
                if !packets.is_empty() {
                    sink.packets(&packets);
                }
            }
            res = shutdown.changed() => {
                if res.is_err() || *shutdown.borrow() {
                    break;
                }
            }
        }
    }

    shared.set_state(EngineState::Idle);
    sink.state(EngineState::Idle);
}
