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
 * To change the live-toggleable subset afterwards, see
 * {@link updateConnectionSettings}.
 */
export function connectPeer(settings: ConnectionSettings): Promise<void> {
  return invoke("connect_peer", { settings });
}

/**
 * Push a settings change into every already-live link (Phase B.4).
 *
 * Only the broadcast/multicast toggles take effect mid-session — they are
 * pure local packet filtering, invisible to the peer. FEC redundancy and
 * extra routed networks still apply only at the next Connect (the former is
 * a wire-format agreement with the peer, the latter needs elevation to
 * change OS routes). The whole `ConnectionSettings` object is sent anyway:
 * the engine ignores the parts it can't apply live, which keeps this call
 * from drifting out of sync with the type as it grows.
 *
 * A no-op when nothing is connected.
 */
export function updateConnectionSettings(settings: ConnectionSettings): Promise<void> {
  return invoke("update_connection_settings", { settings });
}

/** Tear down the live peer link. */
export function disconnectPeer(): Promise<void> {
  return invoke("disconnect_peer");
}

// --- Virtual network (G.1–G.4) ---------------------------------------------

/**
 * Start hosting a new virtual network. `bindAddr` is `ip:port` (port `0`
 * picks an ephemeral port). `gameTag` is display metadata only (e.g.
 * `"minecraft"`); `settings` applies to every auto-connected peer, same as
 * `connectPeer`'s. Resolves to the new network's id (its bound `ip:port` is
 * read back from `getNetworkStatuses`). Can be called while already a
 * member of other networks — each call adds one more active network rather
 * than replacing any existing one.
 *
 * `relayAddr` (`ip:port`), if non-null, registers on that relay instead of
 * binding `bindAddr` directly — reachable across the internet without port
 * forwarding. `null`/omitted preserves direct-bind behavior exactly.
 */
export function createNetwork(
  bindAddr: string,
  networkName: string,
  password: string,
  gameTag: string | null,
  settings: ConnectionSettings,
  relayAddr: string | null = null,
): Promise<string> {
  return invoke("create_network", { bindAddr, networkName, password, gameTag, settings, relayAddr });
}

/**
 * Join an existing virtual network hosted at `hostAddr` (`ip:port`).
 * Resolves to the new network's id. Can be called while already a member of
 * other networks — see `createNetwork`.
 *
 * `relayAddr`, if non-null, connects out to that relay and requests
 * `networkName` instead of dialing `hostAddr` directly — the same relay
 * address the host used with `createNetwork`.
 */
export function joinNetwork(
  hostAddr: string,
  networkName: string,
  password: string,
  gameTag: string | null,
  settings: ConnectionSettings,
  relayAddr: string | null = null,
): Promise<string> {
  return invoke("join_network", { hostAddr, networkName, password, gameTag, settings, relayAddr });
}

/** Leave the virtual network identified by `networkId` (idempotent). */
export function leaveNetwork(networkId: string): Promise<void> {
  return invoke("leave_network", { networkId });
}

/** Status of every currently active virtual network (empty if none). */
export function getNetworkStatuses(): Promise<NetworkStatus[]> {
  return invoke("get_network_statuses");
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
