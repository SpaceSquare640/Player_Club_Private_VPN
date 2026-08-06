//! The real named-pipe transport (Phase E.3, step 3) — the last piece
//! [`super::server`]/[`super::client`] needed, since both were written
//! generic over `AsyncRead`/`AsyncWrite` from the start.
//!
//! # Security — read before touching this file
//!
//! The pipe server runs **elevated**; anything that can connect to it can
//! ask it to perform privileged operations. Windows named pipes are
//! reachable by any local process unless a security descriptor says
//! otherwise, so [`owner_only_sddl`]'s `"D:(A;;GA;;;OW)"` — grant Generic
//! All to the pipe's **Owner** (the same user account both the unelevated
//! main app and the elevated helper run as; elevation is a token change, not
//! a different account) and implicitly deny everyone else — is not a
//! hardening nicety, it is *the* thing standing between this feature and a
//! local-privilege-escalation hole: any other local account being able to
//! connect and ask for `AddExtraRoutes` or a firewall rule would be exactly
//! that.
//!
//! **This has not been verified on a real elevated Windows session** — this
//! development environment has no Administrator privileges. Treat the SDDL
//! string and the `CreateNamedPipe` call it feeds as unverified until
//! someone with real elevated access confirms a second local account
//! actually gets `ERROR_ACCESS_DENIED` connecting to this pipe.

use std::ffi::c_void;
use std::io;
use std::time::Duration;

use tokio::net::windows::named_pipe::{ClientOptions, NamedPipeClient, NamedPipeServer, ServerOptions};

/// Owner-only, full control, nothing for anyone else (no explicit ACE for
/// any other principal — Windows denies by default). See the module doc
/// comment for why this is the load-bearing part of this file.
const OWNER_ONLY_SDDL: &str = "D:(A;;GA;;;OW)";

/// A fresh pipe name for one app-launch's worth of helper communication — a
/// random suffix rather than a fixed name so two instances of the app (or a
/// stale helper from a crashed prior run) can't collide on the same pipe.
pub fn pipe_name(unique_suffix: &str) -> String {
    format!(r"\\.\pipe\PlayerClubVPN-Helper-{unique_suffix}")
}

/// Builds a `SECURITY_ATTRIBUTES` wrapping [`OWNER_ONLY_SDDL`]. Returns the
/// attributes alongside the raw descriptor pointer, which the caller must
/// free with `LocalFree` once the pipe has been created (the descriptor only
/// needs to be alive for the duration of the `CreateNamedPipe` call itself —
/// the kernel copies what it needs from it).
#[cfg(windows)]
unsafe fn owner_only_security_attributes() -> io::Result<(windows_sys::Win32::Security::SECURITY_ATTRIBUTES, *mut c_void)>
{
    use windows_sys::Win32::Security::Authorization::ConvertStringSecurityDescriptorToSecurityDescriptorW;
    use windows_sys::Win32::Security::PSECURITY_DESCRIPTOR;

    let sddl: Vec<u16> = OWNER_ONLY_SDDL.encode_utf16().chain(std::iter::once(0)).collect();
    let mut descriptor: PSECURITY_DESCRIPTOR = std::ptr::null_mut();
    let ok = ConvertStringSecurityDescriptorToSecurityDescriptorW(
        sddl.as_ptr(),
        1, // SDDL_REVISION_1
        &mut descriptor,
        std::ptr::null_mut(),
    );
    if ok == 0 || descriptor.is_null() {
        return Err(io::Error::last_os_error());
    }
    let attrs = windows_sys::Win32::Security::SECURITY_ATTRIBUTES {
        nLength: std::mem::size_of::<windows_sys::Win32::Security::SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: descriptor,
        bInheritHandle: 0,
    };
    Ok((attrs, descriptor))
}

#[cfg(windows)]
unsafe fn free_security_descriptor(descriptor: *mut c_void) {
    use windows_sys::Win32::Foundation::{HLOCAL, LocalFree};
    let _ = LocalFree(descriptor as HLOCAL);
}

/// Creates the first (and, per connection, subsequent — see
/// [`ServerOptions::create`]'s docs on reuse) pipe instance at `name`,
/// restricted by [`owner_only_security_attributes`].
#[cfg(windows)]
pub fn create_server(name: &str) -> io::Result<NamedPipeServer> {
    // Safety: `attrs` (and the descriptor it points to) is kept alive across
    // the `create_with_security_attributes_raw` call below, which is the
    // only place it's read; freed immediately after, matching the API's
    // documented contract that the descriptor need not outlive the call.
    unsafe {
        let (mut attrs, descriptor) = owner_only_security_attributes()?;
        let result = ServerOptions::new().first_pipe_instance(true).create_with_security_attributes_raw(
            name,
            &mut attrs as *mut _ as *mut c_void,
        );
        free_security_descriptor(descriptor);
        result
    }
}

