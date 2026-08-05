/**
 * Thin client for the Rust networking engine: typed `invoke` wrappers and
 * event subscriptions. Mirrors the IPC contract in `src-tauri/src/commands`.
 */
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  ConnectionInfo,
  ConnectionSettings,
  EngineConfig,
  EngineMode,
  EngineNotice,
  EngineState,
  EngineStatus,
  IdentityInfo,
  NetworkStatus,
  PacketLogEntry,
  PrivilegeStatus,
  TelemetrySnapshot,
} from "../types/telemetry";

/** Map a UI mode to the engine config flags the backend branches on. */
export function configForMode(mode: EngineMode): EngineConfig {
  switch (mode) {
    case "probe":
      return { transportProbe: true };
    case "real":
      return { useRealTun: true };
    case "simulated":
    default:
      return {};
  }
}

export const EVT_STATS = "telemetry://stats";
export const EVT_PACKET = "telemetry://packet";
export const EVT_STATE = "engine://state";
export const EVT_NOTICE = "engine://notice";

/** Lifecycle states that represent an active (running) session. */
const ACTIVE_STATES: EngineState[] = ["connecting", "starting", "connected"];

export function isEngineActive(state: EngineState): boolean {
  return ACTIVE_STATES.includes(state);
}

// --- Commands (UI → engine) ------------------------------------------------

export function startEngine(config: EngineConfig = {}): Promise<void> {
  return invoke("start_engine", { config });
}

export function stopEngine(): Promise<void> {
  return invoke("stop_engine");
}

export function getStatus(): Promise<EngineStatus> {
  return invoke("get_status");
}

export function getSnapshot(): Promise<TelemetrySnapshot> {
  return invoke("get_snapshot");
}

export function getPacketLog(): Promise<PacketLogEntry[]> {
  return invoke("get_packet_log");
}

export function getPrivilegeStatus(): Promise<PrivilegeStatus> {
  return invoke("get_privilege_status");
}

export function requestElevation(): Promise<void> {
  return invoke("request_elevation");
}

export function getIdentity(): Promise<IdentityInfo> {
  return invoke("get_identity");
}

// --- Manual signaling (C3) -------------------------------------------------

export function createOffer(): Promise<string> {
  return invoke("create_offer");
}

export function acceptOffer(blob: string): Promise<string> {
  return invoke("accept_offer", { blobStr: blob });
}

export function acceptAnswer(blob: string): Promise<void> {
  return invoke("accept_answer", { blobStr: blob });
}

export function getConnection(): Promise<ConnectionInfo> {
  return invoke("get_connection");
}

// --- Peer connection (C4) --------------------------------------------------

/**
 * Begin the hole-punch handshake to the negotiated peer. `settings` (split-tunnel
 * forwarding + FEC redundancy, Phase B.3) applies once, for this connection.
 */
export function connectPeer(settings: ConnectionSettings): Promise<void> {
  return invoke("connect_peer", { settings });
}

/** Tear down the live peer link. */
export function disconnectPeer(): Promise<void> {
  return invoke("disconnect_peer");
}

// --- Virtual network (G.1–G.4) ---------------------------------------------

/**
 * Start hosting a new virtual network. `bindAddr` is `ip:port` (port `0`
 * picks an ephemeral port). Resolves to the actual bound `ip:port`, for
 * display so others can join.
 */
export function createNetwork(bindAddr: string, networkName: string, password: string): Promise<string> {
  return invoke("create_network", { bindAddr, networkName, password });
}

/** Join an existing virtual network hosted at `hostAddr` (`ip:port`). */
export function joinNetwork(hostAddr: string, networkName: string, password: string): Promise<void> {
  return invoke("join_network", { hostAddr, networkName, password });
}

/** Leave the current virtual network (idempotent). */
export function leaveNetwork(): Promise<void> {
  return invoke("leave_network");
}

/** Current virtual-network status, or `null` if not in one. */
export function getNetworkStatus(): Promise<NetworkStatus | null> {
  return invoke("get_network_status");
}

// --- Events (engine → UI) --------------------------------------------------

export function onStats(cb: (s: TelemetrySnapshot) => void): Promise<UnlistenFn> {
  return listen<TelemetrySnapshot>(EVT_STATS, (e) => cb(e.payload));
}

export function onPackets(cb: (b: PacketLogEntry[]) => void): Promise<UnlistenFn> {
  return listen<PacketLogEntry[]>(EVT_PACKET, (e) => cb(e.payload));
}

export function onState(cb: (s: EngineState) => void): Promise<UnlistenFn> {
  return listen<{ state: EngineState }>(EVT_STATE, (e) => cb(e.payload.state));
}

export function onNotice(cb: (n: EngineNotice) => void): Promise<UnlistenFn> {
  return listen<EngineNotice>(EVT_NOTICE, (e) => cb(e.payload));
}
