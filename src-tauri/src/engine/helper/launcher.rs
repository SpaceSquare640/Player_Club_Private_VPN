//! Elevated launch of `helper.exe` from the main (unelevated) process —
//! the same `ShellExecuteW … "runas"` mechanism
//! `tun::privilege::relaunch_elevated` already uses to relaunch the whole
//! app, aimed at the much smaller helper binary instead.
//!
//! **Unverified in this environment** — no Administrator privileges here to
//! confirm a real UAC prompt appears, that a user declining it surfaces as
//! the expected error, or that the freshly-elevated helper can actually
//! create the pipe and accept the connection this function then waits for.
//! See `engine::helper`'s module doc comment.
#![allow(dead_code)]

use std::io;
use std::path::PathBuf;
use std::time::Duration;

use rand::RngCore;
use tokio::net::windows::named_pipe::NamedPipeClient;

use super::client::HelperClient;
use super::pipe;

/// How long to wait for the elevated helper to create its pipe and accept a
/// connection after the UAC prompt is approved. Generous — the bottleneck is
/// the user answering the prompt, which this can't distinguish from "taking
/// its time to start," so a short timeout would fail interactions that are
/// simply waiting on the user, not actually stuck.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(60);

/// Locates `helper.exe` next to the currently running executable — the
/// layout `cargo build`/Tauri's bundler both produce (every binary target in
/// this package lands in the same output directory).
fn helper_exe_path() -> io::Result<PathBuf> {
    let exe = std::env::current_exe()?;
    let dir = exe.parent().ok_or_else(|| io::Error::other("current executable has no parent directory"))?;
    helper_exe_in(dir)
}

/// `helper_exe_path`'s logic, parameterized on the directory to search — so
/// tests can point it at a directory guaranteed not to contain `helper.exe`
/// instead of depending on whatever `cargo test`'s own output layout happens
/// to be (which does, in practice, sometimes place every binary target
/// alongside the test harness, making that outcome environment-dependent
/// rather than a property of this function).
fn helper_exe_in(dir: &std::path::Path) -> io::Result<PathBuf> {
    let candidate = dir.join("helper.exe");
    if !candidate.exists() {
        return Err(io::Error::new(io::ErrorKind::NotFound, format!("{} not found", candidate.display())));
    }
    Ok(candidate)
}

/// A random-enough suffix to keep this app-launch's pipe name from colliding
/// with another instance's or a stale one's — not a security boundary (the
/// SDDL in `pipe` is), just collision avoidance.
fn random_suffix() -> String {
    let mut bytes = [0u8; 8];
    rand::thread_rng().fill_bytes(&mut bytes);
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Launches `helper.exe` elevated (triggers a UAC prompt) and connects to
/// the pipe it creates. Returns the connected client, or an error if the
/// binary can't be found, the user declines elevation, or the connection
/// doesn't complete within [`CONNECT_TIMEOUT`].
#[cfg(windows)]
pub async fn launch_and_connect() -> io::Result<HelperClient<NamedPipeClient>> {
    use std::os::windows::ffi::OsStrExt;

    use windows_sys::Win32::UI::Shell::ShellExecuteW;
    use windows_sys::Win32::UI::WindowsAndMessaging::SW_HIDE;

    let helper_path = helper_exe_path()?;
    let name = pipe::pipe_name(&random_suffix());

    let verb: Vec<u16> = std::ffi::OsStr::new("runas").encode_wide().chain(std::iter::once(0)).collect();
    let file: Vec<u16> = helper_path.as_os_str().encode_wide().chain(std::iter::once(0)).collect();
    let params: Vec<u16> = std::ffi::OsStr::new(&name).encode_wide().chain(std::iter::once(0)).collect();

    // SW_HIDE: the helper has no window of its own to show (see the
    // `windows_subsystem = "windows"` attribute on `src/bin/helper.rs`'s
    // release build) — this only affects the console window a debug build
    // would otherwise briefly flash.
    let result = unsafe {
        ShellExecuteW(std::ptr::null_mut(), verb.as_ptr(), file.as_ptr(), params.as_ptr(), std::ptr::null(), SW_HIDE)
    };
    if (result as isize) <= 32 {
        return Err(io::Error::other("helper elevation request failed or was declined"));
    }

    pipe::connect_client(&name, CONNECT_TIMEOUT).await.map(HelperClient::new)
}

#[cfg(not(windows))]
pub async fn launch_and_connect() -> io::Result<HelperClient<NamedPipeClient>> {
    Err(io::Error::new(io::ErrorKind::Unsupported, "the elevation helper is only supported on Windows"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn random_suffix_is_hex_and_practically_never_collides() {
        let a = random_suffix();
        let b = random_suffix();
        assert_eq!(a.len(), 16); // 8 bytes, 2 hex chars each
        assert!(a.chars().all(|c| c.is_ascii_hexdigit()));
        assert_ne!(a, b);
    }

    #[test]
    fn helper_exe_in_reports_not_found_when_the_binary_is_absent() {
        let err = helper_exe_in(std::env::temp_dir().as_path()).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::NotFound);
    }

    #[test]
    fn helper_exe_in_finds_a_binary_actually_present() {
        let dir = std::env::temp_dir().join(format!("pcpv_helper_launcher_test_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let fake_helper = dir.join("helper.exe");
        std::fs::write(&fake_helper, b"not a real binary, just needs to exist").unwrap();

        assert_eq!(helper_exe_in(&dir).unwrap(), fake_helper);

        let _ = std::fs::remove_dir_all(&dir);
    }
}
