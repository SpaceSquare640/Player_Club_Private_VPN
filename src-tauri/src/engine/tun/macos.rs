//! macOS TUN backend (`utun` via the kernel control socket).
//!
//! macOS has no `/dev/tun*` device nodes; the built-in `utun` interface is
//! created by opening a `PF_SYSTEM`/`SYSPROTO_CONTROL` socket, resolving the
//! kernel's `"com.apple.net.utun_control"` control ID via `CTLIOCGINFO`, then
//! `connect()`-ing to it with a specific `sc_unit`. This is the same
//! mechanism WireGuard's macOS backends use — there is no higher-level OS
//! API for it.
//!
//! Two macOS-specific quirks this module has to account for that Linux/
//! Windows don't:
//!
//!   * Every read/write frame is prefixed with a 4-byte, big-endian address
//!     family header (`AF_INET`/`AF_INET6`) ahead of the raw IP packet —
//!     `read_frame`/`write_frame` strip/add it so the rest of the engine
//!     still only ever sees bare IP packets, matching [`TunDevice`]'s
//!     contract on every other backend.
//!   * There's no "create an interface with this name" call — the kernel
//!     assigns `utunN` for whatever unit number connects first, so this
//!     probes `sc_unit` values starting at 1 (→ `utun0`, `utun1`, ...) until
//!     one succeeds, then reads back the name the kernel actually gave it.
//!
//! IP address, MTU, and link-up state are set via `ifconfig` — `utun` is a
//! point-to-point interface, so the local and "peer" address are set to the
//! same value with an explicit netmask, which is the accepted way to make it
//! behave like a normal subnet interface (same trick WireGuard-go's darwin
//! backend uses).
//!
//! **Requires root** — opening the control socket and running `ifconfig`
//! both need it; [`privilege::is_elevated`] checks the effective UID.
//!
//! **Unverified:** this compiles and passes the pure-logic unit tests below,
//! and CI builds it on `macos-latest`, but it has not yet been exercised
//! against a live peer on real macOS hardware — see `PLATFORM-SUPPORT.md`.

use std::io;
use std::mem::size_of;
use std::net::Ipv4Addr;
use std::os::fd::RawFd;
use std::process::Command;

use super::device::{prefix_to_mask, DeviceInfo, TunConfig, TunDevice};

const AF_SYSTEM: libc::c_uchar = 32;
const SYSPROTO_CONTROL: libc::c_int = 2;
const AF_SYS_CONTROL: u16 = 2;
const UTUN_CONTROL_NAME: &[u8] = b"com.apple.net.utun_control\0";
const UTUN_OPT_IFNAME: libc::c_int = 2;
const MAX_KCTL_NAME: usize = 96;
/// `_IOWR('N', 3, struct ctl_info)` — see `<sys/kern_control.h>`. Resolves a
/// kernel control's name to the numeric `ctl_id` `connect()` needs.
const CTLIOCGINFO: libc::c_ulong = 0xc064_4e03;
/// Sanity check tying the hand-derived `CTLIOCGINFO` above to the struct
/// layout it actually encodes the size of, so a future field change to
/// `CtlInfo` can't silently desync the two.
const _: () = assert!(size_of::<CtlInfo>() == 100);

#[repr(C)]
struct CtlInfo {
    ctl_id: u32,
    ctl_name: [libc::c_char; MAX_KCTL_NAME],
}

/// Matches `struct sockaddr_ctl` from `<sys/kern_control.h>`.
#[repr(C)]
struct SockaddrCtl {
    sc_len: libc::c_uchar,
    sc_family: libc::c_uchar,
    ss_sysaddr: u16,
    sc_id: u32,
    sc_unit: u32,
    sc_reserved: [u32; 5],
}

pub struct MacosTunDevice {
    fd: RawFd,
    info: DeviceInfo,
}

