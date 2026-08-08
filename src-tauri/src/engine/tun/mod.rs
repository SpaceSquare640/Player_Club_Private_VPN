//! Virtual network device management.
//!
//! A platform-neutral [`TunDevice`] trait, backed by whichever of three
//! platform implementations matches the build target: Wintun on Windows,
//! `/dev/net/tun` on Linux, or a `utun` kernel-control socket on macOS. Every
//! other target (mobile, other BSDs, ...) falls through to an "unsupported"
//! error — see [`open_device`]. Privilege detection and relaunch-elevated
//! live in [`privilege`].
//!
//! The Linux and macOS backends are new and, unlike the Windows one, have
//! not yet been exercised against a live peer on real hardware — see each
//! module's doc comment and `PLATFORM-SUPPORT.md` for exactly what remains
//! unverified.

pub mod device;
pub mod packet;
pub mod privilege;

#[cfg(target_os = "linux")]
pub(crate) mod linux;
#[cfg(target_os = "macos")]
pub(crate) mod macos;
#[cfg(windows)]
pub(crate) mod windows;

use std::io;

pub use device::{TunConfig, TunDevice};
pub use privilege::ElevationStatus;

/// Open the platform virtual device (the caller must have verified elevation
/// first — see [`privilege::is_elevated`]). Unsupported on targets with no
/// backend in this build (mobile, other BSDs, ...).
pub fn open_device(cfg: &TunConfig) -> io::Result<Box<dyn TunDevice>> {
    #[cfg(windows)]
    {
        Ok(Box::new(windows::WintunDevice::open(cfg)?))
    }
    #[cfg(target_os = "linux")]
    {
        Ok(Box::new(linux::LinuxTunDevice::open(cfg)?))
    }
    #[cfg(target_os = "macos")]
    {
        Ok(Box::new(macos::MacosTunDevice::open(cfg)?))
    }
    #[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
    {
        let _ = cfg;
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "real TUN device is not supported on this platform in this build",
        ))
    }
}
