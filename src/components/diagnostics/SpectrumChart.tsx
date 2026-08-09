import { useRef, useState, type MouseEventHandler } from "react";
import { useTranslation } from "react-i18next";
import { useTelemetryStore } from "../../stores/telemetryStore";
import { buildAreaPath, buildLinePath, nearestIndex, niceMax } from "../../lib/spectrum";

/** Internal SVG coordinate space — stretched to the container via `preserveAspectRatio="none"`. */
const VB_WIDTH = 300;
const VB_HEIGHT = 100;

/**
 * Live tx/rx throughput chart. One axis (kbps, shared by both series — never a
 * dual-axis chart), two fixed-hue series matching the packet log's existing
 * tx=violet / rx=cyan convention so color means the same thing everywhere in
 * Diagnostics. Hand-rolled SVG: the underlying math is ~30 lines (`lib/spectrum.ts`),
 * not worth a charting dependency for.
 */
export default function SpectrumChart() {
  const { t } = useTranslation();
  const history = useTelemetryStore((s) => s.spectrumHistory);
  const svgRef = useRef<SVGSVGElement>(null);
  const [hoverIndex, setHoverIndex] = useState<number | null>(null);

  if (history.length === 0) {
    return (
      <div
        data-testid="spectrum-chart"
        className="flex h-full flex-col rounded-xl border border-white/10 bg-surface-2/60 p-4"
      >
        <Header t={t} />
        <div className="flex flex-1 items-center justify-center text-xs text-ink-muted">
          {t("diagnostics.spectrumEmpty")}
        </div>
      </div>
    );
  }

  const tx = history.map((s) => s.txKbps);
  const rx = history.map((s) => s.rxKbps);
  const maxValue = niceMax([...tx, ...rx]);
  const txLine = buildLinePath(tx, VB_WIDTH, VB_HEIGHT, maxValue);
  const rxLine = buildLinePath(rx, VB_WIDTH, VB_HEIGHT, maxValue);
  const txArea = buildAreaPath(tx, VB_WIDTH, VB_HEIGHT, maxValue);
  const rxArea = buildAreaPath(rx, VB_WIDTH, VB_HEIGHT, maxValue);

  const onMove: MouseEventHandler<SVGSVGElement> = (e) => {
    const rect = svgRef.current?.getBoundingClientRect();
    if (!rect || rect.width === 0) return;
    const px = ((e.clientX - rect.left) / rect.width) * VB_WIDTH;
    setHoverIndex(nearestIndex(px, history.length, VB_WIDTH));
  };

  const hover = hoverIndex != null ? history[hoverIndex] : null;
  const hoverX = hoverIndex != null ? (hoverIndex / Math.max(1, history.length - 1)) * VB_WIDTH : 0;

  return (
    <div
      data-testid="spectrum-chart"
      className="flex h-full flex-col rounded-xl border border-white/10 bg-surface-2/60 p-4"
    >
      <Header t={t} maxValue={maxValue} />
      <div className="relative mt-2 flex-1">
        <svg
          ref={svgRef}
          viewBox={`0 0 ${VB_WIDTH} ${VB_HEIGHT}`}
          preserveAspectRatio="none"
          className="h-full w-full"
          onMouseMove={onMove}
          onMouseLeave={() => setHoverIndex(null)}
        >
          {/* Two-stop gradients (opaque near the line, transparent at the
              baseline) instead of a flat low-opacity fill — same area-chart
              shape and semantics, richer fill only. Defined via the same
              brand tokens as the lines themselves, so every theme's own
              hues carry through. */}
          <defs>
            <linearGradient id="spectrum-tx-fill" x1="0" y1="0" x2="0" y2="1">
              <stop offset="0%" stopColor="var(--color-brand-violet)" stopOpacity="0.28" />
              <stop offset="100%" stopColor="var(--color-brand-violet)" stopOpacity="0" />
            </linearGradient>
            <linearGradient id="spectrum-rx-fill" x1="0" y1="0" x2="0" y2="1">
              <stop offset="0%" stopColor="var(--color-brand-cyan)" stopOpacity="0.28" />
              <stop offset="100%" stopColor="var(--color-brand-cyan)" stopOpacity="0" />
            </linearGradient>
          </defs>

          {/* Recessive gridlines — never compete with the data. */}
          <line x1="0" y1={VB_HEIGHT / 2} x2={VB_WIDTH} y2={VB_HEIGHT / 2} className="stroke-white/5" strokeWidth="1" />
          <line x1="0" y1={VB_HEIGHT} x2={VB_WIDTH} y2={VB_HEIGHT} className="stroke-white/10" strokeWidth="1" />

          <path d={txArea} fill="url(#spectrum-tx-fill)" />
          <path d={rxArea} fill="url(#spectrum-rx-fill)" />
          <path d={txLine} className="stroke-brand-violet" strokeWidth="2" fill="none" strokeLinejoin="round" strokeLinecap="round" />
          <path d={rxLine} className="stroke-brand-cyan" strokeWidth="2" fill="none" strokeLinejoin="round" strokeLinecap="round" />

          {hover && (
            <>
              <line x1={hoverX} y1="0" x2={hoverX} y2={VB_HEIGHT} className="stroke-white/20" strokeWidth="1" />
              <circle
                cx={hoverX}
                cy={sampleYFor(hover.txKbps, maxValue)}
                r="2.5"
                className="fill-brand-violet"
              />
              <circle
                cx={hoverX}
                cy={sampleYFor(hover.rxKbps, maxValue)}
                r="2.5"
                className="fill-brand-cyan"
              />
            </>
          )}
        </svg>

        {hover && (
          <div
            data-testid="spectrum-tooltip"
            className="pointer-events-none absolute top-0 rounded border border-white/10 bg-surface px-2 py-1 text-[10px] tabular-nums text-ink shadow-lg"
            style={{
              left: `${(hoverX / VB_WIDTH) * 100}%`,
              transform: hoverX > VB_WIDTH / 2 ? "translateX(-100%)" : undefined,
            }}
          >
            <div className="text-brand-violet">
              {t("diagnostics.spectrumTx")} {Math.round(hover.txKbps)} {t("diagnostics.spectrumUnit")}
            </div>
            <div className="text-brand-cyan">
              {t("diagnostics.spectrumRx")} {Math.round(hover.rxKbps)} {t("diagnostics.spectrumUnit")}
            </div>
          </div>
        )}
      </div>
    </div>
  );
}

function sampleYFor(value: number, maxValue: number): number {
  if (maxValue <= 0) return VB_HEIGHT;
  return VB_HEIGHT - (Math.min(value, maxValue) / maxValue) * VB_HEIGHT;
}

function Header({
  t,
  maxValue,
}: {
  t: (key: string) => string;
  maxValue?: number;
}) {
  return (
    <div className="flex items-center justify-between">
      <span className="text-xs font-semibold uppercase tracking-wider text-ink-muted">
        {t("diagnostics.spectrumHeading")}
      </span>
      <div className="flex items-center gap-3 text-[10px] text-ink-muted">
        {maxValue != null && (
          <span className="tabular-nums">
            {Math.round(maxValue)} {t("diagnostics.spectrumUnit")}
          </span>
        )}
        <span className="flex items-center gap-1">
          <span className="h-2 w-2 rounded-full bg-brand-violet" />
          {t("diagnostics.spectrumTx")}
        </span>
        <span className="flex items-center gap-1">
          <span className="h-2 w-2 rounded-full bg-brand-cyan" />
          {t("diagnostics.spectrumRx")}
        </span>
      </div>
    </div>
  );
}