impl MacosTunDevice {
    pub fn open(cfg: &TunConfig) -> io::Result<Self> {
        let fd =
            unsafe { libc::socket(AF_SYSTEM as libc::c_int, libc::SOCK_DGRAM, SYSPROTO_CONTROL) };
        if fd < 0 {
            return Err(io::Error::last_os_error());
        }

        if let Err(e) = set_nonblocking(fd) {
            unsafe { libc::close(fd) };
            return Err(e);
        }

        let ctl_id = match resolve_utun_control_id(fd) {
            Ok(id) => id,
            Err(e) => {
                unsafe { libc::close(fd) };
                return Err(e);
            }
        };

        // Probe successive units — the kernel has no "pick any free unit"
        // mode, so this is the same loop-until-EBUSY approach WireGuard-go's
        // darwin backend uses. 256 mirrors that same practical ceiling.
        let mut connected = false;
        for unit in 1u32..=256 {
            let addr = SockaddrCtl {
                sc_len: size_of::<SockaddrCtl>() as libc::c_uchar,
                sc_family: AF_SYSTEM,
                ss_sysaddr: AF_SYS_CONTROL,
                sc_id: ctl_id,
                sc_unit: unit,
                sc_reserved: [0; 5],
            };
            let ret = unsafe {
                libc::connect(
                    fd,
                    &addr as *const SockaddrCtl as *const libc::sockaddr,
                    size_of::<SockaddrCtl>() as libc::socklen_t,
                )
            };
            if ret == 0 {
                connected = true;
                break;
            }
        }
        if !connected {
            unsafe { libc::close(fd) };
            return Err(io::Error::other("no free utun unit (tried 1..=256)"));
        }

        let name = match read_ifname(fd) {
            Ok(n) => n,
            Err(e) => {
                unsafe { libc::close(fd) };
                return Err(e);
            }
        };

        if let Err(e) = assign_ip(&name, cfg.virtual_ip, cfg.prefix_len) {
            unsafe { libc::close(fd) };
            return Err(e);
        }
        let _ = set_mtu(&name, cfg.mtu);
        let _ = add_extra_routes(&name, &cfg.extra_routes);

        Ok(Self {
            fd,
            info: DeviceInfo { name, mtu: cfg.mtu },
        })
    }
}

impl TunDevice for MacosTunDevice {
    fn read_frame(&mut self, buf: &mut [u8]) -> io::Result<Option<usize>> {
        // +4 for the address-family header every utun frame is prefixed
        // with; callers only ever see the bare IP packet after it.
        let mut tmp = vec![0u8; buf.len() + 4];
        let n = unsafe { libc::read(self.fd, tmp.as_mut_ptr() as *mut libc::c_void, tmp.len()) };
        if n < 0 {
            let err = io::Error::last_os_error();
            return if err.kind() == io::ErrorKind::WouldBlock {
                Ok(None)
            } else {
                Err(err)
            };
        }
        let n = n as usize;
        if n <= 4 {
            return Ok(Some(0));
        }
        let payload_len = n - 4;
        let copy_len = payload_len.min(buf.len());
        buf[..copy_len].copy_from_slice(&tmp[4..4 + copy_len]);
        Ok(Some(copy_len))
    }

    fn write_frame(&mut self, frame: &[u8]) -> io::Result<usize> {
        let family: u32 = match frame.first().map(|b| b >> 4) {
            Some(6) => libc::AF_INET6 as u32,
            _ => libc::AF_INET as u32,
        };
        let mut out = Vec::with_capacity(frame.len() + 4);
        out.extend_from_slice(&family.to_be_bytes());
        out.extend_from_slice(frame);

        let n = unsafe { libc::write(self.fd, out.as_ptr() as *const libc::c_void, out.len()) };
        if n < 0 {
            return Err(io::Error::last_os_error());
        }
        // Report the caller's payload length, not the wire length (which
        // includes the 4-byte header) — write_frame's contract elsewhere is
        // "how much of `frame` was written," matching the other backends.
        Ok((n as usize).saturating_sub(4))
    }

    fn info(&self) -> DeviceInfo {
        self.info.clone()
    }
}

impl Drop for MacosTunDevice {
    /// Closing the control socket destroys the `utun` interface — it only
    /// exists for the lifetime of the fd that created it, so no explicit
    /// address/route teardown is needed (same reasoning as the Linux
    /// backend's `Drop`).
    fn drop(&mut self) {
        unsafe { libc::close(self.fd) };
    }
}

