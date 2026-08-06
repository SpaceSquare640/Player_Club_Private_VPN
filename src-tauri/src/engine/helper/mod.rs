//! Elevation helper (Phase E.3, step 1 of 2): the IPC protocol between the
//! main (unelevated) app and a small, separately-launched elevated helper
//! process that performs the handful of privileged operations this app
//! needs — creating the Wintun adapter, `netsh`/PowerShell network
//! integration, and OS route management (E.2) — without relaunching the
//! entire GUI elevated the way [`super::tun::privilege::relaunch_elevated`]
//! currently does.
//!
//! Step 1 ([0.36.0]) defined [`protocol`]: the request/response message set
//! and wire framing. This step ([0.37.0]) adds the request/response cycle
//! itself — [`server::run`] dispatches decoded requests to a
//! [`dispatch::HelperDispatcher`] and writes back replies; [`client::HelperClient`]
//! is the other end. Both are generic over the transport (`AsyncRead`/
//! `AsyncWrite`), so the full cycle is tested against an in-memory
//! `tokio::io::duplex` — no real pipe, process, or elevation involved.
//!
//! Still explicitly not done: a real named-pipe transport, the `helper.exe`
//! binary that would run [`server::run`] against one, an elevated-launch
//! path for it, and — even once those exist — wiring `tun::windows` to
//! actually use the helper instead of calling its own functions directly
//! (`relaunch_elevated`'s whole-app relaunch remains the only elevation path
//! today). `dispatch::WindowsDispatcher::create_adapter` is deliberately
//! left unimplemented for the same reason: splitting "the helper creates the
//! adapter" from "the main process opens a session against it" is a real
//! design question (Wintun sessions aren't simply handed across a process
//! boundary) that deserves its own verified step, not an improvised answer
//! here. This environment has no Administrator privileges to verify any of
//! that end to end.

pub mod client;
pub mod dispatch;
pub mod protocol;
pub mod server;
