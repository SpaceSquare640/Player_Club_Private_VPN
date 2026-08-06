import { describe, expect, it } from "vitest";
import { parseConnectionProfile, serializeConnectionProfile } from "./profile";
import type { ConnectionSettings } from "../types/telemetry";

const SETTINGS: ConnectionSettings = {
  forwardBroadcast: false,
  forwardMulticast: true,
  fecParityShards: 3,
  extraRoutes: ["192.168.50.0/24"],
};

describe("serializeConnectionProfile", () => {
  it("round-trips through parseConnectionProfile", () => {
    const json = serializeConnectionProfile(SETTINGS);
    const result = parseConnectionProfile(json);
    expect(result).toEqual({ ok: true, settings: SETTINGS });
  });

  it("stamps the current format version", () => {
    const json = serializeConnectionProfile(SETTINGS);
    expect(JSON.parse(json).formatVersion).toBe(1);
  });
});

describe("parseConnectionProfile", () => {
  it("rejects malformed JSON", () => {
    expect(parseConnectionProfile("{not json")).toEqual({
      ok: false,
      error: "invalid-json",
    });
  });

  it("rejects a non-object payload", () => {
    expect(parseConnectionProfile("42")).toEqual({
      ok: false,
      error: "not-an-object",
    });
  });

  it("rejects an unsupported format version", () => {
    const json = JSON.stringify({ ...SETTINGS, formatVersion: 2 });
    expect(parseConnectionProfile(json)).toEqual({
      ok: false,
      error: "unsupported-version",
    });
  });

  it("rejects a non-boolean forwardBroadcast", () => {
    const json = JSON.stringify({ ...SETTINGS, formatVersion: 1, forwardBroadcast: "yes" });
    expect(parseConnectionProfile(json)).toEqual({
      ok: false,
      error: "invalid-forward-broadcast",
    });
  });

  it("rejects a non-boolean forwardMulticast", () => {
    const json = JSON.stringify({ ...SETTINGS, formatVersion: 1, forwardMulticast: 1 });
    expect(parseConnectionProfile(json)).toEqual({
      ok: false,
      error: "invalid-forward-multicast",
    });
  });

  it("rejects an out-of-range fecParityShards without clamping it", () => {
    const json = JSON.stringify({ ...SETTINGS, formatVersion: 1, fecParityShards: 17 });
    expect(parseConnectionProfile(json)).toEqual({
      ok: false,
      error: "invalid-fec-parity-shards",
    });
  });

  it("rejects a non-integer fecParityShards", () => {
    const json = JSON.stringify({ ...SETTINGS, formatVersion: 1, fecParityShards: 2.5 });
    expect(parseConnectionProfile(json)).toEqual({
      ok: false,
      error: "invalid-fec-parity-shards",
    });
  });

  it("rejects fecParityShards below the minimum", () => {
    const json = JSON.stringify({ ...SETTINGS, formatVersion: 1, fecParityShards: 0 });
    expect(parseConnectionProfile(json)).toEqual({
      ok: false,
      error: "invalid-fec-parity-shards",
    });
  });

  it("rejects a non-array extraRoutes", () => {
    const json = JSON.stringify({ ...SETTINGS, formatVersion: 1, extraRoutes: "192.168.50.0/24" });
    expect(parseConnectionProfile(json)).toEqual({
      ok: false,
      error: "invalid-extra-routes",
    });
  });

  it("rejects an extraRoutes array with a non-string entry", () => {
    const json = JSON.stringify({ ...SETTINGS, formatVersion: 1, extraRoutes: ["192.168.50.0/24", 42] });
    expect(parseConnectionProfile(json)).toEqual({
      ok: false,
      error: "invalid-extra-routes",
    });
  });

  it("treats a missing extraRoutes as an empty list — older exported profiles remain valid", () => {
    const { extraRoutes: _extraRoutes, ...withoutExtraRoutes } = SETTINGS;
    const json = JSON.stringify({ ...withoutExtraRoutes, formatVersion: 1 });
    expect(parseConnectionProfile(json)).toEqual({
      ok: true,
      settings: { ...withoutExtraRoutes, extraRoutes: [] },
    });
  });
});
