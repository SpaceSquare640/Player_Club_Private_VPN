# Player Club Private VPN

> A high-performance **Gaming Virtual Network**. Player Club Private VPN builds
> secure, low-latency virtual LANs over the public internet so geographically
> separated players show up on the same subnet — a **Rust** networking engine
> paired with a **Tauri + React** desktop client.

[![Status](https://img.shields.io/badge/status-alpha-22d3ee)](https://github.com/SpaceSquare640/Player_Club_Private_VPN/wiki/Project-Status)
[![Engine](https://img.shields.io/badge/engine-Rust-orange)](#technology-stack)
[![UI](https://img.shields.io/badge/UI-Tauri%20%2B%20React%20%2B%20TS-blue)](#technology-stack)
[![License](https://img.shields.io/badge/license-Proprietary-red)](LICENSE)
[![Platform](https://img.shields.io/badge/platform-Windows-0078D6)](#technology-stack)

---

## Overview

**Player Club Private VPN** (*PCP-VPN*) emulates a Local Area Network (LAN) over
the public internet, so remote players can join the same virtual subnet and
LAN-only multiplayer titles work as if everyone were in the same room.
Connections are direct, peer-to-peer, and encrypted end to end — there's no
central relay or hosted backend.

**Current status: pre-release / alpha.** The full peer-to-peer data path
(handshake, encryption, NAT traversal, virtual adapter, FEC, split tunneling)
runs and is covered by an automated test harness, but real NAT traversal
between two physical machines has not yet been verified. See the
**[Project Status](https://github.com/SpaceSquare640/Player_Club_Private_VPN/wiki/Project-Status)**
wiki page for the full, itemised build state.

## Technology Stack

| Layer | Technology |
| --- | --- |
| Core engine | Rust |
| Desktop shell | Tauri |
| UI framework | React + TypeScript |
| Styling | Tailwind CSS |
| CI / packaging | GitHub Actions + `tauri-action` |
| Platform | **Windows** (shipping today) · macOS, Linux planned |

> The virtual-adapter code (`src-tauri/src/engine/tun/mod.rs`) is currently
> `#[cfg(windows)]`-only, so that's the only platform CI packages installers
> for. See [Architecture](https://github.com/SpaceSquare640/Player_Club_Private_VPN/wiki/Architecture) for why.

## Quick Start

```bash
pnpm install
pnpm tauri dev        # run in development
pnpm tauri build       # build release + installers
```

Requires Rust (MSVC toolchain), Node.js 20+, pnpm 10+, and the VS C++ Build
Tools on Windows. For prerequisites, troubleshooting, and a full walkthrough
of using the app, see the **[Getting Started](https://github.com/SpaceSquare640/Player_Club_Private_VPN/wiki/Getting-Started)**
and **[User Manual](https://github.com/SpaceSquare640/Player_Club_Private_VPN/wiki/User-Manual)**
wiki pages.

CI builds Windows installers automatically on every `v*` tag push (see
[`.github/workflows/release.yml`](.github/workflows/release.yml)) and
publishes them as a **draft** GitHub Release for review.

## Documentation

| Resource | Location |
| --- | --- |
| Full feature list & build status | [Wiki: Project Status](https://github.com/SpaceSquare640/Player_Club_Private_VPN/wiki/Project-Status), [Wiki: Features](https://github.com/SpaceSquare640/Player_Club_Private_VPN/wiki/Features) |
| Getting started & building from source | [Wiki: Getting Started](https://github.com/SpaceSquare640/Player_Club_Private_VPN/wiki/Getting-Started) |
| **User manual** (how to use the app) | [Wiki: User Manual](https://github.com/SpaceSquare640/Player_Club_Private_VPN/wiki/User-Manual) |
| Architecture & project structure | [Wiki: Architecture](https://github.com/SpaceSquare640/Player_Club_Private_VPN/wiki/Architecture) |
| FAQ | [Wiki: FAQ](https://github.com/SpaceSquare640/Player_Club_Private_VPN/wiki/FAQ) |
| Change history | [`CHANGELOG.md`](./CHANGELOG.md) |
| Third-party components | [`THIRD-PARTY-NOTICES.md`](./THIRD-PARTY-NOTICES.md) |

## Community

| Channel | Use it for |
| --- | --- |
| [Issues](https://github.com/SpaceSquare640/Player_Club_Private_VPN/issues) | Bug reports, feature requests |
| [Discussions](https://github.com/SpaceSquare640/Player_Club_Private_VPN/discussions) | Questions, ideas, general conversation |
| [Wiki](https://github.com/SpaceSquare640/Player_Club_Private_VPN/wiki) | Guides, manuals, and reference material |
| [Security Advisories](https://github.com/SpaceSquare640/Player_Club_Private_VPN/security/advisories/new) | Private vulnerability reports — see [`SECURITY.md`](SECURITY.md) |

## Legal

**Proprietary — All Rights Reserved.** This is **networking software**: it
creates a virtual network adapter (Administrator privileges required), performs
NAT traversal, encrypts traffic between peers, and carries arbitrary IP
traffic between connected machines. **Misconfiguration or misuse can create
serious security risk for you and for third parties.** Use it only on
networks you own or are explicitly authorised to use. It is **pre-release,
alpha-quality, not security-audited**, provided **"as is" with no warranty**,
and the copyright holder accepts **no liability** for damage arising from its
use.

Read the full terms before using or distributing this software:
[`LICENSE`](LICENSE) · [`TERMS_OF_SERVICE.md`](TERMS_OF_SERVICE.md) ·
[`PRIVACY_POLICY.md`](PRIVACY_POLICY.md) · [`SECURITY.md`](SECURITY.md)

Bundled third-party components (notably **Wintun**) remain under their own
licences; see [`src-tauri/resources/wintun/NOTICE.txt`](src-tauri/resources/wintun/NOTICE.txt).
