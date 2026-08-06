import type { ConnectionSettings } from "../types/telemetry";

export const PROFILE_FORMAT_VERSION = 1;

export interface ConnectionProfileFile {
  formatVersion: number;
  forwardBroadcast: boolean;
  forwardMulticast: boolean;
  fecParityShards: number;
  /** Optional: added in Phase E.2. Absent in older exported profiles, which
   * remain valid — parsing treats a missing field as `[]`, not an error. */
  extraRoutes?: string[];
}

/** Mirrors the Rust-side `RsEncoder` clamp (`fec/rs.rs`'s `MAX_R`). */
const MIN_FEC_PARITY_SHARDS = 1;
const MAX_FEC_PARITY_SHARDS = 16;

export type ProfileValidationError =
  | "invalid-json"
  | "not-an-object"
  | "unsupported-version"
  | "invalid-forward-broadcast"
  | "invalid-forward-multicast"
  | "invalid-fec-parity-shards"
  | "invalid-extra-routes";

export type ProfileParseResult =
  | { ok: true; settings: ConnectionSettings }
  | { ok: false; error: ProfileValidationError };

export function serializeConnectionProfile(settings: ConnectionSettings): string {
  const file: ConnectionProfileFile = {
    formatVersion: PROFILE_FORMAT_VERSION,
    forwardBroadcast: settings.forwardBroadcast,
    forwardMulticast: settings.forwardMulticast,
    fecParityShards: settings.fecParityShards,
    extraRoutes: settings.extraRoutes,
  };
  return JSON.stringify(file, null, 2);
}

export function parseConnectionProfile(raw: string): ProfileParseResult {
  let parsed: unknown;
  try {
    parsed = JSON.parse(raw);
  } catch {
    return { ok: false, error: "invalid-json" };
  }

  if (typeof parsed !== "object" || parsed === null) {
    return { ok: false, error: "not-an-object" };
  }
  const file = parsed as Record<string, unknown>;

  if (file.formatVersion !== PROFILE_FORMAT_VERSION) {
    return { ok: false, error: "unsupported-version" };
  }
  if (typeof file.forwardBroadcast !== "boolean") {
    return { ok: false, error: "invalid-forward-broadcast" };
  }
  if (typeof file.forwardMulticast !== "boolean") {
    return { ok: false, error: "invalid-forward-multicast" };
  }
  if (
    typeof file.fecParityShards !== "number" ||
    !Number.isInteger(file.fecParityShards) ||
    file.fecParityShards < MIN_FEC_PARITY_SHARDS ||
    file.fecParityShards > MAX_FEC_PARITY_SHARDS
  ) {
    return { ok: false, error: "invalid-fec-parity-shards" };
  }
  if (
    file.extraRoutes !== undefined &&
    (!Array.isArray(file.extraRoutes) || !file.extraRoutes.every((r) => typeof r === "string"))
  ) {
    return { ok: false, error: "invalid-extra-routes" };
  }

  return {
    ok: true,
    settings: {
      forwardBroadcast: file.forwardBroadcast,
      forwardMulticast: file.forwardMulticast,
      fecParityShards: file.fecParityShards,
      extraRoutes: (file.extraRoutes as string[] | undefined) ?? [],
    },
  };
}
