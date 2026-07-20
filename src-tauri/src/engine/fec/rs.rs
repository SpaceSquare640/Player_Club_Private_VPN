//! Reed-Solomon erasure coding (Phase D.2).
//!
//! A group of up to `k` data packets is protected by `r` parity shards, so **any
//! `r` losses** in the group can be reconstructed — D.1's XOR parity recovered
//! only one, and gave up entirely on the burst losses that are typical of real
//! networks.
//!
//! Reed-Solomon requires every shard to be the same length, so each packet is
//! length-prefixed and zero-padded to the group's longest packet before encoding:
//!
//! ```text
//!   Bᵢ = [lenᵢ : u16 BE] ++ Pᵢ ++ zero-pad(to maxlen)      (all length L = maxlen+2)
//!   RS(k, r).encode(B₁..B_k) → parity shards Q₁..Q_r        (also length L)
//! ```
//!
//! Only the parity costs bandwidth: data packets go on the wire at their natural
//! length, and the receiver re-derives their padded form (it learns `L` from any
//! parity shard). Once **any `k`** of the `k + r` shards have arrived, the
//! missing data packets are reconstructed exactly, original lengths included.
//!
//! All header fields (`k`, `r`, shard index) arrive from the peer and are
//! therefore untrusted: they are range-checked before use, and a malformed or
//! hostile group is dropped rather than allowed to allocate or panic.

use std::collections::{HashMap, VecDeque};

use reed_solomon_erasure::galois_8::ReedSolomon;

/// Sanity bounds on peer-supplied geometry. Reed-Solomon itself allows up to 256
/// total shards; we cap far below that so a hostile peer cannot make us allocate
/// large groups. `k + r` must also stay within the codec's limit.
const MAX_K: usize = 64;
const MAX_R: usize = 16;

/// Upper bound on a shard, so a peer-supplied length cannot drive a large
/// allocation. A shard is one MTU-sized packet plus the 2-byte length prefix.
const MAX_SHARD_LEN: usize = 2048;

/// One parity shard emitted when a group closes.
pub struct RsParity {
    pub group: u32,
    /// Data shards in this group (may be < the configured `k` after a flush).
    pub k: u8,
    /// Parity shards in this group.
    pub r: u8,
    /// Which parity shard this is, in `0..r`.
    pub index: u8,
    pub shard: Vec<u8>,
}

/// Rolling encoder (uplink). Data packets are sent immediately by the caller;
/// this accumulates them and, when the group closes, yields the parity shards to
/// send in addition.
pub struct RsEncoder {
    k: u8,
    r: u8,
    group_id: u32,
    members: Vec<Vec<u8>>,
}

impl RsEncoder {
    pub fn new(k: u8, r: u8) -> Self {
        Self {
            k: (k as usize).clamp(1, MAX_K) as u8,
            r: (r as usize).clamp(1, MAX_R) as u8,
            group_id: 0,
            members: Vec::new(),
        }
    }

    /// Record `ip` in the current group, returning `(group, index)` to tag its
    /// `FEC_DATA` frame. If this closes the group, also returns its parity
    /// shards and rolls to the next group.
    pub fn push(&mut self, ip: &[u8]) -> (u32, u8, Vec<RsParity>) {
        let group = self.group_id;
        let index = self.members.len() as u8;
        self.members.push(ip.to_vec());
        let parity = if self.members.len() >= self.k as usize {
            self.close()
        } else {
            Vec::new()
        };
        (group, index, parity)
    }

    /// Close a partial group (idle flush). Empty when the group is empty.
    pub fn flush(&mut self) -> Vec<RsParity> {
        if self.members.is_empty() {
            Vec::new()
        } else {
            self.close()
        }
    }

    fn close(&mut self) -> Vec<RsParity> {
        let k = self.members.len();
        let r = self.r as usize;
        let group = self.group_id;
        let shard_len = self.members.iter().map(|m| m.len()).max().unwrap_or(0) + 2;

        let mut shards: Vec<Vec<u8>> = Vec::with_capacity(k + r);
        for m in &self.members {
            shards.push(pad_shard(m, shard_len));
        }
        shards.resize(k + r, vec![0u8; shard_len]);

        self.members.clear();
        self.group_id = self.group_id.wrapping_add(1);

        let Ok(codec) = ReedSolomon::new(k, r) else {
            return Vec::new();
        };
        if codec.encode(&mut shards).is_err() {
            return Vec::new();
        }
        shards
            .drain(..k) // discard the data shards; only parity goes on the wire
            .count();
        shards
            .into_iter()
            .enumerate()
            .map(|(i, shard)| RsParity {
                group,
                k: k as u8,
                r: r as u8,
                index: i as u8,
                shard,
            })
            .collect()
    }
}

