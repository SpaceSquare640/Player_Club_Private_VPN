import { describe, it, expect, beforeEach, vi } from "vitest";

// Mock the Tauri IPC bridge — these tests assert the command contract (name and
// argument shape) without a running backend. `vi.hoisted` lets the shared spy
// exist before `vi.mock` is hoisted above the imports.
const { invokeMock } = vi.hoisted(() => ({ invokeMock: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke: invokeMock }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn(async () => () => {}) }));

import {
  acceptAnswer,
  acceptOffer,
  configForMode,
  connectPeer,
  createOffer,
  disconnectPeer,
  isEngineActive,
  startEngine,
  stopEngine,
} from "./engine";
import type { ConnectionSettings, EngineState } from "../types/telemetry";

beforeEach(() => {
  invokeMock.mockReset();
  invokeMock.mockResolvedValue(undefined);
});

describe("configForMode", () => {
  it("maps simulated to an empty config (defaults on the backend)", () => {
    expect(configForMode("simulated")).toEqual({});
  });
  it("maps probe to the transport-probe flag", () => {
    expect(configForMode("probe")).toEqual({ transportProbe: true });
  });
  it("maps real to the real-adapter flag", () => {
    expect(configForMode("real")).toEqual({ useRealTun: true });
  });
});

describe("isEngineActive", () => {
  const active: EngineState[] = ["connecting", "starting", "connected"];
  const inactive: EngineState[] = ["idle", "needs-elevation", "error"];

  it.each(active)("treats %s as active", (s) => {
    expect(isEngineActive(s)).toBe(true);
  });
  it.each(inactive)("treats %s as inactive", (s) => {
    expect(isEngineActive(s)).toBe(false);
  });
});

describe("command contract", () => {
  it("start_engine carries the config under `config`", async () => {
    await startEngine({ transportProbe: true });
    expect(invokeMock).toHaveBeenCalledWith("start_engine", {
      config: { transportProbe: true },
    });
  });

  it("stop_engine takes no arguments", async () => {
    await stopEngine();
    expect(invokeMock).toHaveBeenCalledWith("stop_engine");
  });

  it("create_offer takes no arguments", async () => {
    invokeMock.mockResolvedValue("PCPV1.OFFER.x.y");
    await createOffer();
    expect(invokeMock).toHaveBeenCalledWith("create_offer");
  });

  // The camelCase JS arg must reach the snake_case Rust command as `blobStr`
  // (Tauri maps blobStr -> blob_str). A regression here silently breaks signaling.
  it("accept_offer maps the blob to `blobStr`", async () => {
    invokeMock.mockResolvedValue("PCPV1.ANSWER.x.y");
    await acceptOffer("PCPV1.OFFER.x.y");
    expect(invokeMock).toHaveBeenCalledWith("accept_offer", {
      blobStr: "PCPV1.OFFER.x.y",
    });
  });

  it("accept_answer maps the blob to `blobStr`", async () => {
    await acceptAnswer("PCPV1.ANSWER.x.y");
    expect(invokeMock).toHaveBeenCalledWith("accept_answer", {
      blobStr: "PCPV1.ANSWER.x.y",
    });
  });

  it("connect_peer carries settings under `settings`; disconnect_peer takes no arguments", async () => {
    const settings: ConnectionSettings = {
      forwardBroadcast: false,
      forwardMulticast: true,
      fecParityShards: 2,
    };
    await connectPeer(settings);
    expect(invokeMock).toHaveBeenCalledWith("connect_peer", { settings });
    await disconnectPeer();
    expect(invokeMock).toHaveBeenCalledWith("disconnect_peer");
  });
});
