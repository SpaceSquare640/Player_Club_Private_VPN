import { useState } from "react";
import { useTranslation } from "react-i18next";
import { useTelemetryStore } from "../stores/telemetryStore";
import { configForMode, requestElevation, startEngine, stopEngine } from "../lib/engine";
import { cn } from "../lib/cn";
import type { EngineMode, EngineState } from "../types/telemetry";

const MODE_LABEL_KEYS: Record<EngineMode, string> = {
  simulated: "diagnostics.mode.simulated",
  probe: "diagnostics.mode.probe",
  real: "diagnostics.mode.real",
};

/**
 * Semantic styling per lifecycle state (Cyan idle, Amber transient, …). Labels
 * are translation *keys*, not literal strings — this is a module-level const,
 * evaluated once at import time, so it cannot hold text that needs to change
 * when the user switches language. `t(STATE_STYLES[state].labelKey)` resolves
 * it at render time instead.
 */
const STATE_STYLES: Record<EngineState, { labelKey: string; dot: string; text: string }> = {
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

function StatTile({
  label,
  value,
  unit,
  testid,
  title,
}: {
  label: string;
  value: string;
  unit: string;
  testid: string;
  title?: string;
}) {
  return (
    <div
      data-testid={testid}
      title={title}
      className="rounded-xl border border-white/10 bg-surface-2/60 p-4"
    >
      <div className="text-xs uppercase tracking-wider text-ink-muted">{label}</div>
      <div className="mt-1 flex items-baseline gap-1">
        <span className="text-2xl font-semibold tabular-nums text-ink">{value}</span>
        <span className="text-xs text-ink-muted">{unit}</span>
      </div>
    </div>
  );
}

/**
 * Diagnostics — the live *readout* of whatever session is active: a telemetry
 * source (Simulated / Transport probe / Real adapter) started here, or a peer
 * link established on the Network page. Connection management itself lives on the
 * Network page; this view only observes. Topology map and spectrum monitor
 * arrive with the dedicated Diagnostics module.
 */
export default function Diagnostics() {
  const { t } = useTranslation();
  const running = useTelemetryStore((s) => s.running);
  const state = useTelemetryStore((s) => s.state);
  const snapshot = useTelemetryStore((s) => s.snapshot);
  const packets = useTelemetryStore((s) => s.packets);
  const notice = useTelemetryStore((s) => s.notice);

  const [mode, setMode] = useState<EngineMode>("simulated");

  const sm = STATE_STYLES[state];
  const fmt = (n: number | undefined, d = 1) => (n == null ? "—" : n.toFixed(d));
  // Counters are whole packets — never show "12.0".
  const count = (n: number | undefined) => (n == null ? "—" : String(n));

  // Swallow expected rejections (no Tauri context in a browser preview, or an
  // AlreadyRunning/NotRunning race) so they don't surface as unhandled.
  const start = () => void startEngine(configForMode(mode)).catch(() => {});
  const stop = () => void stopEngine().catch(() => {});
  const relaunch = () => void requestElevation().catch(() => {});

  const needsElevation = state === "needs-elevation";
  // Informational notices (e.g. discovered candidates, data-plane status) shown
  // as a readout when not surfaced by the elevation/error banners. Engine-
  // originated text (`notice.message`) stays in English — see the i18n scope
  // note in the changelog: it needs a structured code+params refactor on the
  // Rust side, not just a React-layer key swap.
  const infoNotice =
    notice && state !== "needs-elevation" && state !== "error" ? notice : null;

  return (
    <section data-testid="page-diagnostics" className="flex h-full flex-col gap-5">
      <header className="flex items-center justify-between">
        <div>
          <h1 className="text-xl font-semibold text-ink">{t("diagnostics.title")}</h1>
          <p className="text-sm text-ink-muted">
            {t("diagnostics.subtitle", { mode: t(MODE_LABEL_KEYS[mode]).toLowerCase() })}
          </p>
        </div>
        <div className="flex items-center gap-3">
          <span className="flex items-center gap-2 text-sm" data-testid="engine-state">
            <span className={cn("h-2.5 w-2.5 rounded-full", sm.dot)} />
            <span className={sm.text}>{t(sm.labelKey)}</span>
          </span>
          {!running && (
            <select
              data-testid="mode-select"
              value={mode}
              onChange={(e) => setMode(e.target.value as EngineMode)}
              title={t("diagnostics.modeSelectTitle")}
              className="rounded-lg border border-white/15 bg-surface-2 px-2 py-1.5 text-xs text-ink-muted"
            >
              <option value="simulated">{t(MODE_LABEL_KEYS.simulated)}</option>
              <option value="probe">{t(MODE_LABEL_KEYS.probe)}</option>
              <option value="real">
                {t(MODE_LABEL_KEYS.real)} {t("diagnostics.mode.realAdminSuffix")}
              </option>
            </select>
          )}
          {running ? (
            <button
              type="button"
              data-testid="engine-stop"
              onClick={stop}
              className="rounded-lg border border-brand-red/40 px-3 py-1.5 text-sm text-brand-red transition-colors hover:bg-brand-red/10"
            >
              {t("diagnostics.stop")}
            </button>
          ) : (
            <button
              type="button"
              data-testid="engine-start"
              onClick={start}
              className="rounded-lg border border-brand-violet/40 px-3 py-1.5 text-sm text-brand-violet transition-colors hover:bg-brand-violet/10"
            >
              {t("diagnostics.start")}
            </button>
          )}
        </div>
      </header>

      {needsElevation && (
        <div
          data-testid="elevation-banner"
          className="flex items-center justify-between gap-4 rounded-xl border border-brand-amber/40 bg-brand-amber/10 px-4 py-3"
        >
          <p className="text-sm text-brand-amber">
            {notice?.message ?? t("diagnostics.elevationDefaultMessage")}
          </p>
          <button
            type="button"
            data-testid="relaunch-admin"
            onClick={relaunch}
            className="shrink-0 rounded-lg border border-brand-amber/50 px-3 py-1.5 text-sm text-brand-amber transition-colors hover:bg-brand-amber/15"
          >
            {t("diagnostics.relaunchAsAdmin")}
          </button>
        </div>
      )}

      {state === "error" && notice && (
        <div
          data-testid="error-banner"
          className="rounded-xl border border-brand-red/40 bg-brand-red/10 px-4 py-3 text-sm text-brand-red"
        >
          {notice.message}
        </div>
      )}

      {infoNotice && (
        <div
          data-testid="info-notice"
          className="rounded-lg border border-white/10 bg-surface-2/40 px-3 py-2 font-mono text-xs text-ink-muted"
        >
          {infoNotice.message}
        </div>
      )}

      <div
        className="grid grid-cols-2 gap-3 sm:grid-cols-3 xl:grid-cols-6"
        data-testid="stat-grid"
      >
        <StatTile
          testid="stat-rtt"
          label={t("diagnostics.statRttLabel")}
          value={fmt(snapshot?.rttMs)}
          unit="ms"
        />
        <StatTile
          testid="stat-jitter"
          label={t("diagnostics.statJitterLabel")}
          value={fmt(snapshot?.jitterMs)}
          unit="ms"
        />
        <StatTile
          testid="stat-loss"
          label={t("diagnostics.statLossLabel")}
          value={fmt(snapshot?.lossPct, 2)}
          unit="%"
        />
        <StatTile
          testid="stat-throughput"
          label={t("diagnostics.statThroughputLabel")}
          value={`${fmt(snapshot?.txKbps, 0)}/${fmt(snapshot?.rxKbps, 0)}`}
          unit="kbps"
        />
        {/* Cumulative for the session — a per-second figure would read as 0 on a
            healthy link, which is exactly when these numbers should reassure. */}
        <StatTile
          testid="stat-fec"
          label={t("diagnostics.statFecLabel")}
          value={count(snapshot?.fecRecovered)}
          unit="pkts"
          title={t("diagnostics.statFecTitle")}
        />
        <StatTile
          testid="stat-blocked"
          label={t("diagnostics.statBlockedLabel")}
          value={count(snapshot?.policyBlocked)}
          unit="pkts"
          title={t("diagnostics.statBlockedTitle")}
        />
      </div>

      <div className="flex min-h-0 flex-1 flex-col rounded-xl border border-white/10 bg-black/40">
        <div className="flex items-center justify-between border-b border-white/10 px-4 py-2">
          <span className="text-xs font-semibold uppercase tracking-wider text-ink-muted">
            {t("diagnostics.packetLog")}
          </span>
          <span className="text-xs tabular-nums text-ink-muted">
            {t("diagnostics.entriesCount", { count: packets.length })}
          </span>
        </div>
        <div
          data-testid="packet-log"
          className="min-h-0 flex-1 overflow-auto p-3 font-mono text-xs leading-relaxed"
        >
          {packets.length === 0 ? (
            <div className="text-ink-muted">{t("diagnostics.noPacketsYet")}</div>
          ) : (
            packets.map((p, i) => (
              <div key={`${p.tMs}-${i}`} className="flex gap-3 whitespace-nowrap">
                <span className="tabular-nums text-ink-muted">
                  {(p.tMs / 1000).toFixed(2)}s
                </span>
                <span className={p.dir === "tx" ? "text-brand-violet" : "text-brand-cyan"}>
                  {p.dir.toUpperCase()}
                </span>
                <span className="text-ink">{p.proto}</span>
                <span className="tabular-nums text-ink-muted">{p.len}B</span>
                <span className="text-ink-muted">{p.note}</span>
              </div>
            ))
          )}
        </div>
      </div>
    </section>
  );
}
