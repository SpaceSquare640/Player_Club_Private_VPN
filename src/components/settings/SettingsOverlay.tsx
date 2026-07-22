import { X } from "lucide-react";
import { useAppStore } from "../../stores/appStore";
import { THEMES } from "../../theme/themes";
import { cn } from "../../lib/cn";

/**
 * Frosted-glass (Mica-style) Settings overlay. Slides in from the right and
 * hosts the theme switcher. Visibility is driven by the store's `settingsOpen`.
 */
const FEC_OPTIONS = [1, 2, 3] as const;

export default function SettingsOverlay() {
  const open = useAppStore((s) => s.settingsOpen);
  const toggle = useAppStore((s) => s.toggleSettings);
  const theme = useAppStore((s) => s.theme);
  const setTheme = useAppStore((s) => s.setTheme);
  const forwardBroadcast = useAppStore((s) => s.forwardBroadcast);
  const forwardMulticast = useAppStore((s) => s.forwardMulticast);
  const setForwardBroadcast = useAppStore((s) => s.setForwardBroadcast);
  const setForwardMulticast = useAppStore((s) => s.setForwardMulticast);
  const fecParityShards = useAppStore((s) => s.fecParityShards);
  const setFecParityShards = useAppStore((s) => s.setFecParityShards);

  return (
    <div
      aria-hidden={!open}
      className={cn(
        "fixed inset-0 z-50 transition-opacity duration-200",
        open ? "pointer-events-auto opacity-100" : "pointer-events-none opacity-0",
      )}
    >
      {/* Scrim */}
      <div
        className="absolute inset-0 bg-black/40"
        onClick={() => toggle(false)}
      />

      {/* Frosted-glass panel */}
      <aside
        data-testid="settings-overlay"
        className={cn(
          "absolute right-0 top-0 h-full w-[380px] border-l border-white/10 p-6 shadow-2xl",
          "bg-surface-2/70 backdrop-blur-xl backdrop-saturate-150",
          "transition-transform duration-200",
          open ? "translate-x-0" : "translate-x-full",
        )}
      >
        <div className="flex items-center justify-between">
          <h2 className="text-lg font-semibold text-ink">Settings</h2>
          <button
            type="button"
            aria-label="Close settings"
            data-testid="settings-close"
            onClick={() => toggle(false)}
            className="flex h-8 w-8 items-center justify-center rounded-lg text-ink-muted transition-colors hover:bg-white/5 hover:text-ink"
          >
            <X size={18} />
          </button>
        </div>

        <section className="mt-6">
          <h3 className="text-xs font-semibold uppercase tracking-wider text-ink-muted">
            Theme
          </h3>
          <div className="mt-3 grid grid-cols-2 gap-2">
            {THEMES.map((t) => (
              <button
                key={t.id}
                type="button"
                data-testid={`theme-${t.id}`}
                aria-pressed={theme === t.id}
                onClick={() => setTheme(t.id)}
                className={cn(
                  "flex items-center gap-2 rounded-lg border px-3 py-2 text-sm transition-colors",
                  theme === t.id
                    ? "border-brand-violet text-ink"
                    : "border-white/10 text-ink-muted hover:border-white/25 hover:text-ink",
                )}
              >
                <span
                  className="h-4 w-4 rounded-full ring-1 ring-white/20"
                  style={{ background: t.swatch }}
                />
                {t.name}
              </button>
            ))}
          </div>
        </section>

        <section className="mt-6" data-testid="settings-connection">
          <h3 className="text-xs font-semibold uppercase tracking-wider text-ink-muted">
            Connection
          </h3>
          <p className="mt-1 text-xs text-ink-muted">
            Applied the next time you press Connect — not to an already-live link.
          </p>

          <div className="mt-3 flex flex-col gap-2">
            <button
              type="button"
              data-testid="settings-forward-broadcast"
              aria-pressed={forwardBroadcast}
              onClick={() => setForwardBroadcast(!forwardBroadcast)}
              title="Broadcast traffic (e.g. LAN game discovery) into the tunnel"
              className={cn(
                "flex items-center justify-between rounded-lg border px-3 py-2 text-sm transition-colors",
                forwardBroadcast
                  ? "border-brand-violet text-ink"
                  : "border-white/10 text-ink-muted hover:border-white/25 hover:text-ink",
              )}
            >
              Forward broadcast
              <span className={forwardBroadcast ? "text-brand-violet" : "text-ink-muted"}>
                {forwardBroadcast ? "On" : "Off"}
              </span>
            </button>

            <button
              type="button"
              data-testid="settings-forward-multicast"
              aria-pressed={forwardMulticast}
              onClick={() => setForwardMulticast(!forwardMulticast)}
              title="Multicast traffic (e.g. mDNS discovery) into the tunnel"
              className={cn(
                "flex items-center justify-between rounded-lg border px-3 py-2 text-sm transition-colors",
                forwardMulticast
                  ? "border-brand-violet text-ink"
                  : "border-white/10 text-ink-muted hover:border-white/25 hover:text-ink",
              )}
            >
              Forward multicast
              <span className={forwardMulticast ? "text-brand-violet" : "text-ink-muted"}>
                {forwardMulticast ? "On" : "Off"}
              </span>
            </button>
          </div>

          <div className="mt-4">
            <div className="flex items-center justify-between">
              <span className="text-sm text-ink">FEC redundancy</span>
              <span className="text-xs text-ink-muted">r = {fecParityShards}</span>
            </div>
            <p className="mt-1 text-xs text-ink-muted">
              Recovers up to r lost packets per group of 8, at more bandwidth.
            </p>
            <div className="mt-2 grid grid-cols-3 gap-2">
              {FEC_OPTIONS.map((n) => (
                <button
                  key={n}
                  type="button"
                  data-testid={`settings-fec-${n}`}
                  aria-pressed={fecParityShards === n}
                  onClick={() => setFecParityShards(n)}
                  className={cn(
                    "rounded-lg border px-3 py-2 text-sm transition-colors",
                    fecParityShards === n
                      ? "border-brand-violet text-ink"
                      : "border-white/10 text-ink-muted hover:border-white/25 hover:text-ink",
                  )}
                >
                  {n}
                </button>
              ))}
            </div>
          </div>
        </section>
      </aside>
    </div>
  );
}
