//! Platform-neutral virtual network device contract.

use std::io;
use std::net::Ipv4Addr;

use serde::{Deserialize, Serialize};

/// Parameters for creating the virtual adapter.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TunConfig {
    /// Adapter name as shown in Windows networking.
    pub name: String,
    /// IPv4 address assigned to this host on the virtual LAN.
    pub virtual_ip: Ipv4Addr,
    /// Subnet prefix length (e.g. 24 → /24).
    pub prefix_len: u8,
    /// Link MTU.
    pub mtu: u16,
    /// Additional `(network, prefix)` pairs to route into this adapter
    /// beyond the peer's own virtual-LAN subnet (Phase E.2 — OS route
    /// management). A plain tuple rather than `split_tunnel::Ipv4Cidr`: that
    /// type lives in a module that itself depends on `TunConfig`, so pulling
    /// it in here would be circular. `engine::connection` converts between
    /// the two where both are in scope.
    #[serde(default)]
    pub extra_routes: Vec<(Ipv4Addr, u8)>,
}

impl Default for TunConfig {
    fn default() -> Self {
        Self {
            name: "PlayerClubVPN".to_string(),
            virtual_ip: Ipv4Addr::new(10, 77, 0, 1),
            prefix_len: 24,
            mtu: 1420,
            extra_routes: Vec::new(),
        }
    }
}

/// Describes an opened device. Read by the diagnostics layer in a later phase.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct DeviceInfo {
    pub name: String,
    pub mtu: u16,
}

/// Convert a CIDR prefix length to a dotted-decimal IPv4 subnet mask.
/// Shared by the Windows (`netsh`) and macOS (`ifconfig`) backends, which
/// both need a mask rather than a `/prefix` — Linux's `ip addr` command
/// takes CIDR notation directly and has no use for this.
pub(crate) fn prefix_to_mask(prefix_len: u8) -> Ipv4Addr {
    let bits: u32 = if prefix_len == 0 {
        0
    } else if prefix_len >= 32 {
        u32::MAX
    } else {
        !(u32::MAX >> prefix_len)
    };
    Ipv4Addr::from(bits)
}

/// A layer-3 virtual network interface.
///
/// `read_frame` is poll-style: it returns `Ok(None)` when no frame is currently
/// available so the capture loop can check for shutdown between reads.
pub trait TunDevice: Send {
    fn read_frame(&mut self, buf: &mut [u8]) -> io::Result<Option<usize>>;
    /// Inject a frame back onto the adapter (the data-plane downlink).
    fn write_frame(&mut self, frame: &[u8]) -> io::Result<usize>;
    #[allow(dead_code)]
    fn info(&self) -> DeviceInfo;
}