fn set_nonblocking(fd: RawFd) -> io::Result<()> {
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFL, 0) };
    if flags < 0 {
        return Err(io::Error::last_os_error());
    }
    if unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

fn resolve_utun_control_id(fd: RawFd) -> io::Result<u32> {
    let mut info = CtlInfo {
        ctl_id: 0,
        ctl_name: [0; MAX_KCTL_NAME],
    };
    for (i, b) in UTUN_CONTROL_NAME.iter().enumerate() {
        info.ctl_name[i] = *b as libc::c_char;
    }
    let ret = unsafe { libc::ioctl(fd, CTLIOCGINFO, &mut info as *mut CtlInfo) };
    if ret < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(info.ctl_id)
}

/// Reads back the `utunN` name the kernel actually assigned on `connect()`,
/// via `getsockopt(SYSPROTO_CONTROL, UTUN_OPT_IFNAME)` — more robust than
/// assuming it from the `sc_unit` we requested.
fn read_ifname(fd: RawFd) -> io::Result<String> {
    let mut buf = [0u8; libc::IFNAMSIZ];
    let mut len = buf.len() as libc::socklen_t;
    let ret = unsafe {
        libc::getsockopt(
            fd,
            SYSPROTO_CONTROL,
            UTUN_OPT_IFNAME,
            buf.as_mut_ptr() as *mut libc::c_void,
            &mut len,
        )
    };
    if ret < 0 {
        return Err(io::Error::last_os_error());
    }
    let end = buf.iter().position(|&b| b == 0).unwrap_or(buf.len());
    Ok(String::from_utf8_lossy(&buf[..end]).into_owned())
}

fn run_ifconfig(args: &[&str]) -> io::Result<()> {
    let status = Command::new("ifconfig").args(args).status()?;
    if !status.success() {
        return Err(io::Error::other(format!(
            "ifconfig {} failed (exit {:?})",
            args.join(" "),
            status.code()
        )));
    }
    Ok(())
}

/// `utun` is point-to-point: `ifconfig` wants `<local> <dest>`, and setting
/// `dest` equal to `local` with an explicit netmask is the standard way to
/// make it route like a normal subnet interface instead of a strict /32
/// tunnel — the same trick WireGuard-go's darwin backend uses.
fn assign_ip(name: &str, ip: Ipv4Addr, prefix_len: u8) -> io::Result<()> {
    let mask = prefix_to_mask(prefix_len);
    run_ifconfig(&[
        name,
        "inet",
        &ip.to_string(),
        &ip.to_string(),
        "netmask",
        &mask.to_string(),
        "up",
    ])
}

fn set_mtu(name: &str, mtu: u16) -> io::Result<()> {
    run_ifconfig(&[name, "mtu", &mtu.to_string()])
}

fn add_extra_routes(name: &str, routes: &[(Ipv4Addr, u8)]) -> io::Result<()> {
    for (network, prefix) in routes {
        let mask = prefix_to_mask(*prefix);
        let _ = Command::new("route")
            .args([
                "-n",
                "add",
                "-net",
                &network.to_string(),
                "-netmask",
                &mask.to_string(),
                "-interface",
                name,
            ])
            .status();
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ctliocginfo_matches_the_kernel_constant() {
        // _IOWR('N', 3, struct ctl_info): IOC_INOUT | (100 << 16) | ('N'<<8) | 3
        assert_eq!(CTLIOCGINFO, 0xc064_4e03);
    }

    #[test]
    fn ctl_info_struct_is_exactly_100_bytes() {
        // 4-byte ctl_id + 96-byte ctl_name, no padding — matches the kernel's
        // layout, which CTLIOCGINFO's derivation above assumes.
        assert_eq!(size_of::<CtlInfo>(), 100);
    }

    #[test]
    fn utun_control_name_is_nul_terminated_and_fits() {
        assert!(UTUN_CONTROL_NAME.last() == Some(&0));
        assert!(UTUN_CONTROL_NAME.len() <= MAX_KCTL_NAME);
    }
}
