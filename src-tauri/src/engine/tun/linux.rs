//! Linux TUN backend (`/dev/net/tun`).
//!
//! Opens the kernel TUN driver directly via `open(2)` + `ioctl(2)` — no
//! external crate, mirroring how wireguard-go and boringtun do it. The
//! device is created in `IFF_TUN | IFF_NO_PI` mode: layer-3 only, no 4-byte
//! packet-info prefix, so frames read/written here are raw IP packets, same
//! shape [`TunDevice`] already expects from the Windows/Wintun backend.
//!
//! The fd is put in non-blocking mode so `read_frame` can return `Ok(None)`
//! when idle, matching the poll-style contract `dataplane::bridge` already
//! relies on for Wintun.
//!
//! IP address, MTU, and link-up state are set via the `ip` command (iproute2)
//! rather than another raw ioctl — `ip` is present on every mainstream distro
//! and this mirrors the Windows backend's own use of `netsh`/PowerShell for
//! the equivalent OS-integration steps.
//!
//! **Requires root or `CAP_NET_ADMIN`** — opening `/dev/net/tun` and running
//! `ip addr add`/`ip link set` both need it. [`privilege::is_elevated`] on
//! Linux checks the effective UID for exactly this reason.
//!
//! **Unverified:** this compiles and passes the pure-logic unit tests below,
//! and CI builds it on `ubuntu-latest`, but it has not yet been exercised
//! against a live peer on real Linux hardware — see `PLATFORM-SUPPORT.md`.

use std::ffi::CString;
use std::fs::File;
use std::io;
use std::net::Ipv4Addr;
use std::os::fd::FromRawFd;
use std::process::Command;

use super::device::{DeviceInfo, TunConfig, TunDevice};

const IFF_TUN: libc::c_short = 0x0001;
const IFF_NO_PI: libc::c_short = 0x1000;
/// `_IOW('T', 202, int)` — see `linux/if_tun.h`. Requests the kernel create
/// (or attach to) the interface named in `ifreq.ifr_name`, with the flags in
/// `ifreq.ifr_ifru.ifru_flags`.
const TUNSETIFF: libc::c_ulong = 0x4004_54ca;
const IFNAMSIZ: usize = 16;

/// Layout matches the kernel's `struct ifreq` for the `ifr_flags` union arm
/// (the only arm `TUNSETIFF` reads). `#[repr(C)]` so the field offsets match
/// what the kernel expects byte-for-byte.
#[repr(C)]
struct IfReqFlags {
    ifr_name: [libc::c_char; IFNAMSIZ],
    ifr_flags: libc::c_short,
}

pub struct LinuxTunDevice {
    file: File,
    info: DeviceInfo,
}

impl LinuxTunDevice {
    pub fn open(cfg: &TunConfig) -> io::Result<Self> {
        let name = pack_ifname(&cfg.name)?;

        let path = CString::new("/dev/net/tun").unwrap();
        let fd = unsafe { libc::open(path.as_ptr(), libc::O_RDWR | libc::O_NONBLOCK) };
        if fd < 0 {
            return Err(io::Error::last_os_error());
        }

        let mut req = IfReqFlags {
            ifr_name: name,
            ifr_flags: IFF_TUN | IFF_NO_PI,
        };
        let ret = unsafe { libc::ioctl(fd, TUNSETIFF, &mut req as *mut IfReqFlags) };
        if ret < 0 {
            let err = io::Error::last_os_error();
            unsafe { libc::close(fd) };
            return Err(err);
        }

        // The kernel may have picked a different concrete name than requested
        // (e.g. if a name collision forced a suffix); re-read what actually
        // landed so `info()`/route commands below target the real interface.
        let actual_name = unpack_ifname(&req.ifr_name);

        assign_ip(&actual_name, cfg.virtual_ip, cfg.prefix_len)?;
        set_mtu(&actual_name, cfg.mtu)?;
        link_up(&actual_name)?;
        let _ = add_extra_routes(&actual_name, &cfg.extra_routes);

        // SAFETY: `fd` was just returned by a successful `open(2)` above and
        // is not used anywhere else in this scope after this point.
        let file = unsafe { File::from_raw_fd(fd) };

        Ok(Self {
            file,
            info: DeviceInfo {
                name: actual_name,
                mtu: cfg.mtu,
            },
        })
    }
}