#[derive(Default)]
struct RsGroup {
    k: Option<u8>,
    r: Option<u8>,
    shard_len: Option<usize>,
    /// index → payload, at natural (unpadded) length.
    data: HashMap<u8, Vec<u8>>,
    /// parity index → shard, already `shard_len` bytes.
    parity: HashMap<u8, Vec<u8>>,
    done: bool,
}

/// Decoder (downlink). Buffers each group until `k` of its shards have arrived,
/// then reconstructs every missing data packet at once. Received packets are
/// forwarded by the caller immediately; this returns only *recovered* ones.
pub struct RsDecoder {
    groups: HashMap<u32, RsGroup>,
    order: VecDeque<u32>,
    cap: usize,
}

impl RsDecoder {
    pub fn new(cap: usize) -> Self {
        Self {
            groups: HashMap::new(),
            order: VecDeque::new(),
            cap: cap.max(1),
        }
    }

    /// Feed a received data packet. Returns any packets this completes.
    pub fn on_data(&mut self, group: u32, index: u8, ip: &[u8]) -> Vec<Vec<u8>> {
        if index as usize >= MAX_K {
            return Vec::new();
        }
        self.ensure(group);
        match self.groups.get_mut(&group) {
            Some(g) if !g.done => g.data.insert(index, ip.to_vec()),
            _ => return Vec::new(),
        };
        self.try_recover(group)
    }

    /// Feed a received parity shard. `k`, `r`, `index` come from the peer and are
    /// validated here. Returns any packets this completes.
    pub fn on_parity(&mut self, group: u32, k: u8, r: u8, index: u8, shard: &[u8]) -> Vec<Vec<u8>> {
        let (ku, ru, iu) = (k as usize, r as usize, index as usize);
        if ku == 0
            || ku > MAX_K
            || ru == 0
            || ru > MAX_R
            || iu >= ru
            || shard.len() < 2
            || shard.len() > MAX_SHARD_LEN
        {
            return Vec::new(); // malformed / hostile geometry
        }
        self.ensure(group);
        let Some(g) = self.groups.get_mut(&group) else {
            return Vec::new();
        };
        if g.done {
            return Vec::new();
        }
        // A shard must be long enough to hold every data packet we have already
        // seen (each is length-prefixed and padded to the shard size). A shorter
        // one is *provably* inconsistent with this group, so reject it rather
        // than let it set the geometry and drive a bogus reconstruction.
        if let Some(longest) = g.data.values().map(|p| p.len()).max() {
            if shard.len() < longest + 2 {
                return Vec::new();
            }
        }
        // All shards in a group must agree on geometry; ignore any that disagree.
        match (g.k, g.r, g.shard_len) {
            (Some(pk), Some(pr), Some(pl)) if pk != k || pr != r || pl != shard.len() => {
                return Vec::new()
            }
            _ => {}
        }
        g.k = Some(k);
        g.r = Some(r);
        g.shard_len = Some(shard.len());
        g.parity.insert(index, shard.to_vec());
        self.try_recover(group)
    }

    fn ensure(&mut self, group: u32) {
        if self.groups.contains_key(&group) {
            return;
        }
        self.groups.insert(group, RsGroup::default());
        self.order.push_back(group);
        while self.order.len() > self.cap {
            if let Some(old) = self.order.pop_front() {
                self.groups.remove(&old);
            }
        }
    }

