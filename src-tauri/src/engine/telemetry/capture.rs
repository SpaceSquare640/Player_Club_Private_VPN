//! Real virtual-adapter capture loop (Phase B).
//!
//! Reads frames off the Wintun device, classifies them, and feeds the same
//! telemetry pipeline as the simulator. Captured frames are dropped — there is
//! no transport yet (that arrives in Phase C), so throughput and the packet log
//! are live while latency metrics stay zero until then.

use std::time::{Duration, Instant};

use tokio::sync::watch;

use crate::engine::config::EngineConfig;
use crate::engine::notice::EngineNotice;
use crate::engine::state::{EngineState, SharedState};
use crate::engine::tun::{self, packet};

use super::metrics::TelemetrySnapshot;
use super::packet_log::{Direction, PacketLogEntry};
use super::sink::TelemetrySink;

/// Entry point: hosts the blocking capture loop on a dedicated blocking thread
/// so the Wintun reads never stall the async runtime.
pub async fn run(
    cfg: EngineConfig,
    sink: Box<dyn TelemetrySink>,
    shared: SharedState,
    shutdown: watch::Receiver<bool>,
) {
    let _ = tauri::async_runtime::spawn_blocking(move || {
        capture_blocking(cfg, sink, shared, shutdown);
    })
    .await;
}

fn capture_blocking(
    cfg: EngineConfig,
    sink: Box<dyn TelemetrySink>,
    shared: SharedState,
    shutdown: watch::Receiver<bool>,
) {
    shared.set_state(EngineState::Starting);
    sink.state(EngineState::Starting);

    let mut dev = match tun::open_device(&cfg.tun) {
        Ok(dev) => dev,
        Err(e) => {
            shared.set_state(EngineState::Error);
            sink.state(EngineState::Error);
            sink.notice(&EngineNotice::error(format!("Failed to open adapter: {e}")));
            return;
        }
    };

    shared.set_state(EngineState::Connected);
    sink.state(EngineState::Connected);

    let mut buf = vec![0u8; cfg.tun.mtu as usize + 128];
    let interval = cfg.tick_interval();
    let mut last_emit = Instant::now();
    let mut bytes_since = 0u64;
    let mut pending: Vec<PacketLogEntry> = Vec::new();

    loop {
        if *shutdown.borrow() {
            break;
        }

        match dev.read_frame(&mut buf) {
            Ok(Some(n)) => {
                bytes_since += n as u64;
                if let Some(meta) = packet::classify(&buf[..n]) {
                    pending.push(PacketLogEntry {
                        t_ms: shared.elapsed_ms(),
                        dir: Direction::Tx, // egress: host → tunnel
                        proto: meta.proto.to_string(),
                        len: meta.len,
                        note: "captured (dropped — no transport yet)".to_string(),
                    });
                }
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(1)),
            Err(_) => std::thread::sleep(Duration::from_millis(5)),
        }

        if last_emit.elapsed() >= interval {
            let secs = last_emit.elapsed().as_secs_f32().max(0.001);
            let tx_kbps = (bytes_since as f32 * 8.0 / 1000.0) / secs;
            let snapshot = TelemetrySnapshot {
                state: EngineState::Connected,
                rtt_ms: 0.0,
                jitter_ms: 0.0,
                loss_pct: 0.0,
                tx_kbps: round2(tx_kbps),
                rx_kbps: 0.0,
                peers: 0,
            };
            shared.set_snapshot(snapshot.clone());
            if !pending.is_empty() {
                shared.push_packets(&pending);
                sink.packets(&pending);
                pending.clear();
            }
            sink.stats(&snapshot);
            bytes_since = 0;
            last_emit = Instant::now();
        }
    }

    drop(dev);
    shared.set_state(EngineState::Idle);
    sink.state(EngineState::Idle);
}

fn round2(v: f32) -> f32 {
    (v * 100.0).round() / 100.0
}
