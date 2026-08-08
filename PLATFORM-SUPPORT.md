🌐 **English** | [繁體中文](PLATFORM-SUPPORT.zh-Hant.md)

# Platform Support

Last updated: 2026-08-08

This file exists so the gap between "an installer exists for this platform"
and "the VPN actually works on this platform" is never ambiguous.

## Summary

| Platform | Installer built by CI | Real VPN tunnel works | Status |
| --- | --- | --- | --- |
| **Windows** | ✅ `.msi`, NSIS `.exe` | ✅ Yes (Wintun) | Fully supported |
| **Linux** | ✅ `.deb`, `.AppImage` | ❌ Not yet | **UI-only preview** |
| **macOS** | ✅ `.dmg` | ❌ Not yet | **UI-only preview** |

## Why

The virtual network adapter — the component that actually creates a tunnel
interface and carries traffic — is implemented only for Windows, via
[Wintun](https://www.wintun.net/). See
[`src-tauri/src/engine/tun/mod.rs`](src-tauri/src/engine/tun/mod.rs): the
real adapter path is `#[cfg(windows)]`-gated, and the non-Windows branch is a
stub that reports the operation as unsupported rather than doing anything.

Everything else — the Rust engine's cryptography, NAT-traversal logic,
signaling, Forward Error Correction, split-tunnel policy, and the entire
React/Tauri UI — is platform-agnostic and does build and run on Linux and
macOS. That's *why* CI can produce installers for all three platforms without
lying about what compiles. It's also why those installers are explicitly
labeled previews: the app opens, the UI works, but connecting to a peer and
tunneling traffic will not function, because there is no real adapter under
it on those platforms yet.

## What "UI-only preview" means in practice, on Linux/macOS

- The app launches and all screens (Network, Diagnostics, Settings,
  Minecraft) render normally.
- Handshake, signaling, and telemetry code paths run, but with no real
  adapter to attach to, there is no actual tunnel and no real traffic flows.
- Do not rely on a Linux or macOS build for actual gaming/VPN use yet.

## Roadmap

Cross-platform TUN support (Linux `/dev/net/tun`, macOS `utun`) is not yet
scheduled. When it lands, this file and the CI workflow's release-notes
disclaimer will be updated to reflect it, and the table above will change.
Track progress in the [Wiki: Project Status](https://github.com/SpaceSquare640/Player_Club_Private_VPN/wiki/Project-Status)
page.

## Where this is enforced

Every GitHub Release published by [`.github/workflows/release.yml`](.github/workflows/release.yml)
automatically includes the platform-support disclaimer above in its release
notes, generated from this file's summary — so the caveat travels with the
download, not just with the docs.
