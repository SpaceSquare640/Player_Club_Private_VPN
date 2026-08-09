import type { EngineState } from "../types/telemetry";

/**
 * Semantic styling per lifecycle state (Cyan idle, Amber transient, Violet
 * connected, Red error). Shared between `Diagnostics` and `Dashboard` so a
 * given `EngineState` always reads the same color/label everywhere in the
 * app — this used to live only in `Diagnostics.tsx`.
 */
export const ENGINE_STATE_STYLES: Record<
  EngineState,
  { labelKey: string; dot: string; text: string }
> = {
  idle: { labelKey: "diagnostics.state.idle", dot: "bg-brand-cyan", text: "text-brand-cyan" },
  connecting: {
    labelKey: "diagnostics.state.connecting",
    dot: "bg-brand-amber animate-pulse",
    text: "text-brand-amber",
  },
  starting: {
    labelKey: "diagnostics.state.starting",
    dot: "bg-brand-amber animate-pulse",
    text: "text-brand-amber",
  },
  connected: {
    labelKey: "diagnostics.state.connected",
    dot: "bg-brand-violet",
    text: "text-brand-violet",
  },
  "needs-elevation": {
    labelKey: "diagnostics.state.needs-elevation",
    dot: "bg-brand-amber",
    text: "text-brand-amber",
  },
  error: { labelKey: "diagnostics.state.error", dot: "bg-brand-red", text: "text-brand-red" },
};
