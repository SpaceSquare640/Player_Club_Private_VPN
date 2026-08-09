import { useState } from "react";
import { useTranslation } from "react-i18next";
import { Ban, Radio, Timer, TrendingDown, Waves, Zap } from "lucide-react";
import { useTelemetryStore } from "../stores/telemetryStore";
import { configForMode, requestElevation, startEngine, stopEngine } from "../lib/engine";
import { cn } from "../lib/cn";
import { ENGINE_STATE_STYLES } from "../lib/engineStateStyles";
import SpectrumChart from "../components/diagnostics/SpectrumChart";
import TopologyView from "../components/diagnostics/TopologyView";
import StatTile from "../components/ui/StatTile";
import Button from "../components/ui/Button";
import type { EngineMode } from "../types/telemetry";

const MODE_LABEL_KEYS: Record<EngineMode, string> = {
  simulated: "diagnostics.mode.simulated",
  probe: "diagnostics.mode.probe",
  real: "diagnostics.mode.real",
};

/**
 * Diagnostics — the live *readout* of whatever session is active: a telemetry
 * source (Simulated / Transport probe / Real adapter) started here, or a peer
 * link established on the Network page. Connection management itself lives on the
 * Network page; this view only observes — including the two-node topology view
 * and the live tx/rx spectrum chart below.
 */
export default function Diagnostics() {
  const { t } = useTranslation();
  const running = useTelemetryStore((s) => s.running);
  const state = useTelemetryStore((s) => s.state);
  const snapshot = useTelemetryStore((s) => s.snapshot);
  const packets = useTelemetryStore((s) => s.packets);
  const notice = useTelemetryStore((s) => s.notice);

  const [mode, setMode] = useState<EngineMode>("simulated");

  const sm = ENGINE_STATE_STYLES[state];
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
          <h1 className="text-2xl font-semibold text-balance text-ink">{t("diagnostics.title")}</h1>
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
            <Button variant="danger" size="sm" data-testid="engine-stop" onClick={stop}>
              {t("diagnostics.stop")}
            </Button>
          ) : (
            <Button variant="secondary" size="sm" data-testid="engine-start" onClick={start}>
              {t("diagnostics.start")}
            </Button>
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
          <Button
            variant="warning"
            size="sm"
            data-testid="relaunch-admin"
            onClick={relaunch}
            className="shrink-0"
          >
            {t("diagnostics.relaunchAsAdmin")}
          </Button>
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
          icon={Timer}
          tone="cyan"
        />
        <StatTile
          testid="stat-jitter"
          label={t("diagnostics.statJitterLabel")}
          value={fmt(snapshot?.jitterMs)}
          unit="ms"
          icon={Zap}
          tone="cyan"
        />
        <StatTile
          testid="stat-loss"
          label={t("diagnostics.statLossLabel")}
          value={fmt(snapshot?.lossPct, 2)}
          unit="%"
          icon={TrendingDown}
          tone="amber"
        />
        <StatTile
          testid="stat-throughput"
          label={t("diagnostics.statThroughputLabel")}
          value={`${fmt(snapshot?.txKbps, 0)}/${fmt(snapshot?.rxKbps, 0)}`}
          unit="kbps"
          icon={Waves}
          tone="violet"
        />
        {/* Cumulative for the session — a per-second figure would read as 0 on a
            healthy link, which is exactly when these numbers should reassure. */}
        <StatTile
          testid="stat-fec"
          label={t("diagnostics.statFecLabel")}
          value={count(snapshot?.fecRecovered)}
          unit="pkts"
          title={t("diagnostics.statFecTitle")}
          icon={Radio}
          tone="violet"
        />
        <StatTile
          testid="stat-blocked"
          label={t("diagnostics.statBlockedLabel")}
          value={count(snapshot?.policyBlocked)}
          unit="pkts"
          title={t("diagnostics.statBlockedTitle")}
          icon={Ban}
          tone="red"
        />
      </div>

      <div
        className="grid h-44 shrink-0 grid-cols-1 gap-3 lg:grid-cols-[1.618fr_1fr]"
        data-testid="viz-row"
      >
        <SpectrumChart />
        <TopologyView />
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
