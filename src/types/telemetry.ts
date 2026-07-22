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
