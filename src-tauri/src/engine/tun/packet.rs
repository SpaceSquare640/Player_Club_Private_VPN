//! Minimal IP header classification for the packet log (no external deps).

/// Lightweight metadata extracted from a layer-3 frame.
pub struct PacketMeta {
    pub proto: &'static str,
    pub len: u16,
}

/// Classify a raw IPv4/IPv6 frame. Returns `None` if it is too short or not a
/// recognizable IP packet.
pub fn classify(frame: &[u8]) -> Option<PacketMeta> {
    let version = frame.first()? >> 4;
    match version {
        4 => classify_v4(frame),
        6 => classify_v6(frame),
        _ => None,
    }
}

fn classify_v4(frame: &[u8]) -> Option<PacketMeta> {
    if frame.len() < 20 {
        return None;
    }
    let total_len = u16::from_be_bytes([frame[2], frame[3]]);
    let len = if total_len == 0 {
        frame.len() as u16
    } else {
        total_len
    };
    Some(PacketMeta {
        proto: proto_name(frame[9]),
        len,
    })
}

fn classify_v6(frame: &[u8]) -> Option<PacketMeta> {
    if frame.len() < 40 {
        return None;
    }
    let payload_len = u16::from_be_bytes([frame[4], frame[5]]);
    Some(PacketMeta {
        proto: proto_name(frame[6]), // next-header
        len: 40u16.saturating_add(payload_len),
    })
}

fn proto_name(proto: u8) -> &'static str {
    match proto {
        1 => "ICMP",
        2 => "IGMP",
        6 => "TCP",
        17 => "UDP",
        58 => "ICMPv6",
        _ => "IP",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_ipv4_udp() {
        // version=4, IHL=5; total length = 0x0030 (48); protocol=17 (UDP).
        let mut f = [0u8; 48];
        f[0] = 0x45;
        f[2] = 0x00;
        f[3] = 0x30;
        f[9] = 17;
        let m = classify(&f).unwrap();
        assert_eq!(m.proto, "UDP");
        assert_eq!(m.len, 48);
    }

    #[test]
    fn parses_ipv4_icmp() {
        let mut f = [0u8; 28];
        f[0] = 0x45;
        f[3] = 28;
        f[9] = 1;
        let m = classify(&f).unwrap();
        assert_eq!(m.proto, "ICMP");
        assert_eq!(m.len, 28);
    }

    #[test]
    fn parses_ipv6_tcp() {
        // version=6; payload length=0x0014 (20); next-header=6 (TCP).
        let mut f = [0u8; 60];
        f[0] = 0x60;
        f[4] = 0x00;
        f[5] = 0x14;
        f[6] = 6;
        let m = classify(&f).unwrap();
        assert_eq!(m.proto, "TCP");
        assert_eq!(m.len, 60);
    }

    #[test]
    fn rejects_short_and_unknown() {
        assert!(classify(&[]).is_none());
        assert!(classify(&[0x45, 0x00]).is_none()); // too short for v4
        assert!(classify(&[0x30]).is_none()); // version 3
    }
}
