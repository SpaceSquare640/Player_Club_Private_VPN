//! Split tunnelling (Phase E.1) — the data-plane policy that decides which
//! outbound packets the encrypted tunnel carries.
//!
//! Because the virtual adapter is a layer-3 Wintun `/24`, the OS already keeps
//! native / internet traffic **off** the adapter — that traffic never reaches
//! this policy. What this layer adds is explicit control over what the tunnel
//! *does* carry:
//!
//!   * **unicast** to the virtual subnet → tunnelled (the peer);
//!   * **broadcast** (limited `255.255.255.255` and subnet-directed, e.g.
//!     `10.77.0.255`) and **multicast** (`224.0.0.0/4`) → tunnelled *when
//!     enabled*, so LAN-discovery games find each other, but gated so they can
//!     be turned off to avoid flooding the point-to-point link;
//!   * **everything else** → dropped (defense-in-depth: a stray or mis-routed
//!     packet is never leaked to the peer).
//!
//! The policy is a pure function of the destination address, so it is fully
//! unit-testable without any adapter or elevation. OS route management (steering
//! additional prefixes into the adapter) is Phase E.2.

use std::net::Ipv4Addr;

use super::tun::TunConfig;

/// An IPv4 CIDR block.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ipv4Cidr {
    pub addr: Ipv4Addr,
    pub prefix: u8,
}

impl Ipv4Cidr {
    /// Whether `ip` falls within this block.
    pub fn contains(&self, ip: Ipv4Addr) -> bool {
        let m = mask(self.prefix);
        (u32::from(ip) & m) == (u32::from(self.addr) & m)
    }
}

/// The routing decision for one outbound packet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    /// Send it to the peer over the encrypted tunnel.
    Tunnel,
    /// Do not forward it.
    Drop,
}

/// The split-tunnel policy applied to every outbound (host → peer) packet.
#[derive(Debug, Clone)]
pub struct SplitPolicy {
    included: Vec<Ipv4Cidr>,
    subnet_broadcast: Ipv4Addr,
    /// Our own address on the virtual LAN — the anchor for the inbound
    /// reverse-path check (see [`SplitPolicy::admits_inbound`]).
    local_ip: Ipv4Addr,
    forward_broadcast: bool,
    forward_multicast: bool,
}

impl SplitPolicy {
    /// Default policy for the point-to-point virtual LAN: admit the adapter's own
    /// subnet, forward broadcast + multicast (so LAN discovery works), drop the
    /// rest. Derived from the data-plane [`TunConfig`].
    pub fn from_tun(cfg: &TunConfig) -> Self {
        // A leak-prevention allow-list must never widen to the whole internet:
        // `prefix_len` is an unvalidated `u8` from config, and a `/0` would make
        // `included` = 0.0.0.0/0 and admit every unicast destination. Anything
        // outside the plausible virtual-LAN range collapses to /32 — the
        // fail-closed direction (obviously-wrong but safe) rather than /0.
        let prefix = if (8..=32).contains(&cfg.prefix_len) {
            cfg.prefix_len
        } else {
            32
        };
        Self {
            included: vec![Ipv4Cidr {
                addr: network(cfg.virtual_ip, prefix),
                prefix,
            }],
            subnet_broadcast: broadcast(cfg.virtual_ip, prefix),
            local_ip: cfg.virtual_ip,
            forward_broadcast: true,
            forward_multicast: true,
        }
    }

    /// Classify a destination address. Broadcast and multicast are checked before
    /// the unicast allow-list so their toggles govern them even though the
    /// subnet-directed broadcast address lies inside the included subnet.
    pub fn classify(&self, dst: Ipv4Addr) -> Decision {
        if dst.is_broadcast() || dst == self.subnet_broadcast {
            return toggle(self.forward_broadcast);
        }
        if dst.is_multicast() {
            return toggle(self.forward_multicast);
        }
        if self.included.iter().any(|c| c.contains(dst)) {
            Decision::Tunnel
        } else {
            Decision::Drop
        }
    }

    /// **Egress.** Whether the given layer-3 frame's destination should enter the
    /// tunnel. Non-IPv4 or malformed frames are **not** admitted (IPv6 is out of
    /// scope for the IPv4 virtual LAN in this phase).
    pub fn admits(&self, frame: &[u8]) -> bool {
        dst_ipv4(frame)
            .map(|d| self.classify(d) == Decision::Tunnel)
            .unwrap_or(false)
    }

