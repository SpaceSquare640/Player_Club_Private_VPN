import type { LucideIcon } from "lucide-react";
import { cn } from "../../lib/cn";
import GlowChip from "./GlowChip";
import type { BadgeTone } from "./Badge";

export interface StatTileProps {
  label: string;
  value: string;
  unit: string;
  testid: string;
  title?: string;
  /** Optional leading glyph, shown in a small `GlowChip`. Omit for the plain, icon-less layout. */
  icon?: LucideIcon;
  /** Colors the icon chip and the bottom accent bar. Defaults to a neutral (colorless) tile when omitted. */
  tone?: Exclude<BadgeTone, "neutral">;
}

const ACCENT_CLASSES: Record<Exclude<BadgeTone, "neutral">, string> = {
  violet: "bg-brand-violet",
  cyan: "bg-brand-cyan",
  amber: "bg-brand-amber",
  red: "bg-brand-red",
};

/**
 * Promoted out of `Diagnostics.tsx` (where it was already well-built) so
 * `Dashboard`'s overview can reuse the exact same stat-tile visual language
 * instead of reinventing it. `icon`/`tone` are both optional so existing
 * call sites keep working unstyled; pass them to opt into the richer
 * icon-chip + accent-bar treatment.
 */
export default function StatTile({ label, value, unit, testid, title, icon: Icon, tone }: StatTileProps) {
  return (
    <div
      data-testid={testid}
      title={title}
      className="relative overflow-hidden rounded-xl border border-white/10 bg-surface-2/60 p-4"
    >
      <div className="flex items-center gap-2.5">
        {Icon && (
          <GlowChip tone={tone ?? "violet"} size="sm">
            <Icon size={14} />
          </GlowChip>
        )}
        <div className="min-w-0">
          <div className="text-xs uppercase tracking-wider text-ink-muted">{label}</div>
          <div className="mt-1 flex items-baseline gap-1">
            <span className="text-2xl font-semibold tabular-nums text-ink">{value}</span>
            <span className="text-xs text-ink-muted">{unit}</span>
          </div>
        </div>
      </div>
      {tone && <span className={cn("absolute inset-x-0 bottom-0 h-0.5", ACCENT_CLASSES[tone])} />}
    </div>
  );
}
