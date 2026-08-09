import type { ComponentPropsWithoutRef } from "react";
import { cn } from "../../lib/cn";

export type ButtonVariant = "primary" | "secondary" | "ghost" | "danger" | "warning";
export type ButtonSize = "sm" | "md";

export interface ButtonProps extends ComponentPropsWithoutRef<"button"> {
  variant?: ButtonVariant;
  size?: ButtonSize;
}

const VARIANT_CLASSES: Record<ButtonVariant, string> = {
  primary:
    // text-surface, not text-white: measured contrast (WCAG) of white-on-violet
    // fails AA in 4 of the app's 6 themes (as low as 2.54:1 in Aurora) since
    // several theme accents are light/pastel violets. Each theme's own
    // (very dark) surface color reads at 4.5–7.4:1 against its violet in
    // every theme — a token-driven fix, not a one-off hardcoded color.
    // The hover glow is a `box-shadow` transition only (no new animation loop).
    "bg-brand-violet text-surface hover:bg-brand-violet/90 hover:shadow-[0_0_16px_-4px_var(--color-brand-violet)] focus-visible:ring-brand-violet/60 disabled:bg-white/5 disabled:text-ink-muted disabled:shadow-none",
  secondary:
    "border border-brand-violet/40 text-brand-violet hover:bg-brand-violet/10 focus-visible:ring-brand-violet/60 disabled:border-white/10 disabled:text-ink-muted",
  ghost:
    "text-ink-muted hover:bg-white/5 hover:text-ink focus-visible:ring-white/20 disabled:text-ink-muted/50",
  danger:
    "border border-brand-red/40 text-brand-red hover:bg-brand-red/10 focus-visible:ring-brand-red/60 disabled:border-white/10 disabled:text-ink-muted",
  warning:
    "border border-brand-amber/50 text-brand-amber hover:bg-brand-amber/15 focus-visible:ring-brand-amber/60 disabled:border-white/10 disabled:text-ink-muted",
};

const SIZE_CLASSES: Record<ButtonSize, string> = {
  sm: "px-3 py-1.5 text-xs",
  md: "px-4 py-2 text-sm",
};

/**
 * Shared button primitive — the single source of truth for interactive
 * feedback (hover/disabled/focus) across the app. Every hand-rolled button
 * string this replaced (Dashboard's ping button, Minecraft's preset button,
 * VirtualNetworkPanel's Create/Join/Leave, Settings' toggles) had its own
 * slightly different hover/disabled treatment; this is why.
 */
export default function Button({
  variant = "primary",
  size = "md",
  disabled,
  className,
  ...props
}: ButtonProps) {
  return (
    <button
      type="button"
      disabled={disabled}
      className={cn(
        "rounded-xl font-medium transition-[background-color,color,box-shadow] duration-150",
        "focus-visible:outline-none focus-visible:ring-2",
        disabled ? "cursor-not-allowed" : "cursor-pointer",
        VARIANT_CLASSES[variant],
        SIZE_CLASSES[size],
        className,
      )}
      {...props}
    />
  );
}
