# Changelog

All notable changes to **Player Club Private VPN** are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [Unreleased]

### Planned
- **Split tunneling — OS route management (E.2's original, narrower scope shipped instead):** steering additional prefixes into the adapter via `netsh`, Windows + elevation.
- **Elevation:** privileged helper-service backend (replacing the relaunch-elevated path behind the existing seam).
- **Site-to-site LAN sharing:** deferred — unverifiable without 3 physical hosts.

---

## [0.43.0] - 2026-08-09

### Changed
**Full UI redesign.** Users reported the app "looks ugly" and has design problems. Audit found the code itself was clean (no gradients, no arbitrary z-index, decent accessibility) — the real cause was that every card/button/panel was hand-rolled Tailwind duplicated near-verbatim across every page, with no shared primitives to keep them in sync, plus a genuine WCAG contrast failure on primary buttons in 4 of the 6 themes.

- **New shared primitive layer** (`src/components/ui/`): `Card`, `Button` (primary/secondary/ghost/danger/warning), `Toggle` (a real switch — `role="switch"`, `aria-checked`, replacing the old "button whose border changes color and says On/Off" pattern), `Badge`, `StatTile` (promoted out of `Diagnostics.tsx`), `IconButton`. Every page and panel re-skinned onto these instead of inline Tailwind strings.
- **Fixed a real accessibility bug, not just a look-and-feel one:** the primary `Button` variant used `text-white` on `bg-brand-violet`, which measured as low as 2.54:1 contrast (WCAG AA needs 4.5:1) in Aurora, and failed in Carbon/Abyss/Ember too — several theme accents are light/pastel violets that don't work with white text. Fixed by switching to each theme's own dark `surface` color as the button text color (token-driven, not a hardcoded fix), which measures 4.5–7.4:1 across all 6 themes.
- **`Dashboard.tsx` rebuilt from scratch** — replaced the scaffolding-era "ping engine" button and hardcoded fake status ("idle", "0 peers connected") with a real overview: live engine status (from the same `telemetryStore` already wired app-wide), quick-action shortcuts to Network/Diagnostics/Minecraft, and a recent-activity feed from the live packet log. No new backend calls — reuses what `useEngineTelemetry` already exposes.
- Consolidated on **one accent color per view** (violet) where panels had previously mixed violet + cyan + amber button colors with no semantic reason for the split (e.g. `PeerConnectionPanel`'s Create Offer vs. Process buttons).
- Standardized page-title sizing (`Network`/`Diagnostics` were `text-xl` while `Dashboard`/`Minecraft` were `text-2xl` for the same role) and added `text-balance`/`text-pretty` to headings/body copy per the project's UI baseline.
- Verified with a live audit of all 6 theme palettes' actual contrast ratios (computed WCAG, not eyeballed) rather than assuming they were fine.

All existing `data-testid`s, i18n keys, and component logic were preserved by design; the only test changes were 3 assertions in `SettingsOverlay.test.tsx` updated from `aria-pressed` to `aria-checked` to match the new `Toggle`'s corrected switch semantics. Full suite: 156/156.

---

## [0.42.1] - 2026-08-09

### Fixed
**Critical: the v0.42.0 installers on all three platforms bundled the wrong binary.** `src-tauri/Cargo.toml` has two binary targets — the main GUI app (`src/main.rs`) and the elevation helper (`src/bin/helper.rs`) — and had no `default-run` set. Without it, Tauri's bundler can't reliably tell which compiled binary is "the app," and in the v0.42.0 release build it guessed wrong: the installers packaged `helper.exe` (a small utility that just prints a message and exits when run outside the elevation flow) instead of the real application. Installing v0.42.0 and launching it did nothing, because there was no GUI binary in the package to launch.

A user reported this directly — installed v0.42.0 on Windows, the Start Menu shortcut launched nothing, and the install directory turned out to contain only `helper.exe` and `uninstall.exe`, no main executable. Root-caused via the release workflow's build log (`Built application at: ...\helper.exe`, confirming the bundler's own binary selection was wrong, not an antivirus or install corruption issue) and fixed by adding `default-run = "player-club-private-vpn"` to `[package]` in `Cargo.toml` — the documented fix for this exact multi-binary-crate ambiguity in Tauri.

**Verified, not just fixed:** ran a full local `pnpm tauri build` after the change and confirmed the build log now reads `Built application at: ...\player-club-private-vpn.exe`, then launched the resulting binary directly and confirmed the process starts and stays running, before cutting this release.

**v0.42.0 is broken on all three platforms and should not be used** — delete/skip it in favor of this release.

---

## [0.42.0] - 2026-08-09

### Added
**Real TUN backends for Linux and macOS.** `src-tauri/src/engine/tun/` no longer stubs out `open_device` on non-Windows targets — it now has a real implementation for all three platforms, selected at compile time in `mod.rs`:

- **Linux (`linux.rs`)** — opens `/dev/net/tun` directly via `open(2)` + the `TUNSETIFF` ioctl (`IFF_TUN | IFF_NO_PI`), non-blocking so `read_frame` matches the poll-style contract the rest of the engine already relies on. IP address, MTU, and link-up state are set via `ip` (iproute2), mirroring how the Windows backend shells out to `netsh`.
- **macOS (`macos.rs`)** — creates a `utun` interface via the `PF_SYSTEM`/`SYSPROTO_CONTROL` kernel-control socket (`CTLIOCGINFO` to resolve `"com.apple.net.utun_control"`, then probing `sc_unit` values until one connects — the same approach WireGuard's own macOS backend uses). Strips/adds the 4-byte address-family header every `utun` frame carries, so the rest of the engine still only ever sees bare IP packets. IP/MTU are set via `ifconfig`.
- **Privilege detection** (`privilege.rs`) — `is_elevated()` now does a real effective-UID check (`libc::geteuid() == 0`) on Linux/macOS instead of hardcoding `false`; `can_create_tun` reflects that. One-click `relaunch_elevated()` remains Windows-only for now (no single cross-distro/version-safe equivalent to UAC exists) — see `PLATFORM-SUPPORT.md`.

A shared `prefix_to_mask` helper moved from `windows.rs` to `device.rs` so the macOS backend (which also needs a dotted-decimal mask for `ifconfig`) doesn't duplicate it.

**Honesty about verification status:** the Windows backend has been exercised end-to-end; Linux and macOS have not — no one has yet confirmed either actually creates a working adapter and carries traffic against a live peer on physical hardware, only that the code compiles and its pure-logic unit tests (ioctl/struct-layout math, name packing) pass in CI. `PLATFORM-SUPPORT.md`, the release-notes disclaimer, and the README were all updated to say "implemented, unverified" rather than "not implemented" — a real distinction, not a downgrade in caution.

**`.github/workflows/ci.yml`** — new workflow, separate from the release pipeline: runs `cargo check --all-targets` + `cargo test` on `windows-latest`/`ubuntu-latest`/`macos-latest`, plus the frontend type-check + test suite, on every push to `main`. This is what actually verifies the new Linux/macOS code compiles — the author's own machine is Windows-only and cannot cross-compile either backend locally.

**In-app "About & Legal" links.** The Settings overlay has a new section linking to the User Manual, Terms of Service, and Privacy Policy — opened in the system browser via the new `tauri-plugin-opener` dependency, not the app's own webview. Links follow the selected UI language where a translation exists (Traditional Chinese has one; Simplified Chinese falls back to English rather than link to a page that doesn't exist).

### Fixed
**Live-settings-change race in the connection driver.** `pipeline.rs`'s `drive()` loop now uses a `biased` `tokio::select!` with a deliberately-ordered branch priority: `cancel` first (shutdown must never be starved by a busy link), then `settings_rx` (must win its specific race against the uplink packet branch — a settings change and a freshly-arrived packet could previously tie, and the default random tie-break could apply the packet under the stale policy), then everything else. This was the cause of an intermittent CI failure on `a_live_settings_change_applies_to_an_already_connected_link`. Verified with the full suite run three times and the previously-flaky test run 20 times, all clean, before pushing.

### Documentation
Updated `PLATFORM-SUPPORT.md` (English + Traditional Chinese), `README.md` (+ zh-Hant), and the release workflow's auto-generated disclaimer to reflect "implemented but unverified" rather than "Windows-only." Trimmed the Wiki User Manual to pure step-by-step instructions, moving *why*-explanations to Architecture/FAQ.

---

## [0.41.1] - 2026-08-08

### Added
**CI release pipeline, three platforms.** `.github/workflows/release.yml` now matrix-builds installers on every `v*` tag push — Windows (`.msi`, NSIS `.exe`), Linux (`.deb`, `.AppImage`), and macOS (`.dmg`) — via `tauri-apps/tauri-action`, and publishes them to a single **draft** GitHub Release for review before going public.

A `notes` job runs first: it extracts the matching `## [version]` section straight out of this file and prepends a platform-support disclaimer, so every release's description is generated automatically — no manual step, and no more forgetting to write one.

**`PLATFORM-SUPPORT.md`** — the real virtual adapter (`src-tauri/src/engine/tun/mod.rs`) is still `#[cfg(windows)]`-only, so this file makes the Linux/macOS gap explicit: installers exist for all three platforms, but only Windows has a working tunnel today. The same disclaimer is stamped into every release's notes automatically, not just the docs.

### Documentation
Rewrote `README.md`, `LICENSE`, and `SECURITY.md`; added `TERMS_OF_SERVICE.md` and `PRIVACY_POLICY.md`; added GitHub Issue and Discussion templates; set repository About/Topics; and moved the detailed build-status table, full feature list, architecture diagram, and project structure out of the README and into the GitHub Wiki, alongside a new step-by-step **User Manual** page. README is now a short overview + quick start that links out to the Wiki for everything else.

---

## [0.41.0] - 2026-08-06

### Added
**Test coverage for the app shell's routing** — `Sidebar`, `Breadcrumb`, and `AppShell` had none. That gap became concrete in `[0.40.1]`: migrating `react-router-dom` → `react-router` v8 crossed a major version, and the only thing verifying navigation still worked was a manual browser walkthrough. Manual checks don't persist; the next router bump would have had nothing guarding it. 23 new tests (156 total, up from 133):

- **`Sidebar`** — every nav item renders, each one navigates to its *exact* path (asserting the argument, not just that a click fired — a wrong path is precisely the failure a router migration produces while everything still renders), active state is marked via both `aria-current` and `data-active`, the settings button opens the overlay without navigating, and buttons carry real translated `aria-label`s rather than raw i18n keys.
- **`Breadcrumb`** — each `RouteId` renders a genuinely translated label. A missing translation surfaces as the raw key (`"nav.network"`), which a plain text assertion would happily pass, so the test explicitly rejects any output containing `"nav."`.
- **`AppShell`** — mounted on a **real** `createMemoryRouter`/`RouterProvider`, not mocked, because that integration is exactly what a major-version bump breaks: URL→store sync for all four routes, page and breadcrumb rendering together, the last-route restore on launch, and — the case worth pinning — an explicit deep link winning over the persisted route, so opening the app *at* a route doesn't bounce you to wherever you were last time.

### Verified
- `pnpm test`: 156/156 passing; `tsc --noEmit` clean; `cargo check --bins --lib` zero warnings.
- **Mutation-tested the new tests rather than trusting a green run.** Two deliberate defects were injected — deleting AppShell's `/minecraft` path mapping, and pointing Sidebar's network item at a wrong path — and confirmed to fail exactly 3 tests, in exactly the relevant files, before being reverted. A test that cannot fail is not a guard, and a green suite alone doesn't distinguish the two.

## [0.40.1] - 2026-08-06

### Security
Cleared every known dependency vulnerability on both sides of the stack — **9 total (5 high, 4 moderate)**, which GitHub had been flagging on the default branch for some time. For a VPN, shipping known-vulnerable dependencies is not a cosmetic issue, so this took priority over new features.

**JavaScript — 7 → 0** (`pnpm audit`):
- `react-router` (5 advisories: an unauthenticated DoS, an open redirect via backslash in `<Link>`/`useNavigate`, arbitrary constructor injection, an `RSCErrorHandler` protocol gap, and an RSC-mode CSRF bypass).
- `postcss` ×2 (path traversal via a previous source map, plus an incomplete-fix follow-up). This one needed no version bump of `vite` at all — vite 6.4.3 already allows `^8.5.3`; the lockfile was simply pinned to a stale 8.5.15. Re-resolving moved it to 8.5.25.

**Rust — 2 → 0** (`cargo audit`):
- `quick-xml` ×2 (both 7.5 high: quadratic runtime on duplicate attribute names, and unbounded namespace-declaration allocation enabling memory-exhaustion DoS). Reached via `plist` → `tauri-utils`; updating `plist` 1.9.0 → 1.10.0 pulled `quick-xml` 0.39.4 → 0.41.0. No Tauri version change was needed.
- `anyhow` 1.0.102 → 1.0.104 additionally cleared an `Error::downcast_mut()` unsoundness warning.

### Changed
- **Migrated `react-router-dom` → `react-router` v8.3.0.** The last remaining advisory (RSC-mode CSRF bypass) is only patched in `react-router` ≥8.3.0, but `react-router-dom@7.18.2` hard-pins `react-router@7.18.2`, so no amount of updating `react-router-dom` could reach it — and `react-router-dom` has no 8.x release at all. Since v7 merged the DOM exports into the main package, `react-router-dom` is now a thin shim, and the migration was a contained one: five APIs (`createHashRouter`, `RouterProvider`, `Outlet`, `useLocation`, `useNavigate`) across four files, changing only the import specifier.

### Verified
- `pnpm audit` and `cargo audit`: **no known vulnerabilities** on either side.
- `pnpm test` 133/133; `cargo test --lib` 139/139; `tsc --noEmit` clean; `pnpm build` succeeds; `cargo check --bins --lib` zero warnings.
- Browser-verified the router v8 migration specifically, since a major-version bump is exactly where routing would break silently: navigated all four routes, confirming the hash URL, the lazy-loaded page, and the breadcrumb all update together. Zero console errors. (One intermediate probe appeared to show a stale breadcrumb — that was a 250 ms snapshot taken before React had re-rendered, not a defect; the settled state was correct.)

### Remaining audit warnings — not vulnerabilities, and why they stay
`cargo audit` still reports 17 `unmaintained` and 1 `unsound` **warnings** (it exits clean; these are not advisories against us):
- 14 are the **gtk-rs GTK3 bindings** (`gtk`, `gdk*`, `atk*`, `glib`) that Tauri pulls in for its **Linux** backend. This project targets Windows; they are never compiled into a shipped artifact here. They are Tauri's to replace, not ours.
- The rest (`unic-*`, `instant`, `proc-macro-error`) are build-time/transitive and have no upstream replacement available at our dependency depth.
Recording these rather than suppressing them: an empty audit achieved by an ignore-list would be less honest than a clean one with a documented tail.

## [0.40.0] - 2026-08-06

### Added
- **Live split-tunnel toggles (Phase B.4)** — flipping broadcast or multicast forwarding now takes effect on an **already-connected link**, with no reconnect. This closes the limitation recorded in `[0.15.0]`, which deferred it as needing "a control channel into the running pipeline"; that channel is a `tokio::sync::watch` carrying `ConnectionSettings`, matching how cancellation is already plumbed through this codebase.
- `ConnectionManager::update_settings` pushes to every live peer, and the `update_connection_settings` Tauri command exposes it. `SplitPolicy` gained in-place `set_forward_broadcast`/`set_forward_multicast` so a running link's policy can be adjusted without rebuilding it from its `TunConfig`.
- `hooks/useLiveConnectionSettings` — mounted app-wide in `AppShell`, alongside the telemetry subscription and i18n sync, for the same reason: it must outlive whichever page is showing.

### Scope — and why only two of the four settings are live
- **Broadcast/multicast are pure local packet filtering.** Flipping one changes only what this side forwards; the peer neither knows nor cares. Safe to change mid-session.
- **FEC redundancy (`r`) is not**, and deliberately stays connect-time: it is a wire-format agreement with the peer, so changing it unilaterally would desynchronise the encoder and decoder. Making it live means renegotiating with the peer — a different feature.
- **Extra routed networks are not**, for a different reason: they mutate the OS routing table and need elevation.
- The frontend sends the whole `ConnectionSettings` object anyway and the engine ignores what it can't apply. A hand-curated "live subset" on the frontend would silently drift out of sync with the type as it grows.

### Design note — watching values, not buttons
- The live push is keyed on the settings *values*, not wired into the Settings overlay's toggle handlers. An earlier draft did the latter and would have silently missed two other writers of the same store fields: the Minecraft page's preset button and JSON profile import. Both should reach a live link, and now do. There is a regression test for exactly this.

### Verified
- `cargo test --lib`: 139/139 passing (1 new integration test): with a pair connected over loopback, a broadcast crosses, `forward_broadcast: false` is pushed into the *running* link, and a subsequent broadcast is dropped while a unicast still crosses. The trailing unicast is what makes it a real assertion rather than a race — it is sent after the broadcast, so its arrival proves the pipeline processed both and the broadcast's absence was a decision, not a packet still in flight. Re-ran 5× to confirm it isn't flaky.
- `cargo check --bins --lib`: zero warnings.
- `pnpm test`: 133/133 passing (6 new): the `updateConnectionSettings` wrapper's invoke shape, and the hook pushing on mount, pushing on a toggle change, **pushing for a non-Settings-originated store write** (the guard described above), not re-pushing when nothing changed, and swallowing a rejected push.
- `tsc --noEmit` clean; `pnpm build` succeeds.
- Browser-verified: with the hook mounted, changing settings at runtime produced zero console errors — the mount-time and change-time pushes both fail silently in a browser preview (no Tauri runtime), which is the designed behavior.

### Note on this release's provenance
- The Rust half of this phase was written in a prior session that was interrupted before committing. It was found uncommitted in the working tree, reviewed, and verified from scratch here rather than rewritten — the frontend half, its tests, and the hook extraction are new in this session.

## [0.39.0] - 2026-08-06

### Added
- **`create_adapter` implemented, and its main-process counterpart (Phase E.3, step 4 of 5).** `dispatch::WindowsDispatcher` is now stateful: it loads `wintun.dll` on first use and **holds every adapter it creates** for the helper process's lifetime. `tun::windows::WintunDevice::attach_existing` is the other half — opens an already-existing adapter *by name* (`Adapter::open`) and starts a session against it, performing no privileged setup of its own.
- `WintunDevice` gained an `owns_integration` flag. `attach_existing` sets it false so `Drop` skips firewall/route teardown — that setup belongs to the helper, and undoing it from the main process would break a still-running helper session rather than merely duplicating work.

### The design question steps 1–3 deferred, and its actual answer
- Those steps declined to implement `create_adapter` because "can the helper create an adapter and hand it to the main process?" was a real question, not something to guess at. Investigating Wintun's semantics gave a concrete answer: **no, not by handing it over.** `WintunCloseAdapter` — which the `wintun` crate calls from `Adapter`'s `Drop` — *removes* an adapter created via `WintunCreateAdapter`. A helper that created one and returned would destroy it on the way out.
- What works instead, and what this ships: the helper creates the adapter and holds the handle for its whole lifetime (so the adapter lives as long as the helper does); the main process opens that same adapter by name and starts its own session. The helper exiting is the teardown. A duplicate-name guard rejects a second `create_adapter` for a name already held, rather than orphaning the first handle.

### Scope — and the one thing still blocking this feature
- **`WintunDevice` is still not rewired to use the helper**, and `relaunch_elevated`'s whole-app relaunch remains the only elevation path the running app actually takes. The blocker is specific and load-bearing: whether the main process's `Adapter::open` + `start_session` succeeds **unelevated**. If it needs elevation too, the helper buys nothing over the existing relaunch and the approach has to change. This environment has no Administrator privileges to answer that, and wiring the app over to an unverified assumption would be building on sand — so step 5 is marked blocked, not planned.
- A test for the duplicate-adapter guard was written and then **removed rather than shipped**: `wintun::Adapter` has no constructor that avoids touching the driver, so the test needed `unreachable!()` scaffolding that would have asserted nothing real. A fake test is worse than no test.

### Verified
- `cargo test --lib`: 138/138 passing (unchanged count — this phase added no new testable-without-elevation surface, which is itself the honest signal about where its risk sits).
- `cargo check --bins --lib`: zero warnings, including the `helper.exe` target.
- Not verified by anyone yet: a real elevation prompt, a second local account being refused by the pipe ACL, the helper actually creating an adapter, and — the decisive one — unelevated `Adapter::open` + `start_session`.

## [0.38.0] - 2026-08-06

### Added
- **Real named-pipe transport + `helper.exe` binary (Phase E.3, step 3 of 4).** `engine::helper::pipe` wraps `tokio::net::windows::named_pipe`, restricted by an **owner-only security descriptor** (`"D:(A;;GA;;;OW)"` — Generic All to the pipe's Owner, implicitly denying every other local principal). This is the actual security boundary of the whole feature: the helper runs elevated, so anything that can connect to its pipe can ask it to perform privileged operations. `src-tauri/src/bin/helper.rs` is a new, separate binary target — `helper.exe <pipe-name>` creates that pipe, accepts exactly one connection, and runs `server::run` with the real `WindowsDispatcher` against it. `engine::helper::launcher::launch_and_connect` is the main-app side: elevates `helper.exe` via the same `ShellExecuteW … "runas"` mechanism `tun::privilege::relaunch_elevated` already uses (aimed at the small helper binary instead of the whole GUI), then connects.
- `engine::pub mod` — `lib.rs`'s `engine` module is now `pub` so `helper.exe`, a separate binary target in this same package, can reach `engine::helper` (this crate is never published; the visibility widening is scoped to this package's own binaries, not an external API).

### Scope
- **Still not wired into `WintunDevice::open`** — `relaunch_elevated`'s whole-app relaunch remains the only elevation path exercised by the running app in this release. `dispatch::WindowsDispatcher::create_adapter` is still the one unimplemented operation, for the reason stated in steps 1–2: splitting adapter creation (helper) from session start (main process) is a real design question, not something to improvise. That integration is step 4.
- Honest split on what could and couldn't be verified here (no Administrator privileges in this environment): pipe creation with the owner-only descriptor, and a full client↔server round trip over a **real** OS named pipe (not `tokio::io::duplex`), both actually ran and passed — that's genuine signal the transport works for same-user connect/accept. What did **not** run: an elevation prompt actually appearing, a *different* local account being refused by the pipe's ACL, or the helper performing a real privileged Windows operation. Anyone continuing this work on a real elevated machine should treat the ACL specifically as unverified until confirmed.

### Verified
- `cargo test --lib`: 138/138 passing (7 new): `pipe_name` produces non-colliding names, `create_server` with the owner-only descriptor succeeds against the real Windows named-pipe API, a full `HelperClient`↔`server::run` round trip over that real pipe works end to end (using `FakeDispatcher`, so no privileged operation actually runs), and the launcher's binary-location logic is tested against a real (temp-directory) present/absent binary rather than depending on `cargo test`'s own output layout (which, in practice, sometimes places every binary target next to the test harness — an early version of this test assumed it never would, and failed on this very machine).
- `cargo check --bins --lib`: zero warnings, including the new `helper.exe` binary target.
- `helper.exe` built and smoke-tested directly: running it with no arguments prints the usage message and exits non-zero, as designed.
- No frontend changes in this phase.

## [0.37.0] - 2026-08-06

### Added
- **Elevation helper dispatch cycle (Phase E.3, step 2 of 3).** `engine::helper::server::run` reads `HelperRequest` lines, dispatches each to a `HelperDispatcher`, writes back a `HelperResponse` — generic over `AsyncRead`/`AsyncWrite`, so it's exercised in tests against an in-memory `tokio::io::duplex`, no real pipe or elevation involved. `engine::helper::client::HelperClient` is the other end of the same cycle, tested as a full round trip against the real server loop (not a hand-rolled stand-in for it).
- `engine::helper::dispatch::HelperDispatcher` trait + `WindowsDispatcher`: 4 of the 5 protocol operations now call the real `tun::windows` functions (`configure_network_integration`, `remove_network_integration`, `add_extra_routes`, `remove_extra_routes` — all newly `pub(crate)` for this). `create_adapter` deliberately still returns an explicit "not yet implemented" error — see Scope below.
- `dispatch::test_support::FakeDispatcher` — records every call instead of touching the OS, letting the server/client tests assert exactly what got dispatched with what arguments, and control success/failure per call, without Windows or elevation.

### Scope
- **`create_adapter` is the one operation not wired to anything real.** A Wintun session (what `WintunDevice::open` ultimately needs) isn't simply created in one process and handed to another — splitting "the helper creates the adapter" from "the main process opens a session against it" is a real design question that deserves its own verified step, not an improvised answer written blind. It returns a clear error rather than silently doing nothing or guessing at an implementation.
- Still no real named-pipe transport, no `helper.exe` binary, no elevated-launch path, and nothing in `tun::windows` calls through the helper yet — `relaunch_elevated`'s whole-app relaunch remains the only elevation path in this release. That's step 3, deferred for the same reason as before: this environment has no Administrator privileges to verify any of it against.

### Verified
- `cargo test --lib`: 131/131 passing (18 new across `server`/`client`): the loop dispatches each request type to the right `HelperDispatcher` method with the right arguments, a dispatcher error becomes an `Error` response, an unsupported version is reported *without* dispatching, malformed input gets an error reply and the loop keeps running (proven by a subsequent real request still getting a real reply), `Shutdown` ends the server task after replying (not before — and not hanging), the client gets its own reply for each of several sequential requests, and the client surfaces a clear I/O error when the other end of the connection disappears mid-request.
- `cargo check`: zero warnings (both new modules carry the same documented `#![allow(dead_code)]` posture as every other not-yet-wired-in module this project has shipped incrementally).
- No frontend changes in this phase.

## [0.36.0] - 2026-08-06

### Added
- **Elevation helper protocol (Phase E.3, step 1 of 2).** `engine::helper::protocol` defines the closed message set a future elevated helper process will accept — `CreateAdapter`, `ConfigureNetworkIntegration`, `RemoveNetworkIntegration`, `AddExtraRoutes`, `RemoveExtraRoutes`, `Shutdown` — each a 1:1 counterpart to an existing `tun::windows` function, and nothing broader (the helper is not a general-purpose remote shell). Framing is newline-delimited JSON, reusing the convention `signaling::protocol` already established for a similar problem (typed request/response over a long-lived stream) rather than inventing a second one.
- `HelperResponse::UnsupportedVersion` — a request whose `v` the helper doesn't understand gets a protocol-level reply, not a transport-level failure, so a future version mismatch is something the caller can react to distinctly from "the pipe broke."

### Scope
- **Protocol only.** No named-pipe server/client, no `helper.exe` binary, and nothing in `tun::windows` calls through this yet — `relaunch_elevated`'s whole-app relaunch remains the only elevation path in this release. Explicitly the first of two steps (see `engine::helper`'s module doc comment): building and wiring the actual IPC transport is deferred to when real Administrator-privileged verification is available, which this environment does not have.

