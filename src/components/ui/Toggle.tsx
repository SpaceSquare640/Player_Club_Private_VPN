import { cn } from "../../lib/cn";

export interface ToggleProps {
  checked: boolean;
  onChange: (checked: boolean) => void;
  /** Required — an icon/switch-only control must always have an accessible name. */
  label: string;
  disabled?: boolean;
  "data-testid"?: string;
}

/**
 * A real switch affordance — track + sliding thumb — replacing the previous
 * pattern of a full-width button whose border color changed and whose label
 * read literally "On"/"Off". Restyled from the cleanest toggle among the
 * UIVerse reference snippets (`Resource/UI Design Tmplate/UI Verse/`),
 * recolored onto this app's theme tokens rather than its original hardcoded
 * palette. `role="switch"` + `aria-checked` gives it real toggle semantics
 * (not just a styled checkbox); the thumb only animates `transform`
 * (compositor-safe) and stays under the 200ms interaction-feedback budget.
 */
export default function Toggle({ checked, onChange, label, disabled, ...rest }: ToggleProps) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      aria-label={label}
      title={label}
      disabled={disabled}
      data-testid={rest["data-testid"]}
      onClick={() => onChange(!checked)}
      className={cn(
        "relative h-6 w-11 shrink-0 rounded-full transition-[background-color,box-shadow] duration-150",
        "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-brand-violet/60",
        checked
          ? // The one deliberate, scoped exception to "no glow-as-affordance" in
            // this app — reserved for the single control whose entire job is
            // communicating on/off, not applied anywhere decorative.
            "bg-brand-violet shadow-[0_0_10px_-2px_var(--color-brand-violet)]"
          : "bg-white/10",
        disabled ? "cursor-not-allowed opacity-50" : "cursor-pointer",
      )}
    >
      <span
        className={cn(
          "absolute top-0.5 left-0.5 size-5 rounded-full bg-white shadow transition-transform duration-150",
          checked && "translate-x-5",
        )}
      />
    </button>
  );
}