    /// **Ingress.** Whether a frame received from the peer may be injected onto
    /// our adapter.
    ///
    /// Decryption proves the frame came from the authenticated peer — it proves
    /// nothing about the *inner* IP header, which the peer writes freely. So a
    /// destination check alone is not enough: without a reverse-path test a peer
    /// could forge a source (`127.0.0.1`, or our own address) and reach services
    /// that trust loopback or the local subnet. We therefore require:
    ///
    ///   * the **source** to be a plausible peer on the virtual LAN, and never
    ///     our own address (no self-spoofing);
    ///   * the **destination** to be us, or a broadcast/multicast we accept —
    ///     never a phantom host, which the OS might forward off-box.
    ///
    /// Note this assumes the current point-to-point (two-node) LAN; supporting
    /// more peers would widen the destination test to the other members.
    pub fn admits_inbound(&self, frame: &[u8]) -> bool {
        let (Some(src), Some(dst)) = (src_ipv4(frame), dst_ipv4(frame)) else {
            return false;
        };
        if src == self.local_ip || !self.included.iter().any(|c| c.contains(src)) {
            return false;
        }
        if dst == self.local_ip {
            return true;
        }
        if dst.is_broadcast() || dst == self.subnet_broadcast {
            return self.forward_broadcast;
        }
        if dst.is_multicast() {
            return self.forward_multicast;
        }
        false
    }

    /// Override whether broadcast traffic is tunnelled (Phase B.3, user-facing).
    /// Chainable — call after [`from_tun`], which defaults both toggles to `true`.
    pub fn forward_broadcast(mut self, on: bool) -> Self {
        self.forward_broadcast = on;
        self
    }

    /// Override whether multicast traffic is tunnelled (Phase B.3, user-facing).
    /// Chainable — call after [`from_tun`], which defaults both toggles to `true`.
    pub fn forward_multicast(mut self, on: bool) -> Self {
        self.forward_multicast = on;
        self
    }
}

fn toggle(on: bool) -> Decision {
    if on {
        Decision::Tunnel
    } else {
        Decision::Drop
    }
}

/// Extract the destination IPv4 address from a layer-3 frame, or `None` if it is
/// not a well-formed IPv4 packet. The IPv4 destination is always at octets
/// 16..20 regardless of header options.
pub fn dst_ipv4(frame: &[u8]) -> Option<Ipv4Addr> {
    if frame.len() < 20 || (frame[0] >> 4) != 4 {
        return None;
    }
    Some(Ipv4Addr::new(frame[16], frame[17], frame[18], frame[19]))
}

/// Extract the source IPv4 address (octets 12..16) from a layer-3 frame.
pub fn src_ipv4(frame: &[u8]) -> Option<Ipv4Addr> {
    if frame.len() < 20 || (frame[0] >> 4) != 4 {
        return None;
    }
    Some(Ipv4Addr::new(frame[12], frame[13], frame[14], frame[15]))
}

/// Prefix-length → 32-bit network mask.
fn mask(prefix: u8) -> u32 {
    if prefix == 0 {
        0
    } else if prefix >= 32 {
        u32::MAX
    } else {
        !(u32::MAX >> prefix)
    }
}

fn network(ip: Ipv4Addr, prefix: u8) -> Ipv4Addr {
    Ipv4Addr::from(u32::from(ip) & mask(prefix))
}

