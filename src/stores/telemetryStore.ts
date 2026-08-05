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

/** One throughput sample for the Spectrum chart. */
export interface SpectrumSample {
  txKbps: number;
  rxKbps: number;
}

/**
 * Max samples retained for the Spectrum chart. Deliberately a sample *count*,
 * not a time window — `tick_hz` is configurable (1–20 Hz), so a fixed count
 * gives a stable-looking chart regardless of rate rather than trying to derive
 * "the last N seconds" from a variable tick interval.
 */
const MAX_SPECTRUM_SAMPLES = 120;

interface TelemetryStore {
  /** True while a session is active (connecting/starting/connected). */
  running: boolean;
  state: EngineState;
  snapshot: TelemetrySnapshot | null;
  /** Recent tx/rx samples, oldest first, for the Spectrum chart. */
  spectrumHistory: SpectrumSample[];
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
  spectrumHistory: [],
  packets: [],
  privilege: null,
  notice: null,
  identity: null,

  setRunning: (running) => set({ running }),
  setState: (state) => set({ state }),
  setSnapshot: (snapshot) =>
    set((s) => {
      const sample: SpectrumSample = { txKbps: snapshot.txKbps, rxKbps: snapshot.rxKbps };
      const merged = [...s.spectrumHistory, sample];
      return {
        snapshot,
        spectrumHistory:
          merged.length > MAX_SPECTRUM_SAMPLES
            ? merged.slice(merged.length - MAX_SPECTRUM_SAMPLES)
            : merged,
      };
    }),
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
    set({
      running: false,
      state: "idle",
      snapshot: null,
      spectrumHistory: [],
      packets: [],
      notice: null,
    }),
}));
