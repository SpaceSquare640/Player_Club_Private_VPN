//! Single-parity XOR erasure code (Phase D.1).
//!
//! A group of up to `k` data packets is protected by one parity packet equal to
//! the XOR of the members. Each member is length-prefixed and zero-padded to the
//! group's longest packet before XOR, so a single missing member is recovered
//! exactly (including its original length):
//!
//! ```text
//!   Bᵢ = [lenᵢ : u16 BE] ++ Pᵢ ++ zero-pad(to maxlen)
//!   Q  = B₁ ⊕ B₂ ⊕ … ⊕ B_k
//!   B_missing = Q ⊕ (⊕ of the received Bⱼ)   →  strip length prefix → P_missing
//! ```
//!
//! This recovers **one** loss per group (~`1/k` overhead). Two or more losses in
//! a group are unrecoverable — the decoder reports nothing rather than emitting
//! corrupt data. Reed-Solomon (Phase D.2) generalises this to multiple parity.

use std::collections::{HashMap, VecDeque};

/// A parity packet emitted when a group closes. `payload` is `Q` (length
/// `maxlen + 2`); `k` is how many data packets the group actually held.
pub struct Parity {
    pub group: u32,
    pub k: u8,
    pub payload: Vec<u8>,
}

/// Rolling XOR encoder (uplink, per-direction). Data packets are sent
/// immediately by the caller; this only accumulates and, when a group closes
/// (reaches `group_size` or is flushed), yields the parity to send additionally.
pub struct XorEncoder {
    group_size: u8,
    group_id: u32,
    members: Vec<Vec<u8>>,
}

impl XorEncoder {
    pub fn new(group_size: u8) -> Self {
        Self {
            group_size: group_size.max(1),
            group_id: 0,
            members: Vec::new(),
        }
    }

    /// Record `ip` in the current group and return `(group, index)` to tag its
    /// `FEC_DATA` frame. If this packet closes the group, also returns the
    /// [`Parity`] to send and rolls to the next group.
    pub fn push(&mut self, ip: &[u8]) -> (u32, u8, Option<Parity>) {
        let group = self.group_id;
        let index = self.members.len() as u8;
        self.members.push(ip.to_vec());
        let parity = if self.members.len() as u8 >= self.group_size {
            Some(self.close())
        } else {
            None
        };
        (group, index, parity)
    }

    /// Close a partial group (idle flush). Returns `None` if the group is empty.
    pub fn flush(&mut self) -> Option<Parity> {
        if self.members.is_empty() {
            None
        } else {
            Some(self.close())
        }
    }

    fn close(&mut self) -> Parity {
        let k = self.members.len() as u8;
        let maxlen = self.members.iter().map(|m| m.len()).max().unwrap_or(0);
        let mut payload = vec![0u8; maxlen + 2];
        for m in &self.members {
            xor_member(&mut payload, m);
        }
        let group = self.group_id;
        self.members.clear();
        self.group_id = self.group_id.wrapping_add(1);
        Parity { group, k, payload }
    }
}

/// XOR one length-prefixed member into `acc` (`acc` is `maxlen + 2` bytes).
fn xor_member(acc: &mut [u8], payload: &[u8]) {
    let len = payload.len() as u16;
    acc[0] ^= (len >> 8) as u8;
    acc[1] ^= (len & 0xff) as u8;
    for (i, &b) in payload.iter().enumerate() {
        if 2 + i < acc.len() {
            acc[2 + i] ^= b;
        }
    }
}

#[derive(Default)]
struct Group {
    members: HashMap<u8, Vec<u8>>,
    parity: Option<(u8, Vec<u8>)>, // (k, Q)
    done: bool,
}

/// XOR decoder (downlink, per-direction). Buffers each group until it can either
/// recover the single missing member or is evicted. Received data packets are
/// forwarded by the caller immediately; this only returns *recovered* packets.
pub struct XorDecoder {
    groups: HashMap<u32, Group>,
    order: VecDeque<u32>,
    cap: usize,
}

impl XorDecoder {
    pub fn new(cap: usize) -> Self {
        Self {
            groups: HashMap::new(),
            order: VecDeque::new(),
            cap: cap.max(1),
        }
    }

    /// Feed a received data packet; returns a recovered packet if this completes
    /// a recoverable group (a *different* member was missing).
    pub fn on_data(&mut self, group: u32, index: u8, ip: &[u8]) -> Option<Vec<u8>> {
        self.ensure(group);
        let g = self.groups.get_mut(&group)?;
        if g.done {
            return None;
        }
        g.members.insert(index, ip.to_vec());
        self.try_recover(group)
    }

    /// Feed a received parity packet; returns a recovered packet if the group is
    /// now missing exactly one member.
    pub fn on_parity(&mut self, group: u32, k: u8, q: &[u8]) -> Option<Vec<u8>> {
        self.ensure(group);
        let g = self.groups.get_mut(&group)?;
        if g.done {
            return None;
        }
        g.parity = Some((k, q.to_vec()));
        self.try_recover(group)
    }

    /// Insert the group if new, evicting the oldest once over capacity.
    fn ensure(&mut self, group: u32) {
        if self.groups.contains_key(&group) {
            return;
        }
        self.groups.insert(group, Group::default());
        self.order.push_back(group);
        while self.order.len() > self.cap {
            if let Some(old) = self.order.pop_front() {
                self.groups.remove(&old);
            }
        }
    }