### Verified
- `cargo test --lib`: 121/121 passing (8 new): every request and response variant round-trips through encode/decode, a version-mismatch response round-trips with its `supported` value, malformed JSON is rejected, a response-shaped line fails to decode as a request and vice versa, a trailing `\r` (as a CRLF-terminated pipe might leave) is tolerated, and — the one genuine bug this phase's own tests caught — encoding is proven to never produce an embedded newline even when the input (e.g. an adapter name) contains one, because `serde_json` escapes control characters inside strings. (The first draft of this test asserted a defensive `encode` rejection that could never fire, since JSON string escaping already makes the newline impossible to produce — removed in favor of testing the actual invariant.)
- `cargo check`: zero warnings (`#![allow(dead_code)]` on the new module, same posture as every other not-yet-wired-in module this project has shipped incrementally — G.1–G.3, mesh.rs).
- No frontend changes in this phase.

## [0.35.0] - 2026-08-06

### Added
- **OS route management (E.2's remaining scope).** `ConnectionSettings.extraRoutes` (a comma-separated list of `"address/prefix"` entries in Settings' Expert → Connection section) steers additional networks into the tunnel beyond the peer's own virtual-LAN subnet. On Windows, `tun::windows::add_extra_routes`/`remove_extra_routes` add/remove the routes via `New-NetRoute`/`Remove-NetRoute`, scoped to the adapter's interface alias (same reasoning as E.2's firewall rule — `netsh` can't scope a route by interface name the way the `NetTCPIP` PowerShell cmdlets can). `SplitPolicy::extra_routes` widens the packet-filtering side symmetrically.
- `split_tunnel::Ipv4Cidr::parse`/`parse_extra_routes` — the former does strict `Result`-returning validation (for a future settings-entry UI to give real errors); the latter silently drops unparseable entries, used on the data-plane path where a single malformed route must not take down the whole connection.
- JSON connection-profile export/import (Settings) now round-trips `extraRoutes`; an older exported profile without the field remains valid (treated as `[]`, not rejected).

### Fixed (pre-release, caught while writing this phase's own live end-to-end test)
- **`extra_routes` initially only widened egress, not ingress delivery.** The first implementation merged extra routes into `SplitPolicy`'s existing `included` list, which widened what a sender's own OS could route *into* the tunnel — but the peer's `admits_inbound` never checked destinations against anything but its own address, broadcast, and multicast, so the receiving side just dropped the traffic. The feature would have compiled, passed a narrower unit test, and silently done nothing end to end. Fixed by giving `extra_routes` its own field, separate from `included`, and widening `admits_inbound`'s *destination* check (not just its source check) — while explicitly *not* merging it into `included`, which would have reintroduced the phantom-in-subnet-host vulnerability `admits_inbound` was written to prevent (an existing regression test guards this: `extra_routes_do_not_loosen_the_base_subnet_phantom_host_rule`). The realistic shape of this feature, as a result: whoever can actually reach a network opts their own inbound side in to deliver traffic toward it, and the sender opts their own outbound side in to route there — each side configures the routes it needs, symmetric with how the manual paste flow already requires both peers to act.

### Scope
- Narrower than full site-to-site LAN sharing: this lets a peer that already knows a network's address route traffic to it through the tunnel (assuming both sides configure it), not full transparent forwarding of a peer's entire real LAN. That distinction — and *why* it's the line drawn here — is unchanged from E.2's original scoping notes.
- No conflict detection against existing system routes; the user is expected to avoid overlaps.

