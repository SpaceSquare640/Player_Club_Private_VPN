//! Round-trip-time tracking via Ping/Pong sequence correlation.

use std::collections::HashMap;
use std::time::{Duration, Instant};

/// Unacked pings older than this are considered lost.
const LOSS_TIMEOUT: Duration = Duration::from_secs(3);

/// Tracks RTT, jitter, and loss from Ping/Pong exchanges.
pub struct RttTracker {
    next_seq: u64,
    sent: HashMap<u64, Instant>,
    rtt_ewma: Option<f32>,
    jitter_ewma: f32,
    last_rtt: Option<f32>,
    ackd: u64,
    lost: u64,
}

impl RttTracker {
    pub fn new() -> Self {
        Self {
            next_seq: 0,
            sent: HashMap::new(),
            rtt_ewma: None,
            jitter_ewma: 0.0,
            last_rtt: None,
            ackd: 0,
            lost: 0,
        }
    }

    /// Allocate the next ping sequence and record its send time.
    pub fn next_ping(&mut self) -> u64 {
        self.prune();
        let seq = self.next_seq;
        self.next_seq += 1;
        self.sent.insert(seq, Instant::now());
        seq
    }

    /// Record a received pong, updating RTT/jitter.
    pub fn on_pong(&mut self, seq: u64) {
        if let Some(sent_at) = self.sent.remove(&seq) {
            let sample = sent_at.elapsed().as_secs_f32() * 1000.0;
            self.ackd += 1;
            self.rtt_ewma = Some(match self.rtt_ewma {
                Some(prev) => prev * 0.8 + sample * 0.2,
                None => sample,
            });
            if let Some(last) = self.last_rtt {
                self.jitter_ewma = self.jitter_ewma * 0.8 + (sample - last).abs() * 0.2;
            }
            self.last_rtt = Some(sample);
        }
    }

    /// `(rtt_ms, jitter_ms, loss_pct)` rounded to 2 decimals.
    pub fn stats(&self) -> (f32, f32, f32) {
        let rtt = self.rtt_ewma.unwrap_or(0.0);
        let total = self.ackd + self.lost;
        let loss = if total == 0 {
            0.0
        } else {
            (self.lost as f32 / total as f32) * 100.0
        };
        (round2(rtt), round2(self.jitter_ewma), round2(loss))
    }

    /// Drop (and count as lost) pings that timed out.
    fn prune(&mut self) {
        let now = Instant::now();
        let before = self.sent.len();
        self.sent
            .retain(|_, sent_at| now.duration_since(*sent_at) < LOSS_TIMEOUT);
        self.lost += (before - self.sent.len()) as u64;
    }
}

impl Default for RttTracker {
    fn default() -> Self {
        Self::new()
    }
}

fn round2(v: f32) -> f32 {
    (v * 100.0).round() / 100.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rtt_updates_and_no_loss_when_all_ackd() {
        let mut t = RttTracker::new();
        for _ in 0..5 {
            let seq = t.next_ping();
            t.on_pong(seq);
        }
        let (rtt, _jitter, loss) = t.stats();
        assert!(rtt >= 0.0);
        assert_eq!(loss, 0.0);
    }

    #[test]
    fn unmatched_pong_is_ignored() {
        let mut t = RttTracker::new();
        t.on_pong(999); // never sent
        let (_rtt, _jitter, loss) = t.stats();
        assert_eq!(loss, 0.0);
    }
}
