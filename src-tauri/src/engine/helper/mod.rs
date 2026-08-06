//! Elevation helper (Phase E.3): the IPC protocol and (as of this step)
//! infrastructure between the main (unelevated) app and a small,
//! separately-launched elevated helper process that performs the handful of
//! privileged operations this app needs — creating the Wintun adapter,
//! `netsh`/PowerShell network integration, and OS route management (E.2) —
//! without relaunching the entire GUI elevated the way
//! [`super::tun::privilege::relaunch_elevated`] currently does.
//!
//! - Step 1 ([0.36.0]): [`protocol`] — the request/response message set and
//!   wire framing.
//! - Step 2 ([0.37.0]): [`server::run`] + [`client::HelperClient`] — the
//!   request/response cycle itself, generic over the transport, tested
//!   against an in-memory `tokio::io::duplex`. [`dispatch::WindowsDispatcher`]
//!   wires 4 of 5 operations to the real `tun::windows` functions.
//! - Step 3 (this step, [0.38.0]): [`pipe`] — the real named-pipe transport,
//!   with an owner-only security descriptor (**read [`pipe`]'s module doc
//!   comment before touching it** — this is the actual security boundary,
//!   not a formality); [`launcher`] — elevated launch of `helper.exe`
//!   (`src/bin/helper.rs`) and connecting to the pipe it creates.
//!
//! Still explicitly not done: nothing in `tun::windows` calls through the
//! helper yet (`relaunch_elevated`'s whole-app relaunch remains the only
//! elevation path in this release), and `dispatch::WindowsDispatcher::create_adapter`
//! is still deliberately unimplemented — splitting "the helper creates the
//! adapter" from "the main process opens a session against it" is a real
//! design question (Wintun sessions aren't simply handed across a process
//! boundary) that deserves its own verified step, not an improvised answer
//! written blind. **This environment has no Administrator privileges**, so
//! while [`pipe`]'s same-user connect/accept path and the protocol/dispatch
//! logic are genuinely tested against real OS primitives, the actual
//! elevation prompt, a second local account being refused by the pipe's
//! ACL, and the helper performing a real privileged operation have not been
//! exercised end to end by anyone yet.

pub mod client;
pub mod dispatch;
#[cfg(windows)]
pub mod launcher;
#[cfg(windows)]
pub mod pipe;
pub mod protocol;
pub mod server;
