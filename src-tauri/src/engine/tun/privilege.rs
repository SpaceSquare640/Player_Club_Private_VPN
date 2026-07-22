//! Elevation detection and relaunch-elevated. OS-level; no Tauri dependency.

use serde::Serialize;

/// Privilege status reported to the UI.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ElevationStatus {
    /// Process is running with an elevated token (Windows admin).
    pub elevated: bool,
    /// True only when a real Wintun adapter can be created in this build/run
    /// (Windows + elevated).
    pub can_create_tun: bool,
    /// Target OS string (for UI messaging).
    pub os: &'static str,
}

/// Current privilege/capability status.
pub fn status() -> ElevationStatus {
    let elevated = is_elevated();
    ElevationStatus {
        elevated,
        can_create_tun: cfg!(windows) && elevated,
        os: std::env::consts::OS,
    }
}

#[cfg(windows)]
pub fn is_elevated() -> bool {
    use std::mem::{size_of, zeroed};

    use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
    use windows_sys::Win32::Security::{
        GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY,
    };
    use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

    unsafe {
        let mut token: HANDLE = std::ptr::null_mut();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) == 0 {
            return false;
        }
        let mut elevation: TOKEN_ELEVATION = zeroed();
        let mut ret_len = 0u32;
        let ok = GetTokenInformation(
            token,
            TokenElevation,
            &mut elevation as *mut _ as *mut core::ffi::c_void,
            size_of::<TOKEN_ELEVATION>() as u32,
            &mut ret_len,
        );
        CloseHandle(token);
        ok != 0 && elevation.TokenIsElevated != 0
    }
}

#[cfg(not(windows))]
pub fn is_elevated() -> bool {
    false
}

/// Relaunch the current executable elevated via the `runas` verb (triggers UAC).
/// The caller should exit the current (non-elevated) instance afterward.
#[cfg(windows)]
pub fn relaunch_elevated() -> std::io::Result<()> {
    use std::os::windows::ffi::OsStrExt;

    use windows_sys::Win32::UI::Shell::ShellExecuteW;
    use windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

    let exe = std::env::current_exe()?;
    let verb: Vec<u16> = std::ffi::OsStr::new("runas")
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let file: Vec<u16> = exe.as_os_str().encode_wide().chain(std::iter::once(0)).collect();

    let result = unsafe {
        ShellExecuteW(
            std::ptr::null_mut(),
            verb.as_ptr(),
            file.as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            SW_SHOWNORMAL,
        )
    };

    // ShellExecuteW returns an HINSTANCE; a value <= 32 indicates failure
    // (including the user declining the UAC prompt).
    if (result as isize) <= 32 {
        return Err(std::io::Error::other(
            "elevation request failed or was declined",
        ));
    }
    Ok(())
}

#[cfg(not(windows))]
pub fn relaunch_elevated() -> std::io::Result<()> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "elevation is only supported on Windows",
    ))
}
