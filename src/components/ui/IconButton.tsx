import type { ComponentPropsWithoutRef } from "react";
import { cn } from "../../lib/cn";

export interface IconButtonProps extends ComponentPropsWithoutRef<"button"> {
  /** Required — an icon-only control must always have an accessible name. */
  label: string;
  active?: boolean;
}

/**
 * Icon-only button primitive (sidebar nav items, settings close/copy
 * buttons). Bundles `aria-label`/`title` from a single required `label`
 * prop so it's structurally impossible to ship an icon button without one.
 */
export default function IconButton({ label, active, className, ...props }: IconButtonProps) {
  return (
    <button
      type="button"
      aria-label={label}
      title={label}
      aria-pressed={active}
      className={cn(
        "flex size-10 items-center justify-center rounded-xl transition-colors duration-150",
        "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-brand-violet/60",
        active
          ? "bg-brand-violet/15 text-brand-violet"
          : "text-ink-muted hover:bg-white/5 hover:text-ink",
        className,
      )}
      {...props}
    />
  );
}
