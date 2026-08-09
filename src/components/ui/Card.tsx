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
          : "border-white/10 bg-surface-2 shadow-lg shadow-black/20",
        className,
      )}
      {...props}
    />
  );
}