impl TunDevice for LinuxTunDevice {
    fn read_frame(&mut self, buf: &mut [u8]) -> io::Result<Option<usize>> {
        use std::io::Read;
        match self.file.read(buf) {
            Ok(n) => Ok(Some(n)),
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => Ok(None),
            Err(e) => Err(e),
        }
    }

    fn write_frame(&mut self, frame: &[u8]) -> io::Result<usize> {
        use std::io::Write;
        self.file.write(frame)
    }

    fn info(&self) -> DeviceInfo {
        self.info.clone()
    }
}

impl Drop for LinuxTunDevice {
    /// No explicit teardown needed: closing the fd (via `File`'s own `Drop`,
    /// which runs after this — field order is declaration order) tells the
    /// kernel to destroy a non-persistent TUN interface automatically, taking
    /// any addresses/routes bound to it with it.
    fn drop(&mut self) {}
}

fn pack_ifname(name: &str) -> io::Result<[libc::c_char; IFNAMSIZ]> {
    if name.as_bytes().len() >= IFNAMSIZ {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "interface name {name:?} is too long (max {} bytes)",
                IFNAMSIZ - 1
            ),
        ));
    }
    let mut out = [0 as libc::c_char; IFNAMSIZ];
    for (i, b) in name.as_bytes().iter().enumerate() {
        out[i] = *b as libc::c_char;
    }
    Ok(out)
}

fn unpack_ifname(raw: &[libc::c_char; IFNAMSIZ]) -> String {
    let bytes: Vec<u8> = raw
        .iter()
        .take_while(|&&c| c != 0)
        .map(|&c| c as u8)
        .collect();
    String::from_utf8_lossy(&bytes).into_owned()
}

fn run_ip(args: &[&str]) -> io::Result<()> {
    let status = Command::new("ip").args(args).status()?;
    if !status.success() {
        return Err(io::Error::other(format!(
            "ip {} failed (exit {:?})",
            args.join(" "),
            status.code()
        )));
    }
    Ok(())
}

fn assign_ip(name: &str, ip: Ipv4Addr, prefix_len: u8) -> io::Result<()> {
    run_ip(&["addr", "add", &format!("{ip}/{prefix_len}"), "dev", name])
}

fn set_mtu(name: &str, mtu: u16) -> io::Result<()> {
    run_ip(&["link", "set", "dev", name, "mtu", &mtu.to_string()])
}

fn link_up(name: &str) -> io::Result<()> {
    run_ip(&["link", "set", "dev", name, "up"])
}

/// Mirrors `windows::add_extra_routes`: best-effort, one bad entry must not
/// block the others or fail adapter creation over a convenience feature.
fn add_extra_routes(name: &str, routes: &[(Ipv4Addr, u8)]) -> io::Result<()> {
    for (network, prefix) in routes {
        let _ = run_ip(&["route", "add", &format!("{network}/{prefix}"), "dev", name]);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tunsetiff_matches_the_kernel_constant() {
        // _IOW('T', 202, int): dir=1(write) size=4 type='T'(0x54) nr=202(0xCA)
        assert_eq!(TUNSETIFF, 0x4004_54ca);
    }

    #[test]
    fn pack_ifname_round_trips_a_short_name() {
        let packed = pack_ifname("pcpvpn0").unwrap();
        assert_eq!(unpack_ifname(&packed), "pcpvpn0");
    }

    #[test]
    fn pack_ifname_rejects_a_name_at_or_over_ifnamsiz() {
        // IFNAMSIZ is 16, so a 15-byte name is the longest that leaves room
        // for the mandatory NUL terminator the kernel expects.
        assert!(pack_ifname(&"a".repeat(15)).is_ok());
        assert!(pack_ifname(&"a".repeat(16)).is_err());
    }

    #[test]
    fn unpack_ifname_stops_at_the_first_nul() {
        let mut raw = [0 as libc::c_char; IFNAMSIZ];
        raw[0] = b'x' as libc::c_char;
        raw[1] = b'0' as libc::c_char;
        // Everything past index 1 is already zero-initialized — the kernel
        // NUL-pads short names the same way.
        assert_eq!(unpack_ifname(&raw), "x0");
    }
}
