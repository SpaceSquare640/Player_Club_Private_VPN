# Security Policy

Player Club Private VPN creates a virtual network adapter, punches through
NATs, and carries encrypted traffic directly between peers' machines. A
security defect here doesn't just affect the person running the app — it can
affect the networks and third parties reachable through a tunnel they open.
Reports are taken seriously and triaged promptly.

---

## Current status: pre-release

This project is **alpha-quality and under active development**. It has **not**
been independently security-audited, penetration-tested, or certified by any
third party.

**Do not use it to protect confidential, regulated, or safety-critical
information**, and do not deploy it in production. Its cryptographic and
networking code may contain defects that expose traffic, systems, or a
network to risk. See [`LICENSE`](LICENSE) §4–§6 for the full disclaimer.

## Supported versions

Only the **latest tagged release** and the default branch receive fixes.
Older tags are not maintained.

| Version | Supported |
| --- | --- |
| Latest tagged release | ✅ |
| Anything older | ❌ |

---

## Reporting a vulnerability

**Do not open a public issue for a security vulnerability.**

Report it privately through
**[GitHub Security Advisories](https://github.com/SpaceSquare640/Player_Club_Private_VPN/security/advisories/new)**
(*Security* tab → *Report a vulnerability*). This keeps the discussion
confidential until a fix is ready.

A useful report includes:

- the affected version, tag, or commit;
- the component involved (e.g. handshake, data plane, split-tunnel policy,
  virtual adapter, signaling);
- what an attacker could achieve, and what access or position they need to do
  it;
- reproduction steps or a proof of concept;
- your assessment of severity and impact.

Please allow a reasonable window to investigate and ship a fix before any
public disclosure. There is currently no paid bug-bounty program; credit will
be given in the release notes for confirmed reports, unless you prefer to
remain anonymous.

## Scope

Areas of particular interest:

- **Cryptography** — handshake authentication, key handling, nonce or counter
  reuse, replay protection.
- **Data plane** — packet parsing, memory safety, traffic leaking outside the
  tunnel.
- **Split tunneling** — policy bypass; traffic reaching a destination the
  configured policy should have blocked.
- **Peer trust** — anything an authenticated-but-hostile peer can do to the
  other side of a connection.
- **Privilege** — misuse of the elevated adapter-creation / helper path.
- **Identity** — exposure or extraction of the locally stored private key.

Out of scope: findings that require an already-compromised host or physical
device access; the documented alpha-stage limitations noted in
[`README.md`](README.md); and issues in a third-party component — please
report those upstream (see [`THIRD-PARTY-NOTICES.md`](THIRD-PARTY-NOTICES.md)).

---

## Using this software safely

- Use it **only** on networks you own or are **explicitly authorised** to use.
- Remember that connecting to a peer **extends your network boundary** — the
  remote machine, and anything reachable from it, becomes adjacent to yours.
- Verify a peer's identity **out of band** before trusting a connection.
- Keep your identity key private; treat the app's configuration directory as
  sensitive.

Acceptable use is a condition of the licence — see [`LICENSE`](LICENSE) §2–§3
— and is further described in [`TERMS_OF_SERVICE.md`](TERMS_OF_SERVICE.md).
For what the app does and does not collect, see
[`PRIVACY_POLICY.md`](PRIVACY_POLICY.md).
