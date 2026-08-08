🌐 **English** | [繁體中文](PLATFORM-SUPPORT.zh-Hant.md)

# Platform Support

Last updated: 2026-08-09

This file exists so the gap between "an installer exists for this platform"
and "the VPN actually works on this platform" is never ambiguous.

## Summary

| Platform | Installer built by CI | Real VPN tunnel | Status |
| --- | --- | --- | --- |
| **Windows** | ✅ `.msi`, NSIS `.exe` | ✅ Implemented (Wintun) | Fully supported |
| **Linux** | ✅ `.deb`, `.AppImage` | 🚧 Implemented, **unverified on real hardware** (`/dev/net/tun`) | Preview |
| **macOS** | ✅ `.dmg` | 🚧 Implemented, **unverified on real hardware** (`utun`) | Preview |

## Why "implemented" isn't the same as "verified"

The virtual network adapter — the component that actually creates a tunnel
interface and carries traffic — now has a real backend for all three
platforms: Wintun on Windows, `/dev/net/tun` on Linux, and a `utun`
kernel-control socket on macOS. See
[`src-tauri/src/engine/tun/`](src-tauri/src/engine/tun/) — `windows.rs`,
`linux.rs`, `macos.rs` respectively, selected at compile time in `mod.rs`.

The Windows backend has been exercised end-to-end (see
[Wiki: Project Status](https://github.com/SpaceSquare640/Player_Club_Private_VPN/wiki/Project-Status)).
The Linux and macOS backends are new: they compile and pass their own unit
tests (ioctl/struct-layout math, name packing) in CI on `ubuntu-latest` and
`macos-latest`, but **neither has been exercised against a live peer on real
Linux or macOS hardware yet** — no one has confirmed the adapter actually
creates a working interface, carries traffic, or survives real-world
permission/firewall setups on those platforms. Treat them as a preview, not
a guarantee, until that verification happens — track it in
[Wiki: Project Status](https://github.com/SpaceSquare640/Player_Club_Private_VPN/wiki/Project-Status).

Two related gaps on Linux/macOS, smaller than the adapter itself but still
open:

- **No one-click elevation.** Windows can relaunch itself elevated via UAC;
  Linux/macOS have no equivalent single, version-safe API (the real options
  are `pkexec`, `sudo`, or a platform auth dialog, and none is a drop-in
  replacement). Until that's built, the app must already be launched with
  root (`sudo`) for the real adapter to be available on these platforms —
  see [`src-tauri/src/engine/tun/privilege.rs`](src-tauri/src/engine/tun/privilege.rs).
- **Windows-only OS-integration niceties** — automatically classifying the
  adapter's network as Private and scoping a firewall allow-rule to it
  (`src-tauri/src/engine/tun/windows.rs`'s `configure_network_integration`)
  has no Linux/macOS equivalent yet. Traffic still flows; you may need to
  adjust your own firewall manually on those platforms.

## What to expect in practice, on Linux/macOS

- The app launches and all screens (Network, Diagnostics, Settings,
  Minecraft) render normally, same as on Windows.
- Launch the app with `sudo` (or run the real-adapter path as root some
  other way) if you want to try the real adapter — without root, it reports
  `Needs Admin` the same way Windows does, but there is no relaunch button
  yet on these platforms.
- The adapter creation, IP assignment, and packet read/write paths are real
  code, not stubs — but again, **unverified on physical hardware**. If you
  try it and it doesn't work, that's expected right now, not necessarily a
  sign something else is broken; please report what you saw either way (see
  [`SECURITY.md`](SECURITY.md) for security-relevant reports,
  [Issues](https://github.com/SpaceSquare640/Player_Club_Private_VPN/issues)
  for anything else).

## Roadmap

Next steps, in rough priority order: verify the Linux and macOS adapters
against a live peer on real hardware; add one-click elevation for both;
bring OS network-integration (firewall/route hygiene) to parity with
Windows. This file and the CI workflow's release-notes disclaimer will be
updated as each of those lands, and the table above will change to match.
Track progress in the
[Wiki: Project Status](https://github.com/SpaceSquare640/Player_Club_Private_VPN/wiki/Project-Status)
page.

## Where this is enforced

Every GitHub Release published by [`.github/workflows/release.yml`](.github/workflows/release.yml)
automatically includes the platform-support disclaimer above in its release
notes, generated from this file's summary — so the caveat travels with the
download, not just with the docs.
