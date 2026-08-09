import type { ComponentPropsWithoutRef } from "react";
import { cn } from "../../lib/cn";

export interface CardProps extends ComponentPropsWithoutRef<"div"> {
  /**
   * `base` — a primary panel sitting directly on the page background.
   * `raised` — a card nested inside another surface (e.g. the two
   * side-by-side forms in `VirtualNetworkPanel`), lifted with a subtle
   * shadow + brighter border rather than a new flat background color, so
   * the elevation reads as depth instead of just "a different grey."
   */
  variant?: "base" | "raised";
}

/**
 * Shared panel primitive. Centralizes what used to be a hand-copied
 * `rounded-2xl bg-surface-2 p-6 ring-1 ring-white/5` string duplicated
 * across every page — the drift between those copies (padding, radius,
 * border opacity) was a real source of the app's inconsistent feel.
 */
export default function Card({ variant = "base", className, ...props }: CardProps) {
  return (
    <div
      className={cn(
        "rounded-2xl border p-6",
        variant === "base"
          ? "border-white/5 bg-surface-2"
          : // A soft inset top-edge highlight on top of the existing drop shadow —
            // the "glass" depth cue: a real panel resting above the surface
            // catches a hint of light on its top edge. `box-shadow` only,
            // never `backdrop-blur` (expensive, and unnecessary for an opaque panel).
            "border-white/10 bg-surface-2 shadow-lg shadow-black/20 [box-shadow:inset_0_1px_0_0_rgba(255,255,255,.06),0_10px_15px_-3px_rgb(0_0_0_/_0.2)]",
        className,
      )}
      {...props}
    />
  );
}
