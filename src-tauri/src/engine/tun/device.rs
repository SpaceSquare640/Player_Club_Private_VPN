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
}

impl Default for TunConfig {
    fn default() -> Self {
        Self {
            name: "PlayerClubVPN".to_string(),
            virtual_ip: Ipv4Addr::new(10, 77, 0, 1),
            prefix_len: 24,
            mtu: 1420,
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

/// A layer-3 virtual network interface.
///
/// `read_frame` is poll-style: it returns `Ok(None)` when no frame is currently
/// available so the capture loop can check for shutdown between reads.
pub trait TunDevice: Send {
    fn read_frame(&mut self, buf: &mut [u8]) -> io::Result<Option<usize>>;
    /// Inject a frame back onto the adapter. Unused until forwarding lands in
    /// Phase C (transport), but part of the device contract.
    #[allow(dead_code)]
    fn write_frame(&mut self, frame: &[u8]) -> io::Result<usize>;
    #[allow(dead_code)]
    fn info(&self) -> DeviceInfo;
}
