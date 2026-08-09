import type { ComponentPropsWithoutRef } from "react";
import { cn } from "../../lib/cn";
import type { BadgeTone } from "./Badge";

export interface GlowChipProps extends ComponentPropsWithoutRef<"div"> {
  tone?: Exclude<BadgeTone, "neutral">;
  size?: "sm" | "md";
}

const TONE_CLASSES: Record<Exclude<BadgeTone, "neutral">, string> = {
  violet: "bg-brand-violet/15 text-brand-violet shadow-[0_0_24px_-6px_var(--color-brand-violet)]",
  cyan: "bg-brand-cyan/15 text-brand-cyan shadow-[0_0_24px_-6px_var(--color-brand-cyan)]",
  amber: "bg-brand-amber/15 text-brand-amber shadow-[0_0_24px_-6px_var(--color-brand-amber)]",
  red: "bg-brand-red/15 text-brand-red shadow-[0_0_24px_-6px_var(--color-brand-red)]",
};

const SIZE_CLASSES: Record<"sm" | "md", string> = {
  sm: "size-8 rounded-lg",
  md: "size-11 rounded-xl",
};

/**
 * An icon container with a soft glow behind it, colored from the same
 * `--color-brand-*` token `Badge` already uses — every theme's own hue
 * glows correctly (Aurora's pastel violet vs. Abyss's teal), never a
 * hardcoded color. `box-shadow` only, no blur filter — cheap, static, no
 * animation loop of its own.
 *
 * Replaces bare `<Icon className="text-brand-*" />` left un-contained
 * (Dashboard's quick-action cards, page-header icons) with a deliberate,
 * repeatable icon treatment instead of one-off styling per call site.
 */
export default function GlowChip({ tone = "violet", size = "md", className, children, ...props }: GlowChipProps) {
  return (
    <div
      className={cn("flex shrink-0 items-center justify-center", SIZE_CLASSES[size], TONE_CLASSES[tone], className)}
      {...props}
    >
      {children}
    </div>
  );
}
