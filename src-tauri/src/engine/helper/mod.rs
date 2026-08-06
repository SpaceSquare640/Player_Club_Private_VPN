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
//! - Step 4 (this step, [0.39.0]): [`dispatch::WindowsDispatcher::create_adapter`]
//!   is implemented, and `tun::windows::WintunDevice::attach_existing` is
//!   its main-process counterpart.
//!
//! # The adapter-ownership question, and its actual answer
//!
//! Steps 1–3 deferred `create_adapter` because "can the helper create an
//! adapter and hand it to the main process?" was a real design question, not
//! something to guess at. The answer, from Wintun's own semantics: **no, not
//! by handing it over.** `WintunCloseAdapter` — which the `wintun` crate
//! calls from `Adapter`'s `Drop` — *removes* an adapter created with
//! `WintunCreateAdapter`. A helper that created an adapter and returned would
//! destroy it on the way out.
//!
//! What does work, and is what step 4 implements: the helper creates the
//! adapter and **holds the handle for its whole lifetime**, so the adapter
//! exists as long as the helper process does; the main process opens that
//! same adapter *by name* (`Adapter::open` → `WintunOpenAdapter`) and starts
//! its own session against it. The helper exiting is the teardown.
//!
//! # What is still unverified — and why nothing is rewired yet
//!
//! Whether the main process's `Adapter::open` + `start_session` succeeds
//! **unelevated** is the load-bearing assumption this entire architecture
//! rests on: if it needs elevation too, the helper buys nothing over the
//! existing whole-app relaunch. **This environment has no Administrator
//! privileges**, so that question is unanswered, and `WintunDevice` is
//! therefore *not* rewired to use the helper — `relaunch_elevated`'s
//! whole-app relaunch remains the only elevation path the running app
//! actually takes. Genuinely tested here: the protocol, the dispatch cycle,
//! and [`pipe`]'s same-user connect/accept over a real OS named pipe. Not
//! tested by anyone yet: a real elevation prompt, a second local account
//! being refused by the pipe's ACL, and the helper performing a real
//! privileged Windows operation.

pub mod client;
pub mod dispatch;
#[cfg(windows)]
pub mod launcher;
#[cfg(windows)]
pub mod pipe;
pub mod protocol;
pub mod server;
