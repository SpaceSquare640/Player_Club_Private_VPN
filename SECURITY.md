# Security Policy

Player Club Private VPN is **networking and cryptographic software**: it creates
a virtual network adapter, punches through NATs, and carries encrypted traffic
between machines. Security defects here can affect not only the user but the
networks and third parties reachable through a tunnel. Reports are taken
seriously.

---

## ⚠️ Current status — pre-release

This project is **alpha-quality and under active development**. It has **not**
been independently security-audited, penetration-tested or certified.

**Do not rely on it to protect confidential, regulated or safety-critical
information**, and do not deploy it in production. Its cryptographic and
networking implementations may contain defects that expose your traffic, your
systems or your network. See [`LICENSE`](LICENSE) §4–§6.

## Supported versions

Only the **latest** version on the default branch receives fixes. Older tags are
not maintained.

| Version | Supported |
| --- | --- |
| Latest tagged release | ✅ |
| Anything older | ❌ |

---

## Reporting a vulnerability

**Please do not open a public issue for a security vulnerability.**

Report it privately via **[GitHub Security Advisories](https://github.com/SpaceSquare640/Player_Club_Private_VPN/security/advisories/new)**
(*Security* → *Report a vulnerability*), which keeps the discussion confidential
until a fix is available.

A useful report includes:

- the affected version or commit;
- the component (e.g. handshake, data plane, split-tunnel policy, adapter);
- what an attacker can achieve, and what access they need to do it;
- reproduction steps or a proof of concept;
- your assessment of impact.

Please give a reasonable opportunity to investigate and issue a fix before any
public disclosure.

## Scope

Especially interested in:

- **Cryptography** — handshake authentication, key handling, nonce/counter reuse, replay.
- **Data plane** — packet parsing, memory safety, traffic leaking outside the tunnel.
- **Split tunnelling** — policy bypass; egress or ingress reaching a destination it should not.
- **Peer trust** — anything an authenticated-but-hostile peer can do to the other side.
- **Privilege** — misuse of the elevated adapter-creation path.
- **Identity** — exposure of the stored private key.

Out of scope: findings that require an already-compromised host or physical
access; the documented alpha limitations above; and anything in a third-party
component — report those upstream (see [`THIRD-PARTY-NOTICES.md`](THIRD-PARTY-NOTICES.md)).

---

## Using this software safely

- Use it **only** on networks you own or are **explicitly authorised** to use.
- Remember that connecting to a peer **extends a network boundary** — the remote
  machine, and anything reachable from it, becomes adjacent to yours.
- Verify a peer's **fingerprint out of band** before trusting a connection.
- Keep your identity key private; treat the app's config directory as sensitive.

Acceptable use is a condition of the licence — see [`LICENSE`](LICENSE) §2–§3.
