import { create } from "zustand";
import type {
  EngineNotice,
  EngineState,
  IdentityInfo,
  PacketLogEntry,
  PrivilegeStatus,
  TelemetrySnapshot,
} from "../types/telemetry";

/** Max packet-log lines retained in the UI (the engine keeps its own ring). */
const MAX_LOG = 200;

interface TelemetryStore {
  /** True while a session is active (connecting/starting/connected). */
  running: boolean;
  state: EngineState;
  snapshot: TelemetrySnapshot | null;
  packets: PacketLogEntry[];
  privilege: PrivilegeStatus | null;
  notice: EngineNotice | null;
  identity: IdentityInfo | null;

  setRunning: (running: boolean) => void;
  setState: (state: EngineState) => void;
  setSnapshot: (snapshot: TelemetrySnapshot) => void;
  appendPackets: (batch: PacketLogEntry[]) => void;
  setPrivilege: (privilege: PrivilegeStatus) => void;
  setNotice: (notice: EngineNotice | null) => void;
  setIdentity: (identity: IdentityInfo) => void;
  reset: () => void;
}

/**
 * Ephemeral live-telemetry store — deliberately NOT persisted (this is
 * real-time data, unlike the shell's UI preferences in `appStore`).
 */
export const useTelemetryStore = create<TelemetryStore>((set) => ({
  running: false,
  state: "idle",
  snapshot: null,
  packets: [],
  privilege: null,
  notice: null,
  identity: null,

  setRunning: (running) => set({ running }),
  setState: (state) => set({ state }),
  setSnapshot: (snapshot) => set({ snapshot }),
  appendPackets: (batch) =>
    set((s) => {
      const merged = [...s.packets, ...batch];
      // Keep the log bounded; trim oldest from the front.
      return {
        packets:
          merged.length > MAX_LOG ? merged.slice(merged.length - MAX_LOG) : merged,
      };
    }),
  setPrivilege: (privilege) => set({ privilege }),
  setNotice: (notice) => set({ notice }),
  setIdentity: (identity) => set({ identity }),
  reset: () =>
    set({ running: false, state: "idle", snapshot: null, packets: [], notice: null }),
}));
