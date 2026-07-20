//! Sliding-window anti-replay filter (64-packet window), used by
//! [`super::session::CryptoSession`] for `Data` frames over reordering UDP.
#![allow(dead_code)] // wired into the data plane in C5; exercised by tests now.

const WINDOW: u64 = 64;

/// Tracks the highest accepted counter and a bitmap of recently seen counters.
pub struct ReplayWindow {
    last: u64,
    bitmap: u64,
    seen_any: bool,
}

impl ReplayWindow {
    pub fn new() -> Self {
        Self {
            last: 0,
            bitmap: 0,
            seen_any: false,
        }
    }

    /// Read-only acceptability check: `true` unless `counter` is too old or has
    /// already been seen. Call before authentication; commit with [`accept`].
    ///
    /// [`accept`]: ReplayWindow::accept
    pub fn check(&self, counter: u64) -> bool {
        if !self.seen_any || counter > self.last {
            return true;
        }
        let diff = self.last - counter;
        if diff >= WINDOW {
            return false;
        }
        (self.bitmap >> diff) & 1 == 0
    }

    /// Commit `counter` as seen. Call only after the packet authenticates, so a
    /// forged packet with a valid counter can't poison the window.
    pub fn accept(&mut self, counter: u64) {
        if !self.seen_any {
            self.seen_any = true;
            self.last = counter;
            self.bitmap = 1;
            return;
        }
        if counter > self.last {
            let shift = counter - self.last;
            self.bitmap = if shift >= WINDOW { 0 } else { self.bitmap << shift };
            self.bitmap |= 1;
            self.last = counter;
        } else {
            let diff = self.last - counter;
            if diff < WINDOW {
                self.bitmap |= 1 << diff;
            }
        }
    }
}

impl Default for ReplayWindow {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_increasing_rejects_duplicates() {
        let mut w = ReplayWindow::new();
        for c in 0..10u64 {
            assert!(w.check(c), "should accept {c}");
            w.accept(c);
            assert!(!w.check(c), "duplicate {c} should be rejected");
        }
    }

    #[test]
    fn accepts_in_window_reorder() {
        let mut w = ReplayWindow::new();
        w.accept(10);
        // Older but within window, not yet seen → accepted.
        assert!(w.check(5));
        w.accept(5);
        assert!(!w.check(5)); // now a duplicate
    }

    #[test]
    fn rejects_too_old() {
        let mut w = ReplayWindow::new();
        w.accept(100);
        assert!(!w.check(100 - WINDOW)); // exactly at/below the window edge
        assert!(!w.check(10));
    }
}
