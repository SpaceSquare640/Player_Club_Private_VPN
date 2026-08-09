/**
 * TypeScript mirrors of the Rust engine DTOs (see `src-tauri/src/engine`).
 * Field names are camelCase to match serde's `rename_all = "camelCase"`.
 */

export type EngineState =
  | "idle"
  | "connecting"
  | "starting"
  | "connected"
  | "needs-elevation"
  | "error";

export interface PrivilegeStatus {
  elevated: boolean;
  canCreateTun: boolean;
  os: string;
}

export interface EngineNotice {
  code: string;
  message: string;
  remediation: string | null;
}

export interface TelemetrySnapshot {
  state: EngineState;
  /** Round-trip time in milliseconds (the "ping"). */
  rttMs: number;
  jitterMs: number;
  lossPct: number;
  txKbps: number;
  rxKbps: number;
  peers: number;
  /** Packets rebuilt by FEC since the link came up (cumulative, not a rate). */
  fecRecovered: number;
  /** Packets the split-tunnel policy refused, either direction (cumulative). */
  policyBlocked: number;
}

export type PacketDirection = "tx" | "rx";

export interface PacketLogEntry {
  /** Milliseconds since the engine session started. */
  tMs: number;
  dir: PacketDirection;
  proto: string;
  len: number;
  note: string;
}

export interface EngineStatus {
  state: EngineState;
  peers: number;
  uptimeS: number;
}

export interface IdentityInfo {
  /** Short human-readable fingerprint, e.g. "PC-3F8A-9C2D-71B0-44EE". */
  peerAddress: string;
  /** Base64 public key — paste into a peer to connect (C3). */
  publicKeyB64: string;
}

export interface ConnectionPeer {
  peerAddress: string;
  candidateCount: number;
}

/** Live peer-link lifecycle (C4), distinct from the signaling `role`. */
export type LinkState = "idle" | "connecting" | "connected" | "failed";

export interface ConnectionInfo {
  role: string;
  localCandidateCount: number;
  link: LinkState;
  peer: ConnectionPeer | null;
}

/** A member of a networked virtual network (Phase G.4), with their live P2P link state. */
export interface NetworkMember {
  pubkey: string;
  fingerprint: string;
  link: LinkState;
}

/** Status of one active virtual-network membership (Phase G.1–G.4+) — a session can have several at once. */
export interface NetworkStatus {
  /** Opaque id, unique among the current session's active networks — pass back to `leaveNetwork`. */
  id: string;
  networkName: string;
  isHost: boolean;
  /** `ip:port` — what a joiner types in. Meaningful for host and joiner alike. */
  hostAddr: string;
  /** Free-form label set at creation/join time (e.g. `"minecraft"`) — display only. */
  gameTag: string | null;
  members: NetworkMember[];
}

export type SimProfile = "stable" | "congested" | "lossy";

export interface TunConfig {
  name?: string;
  virtualIp?: string;
  prefixLen?: number;
  mtu?: number;
}

export interface EngineConfig {
  tickHz?: number;
  simProfile?: SimProfile;
  /** Omit (or null) for OS entropy; set for a reproducible run. */
  seed?: number | null;
  /** Use the real Wintun adapter (Expert mode) instead of the simulator. */
  useRealTun?: boolean;
  /** Adapter parameters; omit to use the engine defaults (10.77.0.1/24). */
  tun?: TunConfig;
  /** Run the C1 transport probe (STUN + Ping/Pong RTT). */
  transportProbe?: boolean;
  stunServer?: string;
  bindPort?: number;
  /** Optional host:port ping target; omit for the loopback self-test. */
  probeTarget?: string | null;
}

/** UI engine modes mapped to config flags in `lib/engine.ts`. */
export type EngineMode = "simulated" | "probe" | "real";

/**
 * User-configurable connection-time settings (Phase B.3). Applied once, when
 * `connectPeer` is called — not retroactively to an already-live link (live
 * toggling needs a control channel into the running pipeline; deferred).
 */
export interface ConnectionSettings {
  /** Forward broadcast traffic (LAN discovery) into the tunnel. Default `true`. */
  forwardBroadcast: boolean;
  /** Forward multicast traffic (LAN discovery) into the tunnel. Default `true`. */
  forwardMulticast: boolean;
  /** FEC parity shards per group of 8 — recovers up to this many losses per group. Default `1`. */
  fecParityShards: number;
  /**
   * Additional `"address/prefix"` networks to route into the tunnel beyond
   * the peer's own virtual-LAN subnet (Phase E.2 — OS route management).
   * Unvalidated strings — the Rust side silently drops any entry that
   * doesn't parse rather than failing the whole connection. Default `[]`.
   */
  extraRoutes: string[];
}

/** Matches the values that were hardcoded before Phase B.3 (plus E.2's empty default). */
export const DEFAULT_CONNECTION_SETTINGS: ConnectionSettings = {
  forwardBroadcast: true,
  forwardMulticast: true,
  fecParityShards: 1,
  extraRoutes: [],
};
