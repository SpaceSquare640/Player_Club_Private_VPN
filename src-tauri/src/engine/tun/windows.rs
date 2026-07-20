//! Wintun-backed virtual adapter (Windows).
//!
//! Loads the bundled, signed `wintun.dll`, creates an adapter, assigns the
//! virtual IPv4 address via `netsh`, and exposes the session as a `TunDevice`.

use std::io;
use std::net::Ipv4Addr;
use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;

use wintun::{Adapter, Session, Wintun};

use super::device::{DeviceInfo, TunConfig, TunDevice};

pub struct WintunDevice {
    // Field order governs drop order: session, then adapter, then the library.
    // `Session`'s methods take `&Arc<Self>`, so it must live in an Arc.
    session: Arc<Session>,
    _adapter: Arc<Adapter>,
    _wintun: Wintun,
    info: DeviceInfo,
}

impl WintunDevice {
    pub fn open(cfg: &TunConfig) -> io::Result<Self> {
        let dll = locate_wintun_dll().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                "wintun.dll not found in bundled resources",
            )
        })?;

        let wintun = unsafe { wintun::load_from_path(&dll) }
            .map_err(|e| io::Error::new(io::ErrorKind::Other, format!("load wintun.dll: {e}")))?;

        let adapter = Adapter::create(&wintun, &cfg.name, "Player Club", None)
            .map_err(|e| io::Error::new(io::ErrorKind::Other, format!("create adapter: {e}")))?;

        assign_ip(&cfg.name, cfg.virtual_ip, cfg.prefix_len)?;

        let session = Arc::new(
            adapter
                .start_session(wintun::MAX_RING_CAPACITY)
                .map_err(|e| io::Error::new(io::ErrorKind::Other, format!("start session: {e}")))?,
        );

        Ok(Self {
            session,
            _adapter: adapter,
            _wintun: wintun,
            info: DeviceInfo {
                name: cfg.name.clone(),
                mtu: cfg.mtu,
            },
        })
    }
}

impl TunDevice for WintunDevice {
    fn read_frame(&mut self, buf: &mut [u8]) -> io::Result<Option<usize>> {
        match self.session.try_receive() {
            Ok(Some(packet)) => {
                let bytes = packet.bytes();
                let n = bytes.len().min(buf.len());
                buf[..n].copy_from_slice(&bytes[..n]);
                Ok(Some(n))
            }
            Ok(None) => Ok(None),
            Err(e) => Err(io::Error::new(io::ErrorKind::Other, e.to_string())),
        }
    }

    fn write_frame(&mut self, frame: &[u8]) -> io::Result<usize> {
        let len = frame.len() as u16;
        let mut packet = self
            .session
            .allocate_send_packet(len)
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;
        packet.bytes_mut().copy_from_slice(frame);
        self.session.send_packet(packet);
        Ok(frame.len())
    }

    fn info(&self) -> DeviceInfo {
        self.info.clone()
    }
}

/// Assign the IPv4 address via `netsh` (requires elevation, already verified
/// before the device is opened).
fn assign_ip(name: &str, ip: Ipv4Addr, prefix_len: u8) -> io::Result<()> {
    let mask = prefix_to_mask(prefix_len);
    let status = Command::new("netsh")
        .args([
            "interface",
            "ipv4",
            "set",
            "address",
            &format!("name={name}"),
            "static",
            &ip.to_string(),
            &mask.to_string(),
        ])
        .status()?;
    if !status.success() {
        return Err(io::Error::new(
            io::ErrorKind::Other,
            format!("netsh set address failed (exit {:?})", status.code()),
        ));
    }
    Ok(())
}

fn prefix_to_mask(prefix_len: u8) -> Ipv4Addr {
    let bits: u32 = if prefix_len == 0 {
        0
    } else if prefix_len >= 32 {
        u32::MAX
    } else {
        !(u32::MAX >> prefix_len)
    };
    Ipv4Addr::from(bits)
}

/// Resolve the bundled `wintun.dll` for the current architecture across both
/// dev and bundled layouts.
fn locate_wintun_dll() -> Option<PathBuf> {
    let arch = if cfg!(target_arch = "x86_64") {
        "amd64"
    } else if cfg!(target_arch = "aarch64") {
        "arm64"
    } else if cfg!(target_arch = "x86") {
        "x86"
    } else {
        return None;
    };
    let rel = format!("resources/wintun/{arch}/wintun.dll");

    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            candidates.push(dir.join(&rel));
            candidates.push(dir.join(format!("wintun/{arch}/wintun.dll")));
            candidates.push(dir.join("wintun.dll"));
        }
    }
    // Dev fallback: the crate's resources dir, baked in at compile time.
    candidates.push(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(&rel));

    candidates.into_iter().find(|p| p.exists())
}
