import { useTranslation } from "react-i18next";
import { useConnection } from "../../hooks/useConnection";
import { useTelemetryStore } from "../../stores/telemetryStore";
import { cn } from "../../lib/cn";
import type { LinkState } from "../../types/telemetry";

const VB_WIDTH = 300;
const VB_HEIGHT = 100;
const NODE_R = 18;
const NODE_A_X = 46;
const NODE_B_X = VB_WIDTH - 46;
const NODE_Y = VB_HEIGHT / 2;

/**
 * The link state carries the status color (idle/connecting/connected/failed) —
 * the same semantic mapping used elsewhere in Diagnostics (cyan idle, amber
 * transient, violet active, red error). Nodes stay a neutral outline: encoding
 * status on both the nodes *and* the line would say the same thing twice for no
 * added information, which is the anti-pattern the dataviz guide calls out.
 */
const LINK_STYLE: Record<LinkState, { stroke: string; dash?: string; labelKey: string }> = {
  idle: { stroke: "stroke-ink-muted/30", dash: "4 4", labelKey: "diagnostics.topologyLinkIdle" },
  connecting: {
    stroke: "stroke-brand-amber animate-pulse",
    dash: "4 4",
    labelKey: "diagnostics.topologyLinkConnecting",
  },
  connected: { stroke: "stroke-brand-violet", labelKey: "diagnostics.topologyLinkConnected" },
  failed: { stroke: "stroke-brand-red", dash: "2 4", labelKey: "diagnostics.topologyLinkFailed" },
};

/**
 * This app is strictly point-to-point — a two-node view, not a general graph.
 * A layout engine for a topology that can only ever have two nodes would be
 * solving a problem this product doesn't have; if multi-peer support ever
 * lands, that is the point a real graph layout becomes worth it.
 */
export default function TopologyView() {
  const { t } = useTranslation();
  const { conn, link } = useConnection();
  const identity = useTelemetryStore((s) => s.identity);
  const rttMs = useTelemetryStore((s) => s.snapshot?.rttMs);

  const style = LINK_STYLE[link];
  const showRtt = link === "connected" && rttMs != null;

  return (
    <div
      data-testid="topology-view"
      className="flex h-full flex-col rounded-xl border border-white/10 bg-surface-2/60 p-4"
    >
      <span className="text-xs font-semibold uppercase tracking-wider text-ink-muted">
        {t("diagnostics.topologyHeading")}
      </span>
      <div className="relative mt-2 flex-1">
        <svg viewBox={`0 0 ${VB_WIDTH} ${VB_HEIGHT}`} preserveAspectRatio="xMidYMid meet" className="h-full w-full">
          <line
            x1={NODE_A_X + NODE_R}
            y1={NODE_Y}
            x2={NODE_B_X - NODE_R}
            y2={NODE_Y}
            className={style.stroke}
            strokeWidth="2"
            strokeDasharray={style.dash}
            strokeLinecap="round"
          />

          <circle cx={NODE_A_X} cy={NODE_Y} r={NODE_R} className="fill-surface-2 stroke-ink-muted/60" strokeWidth="1.5" />
          <circle cx={NODE_B_X} cy={NODE_Y} r={NODE_R} className="fill-surface-2 stroke-ink-muted/60" strokeWidth="1.5" />

          <text x={NODE_A_X} y={NODE_Y + VB_HEIGHT * 0.02} textAnchor="middle" className="fill-ink text-[10px] font-medium" style={{ fontSize: 10 }}>
            {t("diagnostics.topologyThisNode")}
          </text>
          <text x={NODE_B_X} y={NODE_Y + VB_HEIGHT * 0.02} textAnchor="middle" className="fill-ink text-[10px] font-medium" style={{ fontSize: 10 }}>
            {t("diagnostics.topologyPeer")}
          </text>

          {showRtt && (
            <text
              x={VB_WIDTH / 2}
              y={NODE_Y - 8}
              textAnchor="middle"
              className="fill-brand-violet tabular-nums"
              style={{ fontSize: 9 }}
            >
              {Math.round(rttMs as number)} ms
            </text>
          )}
        </svg>
      </div>

      <div className="mt-2 flex items-center justify-between text-[10px]">
        <span data-testid="topology-this-node" className="font-mono text-ink-muted">
          {identity?.peerAddress ?? "—"}
        </span>
        <span
          data-testid="topology-link-label"
          className={cn(
            "font-medium",
            link === "connected"
              ? "text-brand-violet"
              : link === "connecting"
                ? "text-brand-amber"
                : link === "failed"
                  ? "text-brand-red"
                  : "text-ink-muted",
          )}
        >
          {t(style.labelKey)}
        </span>
        <span data-testid="topology-peer" className="font-mono text-ink-muted">
          {conn?.peer?.peerAddress ?? t("diagnostics.topologyNoPeer")}
        </span>
      </div>
    </div>
  );
}
