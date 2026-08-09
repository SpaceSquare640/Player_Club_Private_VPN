export interface StatTileProps {
  label: string;
  value: string;
  unit: string;
  testid: string;
  title?: string;
}

/**
 * Promoted out of `Diagnostics.tsx` (where it was already well-built) so
 * `Dashboard`'s overview can reuse the exact same stat-tile visual language
 * instead of reinventing it.
 */
export default function StatTile({ label, value, unit, testid, title }: StatTileProps) {
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
