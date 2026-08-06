//! Elevation helper (Phase E.3, step 1 of 2): the IPC protocol between the
//! main (unelevated) app and a small, separately-launched elevated helper
//! process that performs the handful of privileged operations this app
//! needs — creating the Wintun adapter, `netsh`/PowerShell network
//! integration, and OS route management (E.2) — without relaunching the
//! entire GUI elevated the way [`super::tun::privilege::relaunch_elevated`]
//! currently does.
//!
//! This step is protocol only: [`protocol`] defines the request/response
//! message set and a wire framing, with round-trip tests. The named-pipe
//! server/client that actually launches a helper process, talks this
//! protocol over it, and the corresponding `helper.exe` binary that runs the
//! privileged side are a later step — deliberately not started here, since
//! this environment has no Administrator privileges to verify any of that
//! end to end. See the [0.36.0] changelog entry for the scoping rationale.

pub mod protocol;