/// Connects to an already-created pipe, retrying while it doesn't exist yet
/// (the helper process — freshly launched, possibly still behind a UAC
/// prompt the user hasn't answered — may not have created it yet) or is
/// momentarily busy, up to `timeout`.
pub async fn connect_client(name: &str, timeout: Duration) -> io::Result<NamedPipeClient> {
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        match ClientOptions::new().open(name) {
            Ok(client) => return Ok(client),
            Err(e) if e.kind() == io::ErrorKind::NotFound || is_pipe_busy(&e) => {
                if tokio::time::Instant::now() >= deadline {
                    return Err(io::Error::new(
                        io::ErrorKind::TimedOut,
                        format!("timed out waiting for helper pipe {name}: {e}"),
                    ));
                }
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
            Err(e) => return Err(e),
        }
    }
}

fn is_pipe_busy(e: &io::Error) -> bool {
    // ERROR_PIPE_BUSY = 231 — another connection attempt is in flight; the
    // client should retry rather than treat this as a hard failure.
    e.raw_os_error() == Some(231)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pipe_name_embeds_the_suffix_and_the_expected_prefix() {
        let name = pipe_name("abc123");
        assert!(name.starts_with(r"\\.\pipe\PlayerClubVPN-Helper-"));
        assert!(name.ends_with("abc123"));
    }

    #[test]
    fn two_calls_with_different_suffixes_never_collide() {
        assert_ne!(pipe_name("one"), pipe_name("two"));
    }

    /// This is the one test in this file that runs on a real OS primitive
    /// (not a fake) — it does not require elevation (creating an
    /// owner-restricted pipe doesn't need Administrator, only the
    /// operations later performed *through* it do), so it can actually run
    /// here and prove the SDDL string is at least well-formed enough for
    /// `CreateNamedPipe` to accept.
    #[cfg(windows)]
    #[tokio::test]
    async fn create_server_with_the_owner_only_descriptor_succeeds() {
        let name = pipe_name("test-create-server");
        let server = create_server(&name);
        assert!(server.is_ok(), "pipe creation with the owner-only SDDL failed: {:?}", server.err());
    }

    /// The one test in this module that proves the full stack over a *real*
    /// OS pipe, not `tokio::io::duplex`: `create_server` + `connect_client`
    /// + `super::super::server::run` + `HelperClient`, end to end, using the
    /// `FakeDispatcher` so no actual privileged operation runs. This is real
    /// signal (same-process, same-user connect/accept genuinely works over
    /// this transport) but it is **not** a test of the access-control
    /// boundary itself — proving a *different* local account is refused
    /// needs a real multi-account elevated setup this environment doesn't
    /// have. See the module doc comment.
    #[cfg(windows)]
    #[tokio::test]
    async fn client_and_server_communicate_over_a_real_named_pipe() {
        use crate::engine::helper::dispatch::test_support::FakeDispatcher;
        use crate::engine::helper::protocol::{HelperRequest, HelperResponse, HELPER_PROTOCOL_VERSION};

        let name = pipe_name("test-round-trip");
        let server_pipe = create_server(&name).unwrap();

        let dispatcher = FakeDispatcher::default();
        let dispatcher_clone = dispatcher.clone();
        let server_task = tokio::spawn(async move {
            server_pipe.connect().await.unwrap();
            let (read_half, write_half) = tokio::io::split(server_pipe);
            super::super::server::run(read_half, write_half, dispatcher_clone).await
        });

        let client_pipe = connect_client(&name, Duration::from_secs(5)).await.unwrap();
        let mut client = super::super::client::HelperClient::new(client_pipe);

        let resp = client
            .request(&HelperRequest::ConfigureNetworkIntegration {
                v: HELPER_PROTOCOL_VERSION,
                adapter_name: "PlayerClubVPN".to_string(),
            })
            .await
            .unwrap();
        assert_eq!(resp, HelperResponse::Ok);
        assert_eq!(dispatcher.calls.lock().unwrap().len(), 1);

        let resp = client.request(&HelperRequest::Shutdown { v: HELPER_PROTOCOL_VERSION }).await.unwrap();
        assert_eq!(resp, HelperResponse::Ok);

        tokio::time::timeout(Duration::from_secs(5), server_task)
            .await
            .expect("server task did not exit after Shutdown")
            .unwrap()
            .unwrap();
    }
}
