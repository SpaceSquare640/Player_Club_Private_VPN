# Third-Party Notices

**Player Club Private VPN** incorporates and/or bundles third-party components.
Each remains subject to **its own licence**, which is **not** superseded by this
project's [`LICENSE`](LICENSE) and which you must observe independently.

The copyright holder of Player Club Private VPN makes no warranty regarding, and
accepts no liability for, any third-party component listed here.

---

## Bundled binaries

### Wintun

A userspace TUN driver for Windows, redistributed **unmodified** as a runtime
resource and loaded dynamically by the engine (see
[`src-tauri/src/engine/tun/windows.rs`](src-tauri/src/engine/tun/windows.rs)).

| | |
| --- | --- |
| **Project** | Wintun — <https://www.wintun.net> |
| **Author** | WireGuard LLC / Jason A. Donenfeld |
| **Version** | 0.14.1 |
| **Source** | <https://www.wintun.net/builds/wintun-0.14.1.zip> |
| **SHA-256** (zip) | `07C256185D6EE3652E09FA55C0B673E2624B565E02C4B9091C79CA7D2F24EF51` |
| **Signature** | Authenticode, `O=WireGuard LLC, C=US` — verified Valid |
| **Architectures** | `amd64`, `arm64`, `x86` |
| **Location in repo** | [`src-tauri/resources/wintun/`](src-tauri/resources/wintun/) |

Wintun is provided under the terms published at <https://www.wintun.net/>.
**That page is the authoritative statement of its licence and redistribution
terms** — consult it before redistributing this software in any form. A copy of
the local notice ships alongside the binaries at
[`src-tauri/resources/wintun/NOTICE.txt`](src-tauri/resources/wintun/NOTICE.txt).

---

## Notable source dependencies

### reed-solomon-erasure

Reed-Solomon erasure coding over GF(2⁸), used by the Forward Error Correction
layer to reconstruct packets lost in transit (see [`src-tauri/src/engine/fec/`](src-tauri/src/engine/fec/)).

| | |
| --- | --- |
| **Project** | <https://github.com/darrenldl/reed-solomon-erasure> |
| **Version** | 6.0.0 |
| **Licence** | MIT |

Used with default features (`std`); the optional `simd-accel` feature — which
would pull a C toolchain via `cc`/`libc` — is deliberately **not** enabled.

---

## Build-time dependencies

The application additionally depends on open-source packages resolved at build
time, each under its own licence as declared by its publisher:

- **Rust crates** — declared in [`src-tauri/Cargo.toml`](src-tauri/Cargo.toml) and
  pinned in [`src-tauri/Cargo.lock`](src-tauri/Cargo.lock). Notable direct
  dependencies include `tauri`, `tokio`, `snow` (Noise protocol), `socket2`,
  `zeroize`, `crc32fast`, `reed-solomon-erasure`, and — on Windows — `wintun` and `windows-sys`.
- **JavaScript / TypeScript packages** — declared in
  [`package.json`](package.json) and pinned in
  [`pnpm-lock.yaml`](pnpm-lock.yaml). Notable direct dependencies include
  `react`, `react-dom`, `react-router-dom`, `zustand`, `@tauri-apps/api`,
  `tailwindcss`, `vite`, and — for localization (Phase i18n) — `i18next` and
  `react-i18next` (both MIT).

To enumerate the full transitive licence set, use a tooling pass such as
`cargo license` / `cargo about` for the Rust graph and `pnpm licenses list` for
the JavaScript graph.

---

## Cryptography notice

This software contains and uses **cryptographic functionality** (X25519 key
agreement, the Noise IK handshake pattern, and ChaCha20-Poly1305 authenticated
encryption, via the `snow` crate). The import, export, distribution and use of
encryption software is **restricted or unlawful in some jurisdictions**. Before
importing, exporting, distributing or using this software, you are responsible
for determining and complying with all laws and regulations that apply to you.
See [`LICENSE`](LICENSE) §3.
