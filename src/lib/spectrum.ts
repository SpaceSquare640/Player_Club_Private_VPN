/**
 * Pure SVG path math for the Spectrum chart — no DOM, no React, so it is
 * trivially unit-testable. Kept separate from `SpectrumChart.tsx` for exactly
 * that reason.
 */

/** A friendly y-axis ceiling with headroom, not just the raw peak sample. */
export function niceMax(values: number[]): number {
  const peak = Math.max(0, ...values);
  if (peak <= 0) return 10; // avoid a degenerate zero-height chart while idle
  const magnitude = Math.pow(10, Math.floor(Math.log10(peak)));
  return Math.ceil((peak * 1.2) / magnitude) * magnitude;
}

/** x-coordinate of sample `index` out of `count` samples across `width`. */
export function sampleX(index: number, count: number, width: number): number {
  if (count <= 1) return 0;
  return (index / (count - 1)) * width;
}

/** y-coordinate for `value` given the chart's `maxValue` ceiling and `height`. */
export function sampleY(value: number, maxValue: number, height: number): number {
  if (maxValue <= 0) return height;
  return height - (Math.min(value, maxValue) / maxValue) * height;
}

/** An `M…L…L…` polyline path through `values`, scaled to `width` × `height`. */
export function buildLinePath(
  values: number[],
  width: number,
  height: number,
  maxValue: number,
): string {
  if (values.length === 0) return "";
  return values
    .map((v, i) => {
      const x = sampleX(i, values.length, width).toFixed(2);
      const y = sampleY(v, maxValue, height).toFixed(2);
      return `${i === 0 ? "M" : "L"}${x},${y}`;
    })
    .join(" ");
}

/** The line path closed down to the baseline, for a filled area under it. */
export function buildAreaPath(
  values: number[],
  width: number,
  height: number,
  maxValue: number,
): string {
  if (values.length === 0) return "";
  const line = buildLinePath(values, width, height, maxValue);
  const lastX = sampleX(values.length - 1, values.length, width).toFixed(2);
  return `${line} L${lastX},${height.toFixed(2)} L0,${height.toFixed(2)} Z`;
}

/** Nearest sample index to a pointer at `pointerX` within a `width`-wide chart. */
export function nearestIndex(pointerX: number, count: number, width: number): number {
  if (count <= 1) return 0;
  const raw = Math.round((pointerX / width) * (count - 1));
  return Math.min(count - 1, Math.max(0, raw));
}
