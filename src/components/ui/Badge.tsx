import type { ComponentPropsWithoutRef } from "react";
import { cn } from "../../lib/cn";

export type BadgeTone = "violet" | "cyan" | "amber" | "red" | "neutral";

export interface BadgeProps extends ComponentPropsWithoutRef<"span"> {
  tone?: BadgeTone;
}

const TONE_CLASSES: Record<BadgeTone, string> = {
  violet: "bg-brand-violet/15 text-brand-violet",
  cyan: "bg-brand-cyan/15 text-brand-cyan",
  amber: "bg-brand-amber/15 text-brand-amber",
  red: "bg-brand-red/15 text-brand-red",
  neutral: "bg-white/10 text-ink-muted",
};

/** Small status/tag chip — replaces one-off `<span className="rounded ...">` markup. */
export default function Badge({ tone = "neutral", className, ...props }: BadgeProps) {
  return (
    <span
      className={cn(
        "inline-flex items-center rounded-md px-1.5 py-0.5 text-[10px] font-medium",
        TONE_CLASSES[tone],
        className,
      )}
      {...props}
    />
  );
}
