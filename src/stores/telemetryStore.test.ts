import { describe, it, expect, beforeEach } from "vitest";
import { useTelemetryStore } from "./telemetryStore";
import type { PacketLogEntry, TelemetrySnapshot } from "../types/telemetry";

const pristine = useTelemetryStore.getState();

function pkt(t: number): PacketLogEntry {
  return { tMs: t, dir: "tx", proto: "UDP", len: 40, note: "" };
}

function snap(txKbps: number, rxKbps: number): TelemetrySnapshot {
  return {
    state: "connected",
    rttMs: 10,
    jitterMs: 1,
    lossPct: 0,
    txKbps,
    rxKbps,
    peers: 1,
    fecRecovered: 0,
    policyBlocked: 0,
  };
}

beforeEach(() => {
  // Replace the whole state so each test starts from the store's initial values.
  useTelemetryStore.setState(pristine, true);
});

describe("telemetryStore", () => {
  it("starts idle and empty", () => {
    const s = useTelemetryStore.getState();
    expect(s.running).toBe(false);
    expect(s.state).toBe("idle");
    expect(s.packets).toEqual([]);
    expect(s.snapshot).toBeNull();
  });

  it("setState and setRunning update in place", () => {
    useTelemetryStore.getState().setState("connected");
    useTelemetryStore.getState().setRunning(true);
    expect(useTelemetryStore.getState().state).toBe("connected");
    expect(useTelemetryStore.getState().running).toBe(true);
  });

  it("setSnapshot stores the latest sample", () => {
    const snap: TelemetrySnapshot = {
      state: "connected",
      rttMs: 12,
      jitterMs: 1,
      lossPct: 0,
      txKbps: 100,
      rxKbps: 90,
      peers: 1,
      fecRecovered: 3,
      policyBlocked: 0,
    };
    useTelemetryStore.getState().setSnapshot(snap);
    expect(useTelemetryStore.getState().snapshot).toEqual(snap);
  });

  it("appendPackets accumulates batches in order", () => {
    useTelemetryStore.getState().appendPackets([pkt(1), pkt(2)]);
    useTelemetryStore.getState().appendPackets([pkt(3)]);
    expect(useTelemetryStore.getState().packets.map((p) => p.tMs)).toEqual([1, 2, 3]);
  });

  it("bounds the log to MAX_LOG (200), trimming the oldest", () => {
    const batch = Array.from({ length: 250 }, (_, i) => pkt(i));
    useTelemetryStore.getState().appendPackets(batch);
    const kept = useTelemetryStore.getState().packets;
    expect(kept).toHaveLength(200);
    // The oldest 50 were dropped from the front; the newest is last.
    expect(kept[0].tMs).toBe(50);
    expect(kept[kept.length - 1].tMs).toBe(249);
  });

  it("reset clears live data back to idle", () => {
    const s = useTelemetryStore.getState();
    s.setRunning(true);
    s.setState("connected");
    s.appendPackets([pkt(1)]);
    s.setNotice({ code: "x", message: "y", remediation: null });
    s.reset();
    const after = useTelemetryStore.getState();
    expect(after.running).toBe(false);
    expect(after.state).toBe("idle");
    expect(after.packets).toEqual([]);
    expect(after.notice).toBeNull();
  });

  // Spectrum chart history (Diagnostics visualization).
  it("setSnapshot accumulates a spectrum sample per call, oldest first", () => {
    useTelemetryStore.getState().setSnapshot(snap(10, 20));
    useTelemetryStore.getState().setSnapshot(snap(30, 40));
    const history = useTelemetryStore.getState().spectrumHistory;
    expect(history).toEqual([
      { txKbps: 10, rxKbps: 20 },
      { txKbps: 30, rxKbps: 40 },
    ]);
  });

  it("bounds spectrumHistory to 120 samples, trimming the oldest", () => {
    for (let i = 0; i < 150; i++) {
      useTelemetryStore.getState().setSnapshot(snap(i, i));
    }
    const history = useTelemetryStore.getState().spectrumHistory;
    expect(history).toHaveLength(120);
    // The oldest 30 were dropped; the newest sample is last.
    expect(history[0].txKbps).toBe(30);
    expect(history[history.length - 1].txKbps).toBe(149);
  });

  it("reset clears spectrumHistory too", () => {
    useTelemetryStore.getState().setSnapshot(snap(5, 5));
    useTelemetryStore.getState().reset();
    expect(useTelemetryStore.getState().spectrumHistory).toEqual([]);
  });
});
