import { describe, it, expect, beforeEach } from "vitest";
import { useTelemetryStore } from "./telemetryStore";
import type { PacketLogEntry, TelemetrySnapshot } from "../types/telemetry";

const pristine = useTelemetryStore.getState();

function pkt(t: number): PacketLogEntry {
  return { tMs: t, dir: "tx", proto: "UDP", len: 40, note: "" };
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
});
