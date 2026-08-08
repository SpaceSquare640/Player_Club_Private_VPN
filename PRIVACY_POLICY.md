🌐 **English (Authoritative)** | [繁體中文（參考譯本）](PRIVACY_POLICY.zh-Hant.md)

# Privacy Policy

Last updated: 2026-08-08

This Privacy Policy explains what Player Club Private VPN (the "Software")
does and does not do with your data. It's written to match what the code
actually does — if you want to verify a claim below, the relevant source is
linked.

**Short version:** the Software does not phone home. It has no telemetry
server, no analytics, no crash reporting, and no account system. Data it
handles either stays on your machine or goes directly, end-to-end encrypted,
to the peer you chose to connect to.

---

## 1. Data the Software does not collect

The Software does not collect, transmit, or sell any usage analytics,
telemetry, crash reports, or personal data to the developer or to any third
party. There is no backend service operated by the project that the app
calls out to. This is a property of the current code, not just a policy
promise: the networking engine's dependencies are limited to local sockets,
peer connections, and (optionally) a self-hosted signaling server — see
[`src-tauri/Cargo.toml`](src-tauri/Cargo.toml) — with no analytics or
crash-reporting SDK included.

The in-app "Diagnostics" view (RTT, jitter, loss, throughput, packet log)
runs entirely locally, inside your own instance of the app, for your own
troubleshooting. None of it is sent anywhere.

## 2. Data stored on your device

The Software stores the following locally, in your OS's per-user application
data directory (via Tauri's `app_config_dir`):

- **Your identity key pair** (X25519) — used to authenticate you to peers
  during the Noise IK handshake. The private key is written with restrictive
  file permissions and is never transmitted; only the corresponding public
  key/fingerprint is shared with a peer as part of establishing a connection.
  See `src-tauri/src/engine/crypto/identity.rs`.
- **App settings and UI state** — theme, language, and connection
  preferences (split-tunnel forwarding, FEC redundancy, extra routes),
  persisted so they survive a restart.
- **Connection profiles you explicitly export or import** — JSON files you
  choose to save via a native file dialog; these are not written or read
  without your action.

None of this leaves your device unless you explicitly export a file yourself
and choose to share it.

## 3. Data shared with a peer

When you connect to another person using the Software, the following is
exchanged **directly with that peer** (or, for a hosted virtual network,
relayed through a signaling connection — see Section 4), never through any
server the developer operates:

- your public key / fingerprint, and your network-reachability candidates
  (IP:port pairs gathered for NAT traversal), as part of the offer/answer
  handshake;
- once connected, the IP traffic your split-tunnel policy is configured to
  carry — this is your actual application/game traffic, encrypted in
  transit between you and the peer.

The peer you connect with can, by the nature of how a virtual LAN works, see
whatever traffic you send into the tunnel and whatever your operating system
exposes to a machine on the same subnet. Only connect with people you trust
for that level of access — see [`SECURITY.md`](SECURITY.md).

## 4. Self-hosted virtual networks (signaling)

The "Virtual Network" feature lets one user host a small WebSocket signaling
server, embedded in their own running copy of the app, so other members can
find and connect to each other without manually pasting connection blobs.
This server:

- runs on the **host's own machine** — it is not infrastructure operated by
  the developer, and the developer has no access to it or its traffic;
- sees the network name/password members use to join, each member's roster
  presence (join/leave events), and relays offer/answer handshake blobs
  between members — it is a coordination channel, not a traffic relay; actual
  game/application traffic still flows peer-to-peer once a connection is
  established;
- is entirely under the host's control: they choose whether to run it, who
  can reach it (their own network exposure/port forwarding), and when to stop
  it.

If you host a network, you are the operator of that signaling server for the
purposes of this policy, and you are responsible for who you allow to join
it.

## 5. Third-party components

The Software bundles Wintun (a signed virtual-adapter driver) and depends on
various open-source Rust and JavaScript packages at build time. These run
locally as part of the app and are not third-party services that receive
your data. See [`THIRD-PARTY-NOTICES.md`](THIRD-PARTY-NOTICES.md) for the
full list and their own licence terms.

## 6. GitHub-hosted Project resources

This repository, its Issues, Discussions, and Wiki are hosted on GitHub. Any
information you provide there (an issue report, a discussion post, a profile)
is handled by GitHub under
[GitHub's own Privacy Statement](https://docs.github.com/site-policy/privacy-policies/github-general-privacy-statement),
not by this policy. Do not include sensitive personal data, private keys, or
credentials in a public issue or discussion; for a security report, use the
private channel described in [`SECURITY.md`](SECURITY.md).

## 7. Children's privacy

The Software is not directed at children, does not knowingly collect
personal data from anyone (see Section 1), and has no age-gating because it
has no account system to gate.

## 8. Changes to this policy

As the Software moves past alpha and features change, this policy will be
updated to match. The "Last updated" date above reflects the latest
revision; material changes will be called out in [`CHANGELOG.md`](CHANGELOG.md).

## 9. Contact

Questions about this policy can be raised via
[GitHub Discussions](https://github.com/SpaceSquare640/Player_Club_Private_VPN/discussions).

---

*This document is provided for transparency about how the Software actually
behaves. It is not legal advice and has not been reviewed by a qualified
lawyer; it is not a substitute for a jurisdiction-specific privacy notice if
this project is ever offered as a commercial service.*