### Verified
- `cargo test --lib`: 113/113 passing (13 new): `Ipv4Cidr::parse` accepts/rejects well- and malformed input, `parse_extra_routes` drops bad entries without failing the batch, `SplitPolicy::extra_routes` widens egress classify/admits and both sides of `admits_inbound` (with an explicit regression guard that it does *not* loosen the base-subnet phantom-host rule), a live two-pipeline test proves a packet bound for an extra-routed network crosses end to end when both sides configure it (and one outside every route still doesn't), and the `New-NetRoute`/`Remove-NetRoute` PowerShell script builders are scoped by interface alias and correctly escape a hostile adapter name (same `ps_quote` guard already proven for the firewall path).
- `cargo check`: zero warnings.
- `pnpm test`: 127/127 passing (10 new): the Settings text field commits on blur (not per keystroke) with whitespace/empty entries stripped, an empty field commits an empty list, export/import round-trips `extraRoutes`, a profile missing the field imports as `[]` rather than erroring, and a profile with a non-array or non-string-array `extraRoutes` is rejected.
- `tsc --noEmit` clean; `pnpm build` succeeds.
- Browser-verified live: typed a comma-separated route list into the new field, confirmed it commits to the store on blur (not while typing), confirmed the Minecraft page's own settings panel is unaffected. Zero console errors.
- **Not verified in this session:** an actual `New-NetRoute` call succeeding, or traffic actually reaching a real extra-routed network — this environment has no Administrator privileges, the same standing limitation as every other Windows-elevation-dependent feature in this project (E.2's firewall integration, adapter creation itself). The PowerShell script generation is unit-tested; its real-world execution is not.

## [0.34.0] - 2026-08-06

### Changed
- **Network page's Virtual Network tab now leads with management, not creation.** When not currently in a network, it shows a one-line hint pointing at the relevant game's own page (e.g. Minecraft) instead of the create/join forms — those forms are still there, one click away behind an explicit "or create a general-purpose network" toggle, for networks that aren't tied to any particular game. Once in a network, the panel is unchanged: name, game tag, host address, live member list, leave — that's the actual "management" the tab is for.
- `VirtualNetworkPanel` gained a `collapseFormsByDefault` prop (Network page passes it; the Minecraft page's instance does not, so its forms still show immediately — it's a direct, single-purpose entry point, not a management surface).

### Scope
- Purely a UI-layer change — no Rust changes, no new Tauri commands, no change to `MeshSession`'s behavior. Creating a "general-purpose" (no `gameTag`) network from the Network page still works exactly as before, just one click further in.

### Verified
- `pnpm test`: 120/120 passing (4 new: the hint shows instead of forms by default, the toggle reveals the forms, the full status view (not the hint) shows once in a network, and the prop being omitted — as on the Minecraft page — keeps the old always-shown-forms behavior). One existing `Network.test.tsx` test updated to expect the hint instead of the forms when switching to the virtual-network tab.
- `tsc --noEmit` clean; `pnpm build` succeeds.
- Browser-verified live: Network page's virtual-network tab shows the hint by default, expanding it reveals the general-purpose forms; Minecraft page's own panel still shows its forms immediately, unaffected. Zero console errors.

## [0.33.0] - 2026-08-06

### Added
- **Minecraft-specific virtual network panel** — the Minecraft page now hosts its own `VirtualNetworkPanel` instance (create/join, live member list — the exact same component and underlying `MeshSession` as the Network page's general panel, not a parallel system), pre-filled with the Minecraft preset (broadcast + multicast forwarding, FEC `r = 2`) so creating a Minecraft network needs no separate settings step.
- **`gameTag`** — free-form display metadata (`Option<String>` on the Rust side, `string | null` on the frontend) settable at network creation/join. Shown as a badge next to the network name wherever the network is viewed — the Network page's general panel included, regardless of which panel instance was used to create or join it. Deliberately a plain string rather than an enum: a future second game is a new tag value, not a code change, matching the explicit intent to add more games later.
- `VirtualNetworkPanel` now accepts optional `gameTag`/`settings` props; `useVirtualNetwork(gameTag, settings)` threads them through to `create_network`/`join_network`. Both default to the prior behavior (`null` tag, `DEFAULT_CONNECTION_SETTINGS`) — the Network page's usage is unchanged.

### Changed
- `create_network`/`join_network` (Tauri commands) and `MeshSession::create`/`join`/`start` (Rust) gained `game_tag: Option<String>` and `settings: ConnectionSettings` parameters — `settings` was previously hardcoded to `ConnectionSettings::default()` inside `MeshSession`, meaning every virtual-network connection ignored whatever the user had configured. It's now threaded through exactly like the manual-signaling `connect_peer`'s `settings` argument.

### Scope
- Kept the Network page's general Virtual Network panel — an earlier direction considered moving it entirely to the Minecraft page, but the general management panel remains the intended way to reach *any* network (Minecraft or otherwise) from one place.
- No per-game settings beyond Minecraft's yet, and no filtering/searching networks by tag — that's for when a second game is actually added.

### Verified
- `cargo test --lib`: 102/102 passing (2 new): `game_tag` and `settings` both carry through `create` into `status()`, and `status().gameTag` is `None` when not supplied.
- `cargo check`: zero warnings across the whole crate.
- `pnpm test`: 116/116 passing (4 new): a fixed `gameTag`/`settings` prop pair is passed through to `create_network`'s `invoke` call, a known tag (`"minecraft"`) renders its friendly label, an unrecognized tag falls back to the raw string, and no badge renders when there's no tag.
- `tsc --noEmit` clean; `pnpm build` succeeds.
- Browser-verified live: opened the Minecraft page, confirmed the new Virtual Network section renders both forms; opened the Network page, confirmed its general Virtual Network tab still works unchanged; zero console errors on a fresh tab.

## [0.32.0] - 2026-08-06

### Added
- **Virtual-network UI (Phase G.4)** — the last piece of the Hamachi/Radmin-style feature, integrated as a "Virtual Network" tab on the existing Network page (not a new sidebar entry — it's another way to reach the same underlying connection, alongside manual pairing). Create-network and join-network forms when not in a network; once in one, the network name, a host badge, the shareable host address, and a live member list (fingerprint + connection-state dot per member) with a Leave button.
- `engine::mesh::MeshSession` — the Tauri-facing lifecycle wrapper around G.1–G.3c: `create`/`join` start a `SignalingServer` (host only) + `SignalingClient` + `MeshOrchestrator` in a background task; `leave` tears the whole thing down (idempotent); `status` reads the live roster and each member's `ConnectionManager` link state without reaching into the background task.
- New Tauri commands: `create_network`, `join_network`, `leave_network`, `get_network_status` (`commands/mesh_cmds.rs`), plus their `lib/engine.ts` wrappers and `types/telemetry.ts` types (`NetworkStatus`, `NetworkMember`).
- `MeshOrchestrator` gained a `roster` mirror (`roster_handle()`) so `MeshSession::status` can read the member list from outside the task the orchestrator's event loop runs in, without needing to hold the `SignalingClient` itself.
- `ConnectionManager` is now Tauri-managed as `Arc<ConnectionManager>` instead of a bare value, so `MeshSession` can hand a shared, cloneable handle into its background task — every existing command (`connect_peer`, `create_offer`, etc.) updated to match; no behavior change.

### Scope
- No network discovery (e.g. auto-finding a host on the LAN) — a joiner types the host's `ip:port` by hand. A host behind NAT without port forwarding remains a known limitation of self-hosting the signaling server (G.1's tradeoff), not something this UI layer can paper over.
- No network persistence/auto-reconnect across app restarts.
- Status is polled (`get_network_status` every 2s while the panel is mounted) rather than pushed — there's no `engine://` event for roster/link changes yet, unlike the manual-signaling flow. Simple and correct, not the most efficient possible; documented as a deliberate tradeoff rather than an oversight.

### Verified
- `cargo test --lib`: 100/100 passing (4 new `MeshSession` tests): status is `None` outside a network, create-then-status reports self as host with an empty member list, creating twice is rejected, and the full Tauri-facing proof — one session hosts, another joins, and both `status()` calls eventually show the other member as `Connected`.
- `cargo check`: zero warnings across the whole crate (workspace + lib), first time `engine::mesh` compiles without any `#[allow(dead_code)]` — everything is finally reachable from a real command.
- `pnpm test`: 112/112 passing (11 new): tab switching between Manual and Virtual Network modes, create/join form validation (buttons disabled until required fields are filled), correct `invoke` argument mapping, inline error display on a rejected `create_network`, and the active-network view (name, host badge, address, member list with per-member link-state dot, empty-roster placeholder, Leave).
- `tsc --noEmit` clean; `pnpm build` succeeds.
- Browser-verified live: opened the Network page, confirmed Manual is the default tab, switched to Virtual Network and confirmed both forms render, typed a network name and password and watched the Create button go from disabled to enabled in real time, zero console errors on a fresh tab.

## [0.31.0] - 2026-08-06

### Added
- **Mesh auto-connect orchestration (Phase G.3c)** — `engine::mesh::MeshOrchestrator` wires the signaling roster (G.2), relay (G.3), and multi-peer `ConnectionManager` (G.3b) together: joining a network now establishes P2P links with every other member automatically, no manual offer/answer paste required. For each member seen (at join time or via a live `Joined` event), a deterministic pubkey tie-break (`should_initiate`) decides which side sends the offer — both sides compute the same answer independently from data they both already have, so exactly one offer is ever sent per pair with no coordination round-trip to arbitrate it. The receiving side answers and connects as `Role::Responder`; the sender connects as `Role::Initiator` once the answer comes back, matched against the pending offer's session id.
- Everything `MeshOrchestrator` sends is the exact same `SignalEnvelope` the manual paste flow (`commands/signaling_cmds.rs`) already produces and validates — this phase is glue, not a new envelope format.
- `ConnectionManager::socket()` — a new accessor so a future caller (Phase G.4) can hand `ensure_socket`'s bound socket to a `MeshOrchestrator` explicitly, keeping the orchestrator decoupled from the manager's internal STUN bootstrap (useful for tests: a directly-bound loopback socket works exactly the same way).

### Fixed (pre-release, caught by the phase's own integration test)
- **Double offer-send race.** A member whose `Joined` event was already queued in the signaling client's channel by the time `MeshOrchestrator::run`'s initial-roster read happened (both reflect the same underlying state update, made atomically by the client's reader task) was processed twice — once from the roster snapshot, once from the queued event — sending two different offers with two different session ids and corrupting the pending-offer bookkeeping (observed as ~50% test flakiness: "answer for the wrong session" / "no matching pending offer"). Fixed with a `seen_members` guard making `on_member_present` idempotent per peer, cleared again when that peer leaves.

### Scope
- No UI yet. See Project Status: a host behind NAT without port forwarding remains a known limitation of the user-hosted signaling architecture (G.1's tradeoff, not something this phase changes).

### Verified
- `cargo test --lib`: 95/95 passing (5 new): the full two-member auto-connect proof (real `SignalingServer` + two real `SignalingClient`s + two `MeshOrchestrator`s + real loopback UDP sockets, reaching `Connected` on both sides with zero manual signaling calls), the tie-break picks exactly one side, the idempotency guard, plus a new `ConnectionManager`-level test (`connect_to_reaches_connected_over_real_loopback_sockets`) proving `connect_to` itself carries a link all the way to `Connected`, not just to `Connecting` (every other G.3b test used an unreachable candidate on purpose). Re-ran the full suite 3 times and the new auto-connect test 10 times individually to confirm the race fix holds — no flakes.
- `cargo check --lib`: zero warnings (`mesh.rs` carries the same documented `#![allow(dead_code)]` posture as G.1/G.2's still-unwired modules, since nothing calls it from a Tauri command yet).
- No frontend changes in this phase.

## [0.30.0] - 2026-08-06

### Added
- **Multi-peer `ConnectionManager` (Phase G.3b)** — `connect_to`/`disconnect_peer`/`link_state_of`/`peer_link_states` track an independent link per peer, keyed by `PeerKey` (the base64 public key). Two different peers can now be `Connecting`/`Connected` at the same time without one rejecting the other — the mesh prerequisite for a Hamachi-style network with more than two members.

### Changed
- `Inner`'s single `link: Arc<Mutex<LinkState>>` / `active: Mutex<Option<Active>>` fields became `peers: Mutex<HashMap<PeerKey, PeerLink>>`. Entries are removed on disconnect rather than kept around `Idle` — a missing key and an `Idle` key mean the same thing to every reader.
- **The legacy single-peer API is unchanged in behavior and signature.** `connect()`/`disconnect()`/`link_state()` (no arguments) now derive their target peer's key from the pending-negotiation slot (`begin_offer`/`begin_answer`/`set_peer`) and delegate to the new per-peer methods — same external behavior as before Phase G.3b, just backed by the general map instead of a single slot. Neither `commands/connection_cmds.rs` nor any frontend code changed.

### Scope
- Still no orchestration: nothing here decides *when* to call `connect_to` for a given peer, or reacts to a roster event (G.2) or an incoming relay (G.3) by automatically connecting. That's the next phase, tentatively G.3c, once the roster/relay/multi-peer pieces built across G.1–G.3b are wired together. No UI yet (G.4).

### Verified
- `cargo test --lib`: 91/91 passing (5 new): two different peers can be `Connecting` simultaneously, reconnecting to an already-connecting peer is rejected (same "one session per peer" guarantee as before, now scoped per-key instead of globally), disconnecting one peer leaves another's state untouched, an untracked key reads as `Idle`, and `peer_link_states` lists every tracked peer. These test the bookkeeping directly via `connect_to` with unreachable candidates — the handshake protocol itself is already covered end-to-end by `pipeline.rs`'s integration tests, not re-tested here.
- `cargo check --lib`: zero warnings (one new method not yet called from a Tauri command carries an explicit, documented `#[allow(dead_code)]`, same posture as G.1/G.2's still-unwired modules).
- No frontend changes in this phase.

## [0.29.0] - 2026-08-06

### Added
- **Offer/answer relay over the signaling connection (Phase G.3)** — `ClientMessage::Relay { to_pubkey, blob }` / `ServerMessage::Relayed { from_pubkey, blob }` let two members exchange offer/answer envelopes through the host they're both already connected to, instead of copy/pasting a blob out of band. The `blob` field is the exact same `signaling::blob::encode`d string the manual paste flow already produces and validates (CRC32 + version + structural checks) — this phase adds a transport, not a new envelope format.
- `SignalingServer` relays `Relay` messages to the named `to_pubkey` if they're currently a member; silently drops it otherwise (a member leaving mid-relay is an expected race, not an error worth surfacing).
- `SignalingClient::relay(to_pubkey, &envelope)` — fire-and-forget send via a new background writer task (the client previously only read; this phase adds the other half of the connection). `MemberEvent::Relayed { from_pubkey, envelope }` surfaces incoming relays through the same event channel as join/leave — a blob that fails to decode is dropped silently, consistent with how every other malformed frame in this reader loop is already handled.

### Scope
- **Transport only.** Nothing in this phase decides *when* to relay an offer or reacts to a received one — that requires generalizing `ConnectionManager` from single-peer to multi-peer (mesh), a substantially larger, separate piece of work than adding a message type. Explicitly deferred to a later phase (tentatively G.3b) rather than folded in here.
- No UI yet (G.4).

### Verified
- `cargo test --lib`: 86/86 passing (3 new): a relay reaches its intended target and no one else, relaying to a nonexistent member is a silent no-op that leaves the connection healthy (proven by a subsequent successful relay), and the server-level behavior is exercised directly over a raw WebSocket too, not only through the client wrapper.
- `cargo check --lib`: zero warnings.
- No frontend changes in this phase.

## [0.28.0] - 2026-08-06

### Added
- **Networked signaling client (Phase G.2)** — `engine::signaling::client::SignalingClient` connects to a `SignalingServer` (G.1), sends `Join`, and on acceptance starts tracking the roster in the background: `members()` returns a live snapshot, and a `mpsc::UnboundedReceiver<MemberEvent>` surfaces `Joined`/`Left` events in real time as the host broadcasts them.
- `SignalingClientError` maps every `JoinRejectReason` to its own variant (`WrongPassword`, `WrongNetworkName`, `UnsupportedVersion`, `AlreadyJoined`) plus connection-level failures (`Connect`, `ClosedBeforeJoin`, `MalformedMessage`), so a caller can react to *why* a join failed instead of a single opaque error.

### Scope
- Still roster tracking only — the client observes membership, nothing more. Automatically relaying offer/answer over this same connection (so joining a network establishes P2P links without manually pasting a blob) is Phase G.3. No UI yet (G.4).

### Verified
- `cargo test --lib`: 83/83 passing (6 new, all against a real `SignalingServer` instance, not mocked): joins an empty network, rejects wrong password/network name without constructing a client, sees an existing member in its initial roster, observes a later join as both a live event and a roster update, observes a departure the same way.
- `cargo check --lib`: zero warnings (same `#![allow(dead_code)]` posture as G.1 — not wired into a Tauri command yet).
- No frontend changes in this phase.

## [0.27.0] - 2026-08-06

### Added
- **Networked signaling server (Phase G.1)** — the first piece of Hamachi/Radmin-style multi-member virtual networking: `engine::signaling::server::SignalingServer` listens on a WebSocket port, gatekeeps connections by network name + SHA-256-hashed password (plaintext never stored or logged), and maintains/broadcasts a live member roster (`MemberInfo { pubkey, fingerprint }`) as members join and leave.
- `engine::signaling::protocol` — the wire format (`ClientMessage::Join`, `ServerMessage::{JoinAccepted, JoinRejected, MemberJoined, MemberLeft}`), versioned (`PROTOCOL_VERSION`) and kept deliberately separate from the existing `message::SignalEnvelope` (manual paste-based offer/answer) — the two signaling paths are independent until Phase G.3 bridges them.
- New dependencies: `tokio-tungstenite` + `futures-util` (both MIT/Apache-2.0) for the WebSocket transport.

### Architecture
- **User-hosted, not centrally hosted.** Whoever creates a network runs the signaling server themselves — this project has no funding for hosted infrastructure, so unlike Hamachi/Radmin's central servers, the "host" is just another player's own machine. The signaling server only ever relays membership and (starting G.3) offer/answer handshake data; actual game traffic stays P2P over the already-existing NAT hole-punch (C4) and encrypted data plane (C5) — nothing about the transport or crypto layers changes.
- Deliberately scoped to roster management only. No offer/answer relay yet (G.3), no client-side connect logic yet (G.2), no UI yet (G.4). A host behind NAT without port forwarding is a known, documented limitation of this architecture — see Project Status.

### Verified
- `cargo test --lib`: 77/77 passing (8 new — join accepted with empty/non-empty roster, wrong password rejected, wrong network name rejected, duplicate pubkey rejected, join/leave broadcasts land on the right peers, live roster snapshot, password hashing is deterministic and never the plaintext). Real WebSocket client connections via `tokio_tungstenite::connect_async` against a server bound to `127.0.0.1:0`, not mocked.
- `cargo check --lib`: zero warnings (explicit `#![allow(dead_code)]` on the two new modules, since neither is wired into any Tauri command yet — that lands in G.2/G.4).
- No frontend changes in this phase.

## [0.26.0] - 2026-08-06

### Added
- **Dedicated Minecraft page**, its own sidebar entry (`RouteId` gains `"minecraft"`, route `/minecraft`). Shows a read-only summary of the currently effective connection settings (broadcast/multicast forwarding, FEC redundancy) and a one-click "Apply Minecraft preset" button, disabled once already applied so the state is visibly reflected rather than just silently idempotent.
- The Minecraft preset logic (broadcast + multicast on, FEC `r = 2`) **moved from Settings to this page** — a single entry point instead of the same feature living in two places. The Settings → Connection section no longer has its own Minecraft button.

### Scope
- Icon is a neutral placeholder (lucide's `Gamepad2`), not Minecraft-branded artwork. Two candidate images were supplied and reviewed — a Creeper-face icon and the official Minecraft app icon (grass block + wordmark) — both carry real Mojang/Microsoft trademark and copyright exposure for a distributed application; this repo's `LICENSE` covers only original content and explicitly-listed licensed third-party software, neither of which extends to third-party game branding. Deferred pending the project owner confirming licensing/rights for the artwork; swapping the icon later is a one-line change in `Sidebar.tsx` and `pages/Minecraft.tsx`.

### Verified
- `pnpm test`: 101/101 passing (3 new page tests — summary reflects live store state, preset click applies all three fields, button disables once already applied).
- `tsc --noEmit` clean; `pnpm build` succeeds.
- Browser-verified live: navigated to the new Minecraft nav item, confirmed breadcrumb and page content render, clicked the preset (FEC visibly updated `r = 1` → `r = 2`, button label switched to "applied" and disabled), then opened Settings → Expert → Connection and confirmed the old preset button is gone. Zero console errors.
- `cargo check`/`test --lib`: no Rust changes in this phase.

## [0.25.0] - 2026-08-06

### Added
- **Settings: Minecraft quick-setup preset.** A one-click button in the Connection section applies a Minecraft-tuned configuration — broadcast + multicast forwarding on (both editions rely on them for LAN-world discovery) and FEC redundancy `r = 2` (one step above the default, without maxing it out).

### Scope
- Deliberately a manual shortcut, not automatic game detection. Real process detection would need new Windows API calls and a materially larger permission surface for something that boils down to setting three already-existing store fields; a one-click preset gets the same practical outcome — the user does not have to know or remember the "right" settings for Minecraft — without any of that. No background polling, no process scanning, no new capability grants.
- Only affects the three fields already covered by `ConnectionSettings` (forwarding + FEC); does not touch theme, language, or Expert-mode visibility.

### Verified
- `pnpm test`: 99/99 passing (1 new — clicking the preset button applies all three fields in a single interaction).
- `tsc --noEmit` clean; `pnpm build` succeeds.
- Browser-verified live: set FEC to 1 and both forwarding toggles off, clicked the preset button once, confirmed via `localStorage` that all three fields flipped to the preset values (`forwardBroadcast: true`, `forwardMulticast: true`, `fecParityShards: 2`) in one click. Zero console errors.
- `cargo check`/`test --lib`: no Rust changes in this phase.

## [0.24.0] - 2026-08-05

### Added
- **Simplified Chinese (简体中文) locale.** Third bundled language alongside English and Traditional Chinese, selectable live from Settings and persisted like the other two. `src/i18n/locales/zh-Hans/common.json` uses Mainland terminology, not a mechanical script conversion of the Traditional file (e.g. "数据包" rather than a literal "封包" conversion).
- `SupportedLanguage` gains `"zh-Hans"`; `SettingsOverlay`'s language picker is now a 3-column grid with a `简体中文` button.

### Changed
- **`detectDefaultLanguage()` in `appStore.ts`** now splits `zh-*` locales three ways instead of collapsing every `zh` prefix to Traditional: `zh-TW`/`zh-HK`/`zh-MO`/any `-Hant` subtag → `zh-Hant`; every other `zh` locale (`zh-CN`, `zh-SG`, or a bare `zh`) → `zh-Hans`, the more common default. Only affects first-run detection — an already-persisted language choice is untouched.
- **`i18n/index.test.ts`'s key-parity check** generalized from a hardcoded two-locale diff to a pairwise check across every bundled locale, so adding a future locale is automatically covered by the same regression test rather than needing a rewrite.

### Verified
- `pnpm test`: 98/98 passing (8 new — 2 key-parity tests generalized to cover the new locale pairwise, 5 `detectDefaultLanguage` branch tests via `vi.resetModules` + dynamic re-import with a stubbed `navigator.language`, 1 new SettingsOverlay button test).
- `tsc --noEmit` clean; `pnpm build` succeeds.
- Browser-verified live: opened Settings, clicked 简体中文, confirmed the entire visible UI (nav labels, theme names, Expert-mode copy) re-rendered in Simplified Chinese with zero remount, matching the same live-switch behavior already verified for zh-Hant in the i18n phase; confirmed via `localStorage` that the choice persisted as `"zh-Hans"`; zero console errors.
- `cargo check`/`test --lib`: no Rust changes in this phase.

## [0.23.0] - 2026-08-05

### Added
- **Settings: JSON connection-profile import/export.** Expert-mode Connection section gains **Export Profile** / **Import Profile** buttons that save or load the current split-tunnel forwarding + FEC redundancy settings as a versioned JSON file, via native OS save/open dialogs.
- `src/lib/profile.ts` — `serializeConnectionProfile`/`parseConnectionProfile`, a pure, dependency-free codec independent of the dialog/fs plumbing.
- `tauri-plugin-dialog` and `tauri-plugin-fs` (Rust) + `@tauri-apps/plugin-dialog` and `@tauri-apps/plugin-fs` (npm), registered in `lib.rs`'s builder. New capability grants in `capabilities/default.json`: `dialog:default`, and `fs:allow-write-text-file`/`fs:allow-read-text-file` scoped to `**` — a wildcard is appropriate here specifically because every path reaching those commands was just explicitly chosen by the user through a native file dialog, not supplied by any other code path.

### Format
- File shape: `{ "formatVersion": 1, "forwardBroadcast": bool, "forwardMulticast": bool, "fecParityShards": number }`. `formatVersion` lets a future field addition detect and reject old/new-incompatible files instead of silently misreading them.
- Import validation is strict, not lenient: wrong version, non-boolean fields, a non-integer `fecParityShards`, or a value outside `1..=16` (mirroring the Rust `RsEncoder`'s `MAX_R` clamp in `fec/rs.rs`) are all rejected with a specific inline error — never silently coerced or clamped. A rejected import leaves the store untouched.

### Scope
- Only `ConnectionSettings` (forwarding + FEC) is covered — theme, language, and Expert-mode visibility are local UI preferences, not "connection profile" data, and are deliberately excluded.
- Automatic game detection remains planned, unrelated scope.

### Verified
- `pnpm test`: 90/90 passing (15 new — 9 codec unit tests in `profile.test.ts`, 6 component tests in `SettingsOverlay.test.tsx` mocking `@tauri-apps/plugin-dialog`/`plugin-fs`, covering export, cancel-on-export, valid import, malformed-file rejection, and cancel-on-import).
- `tsc --noEmit` clean; `pnpm build` succeeds.
- `cargo check --lib` and `cargo test --lib`: 69/69 passing, unaffected (this phase adds plugin registration only, no engine logic changed).
- **Not verified in this session:** an actual native save/open dialog round-trip end-to-end, since this environment's dev-server preview cannot host a real Tauri window (no OS-level dialog surface to drive). The dialog/fs plugin calls are exercised through mocks that assert the exact arguments passed (path, JSON payload); the underlying plugins themselves are the official, actively-maintained Tauri v2 plugins.

## [0.22.0] - 2026-07-03

### Added
- **Settings: layered Basic/Expert access.** The Connection section (split-tunnel broadcast/multicast forwarding, FEC redundancy) is now gated behind an **Expert settings** toggle, off by default — a first-time user sees only Theme and Language. Theme and Language are never gated; they are basic personalization, not an "expert" concern.
- `appStore.expertMode: boolean` (default `false`), persisted like every other preference here.

### Scope
- **Purely a display filter, never a functional gate.** Hiding the Connection section does not reset or stop applying whatever it was set to — a changed FEC redundancy or forwarding toggle stays in effect at the next Connect whether or not its section is currently visible. This is the conventional "advanced settings" pattern (progressive disclosure of already-effective settings), not a feature switch.
- **JSON profile import/export and automatic game detection remain planned**, deliberately not bundled into this phase. Checked before writing any code: the app has no `dialog`/`fs` Tauri plugin installed and `capabilities/default.json` grants only `core:default` — profile import/export needs new plugin dependencies and explicit capability permissions, a materially larger and separate piece of work.

### Verified
- `pnpm test` — **75/75 pass** (5 new: `expertMode`'s default/setter/persistence, the Connection section hidden by default, revealed by the toggle, and — the guarantee that matters — a changed setting surviving the section being hidden again).
- `tsc` clean; `pnpm build` succeeds; `cargo test` unaffected (no Rust changes).
- **Browser-verified the full round trip**, not just the component tests: opened Settings, confirmed the Connection section absent by default, switched Expert mode on, set FEC redundancy to `2`, switched Expert mode back off, and confirmed via `localStorage` that `fecParityShards` was still `2` even with the section hidden. Zero console errors throughout.

---

## [0.21.0] - 2026-07-02

### Added
- **Diagnostics visualization: topology view + spectrum chart.** The last item that had carried a 🚧 in the Project Status table since Phase A.
- **`TopologyView`** — this node and the negotiated peer as two nodes with a connecting line, colored by `LinkState` (idle/connecting/connected/failed) using the same status-color convention already used elsewhere in Diagnostics, with live RTT annotated once connected. Deliberately a **two-node view, not a general graph layout engine** — the product is strictly point-to-point today; a generic multi-node graph would be solving a problem this app doesn't have. If multi-peer support ever lands, that is the point a real layout engine becomes worth it.
- **`SpectrumChart`** — a live tx/rx throughput line+area chart with a hover crosshair and tooltip. Hand-rolled SVG (`lib/spectrum.ts` is ~50 lines of pure coordinate math) rather than a charting dependency. One axis (kbps, shared by both series — never a dual-axis chart); tx/rx reuse the app's existing violet/cyan convention from the packet log, so color means the same thing everywhere in Diagnostics rather than introducing a fresh palette.
- `telemetryStore.spectrumHistory` — a 120-sample ring buffer (a sample *count*, not a time window, since `tick_hz` is configurable 1–20 Hz) populated by `setSnapshot`, cleared by `reset()`.
- Followed the project's `dataviz` skill: single axis, fixed-order categorical color reused from an already-shipped convention (not a fresh palette needing separate validation), status color reserved for state (never doubled up with the node encoding — the nodes stay neutral so status is said once, not twice), a legend for the two series, recessive gridlines, and a hover layer shipped by default rather than as a follow-up.

### Verified
- `pnpm test` — **70/70 pass** (23 new: `lib/spectrum.ts`'s coordinate math — including the zero-max and single-sample edge cases — `spectrumHistory`'s accumulation/120-cap/reset behavior, and component tests for both the empty and populated states of each view, including a hover test against a stubbed `getBoundingClientRect`).
- `tsc` clean; `pnpm build` succeeds; `cargo test` — **69/69**, unaffected (no Rust changes).
- **Browser-verified with real data, not just component tests:** Vite's dev server serves unbundled ES modules, so a dynamic `import()` from the browser console reached the *exact same* `telemetryStore` singleton the running app reads from — pushing 20 live samples through it and watching the empty-state placeholder replaced by a real drawn chart, a real dynamic y-axis ceiling, and the injected identity appear in the topology view, all through the actual render path rather than a simulated one. A dispatched `mousemove` against the real (non-mocked) SVG geometry produced a correctly-positioned, correctly-valued tooltip. Confirmed responsive down to mobile width (grid collapses to one column, no horizontal overflow) and zero console errors throughout.

---

## [0.20.0] - 2026-07-01

### Added
- **Windows network integration (Phase E.2).** `WintunDevice::open` now, best-effort, classifies the fresh adapter's network as **Private** and adds an inbound firewall allow-rule **scoped to exactly that interface** — automating the fix that `DOC/Two_Machine_Verification.md` had previously documented as a manual step (a new virtual adapter often defaults to `Public`, which silently drops the very traffic this app exists to carry).
- Uses the `NetSecurity`/`NetConnection` PowerShell cmdlets (`Set-NetConnectionProfile -InterfaceAlias`, `New-NetFirewallRule -InterfaceAlias`) rather than `netsh advfirewall`, which cannot scope a rule to one specific interface by name — only by `interfacetype` (e.g. `Lan`), which would also match the host's real Ethernet/Wi-Fi adapters.
- The rule is removed on teardown via `impl Drop for WintunDevice` (the first `Drop` impl in the engine), so it runs however the device stops existing rather than needing every disconnect path to remember to call it.

### Scope — a genuine fork, decided explicitly
"OS route management" was ambiguous between two very different features, and the fork was surfaced and decided before writing any code:
- **Built now: Windows network/firewall hygiene** (this entry) — no routing, no change to what traffic reaches the adapter (still entirely `split_tunnel`'s job), no change to the two-node trust model. Safe, and — unusually for this project — **verifiable on a single machine**.
- **Deferred: site-to-site LAN sharing** — letting a peer advertise routes to its *own* real LAN (e.g. `192.168.1.0/24`) so traffic beyond the tunnel's own `/24` gets forwarded through them. This would require the peer to run IP forwarding + NAT (a substantial Windows-side feature on its own), and would break the two-node assumption `split_tunnel/mod.rs`'s own doc comment names explicitly — its ingress policy would need to move from hardcoded endpoints to a peer-scoped, explicitly-approved CIDR allow-list, with real thought given to a malicious or compromised peer advertising an unexpectedly broad route. Verifying it end-to-end needs **three** physical hosts (both peers plus a device on one peer's real LAN) — a harder bar than the standing, still-unmet two-machine NAT-traversal verification. Deferred for the same reason relay/TURN fallback is: building unverifiable code on top of already-unverified code compounds risk rather than reducing it.

### Fixed
- Both PowerShell calls are best-effort: a failure never blocks bringing up the data plane. What may reach the adapter is decided by `split_tunnel` before any packet gets this far, so a failure here degrades to "Windows Firewall might still need a manual nudge" (the fallback instructions remain in the verification checklist), not a broken connection.

### Verified
- `cargo test` — **69/69 pass** (3 new: PowerShell single-quote escaping — including a string containing `'; Remove-Item C:\ -Recurse; '`, to make the point concretely — round-trips exactly; the deterministic, prefixed firewall-rule naming that lets teardown find what setup created).
- **This phase could not be fully verified from here — elevation is genuinely required, and this session is not elevated (confirmed directly).** What *was* verified: the exact PowerShell command strings the Rust code constructs were dry-run with `-WhatIf` against a real (non-elevated) PowerShell — both `Set-NetConnectionProfile` and `New-NetFirewallRule` correctly resolved cmdlet and parameter names and correctly un-escaped a deliberately quote-containing adapter name back to its literal form (the error text echoed the exact literal back), failing only on "no such interface exists" — the expected outcome for a fabricated name. This confirms the syntax and escaping are correct; it does **not** confirm the happy path against a real elevated Wintun adapter, which needs your own elevated run to complete.

---

## [0.19.0] - 2026-06-30

### Added
- **Multi-language support (English + Traditional Chinese).** Every static UI string across the app — nav, breadcrumb, Dashboard, Network, Diagnostics, Settings, and both Network/PeerConnection sub-components — now renders through `react-i18next`. A **Language** section in Settings (English / 繁體中文) switches live, with no reload, and persists in `appStore` alongside the theme.
- `src/i18n/` — `i18next` initialized with both locales bundled statically (no `i18next-http-backend`; a desktop app ships every language, so there is nothing to fetch at runtime) and no language-detector plugin (`appStore` picks a default from `navigator.language` — a three-line check — then persists the user's explicit choice, mirroring how the theme already works).
- `appStore.language: "en" | "zh-Hant"`, synced to i18next by a `useEffect` in `AppShell` — the same app-wide-side-effect pattern `useEngineTelemetry` already established in Phase B.2.
- **Key-parity regression test** (`i18n/index.test.ts`): asserts the English and Traditional Chinese JSON files expose exactly the same key set, and that neither has an empty value. This is the guard that actually matters going forward — it catches "added an English string, forgot to translate it" at test time instead of at runtime as a raw key falling back onto the screen.

### Changed
- `Diagnostics.tsx`'s `STATE_STYLES` / `MODE_LABELS` — module-level `Record`s that held literal English strings — now hold **translation keys**, resolved via `t()` inside the component at render time. A module-level const is evaluated once at import; it cannot hold text that needs to change when the user switches language, so this restructuring was necessary, not cosmetic.
- `Network.tsx`'s closing sentence (which embeds a styled `<span>` around "Diagnostics") uses `<Trans>` rather than three concatenated translation keys, so each locale keeps its own natural word order around the embedded element instead of being forced into English sentence structure.
- New dependencies **`i18next`** and **`react-i18next`** (both MIT), attributed in `THIRD-PARTY-NOTICES.md`.
- Versions aligned to `0.19.0`.

### Scope
- **This phase covers only static React-layer UI copy.** `EngineNotice.message` and Tauri command error strings are constructed in Rust with `format!()` and sent as finished English sentences — localizing them needs the backend to send structured codes + parameters instead (the `EngineNotice.code` field already exists and the UI already branches on it for some notices), which is a materially different, Rust-side change disproportionate to this phase. They remain English, as does the raw `conn.link` / `conn.role` state-machine text in `PeerConnectionPanel` (technical identifiers, not prose).
- **Simplified Chinese was scoped out**, not because it is hard, but to keep this phase to the languages already committed to — the key set is now frozen and translated once, so adding a third locale later is a JSON file, not a redesign.

### Verified
- `pnpm test` — **47/47 pass** (5 new: `appStore` language default/setter/persistence; the two key-parity/empty-value tests; a `SettingsOverlay` test proving language selection updates the store; and — the test that actually matters — proving `i18n.changeLanguage` re-renders **already-mounted** components with new visible text, not just a store value).
- `tsc` clean; `pnpm build` succeeds; `cargo test` — **66/66**, unaffected (no Rust changes).
- **Browser-verified beyond the test suite:** the preview browser's own locale is Chinese, so the app **auto-detected zh-Hant on first load** — a real confirmation of the detection path, not a simulated one. Switched to English via Settings and confirmed every page (Dashboard, Network with its `<Trans>` sentence, Diagnostics including the state-label restructuring and the `{{count}}`-interpolated packet-log text) updated live with no reload. **Reloaded the page and confirmed the English choice persisted** — together with the B.2 route-restore feature correctly returning to the last-viewed page, in English. Zero console errors throughout.

---

## [0.18.0] - 2026-06-29

### Added
- **Settings content — Connection section (Phase B.3).** The Settings overlay gains its first real content beyond the theme switcher: **Forward broadcast** / **Forward multicast** toggles and an **FEC redundancy** selector (`r = 1` / `2` / `3`), persisted like the theme. These wire up the two engine knobs that were previously compile-time constants — flagged as half-wired in the earlier audit.
- `ConnectionSettings` (Rust `engine/connection.rs` + mirrored TS `types/telemetry.ts`): `forwardBroadcast`, `forwardMulticast`, `fecParityShards`. Threaded end to end: Settings UI → persisted `appStore` → read once at `useConnection.onConnect` → `connectPeer(settings)` → `connect_peer` command → `ConnectionManager::connect` → `pipeline::run` → `SplitPolicy::from_tun(&cfg).forward_broadcast(..).forward_multicast(..)` and `RsEncoder::new(FEC_GROUP_SIZE, settings.fec_parity_shards)`.
- `SplitPolicy` gains chainable `forward_broadcast(bool)` / `forward_multicast(bool)` builder setters (the existing `from_tun` constructor is untouched, so every prior test keeps working unmodified).

### Changed
- `pipeline::run` and `ConnectionManager::connect` / the `connect_peer` command now take a `ConnectionSettings` parameter. `ConnectionSettings::default()` reproduces the exact values that were previously hardcoded (`true`, `true`, `1`), pinned by a regression test — a caller (or a stale cached JS bundle) that omits it gets identical behaviour to before this phase.
- Versions aligned to `0.18.0`.

### Scope
- Settings apply **at the next Connect only** — not retroactively to an already-live link, per the boundary recorded in [0.15.0] (live toggling needs a control channel into the running pipeline task; still deferred).
- **Deliberately not touched:** Basic/Expert layered settings access, JSON profile import/export, and automatic game detection remain planned. Adding an Expert-mode gate just to house these three controls would have been scope creep for a feature (layered access) that deserves its own design pass.

### Verified
- `cargo test` — **66/66 pass** (4 new: two `SplitPolicy` builder-setter tests, a `ConnectionSettings::default()` regression guard, and an integration test proving a disabled broadcast toggle is enforced on the **live** loopback path, not just in the policy unit tests).
- `pnpm test` — **42/42 pass** (7 new: `appStore` default/setter/persistence coverage for the three fields, and four `SettingsOverlay` component tests covering render, both toggles, and the redundancy selector).
- `tsc` clean; `pnpm build` succeeds.
- **Browser-verified beyond the test suite:** opened the Settings overlay, toggled broadcast off and set FEC redundancy to 2, confirmed the UI and `localStorage` reflected it, then **reloaded the page** and confirmed both survived — true persistence, not just in-memory state. Zero console errors throughout.

---

## [0.17.0] - 2026-06-28

### Added
- **Network page (Phase B.2).** Peer-connection management — this node's identity, manual-signaling blob exchange, and Connect / Disconnect — now lives on the **Network** page (previously a "coming soon" stub). Diagnostics is slimmed to a pure telemetry *readout*: manage the connection on Network, observe it on Diagnostics.
- **First React component tests** (the part B.1 deferred): the Network page renders identity and the connection panel; Create offer surfaces the blob; a pasted offer is processed into an answer (asserting the `blob → blobStr` mapping end to end through the component); and Connect is disabled until a peer is negotiated, then enabled. **35 frontend tests total.**

### Changed
- **The telemetry subscription is now app-wide.** `useEngineTelemetry` was mounted only on Diagnostics, so navigating away tore down the engine event streams — and with connections now driven from a *different* page, that would have meant the Network page never sees `connecting → connected` progress. It is hoisted to `AppShell` and runs once for the app's lifetime.
- Connection state and actions extracted into a `useConnection` hook; the identity readout and the connection panel into `components/network/` components — mirroring the existing `useEngineTelemetry` pattern and making them unit-testable.
- Versions aligned to `0.17.0`.

### Verified
- `pnpm test` — **35/35 pass** (5 new component tests). `tsc` clean. `pnpm build` succeeds.
- **Browser-verified:** Network renders the connection panel and controls; Diagnostics no longer carries the peer-connection or identity blocks (they moved); navigation between pages works with **zero console errors**. `cargo test` — **62/62**, unaffected.

---

## [0.16.0] - 2026-06-27

### Added
- **Frontend test foundation (Phase B.1).** The React app had **zero** tests against the engine's 62; this establishes the harness and covers the pure logic:
  - **Tooling** — `vitest` + `happy-dom`, a `test` block in `vite.config.ts`, `pnpm test` / `pnpm test:watch` scripts, and a setup file that also registers `@testing-library/jest-dom` matchers for the component tests to come in B.2.
  - **`lib/engine.test.ts`** — `configForMode` (each UI mode → the right backend flags), `isEngineActive` (which states count as active), and the **IPC command contract**: every wrapper invokes the right command name, and the signaling wrappers map the JS `blob` argument to `blobStr` (which Tauri maps to Rust's `blob_str`) — a mismatch there would silently break signaling.
  - **`lib/cn.test.ts`** — class merging and Tailwind conflict resolution.
  - **`stores/telemetryStore.test.ts`** — state transitions, batch accumulation, and the 200-entry log bound (trims oldest, keeps newest).
  - **`stores/appStore.test.ts`** — route/theme/settings actions and `localStorage` persistence.
  - **30 tests, all passing.**

### Fixed
- happy-dom v20 ships a non-functional global `localStorage` here; the test setup installs a deterministic in-memory `Storage` so the persisted store is testable.

### Verified
- `pnpm test` — **30/30 pass**. `tsc` clean (test files typecheck; `noEmit` keeps them out of the build). `pnpm build` succeeds with test files correctly absent from the bundle. `cargo test` — **62/62**, unaffected.

### Scope
- B.1 covers pure logic and stores. Full React component-render tests wait for **B.2**, alongside the Network page they will exercise.

---

## [0.15.3] - 2026-06-26

### Removed
- **Four dead backend scaffold directories** — `src-tauri/src/{config,diagnostics,game_detection,utils}/`. Each held only a `.gitkeep`, none was ever declared as a module, and they *duplicated* the real structure under `engine/` (the actual config is `engine/config.rs`, telemetry is `engine/telemetry/`), so they were actively misleading. The real `engine/` layout is untouched.
- **The duplicate `src/themes/` directory** — the six real themes live in `src/theme/` (singular); the empty plural sibling only invited confusion.
- **`EngineConfig::peer_label`** — an unused config field (and its `default_peer_label` helper and the mirror `peerLabel` on the TypeScript side). Nothing consumed it.

### Note
Legitimate but currently-empty React scaffolding (`src/assets/`, `src/i18n/locales/`, `src/components/diagnostics/`, and similar) was **kept** — those are the conventional homes for planned work, and deleting standard project directories is churn, not cleanup.

### Verified
- `cargo clippy --all-targets -- -D warnings` — clean. `cargo test` — **62/62 pass**. `tsc` clean. No behaviour change (all removed items were unused).

---

## [0.15.2] - 2026-06-25

### Fixed
- **README accuracy.** The *Key Features*, *Architecture* and *Settings* sections read as though everything was delivered, while the *Project Status* table (correctly) marked most of it planned. Every Key Features bullet is now tagged **✅ built / 🚧 in progress / ⏳ planned**, with the status table named as authoritative; the Architecture diagram is captioned as the *target* shape (Game Detection and the Config/Profile Store are planned); and the NAT-traversal entry now states plainly that it is implemented but **not yet verified on two physical machines**. No code change.

### Known gaps (recorded, not yet built)
For visibility — beyond the standing **real-NAT-traversal** risk:
- **Frontend lags the engine.** The Network page is a "coming soon" stub; `components/diagnostics/` is empty (no topology map or spectrum monitor yet); the Settings overlay is a shell with no content.
- **Whole planned pillars have no code:** layered Basic/Expert settings, JSON profile import/export, automatic game detection, and i18n.
- **Half-wired engine knobs:** the FEC parity count (`r`) and the split-tunnel broadcast/multicast toggles exist but are compile-time constants, not user-configurable.
- **Deferred by earlier decisions:** packet-log backfill for a peer session ([0.12.1]), live split-tunnel toggles ([0.15.0]), OS route management (E.2), and relay/TURN fallback.
- **Mobile targets** (Android / iOS / iPadOS) are listed as intended but have never been built or tested.

---

## [0.15.1] - 2026-06-25

Quality-consolidation pass after fifteen fast-moving releases. No behaviour change; the point was to make the compiler and linter enforce what was previously enforced by hand.

### Changed
- **`cargo clippy` now passes clean under `-D warnings`** — it had never been run before this. Fourteen idiomatic fixes were applied (`io::Error::other(..)` for `ErrorKind::Other` sites, `is_some_and`/`is_none_or` for `map_or`, `is_none()` for redundant `matches!`, an unnecessary same-type cast), plus a type alias for a complex FEC test signature.
- **Dead code is now caught rather than masked.** The blanket `#![allow(dead_code)]` on `crypto/handshake.rs` was hiding four items — `initiate`, `respond`, `recv_handshake`, `HANDSHAKE_TIMEOUT` — that became dead in production when the C4 fan-out replaced the single-target handshake; they are used only by tests. They are now `#[cfg(test)]` (so they cost nothing in release), and the blanket allow is gone, so any *future* dead code in that module is a warning again.
- Removed a **stale** `#[allow(dead_code)]` on `TunDevice::write_frame` — it has been live since the C5 data-plane bridge shipped.
- Versions aligned to `0.15.1`.

### Verified
- `cargo clippy --all-targets -- -D warnings` — clean. `cargo test` — **62/62 pass**. `tsc` clean.

### Note
- `TunDevice::info` / `DeviceInfo` remain genuinely unused and keep an explicit `#[allow(dead_code)]` as intentionally-retained trait contract for a future diagnostics readout. Left in place rather than deleted, since removing production trait surface warrants a deliberate decision, not a drive-by cleanup.

---

## [0.15.0] - 2026-06-24

### Added
- **Phase F.1 — the engine's work is now visible.** Two counters the engine had been keeping to itself are surfaced on the Diagnostics readout:
  - **FEC recovered** — packets rebuilt from parity instead of being lost.
  - **Blocked** — packets the split-tunnel policy refused, in *either* direction (egress, ingress, and FEC-recovered packets that fail the ingress check are all counted).
  Both are **cumulative for the connection**, not rates: "this link has recovered 47 packets" is the useful framing, whereas a per-second figure rounds to zero on a healthy link — precisely when the number should be reassuring.

### Changed
- `TelemetrySnapshot` gains `fecRecovered` and `policyBlocked`, and now derives `Default`; producers that do not measure these fill them via `..Default::default()`. `EngineState` derives `Default` (`Idle`) to make that possible. This stops every future field addition from forcing an edit at each construction site across the engine.
- The stat grid goes from four tiles to six (`2 → 3 → 6` columns as the window widens), and `StatTile` accepts an optional tooltip explaining what a number means.

### Not done, deliberately
- The **packet-log backfill** noted in [0.12.1] remains open. Fixing it means coupling the peer link to `EngineController`'s shared state, and it only affects a UI remount mid-session — the live event stream is unaffected. Architectural coupling for a rarely-hit path is not a good trade; it stays a documented limitation.
- **Live split-tunnel toggles** are deferred: they need a control channel into the running pipeline, which is its own piece of work.

### Verified
- `cargo test` — **62/62 pass**, warning-free. `tsc` clean.
- **Browser-verified**, not merely typechecked: the grid renders exactly six tiles with the right labels, units and tooltips; the layout collapses 6 → 2 columns at mobile width with **no horizontal overflow**; zero console errors.

---

## [0.14.0] - 2026-06-23

### Added
- **Full-path integration harness (Phase F.0).** Two complete `pipeline::run` tasks are now connected over loopback with a mock adapter on each side, so a packet handed to one virtual adapter must emerge from the other having travelled the **real** path — hole-punch handshake, Noise session, FEC, split-tunnel policy, UDP transport and the data-plane bridge. Four tests: the handshake completes through the fan-out (ignoring a dead candidate); **a packet crosses the tunnel byte-identical**; the split-tunnel policy blocks an out-of-subnet packet on the live path; and cancellation returns both sides to `Idle`.
- `DataPlaneSource` — the seam that makes this possible. `None` / `Adapter(TunConfig)` are the production paths; a `Device(..)` variant lets a test drive a mock adapter and is `#[cfg(test)]`, so it does not exist in release builds.
- A reusable `MockTun` in `engine::dataplane`, replacing the ad-hoc mock the bridge test carried.

### Changed
- `pipeline::run` takes a `DataPlaneSource` in place of `Option<TunConfig>`; `connection.rs` builds one via `dataplane_source_for(role)`.
- Versions aligned to `0.14.0` across `Cargo.toml`, `package.json`, `tauri.conf.json`.

### Verified
- `cargo test` — **62/62 pass**, warning-free (4 new integration tests).
- **The harness was mutation-tested to prove it has teeth:** deliberately breaking the downlink injection made exactly the two traffic tests fail with timeouts, while the handshake and teardown tests correctly still passed. The change was then reverted and the suite re-run green.
- `tsc` clean.

### ⚠️ Verification status — corrected
Earlier entries (C4 through D.2) each carried a *"Pending manual verification — two-machine test"* note. **That test has not been performed and cannot currently be performed by the maintainer.** Those notes are superseded by this entry, and the honest position is:

| Covered automatically, end to end | Still unverified |
| --- | --- |
| Hole-punch handshake and candidate fan-out | **Real NAT traversal** — loopback has no NAT, so whether hole-punching survives real-world NATs is unproven |
| Noise session, seal/open, anti-replay | The real **Wintun** driver (mocked in tests) |
| Data-plane bridge, uplink and downlink | Windows **firewall / routing** interaction |
| FEC encode, transmit and reconstruct | Real-world latency, loss and MTU behaviour |
| Split-tunnel egress and ingress policy | |
| Connect / disconnect lifecycle | |

**NAT traversal is the material open risk.** It is also the reason relay/TURN fallback has *not* been built yet: it could not be verified either, and adding unverifiable code on top of unverified code compounds the problem rather than reducing it. A checklist for the two-machine test is kept at `DOC/Two_Machine_Verification.md` (outside this repository) for whenever a second machine is available.

---

## [0.13.0] - 2026-06-22

### Added
- **Networking engine — Phase D.2 (Reed-Solomon FEC).** The data plane now recovers **any `r` losses per group** instead of D.1's single loss:
  - `engine/fec/rs.rs` *(new)* — `RsEncoder` rolls a group of `k` packets and emits `r` parity shards on close; `RsDecoder` buffers a group until **any `k`** of its `k + r` shards have arrived, then reconstructs every missing packet at once. Shard construction is unchanged from D.1 (length-prefixed, zero-padded to the group's longest packet), so **only the parity costs bandwidth** — data packets still travel at their natural length.
  - Burst loss is the case this addresses: XOR parity gave up the moment a group lost two packets, which is exactly how real networks drop traffic.

### Changed
- **FEC wire format.** `FEC_PARITY` is now `[5][group:4][k:1][r:1][parity index:1][shard…]` — it previously carried no `r` or shard index. ⚠️ **The FEC path is therefore incompatible with a v0.12.1 peer**; both ends must run the same version. `FEC_DATA` is unchanged.
- `RS(k, 1)` is equivalent to the XOR parity it replaces, so the previous behaviour remains available by configuring `r = 1` — which is the shipped default, keeping overhead at 1/k until the geometry is made configurable.
- **Removed `engine/fec/xor.rs`** — Reed-Solomon is a strict superset, and maintaining two equivalent codecs was avoidable duplication. Its five tests are subsumed by the nine in `rs.rs`, including one that pins `r = 1` to the old behaviour.
- New dependency **`reed-solomon-erasure` 6.0.0** (MIT). Default features only; `simd-accel` is deliberately not enabled as it would pull a C toolchain. Attributed in `THIRD-PARTY-NOTICES.md`. The `alloc-no-stdlib`/`brotli` lock pins were re-verified intact after the graph change.
- Versions aligned to `0.13.0` across `Cargo.toml`, `package.json`, `tauri.conf.json`.

### Security
- **Peer-supplied FEC geometry is untrusted and range-checked.** `k`, `r` and the shard index arrive from the peer, so they are bounded (`k ≤ 64`, `r ≤ 16`, index `< r`, shard `≤ 2048` bytes) before any allocation, and a codec error is handled rather than unwrapped — a malformed group is dropped, never panicked on.
- **A parity shard cannot silently set an inconsistent geometry.** Writing the tests surfaced a real weakness: a shard arriving *before* any genuine parity established the group's dimensions unchallenged, and could drive a reconstruction that produced garbage. A shard shorter than a data packet already held is now *provably* inconsistent and rejected, and shards disagreeing with an established geometry are ignored.

### Verified
- `cargo test` — **58/58 pass**, warning-free (9 new, 5 retired with the XOR codec): recovers two losses with two parity; three losses with two parity yield **nothing** rather than corrupt data; `r = 1` reproduces the D.1 behaviour exactly; parity-first and reordered arrival; parity-only loss is a no-op; flush closes a partial group and still recovers; malformed/hostile headers rejected; geometry disagreement rejected; group buffer stays bounded.
- **End-to-end** — two handshake-established sessions, **two** data packets dropped in transit, both reconstructed byte-identical through the real `seal → transport → open` path.
- `tsc` clean.
- **Pending manual verification (your run):** on a lossy link, confirm recovery still holds where D.1 would have given up (two losses inside one group of eight).

---

## [0.12.1] - 2026-06-21

Hardening from a **complete** adversarial review of the E.1 data plane (21 agents, 0 failures, 16 raw findings, **6 confirmed**, 0 unverified — unlike the truncated run recorded in 0.12.0). The 6 confirmed findings deduplicated to 4 defects; 3 are fixed here and 1 is documented below as a known limitation.

### Security
- **Ingress now validates the source, not just the destination.** Decryption proves a frame came from the authenticated peer; it proves nothing about the *inner* IP header, which the peer writes freely. The 0.12.0 downlink filter checked only the destination, so a hostile peer could forge a source — `127.0.0.1`, or our own virtual address — and reach services that trust loopback or the local subnet. New `SplitPolicy::admits_inbound` applies a reverse-path test: the **source** must be on the virtual LAN and never our own address (no self-spoofing), and the **destination** must be us or an accepted broadcast/multicast — never a phantom in-subnet host, which the OS could forward off-box.

### Fixed
- **Packet-log batch is now bounded.** `drive()` accumulated log entries in an unbounded `Vec` drained only once per keepalive second, then handed the whole batch to the IPC layer in a single `emit`. At high packet rates this both grew the buffer within a tick and produced one enormous payload to serialize. Entries are now capped (`PACKET_LOG_BATCH_CAP = 256`) with the overflow **counted and reported** as a summary line, so the gap is visible rather than silent. (Pre-existing on the send path; widened by E.1's drop-path logging.)
- **The packet log no longer claims undelivered packets were injected.** `inject_downlink` returned "injected" even when nothing was written — no channel, or a full adapter queue. It now reports `Delivered` / `Blocked` / `Queueless`, so a full-queue drop shows as `adapter queue full — packet dropped` instead of appearing as a clean receive while the adapter got nothing.

### Known limitation
- **`get_packet_log` returns empty for a peer session.** `capture.rs` and `probe.rs` push entries into the bounded `RingBuffer` in `SharedState`; the peer-link `drive()` only emits live `telemetry://packet` events, because it is handed a `&dyn TelemetrySink` and has no `SharedState` handle. The live stream is unaffected — only the pull-on-late-mount path returns nothing. Wiring it would couple the peer link to `EngineController`'s shared state, so it is deferred rather than rushed.

### Verified
- `cargo test` — **54/54 pass**, warning-free (4 new: forged-source rejection incl. `127.0.0.1` and self-spoofing, ingress destination restriction incl. phantom hosts, ingress broadcast/multicast toggles, and the bounded log batch reporting its suppression count).
- `tsc` clean; versions aligned to `0.12.1` across `Cargo.toml`, `package.json`, `tauri.conf.json`.

---

## [0.12.0] - 2026-06-20

### Added
- **Networking engine — Phase E.1 (split tunnelling).** The data plane now applies an explicit policy to every packet crossing the tunnel:
  - `engine/split_tunnel/mod.rs` *(new)* — `SplitPolicy`, a pure function of the destination address: **in-subnet unicast** → tunnel; **broadcast** (limited `255.255.255.255` and subnet-directed, e.g. `10.77.0.255`) and **multicast** (`224.0.0.0/4`) → tunnel *when enabled*, so LAN-discovery games find each other while the flooding stays gated; **everything else** → drop. Includes `Ipv4Cidr`, prefix→mask arithmetic, and `dst_ipv4()` (destination at octets 16..20, guarded on length and IP version).
  - `pipeline.rs` — the uplink pump gates every outbound packet through the policy before it is sealed; blocked packets are logged, never sent. The policy is derived from the data-plane `TunConfig` in `run()`.
- Because the virtual adapter is a layer-3 `/24`, native and internet traffic never reaches the adapter in the first place — this layer adds explicit control over what the tunnel *does* carry.

### Changed
- `engine/mod.rs` registers `pub mod split_tunnel`.
- Versions aligned to `0.12.0` across `Cargo.toml`, `package.json`, `tauri.conf.json`.

### Security
Hardened after an adversarial multi-agent review of the new egress policy (findings triaged manually — see *Verified*):
- **The gate is fail-closed.** The uplink/downlink check was centralised in `admits_packet(policy, frame)`, which admits **nothing** when the policy is absent. Previously an absent policy meant "admit", so a leak-prevention filter's correctness rested on two locals staying coupled rather than on the gate itself.
- **Downlink is now filtered symmetrically.** Inbound `Data`, `FEC_DATA`, and FEC-recovered packets pass the same destination policy before injection, so an authenticated-but-hostile peer cannot inject packets for arbitrary destinations onto our adapter. FEC bookkeeping still sees every packet, so legitimate losses stay recoverable.
- **The allow-list can no longer widen to the internet.** `prefix_len` is an unvalidated `u8` from config; a `/0` would have made the allow-list `0.0.0.0/0` and admitted every unicast destination. A prefix outside the plausible LAN range (`/8`–`/32`) now collapses to `/32` — the fail-closed direction.

### Verified
- `cargo test` — **50/50 pass**, warning-free (10 new: CIDR/prefix arithmetic, subnet/broadcast derivation at `/24`, **`/16` and `/25`** (guards against a hardcoded-last-octet regression), unicast classification, broadcast + multicast forwarding, both toggles gating, `dst_ipv4` parse/reject incl. IPv6 and short frames, `admits()` on full frames, **nonsensical prefixes failing closed**, and the **gate polarity + fail-closed default**).
- `tsc` clean.
- **Review caveat, recorded honestly:** the adversarial workflow reported `confirmedCount: 0`, but **4 of its verify agents died on a session limit**, so that figure is *inconclusive, not an all-clear*. The raw finder output was read from the run journal and triaged by hand: 5 findings across 5 dimensions (2 dimensions clean), 3 acted on above, 2 covered by the new tests. No finding indicated a live leak in the shipped configuration — all three fixes harden latent/defensive gaps.
- **Pending manual verification (your run):** with two peers connected, run a LAN-discovery game/tool and confirm they find each other; then disable broadcast forwarding and confirm discovery stops — proving the gate works.

---

## [0.11.0] - 2026-06-19

### Added
- **Networking engine — Phase D.1 (Forward Error Correction, XOR parity).** The tunnelled data plane now recovers isolated packet losses **without retransmission**:
  - `engine/fec/xor.rs` *(new)* — a single-parity XOR erasure code. `XorEncoder` rolls a group of up to `k=8` data packets and, when the group closes (full or idle-flushed), emits one parity packet = the XOR of its length-prefixed, zero-padded members. `XorDecoder` buffers each group (bounded, tolerant of reordering and late parity) and reconstructs the **single** missing member exactly — original length included — from the parity; **two or more** losses in a group are reported unrecoverable rather than emitting corrupt data.
  - `engine/fec/mod.rs` *(new)* — module + re-exports; documents the FEC-then-encrypt placement and the D.2 Reed-Solomon path.
  - `pipeline.rs` — the data-plane pumps now run FEC **over the inner IP plaintext**: uplink packets are sent immediately as `FEC_DATA` (never delayed) with a `FEC_PARITY` following when the group closes or a ~30 ms idle-flush timer fires; downlink `FEC_DATA` is injected immediately and fed to the decoder, and a `FEC_PARITY` may reconstruct a missing packet, which is then injected. Two inner tags (`FEC_DATA=4`, `FEC_PARITY=5`) join `PING`/`PONG`/`DATA`.
- Recovered packets and parity surface in the Diagnostics packet log (`FEC-PAR` sends, `recovered via FEC` injections); the data-plane notice now reads `… · FEC on (XOR k=8)`.

### Changed
- `engine/mod.rs` registers `pub mod fec`. FEC is enabled **per-direction** whenever the data plane is up (control-only links have none) and is transparent to a peer running the same codec — no signaling change.
- Versions aligned to `0.11.0` across `Cargo.toml`, `package.json`, `tauri.conf.json`.

### Security
- **FEC runs inside the encryption boundary:** parity is computed over inner IP *plaintext*, and every data and parity packet is individually sealed. The receiver decrypts each, then recovers on plaintext — so a reconstructed packet needs no per-packet nonce, and the C4 single-session guarantee and 64-packet anti-replay window are untouched.
- **No corrupt output:** the decoder reconstructs only when exactly one member of a group is missing; 2+ losses yield nothing. Group buffers are bounded (64 recent groups, FIFO-evicted), so loss or reordering cannot grow memory without bound.
- **Latency-safe redundancy:** data packets are never buffered for FEC — only parity is additive — so the no-loss path adds zero latency; only genuinely-lost packets pay the (bounded) recovery delay.

### Verified
- `cargo test` — **40/40 pass**, warning-free (6 new: XOR recovers a single loss; recovers with parity-first / reordered arrival; reports a 2-loss group unrecoverable; flush closes a partial group and recovers; no-loss recovers nothing; **end-to-end** — a data packet dropped in transit is reconstructed from parity through the real seal → transport → open path across two handshake-established sessions).
- `tsc` clean.
- **Pending manual verification (your run):** on a lossy link (or with induced loss), confirm ping/game latency is visibly steadier with FEC on, and the packet log shows `recovered via FEC` entries.

---

## [0.10.0] - 2026-06-18

### Added
- **Networking engine — Phase C5 (data-plane join).** The C4 peer link now carries **real IP traffic** between the two virtual adapters:
  - `engine/dataplane/mod.rs` *(new)* — a bridge coupling the blocking Wintun `TunDevice` to the async driver via a **bounded channel pair**. One dedicated blocking thread interleaves **uplink** (`read_frame` → `tokio::mpsc`) and **downlink** (`std::mpsc::sync_channel` → `write_frame`), blocking briefly on the downlink when the device is idle so injected packets stay low-latency. `open()` creates the adapter off-thread (so `connect` never stalls); `spawn_bridge()` runs the loop over an already-open device (mockable in tests).
  - `pipeline.rs` — the steady-state driver gains **uplink/downlink pumps**: outbound IP packets are sealed and sent to the nominated peer; inbound `Data` frames are decrypted and injected onto the adapter. Payloads inside the encrypted `Data` frame now carry a **1-byte inner tag** (`PING`/`PONG`/`DATA`) so keepalive control and tunnelled IP share one channel (an IPv4 packet's `0x45` first byte never collides). Real tx/rx **throughput** is now measured and reported.
  - `connection.rs` — `connect` opens the data plane only when a real adapter is available (**Windows + elevated**), assigning a **role-based virtual IP** for the point-to-point LAN (Initiator `10.77.0.1`, Responder `10.77.0.2`, `/24`, MTU 1420). Otherwise the link runs **control-only** (encrypted keepalive), unchanged from C4. The adapter's lifetime equals the Connected phase — it is joined and released on disconnect before the link reports Idle.
- **Diagnostics:** a **data plane / control-only** badge in the Peer-connection panel (driven by the `data_plane` / `data_plane_off` notice); the throughput tiles and packet log now surface real tunnelled traffic, classified by protocol.

### Changed
- `engine/mod.rs` registers `pub mod dataplane`; `pipeline::run` now takes an `Option<TunConfig>` (the role-assigned data-plane config, `None` → control-only).
- The C4 keepalive payload codec was generalized into the shared inner-frame codec (`PING`/`PONG`/`DATA`).
- Versions aligned to `0.10.0` across `Cargo.toml`, `package.json`, `tauri.conf.json`.

### Security
- **One `CryptoSession`, one owner:** `seal` (uplink + keepalive) and `open` (downlink) both run in the single async driver task — no cross-thread sharing, no lock — so the C4 single-session guarantee and the 64-packet anti-replay window are preserved unchanged. The TUN bridge thread only touches the channels and the device, never the session.
- **Backpressure cannot stall the link:** both channel directions are bounded and **drop-newest** on saturation (game traffic favours fresh over buffered); a slow consumer never back-pressures the async driver or the device.
- **Data plane is gated on elevation:** without a real adapter (unelevated / non-Windows) the link degrades to a control-only encrypted keepalive with a clear notice — it never silently pretends to tunnel.

### Verified
- `cargo test` — **34/34 pass**, warning-free (5 new, 1 retired: data-plane bridge pumps both directions via a mock device; inner-codec `PING`/`PONG`/`DATA` round-trips + rejects unknown/short; **end-to-end** — a tunnelled IP packet survives `dp_encode → seal → Data frame → transport → open → decode_inner` across two handshake-established sessions, no adapter needed).
- `tsc` clean.
- **Pending manual verification (your run):** two-machine test — both elevated, exchange offer/answer, **Connect** → **Connected** showing the **data plane** badge, then `ping 10.77.0.2` from the `.1` side succeeds while the packet log + throughput tiles show real traffic; then a LAN game.

---

## [0.9.0] - 2026-06-17

### Added
- **Networking engine — Phase C4 (hole-punch-as-handshake).** The negotiated peer (C3) is now turned into a live, authenticated P2P link:
  - `crypto/handshake.rs` — **`initiate_fanout`** builds **one** Noise IK message-1 and broadcasts the identical bytes to *every* peer candidate (each send punches that NAT path), retransmitting every 250 ms until the first valid message-2. Inbound handshake frames are fed one-at-a-time into the **single** `HandshakeState`; the first that validates is consumed by `into_transport_mode()`, and the winning datagram's source address is the **nominated endpoint**. **`respond_punch`** punches the initiator's candidates with `Ping` probes while awaiting message-1, authorizes the initiator's static key, replies message-2, and caches it for retransmit. Both honor an 8 s overall deadline and a `watch` cancellation channel.
  - `pipeline.rs` *(new)* — the post-handshake **session driver**: promotes the nominated `CryptoSession`, then runs an **encrypted keepalive** (ping/pong sealed inside `Data` frames) to the peer for real RTT/jitter/loss over the authenticated channel, re-answers a duplicate message-1 with the cached message-2 (handles a lost reply), and streams telemetry through the existing sink. The C5 data plane layers onto this same driver.
  - `connection.rs` — **`connect`/`disconnect`** orchestration with a `LinkState` (`idle`/`connecting`/`connected`/`failed`) guard that permits **one session per peer**; spawns `pipeline::run` for the negotiated `Role`, de-dupes candidates, and tears the task down on disconnect.
- **`connect_peer` / `disconnect_peer` IPC commands** and a **Connect / Connecting… / Disconnect** control in the Diagnostics "Peer connection" panel, with a live `link` readout. The lifecycle badge shows **Connecting** across the ≤ 8 s handshake window, then **Connected** (or **Error** on timeout).

### Changed
- `transport/frame.rs`: `Handshake`/`Data` frames are now live (their `dead_code` allowances dropped); `crypto/session.rs` likewise — `seal`/`open` drive the C4 keepalive.
- `engine/mod.rs` registers `pub mod pipeline`; `lib.rs` registers the 2 new commands.
- Versions aligned to `0.9.0` across `Cargo.toml`, `package.json`, `tauri.conf.json`.

### Security
- **Single-session guarantee is structural, not a check:** there is exactly one `HandshakeState` per side; `into_transport_mode()` consumes it by value, so no later/duplicate response can spawn a second session. `snow`'s `read_message` is transactional (it checkpoints and restores the symmetric state on failure), so a spoofed or garbage handshake frame rolls back cleanly and cannot poison the real handshake.
- **Initiator authorization preserved from C2:** the responder accepts only a message-1 whose static key matches the expected peer; an impostor who holds our public key produces a valid IK message-1 but is **discarded** (the responder rebuilds and keeps waiting for the genuine peer) rather than aborting the attempt — resisting a handshake-race DoS.
- The keepalive runs **inside** the encrypted session (AEAD + 64-packet anti-replay), not in cleartext.
- Symmetric-NAT ↔ symmetric-NAT relay/TURN fallback is intentionally deferred; an un-punchable pair reports a clean timeout + notice.

### Verified
- `cargo build` warning-free; `cargo test` — **30/30 pass** (4 new: fan-out nominates the live candidate and ignores a dead one with a working encrypted echo; responder waits out an unexpected initiator to a clean timeout; `cancel` aborts an in-flight handshake promptly; keepalive payload round-trip).
- `tsc` clean; preview confirms the Connect / Connecting… / Disconnect controls render and gate correctly.
- **Pending manual verification (your run):** two-machine test — exchange offer/answer, click **Connect** on both, reach **Connected** with matching fingerprints and **live RTT** between the two networks.

---

## [0.8.0] - 2026-06-14

### Added
- **Networking engine — Phase C3 (manual signaling).** New `engine/signaling/` module:
  - `message.rs` — `SignalEnvelope { v, kind, sid, pk, cands }` + `WireCandidate` (compact field names).
  - `blob.rs` — the paste-robust codec **`PCPV1.<KIND>.<base64url(json)>.<crc32>`**. `decode` validates magic/version, CRC32 (catches paste corruption before any crypto), kind/label agreement, 32-byte key length, and candidate-address parsing — with friendly typed errors.
- **`engine/connection.rs`** — `ConnectionManager`: binds the shared UDP socket **once** and gathers candidates (the same NAT mapping C4 will punch toward); tracks role, session id, and the negotiated peer.
- **Identity refactor** — extracted `crypto::identity::fingerprint_of` so a peer's public key renders the same `PC-XXXX-XXXX-XXXX-XXXX` address as our own.
- **Signaling IPC + UI:** `create_offer` / `accept_offer` / `accept_answer` / `get_connection` commands, and a **"Peer connection"** panel in Diagnostics (create offer → copy; paste peer blob → produce answer / finalize; live role + negotiated-peer status).

### Changed
- `Cargo.toml`: added `crc32fast`; crate → `0.8.0`.
- `lib.rs`: manages `ConnectionManager`; registers the 4 signaling commands.
- Versions aligned to `0.8.0` across `Cargo.toml`, `package.json`, `tauri.conf.json`.

### Security
- The blob carries only public data (public key + candidate addresses); the CRC32 is integrity-against-corruption, **not** authenticity. Authenticity comes from the out-of-band channel, fingerprint verification, and the C4 IK handshake (which authorizes only the expected key) — documented in `blob.rs`.

### Verified
- `cargo check`/`build` warning-free; integrated `tauri build --no-bundle --debug` links the binary.
- `cargo test` — **26/26 pass** (6 new blob tests: round-trip, checksum corruption, truncation/bad-prefix, kind/label mismatch, bad version, bad key/candidate).
- `tsc` clean; preview confirms the "Peer connection" panel renders and the client-side blob validation rejects malformed input (zero console errors).
- **Pending manual verification (your run):** two-machine offer/answer exchange showing **"Negotiated with PC-… · N candidates"** and matching fingerprints on both ends.

---

## [0.7.0] - 2026-06-14

### Added
- **Networking engine — Phase C2 (crypto & identity).** New `engine/crypto/` module:
  - `identity.rs` — persistent static **X25519** identity (`identity.json`, versioned). Generated on first run via `snow`; the private key is written **owner-only (`0600`) on Unix** (Windows inherits the per-user AppData ACL) and never logged or shared. Derives `publicKeyB64` (canonical, for signaling) and a short fingerprint `peerAddress` (`PC-XXXX-XXXX`, SHA-256 of the public key) for human verification.
  - `handshake.rs` — **Noise IK** (`Noise_IK_25519_ChaChaPoly_BLAKE2s`) over the shared UDP transport: 2-message, mutually authenticated `initiate`/`respond`. The initiator message is structured to double as the C4 hole-punch.
  - `session.rs` — `CryptoSession`: AEAD seal/open with an explicit per-packet counter; the replay window is advanced only **after** authentication.
  - `replay.rs` — 64-packet sliding-window anti-replay filter (`check`/`accept`).
- **`Handshake`/`Data` wire frames** in `transport/frame.rs` (`encode_handshake`, `encode_data`/`decode_data`) — slotting into the existing C1 demux.
- **Identity IPC + UI:** `get_identity` command and a **"This node"** Diagnostics readout showing the Peer Address with a copyable public key (sets up C3 manual signaling).

### Changed
- `Cargo.toml`: added `snow`, `base64`, `sha2`; crate → `0.7.0`.
- `lib.rs`: `setup()` loads/generates the identity into the app config dir and shares it via managed state.
- Versions aligned to `0.7.0` across `Cargo.toml`, `package.json`, `tauri.conf.json`.

### Security
- The crypto layer was reviewed by a multi-agent adversarial pass (independent reviewers per dimension — Noise usage, nonce/replay, key storage, fingerprint, IPC surface; the DoS/panics reviewer and the automated verifiers were cut short by a session limit, so the 7 raw findings were triaged manually). Fixes applied:
  - **Responder authorization** — `handshake::respond` now verifies the initiator's static key against the expected peer (Noise IK transmits but does not authorize the initiator); rejects mismatches before replying. Covered by a new test.
  - **Private-key zeroization** — the in-memory private key is held in a `zeroize::Zeroizing` buffer (wiped on drop).
  - **Enforced `0600`** — `set_permissions` is applied after writing so the mode holds even if the key file pre-existed with looser permissions.
  - **Corruption resilience** — an unreadable `identity.json` is backed up to `identity.corrupt` and regenerated instead of failing app launch; key lengths are validated on load.
  - **Stronger fingerprint** — the Peer Address widened from 32-bit to **64-bit** (`PC-XXXX-XXXX-XXXX-XXXX`) for safer voice/chat verification; the full public key remains authoritative.
  - **UI clarity** — the Diagnostics readout labels the value a "fingerprint" and points users to the full public key as the identifier to share.
  - _Accepted (documented):_ on Windows the key file relies on the per-user AppData ACL (an explicit owner-only DACL is tracked as future hardening).

### Verified
- `cargo check`/`build` warning-free; integrated `tauri build --no-bundle --debug` links the binary.
- `cargo test` — **19/19 pass**, including a full localhost **Noise IK handshake → encrypted echo → replay-rejected → tamper-rejected** test, identity persist/reload, peer-address determinism, and the anti-replay window (reorder/duplicate/too-old).
- `tsc` clean; preview confirms no regression (identity readout correctly hidden without a Tauri runtime), zero console errors.
- **Pending manual verification (your run):** stable Peer Address shown in Diagnostics and unchanged across restarts (persistence).

---

## [0.6.0] - 2026-06-14

### Added
- **Networking engine — Phase C1 (transport + STUN + RTT keepalive).**
  - `engine/transport/socket.rs` — `UdpTransport`: one shared, `socket2`-tuned (`SO_REUSEADDR`) UDP socket, payload-agnostic (send/recv/local_addr only). The same bound port serves STUN, Ping/Pong, and future encrypted data so the NAT mapping is consistent.
  - `engine/transport/frame.rs` — the demux: STUN identified by magic cookie (`0x2112A442` @ offset 4); otherwise a 1-byte `FrameKind` tag (`Ping`/`Pong`, `Handshake`/`Data` reserved). `classify()` + Ping/Pong (de)framing.
  - `engine/transport/keepalive.rs` — `RttTracker`: Ping/Pong sequence correlation → EWMA RTT + jitter, with a loss-timeout window.
  - `engine/nat/stun.rs` — minimal hand-rolled STUN binding client → `XOR-MAPPED-ADDRESS` (reflexive address); borrows the shared transport, retransmits.
  - `engine/nat/candidate.rs` — host (primary local IPv4) + reflexive candidate gathering.
  - `engine/transport/probe.rs` — the C1 probe task: bind → STUN → Ping/Pong (loopback by default, `probeTarget` for true network RTT) feeding live `rttMs`/`jitterMs`/`lossPct` + Ping/Pong packet-log lines into the telemetry spine.
- **Config:** `transportProbe`, `stunServer` (default `stun.l.google.com:19302`), `bindPort`, `probeTarget`. `EngineNotice::info` for the discovered-candidates readout.
- **Frontend:** a **mode selector** (Simulated · Transport probe · Real adapter) replacing the single Expert checkbox, plus a reflexive/candidates info readout in Diagnostics. Stale notices clear on return to Idle.

### Changed
- `Cargo.toml`: tokio `net` feature + `socket2`; crate → `0.6.0`.
- `controller.rs` mode precedence: real-TUN → transport-probe → simulator.
- Versions aligned to `0.6.0` across `Cargo.toml`, `package.json`, `tauri.conf.json`.

### Verified
- `cargo check`/`build` warning-free; integrated `tauri build --no-bundle --debug` links the binary.
- `cargo test` — **13/13 pass** (new: STUN `XOR-MAPPED-ADDRESS` parse + txid rejection, frame demux STUN-vs-Ping/Pong, RTT-tracker math).
- `tsc` clean; browser preview confirms the mode selector (3 options, subtitle reacts) and the simulated path with zero console errors.
- **Pending manual verification (your run):** live reflexive discovery against public STUN + loopback/peer RTT — see the C1 recipe in the status report.

---

## [0.5.0] - 2026-06-14

### Added
- **Networking engine — Phase B (TUN/TAP management).** New `src-tauri/src/engine/tun/` module:
  - `device.rs` — platform-neutral `TunDevice` trait + `TunConfig` (default virtual IP `10.77.0.1/24`, MTU 1420).
  - `windows.rs` — `WintunDevice`: loads the bundled signed `wintun.dll`, creates the adapter, assigns the IPv4 via `netsh`, and exposes the session as a `TunDevice` (`#[cfg(windows)]`).
  - `privilege.rs` — elevation detection (`TOKEN_ELEVATION` via `windows-sys`) and relaunch-elevated (`ShellExecuteW` `runas`); Tauri-agnostic.
  - `packet.rs` — minimal IPv4/IPv6 header classification (proto + length) for the packet log, no extra deps.
- **Real packet capture** (`engine/telemetry/capture.rs`): when Expert mode is enabled, frames are read off the adapter on a blocking thread, classified, and fed into the existing telemetry spine (live throughput + packet log; latency stays zero until the transport lands). Captured frames are dropped — no forwarding yet.
- **Graceful elevation lifecycle:** new `EngineState::Starting` and `EngineState::NeedsElevation`; the controller gates real-adapter mode on elevation and emits a structured `engine://notice` with remediation instead of failing.
- **Privilege IPC:** commands `get_privilege_status` and `request_elevation`; `engine://notice` event; `EngineConfig` gains `useRealTun` + `tun`.
- **Frontend:** privilege/notice in the telemetry store, an Expert **"Real adapter (Admin)"** toggle, a **Needs Admin** banner with a one-click **Relaunch as Administrator**, and `Starting`/`needs-elevation` state styling.
- **Bundled Wintun 0.14.1** (amd64/arm64/x86), signed by WireGuard LLC, under `src-tauri/resources/wintun/` (+ `NOTICE.txt`); wired into `tauri.conf.json` bundle resources.

### Changed
- `Cargo.toml`: added `[target.'cfg(windows)'.dependencies]` — `wintun`, `windows-sys`; crate version → `0.5.0`.
- `controller.rs` branches between the simulator and the real capture loop; `state.rs`/`config.rs` extended.
- Versions aligned to `0.5.0` across `Cargo.toml`, `package.json`, `tauri.conf.json`.

### Verified
- `cargo check`/`build` warning-free; integrated `tauri build --no-bundle --debug` links the binary.
- `cargo test` — 6/6 pass: IPv4/IPv6 packet classification (UDP/ICMP/TCP, short/unknown rejection) + the seeded-simulator suite.
- `tsc --noEmit` clean; browser preview confirms the simulated readout, the Expert toggle (subtitle switches to "real adapter (Expert)"), and graceful no-Tauri fallback with zero console errors.
- **Pending manual verification (requires an elevated run):** real Wintun adapter creation + live capture — see the manual recipe in the status report.

---

## [0.4.0] - 2026-06-14

### Added
- **Networking engine — Phase A (telemetry spine).** New Rust modules under `src-tauri/src/engine/`:
  - `controller.rs` — lifecycle (start/stop) over a `tokio` task hosted on Tauri's shared runtime, with `watch`-based shutdown; owns the shared live state.
  - `config.rs` — `EngineConfig` (peer label, tick rate, `SimProfile`, optional PRNG `seed`).
  - `state.rs` — `EngineState` lifecycle + thread-safe `SharedState` backing the pull commands.
  - `error.rs` — `EngineError` serialized to the UI as a readable string.
  - `telemetry/` — `TelemetrySnapshot` + bounded packet-log `RingBuffer`, the `TelemetrySink` trait (keeps the engine Tauri-agnostic), a **seedable** `Simulator` (reproducible runs with a seed, OS entropy otherwise), and the async emission loop.
- **IPC bridge** (`src-tauri/src/commands/`): commands `start_engine`, `stop_engine`, `get_status`, `get_snapshot`, `get_packet_log`; events `telemetry://stats`, `telemetry://packet`, `engine://state`; `TauriSink` forwards telemetry via `AppHandle::emit`.
- **Frontend consumer:** `types/telemetry.ts` (DTO mirrors), an **ephemeral** `telemetryStore` (not persisted), `lib/engine.ts` (typed `invoke`/`listen` wrappers), the `useEngineTelemetry` subscription hook, and a rebuilt **Diagnostics** page — connection-state badge (semantic colors), live RTT/jitter/loss/throughput tiles, and a terminal-style packet log.

### Changed
- `Cargo.toml`: added `tokio`, `thiserror`, `rand`; crate version → `0.4.0`.
- `lib.rs`: registers the `EngineController` in managed state and the five new commands.
- Versions aligned to `0.4.0` across `Cargo.toml`, `package.json`, `tauri.conf.json`.

### Verified
- `cargo check` / `cargo build` warning-free; integrated `tauri build --no-bundle --debug` links the binary with the new commands.
- Unit tests (`cargo test`): seeded simulator runs are bit-for-bit reproducible; generated metrics stay within range.
- `tsc --noEmit` clean; Diagnostics renders the Idle baseline in the browser preview (graceful no-Tauri fallback, zero console errors); Start handler swallows expected rejections.

---

## [0.3.1] - 2026-06-14

### Added
- **Persistent UI state** — the Zustand store now uses the `persist` middleware (`localStorage`, key `pcpv-app-store`, schema `version: 1`). The `theme`, `settingsOpen`, and `activeRoute` fields survive application restarts; only these serializable fields are written (`partialize`), never the action functions. Synchronous storage means the persisted theme is applied before first paint (no flash of the default palette).
- **Last-page restore** — on launch `AppShell` restores the previously active route when the app opens on the default root (`/`); an explicit deep-link to a real path still wins.

### Changed
- `appStore.ts`: wrapped the store in `persist` + `createJSONStorage`.
- `AppShell.tsx`: captures the rehydrated route at first render and navigates to it once on mount (guarded so it never overrides a deep-link).
- Bumped `package.json` and `tauri.conf.json` to `0.3.1` to align the app version with the changelog.

### Verified
- Fresh load writes the default slice to `localStorage` (`{activeRoute, theme, settingsOpen}` + `version: 1`); no action functions serialized.
- Write path: toggling the settings overlay and selecting a theme persist synchronously on each store action.
- Rehydration: a seeded slice (`theme: aurora`, `settingsOpen: true`) restores on reload — theme applied via `data-theme`, switcher `aria-pressed`, overlay reopened.
- Route restore: a seeded `diagnostics` route reopens the Diagnostics page (`#/diagnostics`, active rail, breadcrumb); deep-linking to `#/network` is not hijacked.
- `tsc --noEmit` clean; no console errors across the reload cycles.

---

## [0.3.0] - 2026-06-14

### Added
- **Application Shell** — the structural frame for all feature screens:
  - 60px icon sidebar (lucide-react) with active-state highlighting and a Settings button.
  - Breadcrumb bar synced to the active route.
  - Golden-ratio (φ ≈ 1.618) content grid; fixed 60px rail with zero layout shift across routes.
  - Hash-based routing (React Router 7) with lazy-loaded, code-split page stubs (Dashboard, Network, Diagnostics) behind a `Suspense` → Skeleton fallback.
  - Frosted-glass (Mica-style) **Settings Overlay** with `backdrop-filter` blur, sliding in from the right.
  - **Theme engine** — 6 predefined themes (Midnight, Carbon, Nebula, Abyss, Aurora, Ember) applied via `data-theme`, overriding the semantic CSS tokens app-wide.
  - **Zustand** store (`activeRoute`, `theme`, `settingsOpen`) as the single source of truth shared by the sidebar, breadcrumb, and overlay.
- Engine IPC probe (`ping`) preserved on the Dashboard.

### Changed
- `App.tsx` now composes `ThemeProvider` + hash `RouterProvider` + `AppShell` (replacing the single-card baseline).
- Added dependencies: `react-router-dom`, `zustand`, `lucide-react`, `clsx`, `tailwind-merge`.

### Verified
- Navigation across all three tabs (router + breadcrumb + active rail in sync); 60px rail constant; no horizontal overflow or layout shift.
- All 6 themes produce 6 distinct surface palettes via live CSS variables.
- Settings overlay open/close geometry (docked right at 720px / off-screen at 1100px) and frosted glass (`blur(24px) saturate(1.5)`, translucent surface).
- Golden-ratio grid measured at 598.25 / 369.734 px = 1.618.
- `ping` IPC wiring intact; integrated debug build succeeds.

---

## [0.2.0] - 2026-06-14

### Added
- Initialized the **Tauri 2.x + React + TypeScript + Vite + Tailwind v4** desktop application under `Source_Code/`.
  - Frontend: `package.json` (pnpm), `vite.config.ts`, `tsconfig.json`, `index.html`, `src/` entry, and a Tailwind v4 CSS-first theme (`src/styles/index.css`) declaring the Cyan/Violet/Amber/Red semantic tokens.
  - Backend (`src-tauri/`): `Cargo.toml`, `tauri.conf.json` (identifier `com.playerclub.privatevpn`, golden-ratio 1100×680 window), `build.rs`, `main.rs`/`lib.rs` with a `ping` IPC command, and a default window capability.
- Generated the full application icon set (desktop `.ico`/`.icns`/PNG, iOS, Android, Windows Store) into `src-tauri/icons/` from a 1024×1024 square crop of the source art.
- Verified an end-to-end build: frontend (`tsc` + Vite) and the Rust binary (`target/debug/player-club-private-vpn.exe`, ~12 MB).

### Changed
- `package.json`: enabled esbuild's build script via `pnpm.onlyBuiltDependencies`.
- `tsconfig.json`: simplified to a single config (removed the composite project reference that violated TS6310).

### Fixed
- Rust build failure in the `brotli` crate (E0277, ~36 errors). Pinned `alloc-stdlib` → 0.2.2 and `brotli-decompressor` → 5.0.1 in `Cargo.lock` to unify the graph on `alloc-no-stdlib 2.0.4`; `alloc-no-stdlib 3.0.0` is incompatible with `brotli 8.0.3`.

### Toolchain
- Installed the Rust stable MSVC toolchain (1.96.0) and Visual Studio C++ Build Tools 2022 (MSVC linker); added `~/.cargo/bin` to the user PATH.

---

## [0.1.2] - 2026-06-14

### Removed
- Redundant `win/` folders under `Package_Program/` and `Package_Program_Installer/` (empty placeholders superseded by the canonical `Windows/` folders). Performed under the double-consent safety protocol.

### Notes
- Identified the application icon source: `[Not For upload]/Photo/Player_Club_Private_VPN_Icon.png` (5632×3072). To be used for icon configuration during Tauri initialization; requires cropping/padding to a square source (≥1024×1024) beforehand.

---

## [0.1.1] - 2026-06-14

### Changed
- Set target platform scope to **Desktop + Mobile**: Windows, macOS, Linux (desktop) and Android, iOS, iPadOS (mobile, via Tauri 2.x).
- Adopted the existing capitalized platform-folder convention (`Windows/`, `MacOS/`, `Linux/`, `Android/`, `IOS/`, `iPadOS/`) as canonical under `Package_Program/` and `Package_Program_Installer/`.
- Updated `README.md` project structure and added a **Target Platforms** matrix to the technology stack.

---

## [0.1.0] - 2026-06-14

### Added
- Initial repository scaffolding and project documentation.
- Top-level structure: `Source_Code/`, `Package_Program/`, `Package_Program_Installer/`, `DOC/`.
- `Source_Code/src-tauri/` Rust/Tauri backend layout: `engine/`, `commands/`, `config/`, `diagnostics/`, `game_detection/`, `utils/`, plus `capabilities/` and `icons/`.
- `Source_Code/src/` React + TypeScript frontend layout: `components/{layout,diagnostics,settings,common}`, `pages/`, `hooks/`, `stores/`, `lib/`, `styles/`, `themes/`, `i18n/locales/`, `assets/`, `types/`, and `public/`.
- Platform output folders for `Package_Program/` and `Package_Program_Installer/` (`win`, `macos`, `linux`).
- `DOC/` documentation repository: `README.md`, `Change_Log.md`, and `User_Manual/`, `Wiki/`, `API/`, `assets/` sections.
- Project `README.md` documenting overview, feature set, technology stack, architecture (Mermaid), structure, and development protocol.

[Unreleased]: #unreleased
[0.22.0]: #0220---2026-07-03
[0.21.0]: #0210---2026-07-02
[0.20.0]: #0200---2026-07-01
[0.19.0]: #0190---2026-06-30
[0.18.0]: #0180---2026-06-29
[0.17.0]: #0170---2026-06-28
[0.16.0]: #0160---2026-06-27
[0.15.3]: #0153---2026-06-26
[0.15.2]: #0152---2026-06-25
[0.15.1]: #0151---2026-06-25
[0.15.0]: #0150---2026-06-24
[0.14.0]: #0140---2026-06-23
[0.13.0]: #0130---2026-06-22
[0.12.1]: #0121---2026-06-21
[0.12.0]: #0120---2026-06-20
[0.11.0]: #0110---2026-06-19
[0.10.0]: #0100---2026-06-18
[0.9.0]: #090---2026-06-17
[0.8.0]: #080---2026-06-14
[0.7.0]: #070---2026-06-14
[0.6.0]: #060---2026-06-14
[0.5.0]: #050---2026-06-14
[0.4.0]: #040---2026-06-14
[0.3.1]: #031---2026-06-14
[0.3.0]: #030---2026-06-14
[0.2.0]: #020---2026-06-14
[0.1.2]: #012---2026-06-14
[0.1.1]: #011---2026-06-14
[0.1.0]: #010---2026-06-14