    fn try_recover(&mut self, group: u32) -> Option<Vec<u8>> {
        let mut recovered = None;
        let mut done = false;
        if let Some(g) = self.groups.get(&group) {
            if g.done {
                return None;
            }
            if let Some((k, q)) = &g.parity {
                let k = *k as usize;
                if q.len() >= 2 && k >= 1 {
                    if g.members.len() >= k {
                        done = true; // all present — nothing to recover
                    } else if g.members.len() == k - 1 {
                        let maxlen = q.len() - 2;
                        let mut acc = q.clone();
                        for payload in g.members.values() {
                            xor_member(&mut acc, payload);
                        }
                        let rec_len = (((acc[0] as usize) << 8) | acc[1] as usize).min(maxlen);
                        recovered = Some(acc[2..2 + rec_len].to_vec());
                        done = true;
                    }
                }
            }
        }
        if done {
            self.mark_done(group);
        }
        recovered
    }

    fn mark_done(&mut self, group: u32) {
        if let Some(g) = self.groups.get_mut(&group) {
            g.done = true;
            g.members.clear();
            g.parity = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Encode `n` varying-length payloads through one full group; returns the
    /// `(group, index, payload)` data frames and the parity.
    fn encode_group(n: usize) -> (Vec<(u32, u8, Vec<u8>)>, Parity) {
        let mut enc = XorEncoder::new(n as u8);
        let mut frames = Vec::new();
        let mut parity = None;
        for i in 0..n {
            let p = vec![i as u8 + 1; 8 + i * 5]; // distinct lengths & contents
            let (g, idx, par) = enc.push(&p);
            frames.push((g, idx, p));
            if let Some(x) = par {
                parity = Some(x);
            }
        }
        (frames, parity.expect("full group yields parity"))
    }

    #[test]
    fn recovers_a_single_middle_loss() {
        let (frames, parity) = encode_group(8);
        let lost = 3u8;
        let mut dec = XorDecoder::new(64);
        let mut recovered = None;
        for (g, idx, p) in &frames {
            if *idx == lost {
                continue;
            }
            if let Some(r) = dec.on_data(*g, *idx, p) {
                recovered = Some(r);
            }
        }
        if let Some(r) = dec.on_parity(parity.group, parity.k, &parity.payload) {
            recovered = Some(r);
        }
        assert_eq!(recovered.unwrap(), frames[lost as usize].2);
    }

    #[test]
    fn recovers_when_parity_arrives_first_and_reordered() {
        let (frames, parity) = encode_group(6);
        let lost = 5u8; // the last one
        let mut dec = XorDecoder::new(64);
        // Parity first, then data out of order.
        let mut recovered = dec.on_parity(parity.group, parity.k, &parity.payload);
        for (g, idx, p) in frames.iter().rev() {
            if *idx == lost {
                continue;
            }
            if let Some(r) = dec.on_data(*g, *idx, p) {
                recovered = Some(r);
            }
        }
        assert_eq!(recovered.unwrap(), frames[lost as usize].2);
    }

    #[test]
    fn two_losses_are_unrecoverable() {
        let (frames, parity) = encode_group(8);
        let mut dec = XorDecoder::new(64);
        let mut recovered = None;
        for (g, idx, p) in &frames {
            if *idx == 2 || *idx == 5 {
                continue; // drop two
            }
            if let Some(r) = dec.on_data(*g, *idx, p) {
                recovered = Some(r);
            }
        }
        if let Some(r) = dec.on_parity(parity.group, parity.k, &parity.payload) {
            recovered = Some(r);
        }
        assert!(recovered.is_none(), "two losses must not yield (wrong) data");
    }

    #[test]
    fn flush_closes_a_partial_group_and_recovers() {
        let mut enc = XorEncoder::new(8);
        let mut frames = Vec::new();
        for i in 0..3 {
            let p = vec![0xA0 + i as u8; 12 + i * 2];
            let (g, idx, par) = enc.push(&p);
            assert!(par.is_none(), "partial group should not auto-close");
            frames.push((g, idx, p));
        }
        let parity = enc.flush().expect("flush closes the partial group");
        assert_eq!(parity.k, 3);

        let mut dec = XorDecoder::new(64);
        let mut recovered = None;
        for (g, idx, p) in &frames {
            if *idx == 1 {
                continue;
            }
            if let Some(r) = dec.on_data(*g, *idx, p) {
                recovered = Some(r);
            }
        }
        if let Some(r) = dec.on_parity(parity.group, parity.k, &parity.payload) {
            recovered = Some(r);
        }
        assert_eq!(recovered.unwrap(), frames[1].2);
    }

    #[test]
    fn no_loss_recovers_nothing() {
        let (frames, parity) = encode_group(4);
        let mut dec = XorDecoder::new(64);
        let mut recovered = None;
        for (g, idx, p) in &frames {
            if let Some(r) = dec.on_data(*g, *idx, p) {
                recovered = Some(r);
            }
        }
        if let Some(r) = dec.on_parity(parity.group, parity.k, &parity.payload) {
            recovered = Some(r);
        }
        assert!(recovered.is_none());
    }
}
