//! Phase-C1 transport probe.
//!
//! A temporary affordance (superseded by the real `PeerSession` in C4/C5): it
//! binds the shared socket, discovers candidates via STUN, then runs Ping/Pong
//! against a target — loopback by default, or `probe_target` for true network
//! RTT against a second instance — feeding live latency into the telemetry spine.

use std::net::{Ipv4Addr, SocketAddr};

use tokio::sync::watch;
use tokio::time;

use crate::engine::config::EngineConfig;
use crate::engine::nat::candidate;
use crate::engine::notice::EngineNotice;
use crate::engine::state::{EngineState, SharedState};
use crate::engine::telemetry::metrics::TelemetrySnapshot;
use crate::engine::telemetry::packet_log::{Direction, PacketLogEntry};
use crate::engine::telemetry::sink::TelemetrySink;

use super::frame::{self, FrameKind, Inbound};
use super::keepalive::RttTracker;
use super::socket::UdpTransport;

pub async fn run(
    cfg: EngineConfig,
    sink: Box<dyn TelemetrySink>,
    shared: SharedState,
    mut shutdown: watch::Receiver<bool>,
) {
    shared.set_state(EngineState::Starting);
    sink.state(EngineState::Starting);

    let transport = match UdpTransport::bind(cfg.bind_port).await {
        Ok(t) => t,
        Err(e) => {
            shared.set_state(EngineState::Error);
            sink.state(EngineState::Error);
            sink.notice(&EngineNotice::error(format!("UDP bind failed: {e}")));
            return;
        }
    };
    let local_port = transport.local_addr().map(|a| a.port()).unwrap_or(0);

    // Candidate gathering (host + reflexive via STUN). Best-effort.
    let candidates = candidate::gather(&transport, &cfg.stun_server).await;
    let summary = if candidates.is_empty() {
        "none (STUN unavailable)".to_string()
    } else {
        candidates
            .iter()
            .map(|c| format!("{}={}", c.kind.as_str(), c.addr))
            .collect::<Vec<_>>()
            .join("  ")
    };
    sink.notice(&EngineNotice::info("transport", format!("Candidates: {summary}")));

    // Ping target: explicit `probe_target`, else loopback to our own port.
    let target = match cfg.probe_target.as_deref() {
        Some(s) => match s.parse::<SocketAddr>() {
            Ok(addr) => addr,
            Err(_) => {
                shared.set_state(EngineState::Error);
                sink.state(EngineState::Error);
                sink.notice(&EngineNotice::error(format!("invalid probe target: {s}")));
                return;
            }
        },
        None => SocketAddr::from((Ipv4Addr::LOCALHOST, local_port)),
    };

    shared.set_state(EngineState::Connected);
    sink.state(EngineState::Connected);

    let mut tracker = RttTracker::new();
    let mut ticker = time::interval(cfg.tick_interval());
    let mut buf = vec![0u8; 2048];
    let mut pending: Vec<PacketLogEntry> = Vec::new();

    loop {
        tokio::select! {
            _ = ticker.tick() => {
                let seq = tracker.next_ping();
                let ping = frame::encode_ping(seq);
                let _ = transport.send_to(&ping, target).await;
                pending.push(entry(&shared, Direction::Tx, "PING", ping.len(), format!("seq={seq} → {target}")));

                let (rtt, jitter, loss) = tracker.stats();
                let snapshot = TelemetrySnapshot {
                    state: EngineState::Connected,
                    rtt_ms: rtt,
                    jitter_ms: jitter,
                    loss_pct: loss,
                    tx_kbps: 0.0,
                    rx_kbps: 0.0,
                    peers: 1,
                };
                shared.set_snapshot(snapshot.clone());
                if !pending.is_empty() {
                    shared.push_packets(&pending);
                    sink.packets(&pending);
                    pending.clear();
                }
                sink.stats(&snapshot);
            }
            result = transport.recv_from(&mut buf) => {
                if let Ok((n, from)) = result {
                    match frame::classify(&buf[..n]) {
                        Inbound::Frame(FrameKind::Ping, payload) => {
                            if let Some(seq) = frame::decode_seq(payload) {
                                let pong = frame::encode_pong(seq);
                                let _ = transport.send_to(&pong, from).await;
                                pending.push(entry(&shared, Direction::Rx, "PING", n, format!("seq={seq} ← {from}")));
                                pending.push(entry(&shared, Direction::Tx, "PONG", pong.len(), format!("seq={seq} → {from}")));
                            }
                        }
                        Inbound::Frame(FrameKind::Pong, payload) => {
                            if let Some(seq) = frame::decode_seq(payload) {
                                tracker.on_pong(seq);
                                pending.push(entry(&shared, Direction::Rx, "PONG", n, format!("seq={seq} ← {from}")));
                            }
                        }
                        _ => {}
                    }
                }
            }
            changed = shutdown.changed() => {
                if changed.is_err() || *shutdown.borrow() {
                    break;
                }
            }
        }
    }

    shared.set_state(EngineState::Idle);
    sink.state(EngineState::Idle);
}

fn entry(
    shared: &SharedState,
    dir: Direction,
    proto: &str,
    len: usize,
    note: String,
) -> PacketLogEntry {
    PacketLogEntry {
        t_ms: shared.elapsed_ms(),
        dir,
        proto: proto.to_string(),
        len: len as u16,
        note,
    }
}
