//! Phase-A telemetry generator.
//!
//! Produces authentic-feeling metrics via a bounded, mean-reverting random walk
//! around per-profile baselines. The PRNG is a `StdRng`; when the config
//! carries a `seed`, the entire run is reproducible for UI stress testing.

use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

use crate::engine::config::{EngineConfig, SimProfile};
use crate::engine::state::EngineState;

use super::metrics::TelemetrySnapshot;
use super::packet_log::{Direction, PacketLogEntry};

/// Per-profile baseline values the random walk reverts toward.
struct Baseline {
    rtt: f32,
    jitter: f32,
    loss: f32,
    throughput: f32,
}

impl Baseline {
    fn for_profile(profile: SimProfile) -> Self {
        match profile {
            SimProfile::Stable => Baseline {
                rtt: 18.0,
                jitter: 1.5,
                loss: 0.1,
                throughput: 1800.0,
            },
            SimProfile::Congested => Baseline {
                rtt: 46.0,
                jitter: 8.0,
                loss: 1.2,
                throughput: 1200.0,
            },
            SimProfile::Lossy => Baseline {
                rtt: 72.0,
                jitter: 14.0,
                loss: 4.5,
                throughput: 900.0,
            },
        }
    }
}

/// Stateful simulator: call [`Simulator::tick`] once per emission interval.
pub struct Simulator {
    rng: StdRng,
    base: Baseline,
    peers: u32,
    rtt: f32,
    throughput: f32,
    seq: u64,
}

impl Simulator {
    pub fn new(cfg: &EngineConfig) -> Self {
        let rng = match cfg.seed {
            Some(seed) => StdRng::seed_from_u64(seed),
            None => StdRng::from_entropy(),
        };
        let base = Baseline::for_profile(cfg.sim_profile);
        let rtt = base.rtt;
        let throughput = base.throughput;
        Self {
            rng,
            base,
            peers: 1,
            rtt,
            throughput,
            seq: 0,
        }
    }

    /// Advance one tick. Returns the stats snapshot plus a small batch of
    /// packet-log entries timestamped at `t_ms` since the session start.
    pub fn tick(&mut self, t_ms: u64) -> (TelemetrySnapshot, Vec<PacketLogEntry>) {
        // RTT: mean-revert toward the baseline, plus jitter noise.
        let drift = (self.base.rtt - self.rtt) * 0.15;
        let noise = self.rng.gen_range(-self.base.jitter..=self.base.jitter);
        self.rtt = (self.rtt + drift + noise).clamp(1.0, 400.0);
        let jitter = (noise.abs() + self.base.jitter * 0.5).clamp(0.2, 60.0);

        // Loss: baseline wander, with an occasional spike.
        let spike = if self.rng.gen_bool(0.08) {
            self.rng.gen_range(0.0..=self.base.loss * 3.0)
        } else {
            0.0
        };
        let loss = (self.base.loss
            + self.rng.gen_range(-self.base.loss..=self.base.loss)
            + spike)
            .clamp(0.0, 100.0);

        // Throughput: mean-revert with wider noise; rx trails tx a little.
        let t_drift = (self.base.throughput - self.throughput) * 0.2;
        let t_noise = self.rng.gen_range(-150.0..=150.0);
        self.throughput = (self.throughput + t_drift + t_noise).clamp(50.0, 5000.0);
        let tx = self.throughput;
        let rx = (self.throughput * self.rng.gen_range(0.6..=0.95)).max(20.0);

        let snapshot = TelemetrySnapshot {
            state: EngineState::Connected,
            rtt_ms: round2(self.rtt),
            jitter_ms: round2(jitter),
            loss_pct: round2(loss),
            tx_kbps: round2(tx),
            rx_kbps: round2(rx),
            peers: self.peers,
        };

        let count = self.rng.gen_range(1..=3);
        let mut packets = Vec::with_capacity(count);
        for _ in 0..count {
            self.seq += 1;
            packets.push(self.fake_packet(t_ms));
        }

        (snapshot, packets)
    }

    fn fake_packet(&mut self, t_ms: u64) -> PacketLogEntry {
        const PROTOS: [&str; 4] = ["UDP", "UDP", "ICMP", "TCP"];
        let dir = if self.rng.gen_bool(0.5) {
            Direction::Tx
        } else {
            Direction::Rx
        };
        let proto = PROTOS[self.rng.gen_range(0..PROTOS.len())].to_string();
        let len = self.rng.gen_range(40..=1400) as u16;
        let note = format!("seq={} flow=game-session", self.seq);
        PacketLogEntry {
            t_ms,
            dir,
            proto,
            len,
            note,
        }
    }
}

fn round2(v: f32) -> f32 {
    (v * 100.0).round() / 100.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::config::EngineConfig;

    fn seeded(seed: u64) -> EngineConfig {
        EngineConfig {
            seed: Some(seed),
            sim_profile: SimProfile::Stable,
            ..Default::default()
        }
    }

    #[test]
    fn seeded_runs_are_reproducible() {
        let mut a = Simulator::new(&seeded(42));
        let mut b = Simulator::new(&seeded(42));
        for t in 0..50u64 {
            let (sa, pa) = a.tick(t);
            let (sb, pb) = b.tick(t);
            assert_eq!(sa.rtt_ms, sb.rtt_ms);
            assert_eq!(sa.loss_pct, sb.loss_pct);
            assert_eq!(sa.tx_kbps, sb.tx_kbps);
            assert_eq!(pa.len(), pb.len());
        }
    }

    #[test]
    fn values_stay_in_range() {
        let mut s = Simulator::new(&seeded(7));
        for t in 0..200u64 {
            let (snap, packets) = s.tick(t);
            assert!((1.0..=400.0).contains(&snap.rtt_ms));
            assert!((0.0..=100.0).contains(&snap.loss_pct));
            assert!((50.0..=5000.0).contains(&snap.tx_kbps));
            assert!(!packets.is_empty() && packets.len() <= 3);
            for p in packets {
                assert!((40..=1400).contains(&p.len));
            }
        }
    }
}
