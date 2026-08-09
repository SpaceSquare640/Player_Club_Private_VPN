import { useNavigate } from "react-router";
import { useTranslation } from "react-i18next";
import { Activity, Gamepad2, Network as NetworkIcon, Radio, Timer, TrendingDown, Waves } from "lucide-react";
import { useTelemetryStore } from "../stores/telemetryStore";
import { ENGINE_STATE_STYLES } from "../lib/engineStateStyles";
import { cn } from "../lib/cn";
import Card from "../components/ui/Card";
import Button from "../components/ui/Button";
import StatTile from "../components/ui/StatTile";
import GlowChip from "../components/ui/GlowChip";

/**
 * Home page: a real overview of the live engine state plus shortcuts to the
 * other pages — replaces the earlier scaffolding-era stub (a "ping engine"
 * button over hardcoded fake status). All data here already flows app-wide
 * via `useEngineTelemetry` (subscribed once in `AppShell`); this page adds
 * no new IPC calls of its own.
 */
export default function Dashboard() {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const running = useTelemetryStore((s) => s.running);
  const state = useTelemetryStore((s) => s.state);
  const snapshot = useTelemetryStore((s) => s.snapshot);
  const packets = useTelemetryStore((s) => s.packets);

  const sm = ENGINE_STATE_STYLES[state];
  const fmt = (n: number | undefined, d = 1) => (n == null ? "—" : n.toFixed(d));
  const recent = packets.slice(-5).reverse();

  return (
    <div className="space-y-6" data-testid="page-dashboard">
      <div>
        <h1 className="text-2xl font-semibold text-balance text-ink">{t("dashboard.title")}</h1>
        <p className="text-sm text-pretty text-ink-muted">{t("dashboard.subtitle")}</p>
      </div>

      <Card variant="raised" data-testid="dashboard-status">
        <div className="flex items-center justify-between">
          <h2 className="text-xs font-semibold uppercase tracking-wider text-ink-muted">
            {t("dashboard.statusHeading")}
          </h2>
          <span className="flex items-center gap-2 text-sm" data-testid="dashboard-engine-state">
            <span className={cn("size-2.5 rounded-full", sm.dot)} />
            <span className={sm.text}>{t(sm.labelKey)}</span>
          </span>
        </div>

        {running ? (
          <div className="mt-4 grid grid-cols-2 gap-3 sm:grid-cols-4">
            <StatTile
              testid="dashboard-stat-rtt"
              label={t("diagnostics.statRttLabel")}
              value={fmt(snapshot?.rttMs)}
              unit="ms"
              icon={Timer}
              tone="cyan"
            />
            <StatTile
              testid="dashboard-stat-throughput"
              label={t("diagnostics.statThroughputLabel")}
              value={`${fmt(snapshot?.txKbps, 0)}/${fmt(snapshot?.rxKbps, 0)}`}
              unit="kbps"
              icon={Waves}
              tone="violet"
            />
            <StatTile
              testid="dashboard-stat-loss"
              label={t("diagnostics.statLossLabel")}
              value={fmt(snapshot?.lossPct, 2)}
              unit="%"
              icon={TrendingDown}
              tone="amber"
            />
            <StatTile
              testid="dashboard-stat-fec"
              label={t("diagnostics.statFecLabel")}
              value={snapshot?.fecRecovered == null ? "—" : String(snapshot.fecRecovered)}
              unit="pkts"
              icon={Radio}
              tone="violet"
            />
          </div>
        ) : (
          <div className="mt-4 flex items-center justify-between gap-4 rounded-xl border border-white/10 bg-black/20 px-4 py-3">
            <p className="text-sm text-pretty text-ink-muted">{t("dashboard.notRunning")}</p>
            <Button
              variant="secondary"
              size="sm"
              data-testid="dashboard-open-diagnostics"
              onClick={() => navigate("/diagnostics")}
              className="shrink-0"
            >
              {t("dashboard.goDiagnostics")}
            </Button>
          </div>
        )}
      </Card>

      <div>
        <h2 className="mb-3 text-xs font-semibold uppercase tracking-wider text-ink-muted">
          {t("dashboard.quickActionsHeading")}
        </h2>
        <div className="grid grid-cols-1 gap-3 sm:grid-cols-3" data-testid="dashboard-quick-actions">
          <button
            type="button"
            data-testid="dashboard-go-network"
            onClick={() => navigate("/network")}
            className="flex items-center gap-3 rounded-2xl border border-white/5 bg-surface-2 p-4 text-left transition-colors duration-150 hover:border-white/10 hover:bg-white/5"
          >
            <GlowChip tone="cyan">
              <NetworkIcon size={18} />
            </GlowChip>
            <div className="min-w-0">
              <div className="text-sm font-medium text-ink">{t("nav.network")}</div>
              <div className="truncate text-xs text-ink-muted">{t("dashboard.goNetworkHint")}</div>
            </div>
          </button>

          <button
            type="button"
            data-testid="dashboard-go-diagnostics"
            onClick={() => navigate("/diagnostics")}
            className="flex items-center gap-3 rounded-2xl border border-white/5 bg-surface-2 p-4 text-left transition-colors duration-150 hover:border-white/10 hover:bg-white/5"
          >
            <GlowChip tone="violet">
              <Activity size={18} />
            </GlowChip>
            <div className="min-w-0">
              <div className="text-sm font-medium text-ink">{t("nav.diagnostics")}</div>
              <div className="truncate text-xs text-ink-muted">{t("dashboard.goDiagnosticsHint")}</div>
            </div>
          </button>

          <button
            type="button"
            data-testid="dashboard-go-minecraft"
            onClick={() => navigate("/minecraft")}
            className="flex items-center gap-3 rounded-2xl border border-white/5 bg-surface-2 p-4 text-left transition-colors duration-150 hover:border-white/10 hover:bg-white/5"
          >
            <GlowChip tone="amber">
              <Gamepad2 size={18} />
            </GlowChip>
            <div className="min-w-0">
              <div className="text-sm font-medium text-ink">{t("nav.minecraft")}</div>
              <div className="truncate text-xs text-ink-muted">{t("dashboard.goMinecraftHint")}</div>
            </div>
          </button>
        </div>
      </div>

      <div>
        <h2 className="mb-3 text-xs font-semibold uppercase tracking-wider text-ink-muted">
          {t("dashboard.recentActivityHeading")}
        </h2>
        <Card className="p-0" data-testid="dashboard-recent-activity">
          {recent.length === 0 ? (
            <p className="p-4 text-sm text-pretty text-ink-muted">{t("dashboard.noActivity")}</p>
          ) : (
            <ul className="divide-y divide-white/5">
              {recent.map((p, i) => (
                <li
                  key={`${p.tMs}-${i}`}
                  className="flex items-center gap-3 px-4 py-2 font-mono text-xs"
                >
                  <span className="tabular-nums text-ink-muted">{(p.tMs / 1000).toFixed(2)}s</span>
                  <span className={p.dir === "tx" ? "text-brand-violet" : "text-brand-cyan"}>
                    {p.dir.toUpperCase()}
                  </span>
                  <span className="text-ink">{p.proto}</span>
                  <span className="tabular-nums text-ink-muted">{p.len}B</span>
                  <span className="truncate text-ink-muted">{p.note}</span>
                </li>
              ))}
            </ul>
          )}
        </Card>
      </div>
    </div>
  );
}
