//! Virtual network device management.
//!
//! Phase B implements the Windows/Wintun backend behind a platform-neutral
//! [`TunDevice`] trait; other targets fall through to an "unsupported" error.
//! Privilege detection and relaunch-elevated live in [`privilege`].

pub mod device;
pub mod packet;
pub mod privilege;

#[cfg(windows)]
mod windows;

use std::io;

pub use device::{TunConfig, TunDevice};
pub use privilege::ElevationStatus;

/// Open the platform virtual device. On Windows this creates a Wintun adapter
/// (the caller must have verified elevation first); elsewhere it is
/// unsupported in this build.
pub fn open_device(cfg: &TunConfig) -> io::Result<Box<dyn TunDevice>> {
    #[cfg(windows)]
    {
        Ok(Box::new(windows::WintunDevice::open(cfg)?))
    }
    #[cfg(not(windows))]
    {
        let _ = cfg;
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "real TUN device is only supported on Windows in this build",
        ))
    }
}