fn broadcast(ip: Ipv4Addr, prefix: u8) -> Ipv4Addr {
    Ipv4Addr::from((u32::from(ip) & mask(prefix)) | !mask(prefix))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn policy() -> SplitPolicy {
        // 10.77.0.1/24 → subnet 10.77.0.0/24, broadcast 10.77.0.255.
        SplitPolicy::from_tun(&TunConfig {
            virtual_ip: Ipv4Addr::new(10, 77, 0, 1),
            prefix_len: 24,
            ..TunConfig::default()
        })
    }

    #[test]
    fn cidr_contains_respects_prefix() {
        let c = Ipv4Cidr {
            addr: Ipv4Addr::new(10, 77, 0, 0),
            prefix: 24,
        };
        assert!(c.contains(Ipv4Addr::new(10, 77, 0, 5)));
        assert!(c.contains(Ipv4Addr::new(10, 77, 0, 254)));
        assert!(!c.contains(Ipv4Addr::new(10, 77, 1, 5)));
        // /32 matches exactly one; /0 matches everything.
        let host = Ipv4Cidr { addr: Ipv4Addr::new(1, 2, 3, 4), prefix: 32 };
        assert!(host.contains(Ipv4Addr::new(1, 2, 3, 4)));
        assert!(!host.contains(Ipv4Addr::new(1, 2, 3, 5)));
        let all = Ipv4Cidr { addr: Ipv4Addr::UNSPECIFIED, prefix: 0 };
        assert!(all.contains(Ipv4Addr::new(8, 8, 8, 8)));
    }

    #[test]
    fn from_tun_derives_subnet_and_broadcast() {
        let p = policy();
        assert_eq!(p.subnet_broadcast, Ipv4Addr::new(10, 77, 0, 255));
        assert_eq!(p.included[0].addr, Ipv4Addr::new(10, 77, 0, 0));
        assert_eq!(p.included[0].prefix, 24);
    }

    /// Guards against a broadcast derivation that hardcodes the last octet
    /// (correct at /24, wrong everywhere else).
    #[test]
    fn from_tun_handles_non_24_prefixes() {
        let p16 = SplitPolicy::from_tun(&TunConfig {
            virtual_ip: Ipv4Addr::new(10, 77, 0, 1),
            prefix_len: 16,
            ..TunConfig::default()
        });
        assert_eq!(p16.included[0].addr, Ipv4Addr::new(10, 77, 0, 0));
        assert_eq!(p16.included[0].prefix, 16);
        assert_eq!(p16.subnet_broadcast, Ipv4Addr::new(10, 77, 255, 255));
        assert_eq!(p16.classify(Ipv4Addr::new(10, 77, 200, 5)), Decision::Tunnel);

        let p25 = SplitPolicy::from_tun(&TunConfig {
            virtual_ip: Ipv4Addr::new(10, 77, 0, 130),
            prefix_len: 25,
            ..TunConfig::default()
        });
        assert_eq!(p25.included[0].addr, Ipv4Addr::new(10, 77, 0, 128));
        assert_eq!(p25.subnet_broadcast, Ipv4Addr::new(10, 77, 0, 255));
        // .5 is in the lower half — outside a /25 based at .128.
        assert_eq!(p25.classify(Ipv4Addr::new(10, 77, 0, 5)), Decision::Drop);
    }

    /// A nonsensical prefix must narrow the allow-list, never widen it to
    /// 0.0.0.0/0 (which would admit the entire internet).
    #[test]
    fn nonsensical_prefix_fails_closed_not_open() {
        for bad in [0u8, 1, 7, 33, 255] {
            let p = SplitPolicy::from_tun(&TunConfig {
                virtual_ip: Ipv4Addr::new(10, 77, 0, 1),
                prefix_len: bad,
                ..TunConfig::default()
            });
            assert_eq!(p.included[0].prefix, 32, "prefix {bad} must clamp to /32");
            assert_eq!(p.classify(Ipv4Addr::new(8, 8, 8, 8)), Decision::Drop);
            assert_eq!(p.classify(Ipv4Addr::new(10, 77, 0, 1)), Decision::Tunnel);
        }
    }

    #[test]
    fn classifies_unicast_by_subnet() {
        let p = policy();
        assert_eq!(p.classify(Ipv4Addr::new(10, 77, 0, 2)), Decision::Tunnel);
        assert_eq!(p.classify(Ipv4Addr::new(10, 77, 1, 2)), Decision::Drop);
        assert_eq!(p.classify(Ipv4Addr::new(8, 8, 8, 8)), Decision::Drop);
    }

    #[test]
    fn forwards_broadcast_and_multicast_when_enabled() {
        let p = policy();
        assert_eq!(p.classify(Ipv4Addr::BROADCAST), Decision::Tunnel);
        assert_eq!(p.classify(Ipv4Addr::new(10, 77, 0, 255)), Decision::Tunnel);
        assert_eq!(p.classify(Ipv4Addr::new(224, 0, 0, 251)), Decision::Tunnel); // mDNS
    }

    #[test]
    fn toggles_gate_broadcast_and_multicast() {
        let mut p = policy();
        p.forward_broadcast = false;
        assert_eq!(p.classify(Ipv4Addr::new(10, 77, 0, 255)), Decision::Drop);
        assert_eq!(p.classify(Ipv4Addr::BROADCAST), Decision::Drop);
        // multicast still on
        assert_eq!(p.classify(Ipv4Addr::new(224, 0, 0, 251)), Decision::Tunnel);
        p.forward_multicast = false;
        assert_eq!(p.classify(Ipv4Addr::new(224, 0, 0, 251)), Decision::Drop);
        // in-subnet unicast unaffected by the toggles
        assert_eq!(p.classify(Ipv4Addr::new(10, 77, 0, 9)), Decision::Tunnel);
    }

    #[test]
    fn dst_ipv4_parses_and_rejects() {
        let mut f = [0u8; 20];
        f[0] = 0x45; // IPv4, IHL 5
        f[16] = 10;
        f[17] = 77;
        f[18] = 0;
        f[19] = 2;
        assert_eq!(dst_ipv4(&f), Some(Ipv4Addr::new(10, 77, 0, 2)));
        assert_eq!(dst_ipv4(&f[..19]), None); // too short
        let mut v6 = [0u8; 40];
        v6[0] = 0x60; // IPv6
        assert_eq!(dst_ipv4(&v6), None);
        assert_eq!(dst_ipv4(&[]), None);
    }

    /// Build a minimal IPv4 frame with the given source and destination.
    fn frame(src: [u8; 4], dst: [u8; 4]) -> [u8; 20] {
        let mut f = [0u8; 20];
        f[0] = 0x45;
        f[12..16].copy_from_slice(&src);
        f[16..20].copy_from_slice(&dst);
        f
    }

    /// Ingress must not trust the inner header just because the frame decrypted:
    /// a forged source is the whole attack, and decryption cannot detect it.
    #[test]
    fn inbound_rejects_spoofed_sources() {
        let p = policy(); // local_ip 10.77.0.1
        // Legitimate: from the peer, to us.
        assert!(p.admits_inbound(&frame([10, 77, 0, 2], [10, 77, 0, 1])));
        // Forged loopback source — would reach services trusting 127.0.0.0/8.
        assert!(!p.admits_inbound(&frame([127, 0, 0, 1], [10, 77, 0, 1])));
        // Self-spoofing: claims to be us.
        assert!(!p.admits_inbound(&frame([10, 77, 0, 1], [10, 77, 0, 1])));
        // Off-subnet source.
        assert!(!p.admits_inbound(&frame([8, 8, 8, 8], [10, 77, 0, 1])));
    }

    #[test]
    fn inbound_restricts_destinations_to_us_or_group_traffic() {
        let p = policy();
        let peer = [10, 77, 0, 2];
        assert!(p.admits_inbound(&frame(peer, [10, 77, 0, 1]))); // us
        assert!(p.admits_inbound(&frame(peer, [10, 77, 0, 255]))); // subnet broadcast
        assert!(p.admits_inbound(&frame(peer, [255, 255, 255, 255]))); // limited broadcast
        assert!(p.admits_inbound(&frame(peer, [224, 0, 0, 251]))); // multicast
        // A phantom host in-subnet: not us, and the OS might forward it off-box.
        assert!(!p.admits_inbound(&frame(peer, [10, 77, 0, 99])));
        // Off-subnet destination.
        assert!(!p.admits_inbound(&frame(peer, [8, 8, 8, 8])));
        // Non-IPv4 / malformed.
        assert!(!p.admits_inbound(&[0x60; 40]));
        assert!(!p.admits_inbound(&[]));
    }

    #[test]
    fn inbound_honours_the_broadcast_and_multicast_toggles() {
        let mut p = policy();
        let peer = [10, 77, 0, 2];
        p.forward_broadcast = false;
        assert!(!p.admits_inbound(&frame(peer, [10, 77, 0, 255])));
        assert!(p.admits_inbound(&frame(peer, [224, 0, 0, 251])));
        p.forward_multicast = false;
        assert!(!p.admits_inbound(&frame(peer, [224, 0, 0, 251])));
        // Unicast to us is unaffected by the group toggles.
        assert!(p.admits_inbound(&frame(peer, [10, 77, 0, 1])));
    }

    /// The public builder setter (what the connect-time UI settings call) must
    /// actually flip the outcome, not just the field the earlier field-mutation
    /// tests above poked directly.
    #[test]
    fn forward_broadcast_setter_disables_broadcast_forwarding() {
        let p = policy().forward_broadcast(false);
        assert_eq!(p.classify(Ipv4Addr::new(10, 77, 0, 255)), Decision::Drop);
        assert_eq!(p.classify(Ipv4Addr::BROADCAST), Decision::Drop);
        // Multicast is unaffected by the broadcast setter.
        assert_eq!(p.classify(Ipv4Addr::new(224, 0, 0, 251)), Decision::Tunnel);
    }

    #[test]
    fn forward_multicast_setter_disables_multicast_forwarding() {
        let p = policy().forward_multicast(false);
        assert_eq!(p.classify(Ipv4Addr::new(224, 0, 0, 251)), Decision::Drop);
        // Broadcast is unaffected by the multicast setter.
        assert_eq!(p.classify(Ipv4Addr::new(10, 77, 0, 255)), Decision::Tunnel);
    }

    #[test]
    fn admits_full_frames() {
        let p = policy();
        let mut f = [0u8; 20];
        f[0] = 0x45;
        f[16] = 10;
        f[17] = 77;
        f[18] = 0;
        f[19] = 2;
        assert!(p.admits(&f)); // in-subnet unicast
        f[18] = 9; // 10.77.9.2 — out of subnet
        assert!(!p.admits(&f));
        let mut v6 = [0u8; 40];
        v6[0] = 0x60;
        assert!(!p.admits(&v6)); // IPv6 not admitted
    }
}