    fn try_recover(&mut self, group: u32) -> Vec<Vec<u8>> {
        let mut recovered = Vec::new();
        let mut done = false;

        if let Some(g) = self.groups.get(&group) {
            if g.done {
                return recovered;
            }
            if let (Some(k), Some(r), Some(shard_len)) = (g.k, g.r, g.shard_len) {
                let (k, r) = (k as usize, r as usize);
                if g.data.len() >= k {
                    done = true; // every data packet arrived — nothing to rebuild
                } else if g.data.len() + g.parity.len() >= k {
                    if let Ok(codec) = ReedSolomon::new(k, r) {
                        let mut shards: Vec<Option<Vec<u8>>> = vec![None; k + r];
                        for (&i, payload) in &g.data {
                            if (i as usize) < k && payload.len() + 2 <= shard_len {
                                shards[i as usize] = Some(pad_shard(payload, shard_len));
                            }
                        }
                        for (&i, shard) in &g.parity {
                            if (i as usize) < r && shard.len() == shard_len {
                                shards[k + i as usize] = Some(shard.clone());
                            }
                        }
                        if codec.reconstruct_data(&mut shards).is_ok() {
                            for (i, slot) in shards.iter().take(k).enumerate() {
                                if g.data.contains_key(&(i as u8)) {
                                    continue; // we already had this one
                                }
                                if let Some(p) = slot.as_ref().and_then(|s| unpad(s)) {
                                    recovered.push(p);
                                }
                            }
                            done = true;
                        }
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
            g.data.clear();
            g.parity.clear();
        }
    }
}

/// `[len:u16 BE][payload][zero pad]`, exactly `shard_len` bytes.
fn pad_shard(payload: &[u8], shard_len: usize) -> Vec<u8> {
    let mut s = vec![0u8; shard_len];
    s[0..2].copy_from_slice(&(payload.len() as u16).to_be_bytes());
    let n = payload.len().min(shard_len.saturating_sub(2));
    s[2..2 + n].copy_from_slice(&payload[..n]);
    s
}

/// Strip the length prefix from a reconstructed shard.
fn unpad(shard: &[u8]) -> Option<Vec<u8>> {
    if shard.len() < 2 {
        return None;
    }
    let len = (u16::from_be_bytes([shard[0], shard[1]]) as usize).min(shard.len() - 2);
    Some(shard[2..2 + len].to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Encode one full group of `k` varying-length packets with `r` parity.
    fn encode_group(k: u8, r: u8) -> (Vec<(u32, u8, Vec<u8>)>, Vec<RsParity>) {
        let mut enc = RsEncoder::new(k, r);
        let mut frames = Vec::new();
        let mut parity = Vec::new();
        for i in 0..k as usize {
            let p = vec![i as u8 + 1; 8 + i * 5]; // distinct contents and lengths
            let (g, idx, par) = enc.push(&p);
            frames.push((g, idx, p));
            parity.extend(par);
        }
        assert_eq!(parity.len(), r as usize, "a closed group yields r parity shards");
        (frames, parity)
    }

    /// Feed everything except the dropped data indices, then all parity.
    fn run_decoder(
        frames: &[(u32, u8, Vec<u8>)],
        parity: &[RsParity],
        drop: &[u8],
    ) -> Vec<Vec<u8>> {
        let mut dec = RsDecoder::new(64);
        let mut out = Vec::new();
        for (g, idx, p) in frames {
            if drop.contains(idx) {
                continue;
            }
            out.extend(dec.on_data(*g, *idx, p));
        }
        for q in parity {
            out.extend(dec.on_parity(q.group, q.k, q.r, q.index, &q.shard));
        }
        out
    }

    #[test]
    fn recovers_two_losses_with_two_parity() {
        let (frames, parity) = encode_group(8, 2);
        let recovered = run_decoder(&frames, &parity, &[2, 5]);
        assert_eq!(recovered.len(), 2);
        assert!(recovered.contains(&frames[2].2));
        assert!(recovered.contains(&frames[5].2));
    }

    #[test]
    fn three_losses_with_two_parity_are_unrecoverable() {
        let (frames, parity) = encode_group(8, 2);
        let recovered = run_decoder(&frames, &parity, &[1, 4, 6]);
        assert!(recovered.is_empty(), "must not emit (wrong) data beyond capacity");
    }

    /// r=1 must behave exactly like the D.1 XOR parity it replaces.
    #[test]
    fn single_parity_matches_the_xor_behaviour_it_replaces() {
        let (frames, parity) = encode_group(8, 1);
        let one = run_decoder(&frames, &parity, &[3]);
        assert_eq!(one, vec![frames[3].2.clone()]);

        let (frames, parity) = encode_group(8, 1);
        let two = run_decoder(&frames, &parity, &[3, 4]);
        assert!(two.is_empty());
    }

    #[test]
    fn recovers_with_parity_first_and_data_reordered() {
        let (frames, parity) = encode_group(6, 2);
        let mut dec = RsDecoder::new(64);
        let mut out = Vec::new();
        for q in &parity {
            out.extend(dec.on_parity(q.group, q.k, q.r, q.index, &q.shard));
        }
        for (g, idx, p) in frames.iter().rev() {
            if *idx == 0 || *idx == 5 {
                continue;
            }
            out.extend(dec.on_data(*g, *idx, p));
        }
        assert_eq!(out.len(), 2);
        assert!(out.contains(&frames[0].2));
        assert!(out.contains(&frames[5].2));
    }

    /// Losing only parity costs nothing — the data is already complete.
    #[test]
    fn no_data_loss_recovers_nothing() {
        let (frames, parity) = encode_group(4, 2);
        let recovered = run_decoder(&frames, &parity[..1], &[]);
        assert!(recovered.is_empty());
    }

    #[test]
    fn flush_closes_a_partial_group_and_recovers() {
        let mut enc = RsEncoder::new(8, 2);
        let mut frames = Vec::new();
        for i in 0..3usize {
            let p = vec![0xA0 + i as u8; 12 + i * 3];
            let (g, idx, par) = enc.push(&p);
            assert!(par.is_empty(), "a partial group must not auto-close");
            frames.push((g, idx, p));
        }
        let parity = enc.flush();
        assert_eq!(parity.len(), 2);
        assert_eq!(parity[0].k, 3, "parity reports the actual member count");

        let recovered = run_decoder(&frames, &parity, &[0, 2]);
        assert_eq!(recovered.len(), 2);
        assert!(recovered.contains(&frames[0].2));
        assert!(recovered.contains(&frames[2].2));
    }

    /// Header fields arrive from the peer: malformed geometry must be dropped,
    /// never trusted into an allocation or a panic.
    #[test]
    fn rejects_malformed_or_hostile_headers() {
        let (frames, parity) = encode_group(4, 2);
        let q = &parity[0];
        let mut dec = RsDecoder::new(64);
        for (g, idx, p) in &frames {
            if *idx == 1 {
                continue;
            }
            dec.on_data(*g, *idx, p);
        }
        // k = 0, r = 0, index >= r, oversized k, truncated shard, oversized shard.
        assert!(dec.on_parity(q.group, 0, 2, 0, &q.shard).is_empty());
        assert!(dec.on_parity(q.group, 4, 0, 0, &q.shard).is_empty());
        assert!(dec.on_parity(q.group, 4, 2, 9, &q.shard).is_empty());
        assert!(dec.on_parity(q.group, 200, 2, 0, &q.shard).is_empty());
        assert!(dec.on_parity(q.group, 4, 2, 0, &[0u8]).is_empty());
        assert!(dec
            .on_parity(q.group, 4, 2, 0, &vec![0u8; MAX_SHARD_LEN + 1])
            .is_empty());
        // A shard too short to hold a data packet we already hold is provably
        // inconsistent with this group — it must not set the geometry.
        assert!(dec.on_parity(q.group, 4, 2, 0, &[0u8, 0u8]).is_empty());

        // The genuine shard still works afterwards.
        assert_eq!(
            dec.on_parity(q.group, q.k, q.r, q.index, &q.shard),
            vec![frames[1].2.clone()]
        );
    }

    /// Once a group's geometry is established by a valid parity, a later shard
    /// that disagrees about its length is ignored rather than mixed in.
    #[test]
    fn rejects_shards_disagreeing_with_established_geometry() {
        let (frames, parity) = encode_group(4, 2);
        let mut dec = RsDecoder::new(64);
        for (g, idx, p) in &frames {
            if *idx == 1 {
                continue;
            }
            dec.on_data(*g, *idx, p);
        }
        let q = &parity[0];
        // Establish geometry with the genuine shard — this also recovers index 1.
        assert_eq!(
            dec.on_parity(q.group, q.k, q.r, q.index, &q.shard),
            vec![frames[1].2.clone()]
        );
        // A disagreeing shard afterwards is ignored (the group is also complete).
        assert!(dec
            .on_parity(q.group, q.k, q.r, 1, &vec![0u8; q.shard.len() + 7])
            .is_empty());
    }

    #[test]
    fn old_groups_are_evicted() {
        let mut dec = RsDecoder::new(2);
        for g in 0..5u32 {
            dec.on_data(g, 0, &[1, 2, 3]);
        }
        assert!(dec.groups.len() <= 2, "group buffer must stay bounded");
    }
}
