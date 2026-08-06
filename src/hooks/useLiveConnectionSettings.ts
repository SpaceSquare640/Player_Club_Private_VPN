import { useEffect } from "react";
import { useAppStore } from "../stores/appStore";
import { updateConnectionSettings } from "../lib/engine";

/**
 * Pushes connection-settings changes into any already-running link (Phase
 * B.4 — live toggles).
 *
 * Keyed on the settings *values* rather than wired into the Settings
 * overlay's toggle handlers, because those are not the only writers: the
 * Minecraft page's preset button and JSON profile import both set the same
 * store fields, and both should reach a live link too. Watching the values
 * catches every writer; watching the buttons would have quietly missed two.
 *
 * The engine ignores whatever it cannot apply mid-session — FEC geometry is
 * a wire-format agreement with the peer, and extra routed networks need
 * elevation to change OS routes — so the whole object is sent and the
 * engine decides. That keeps this from drifting out of sync with
 * `ConnectionSettings` as it grows.
 *
 * Mounted once, app-wide (see `components/layout/AppShell.tsx`), alongside
 * the telemetry subscription and i18n sync for the same reason: it must
 * outlive whichever page happens to be showing.
 */
export function useLiveConnectionSettings(): void {
  const forwardBroadcast = useAppStore((s) => s.forwardBroadcast);
  const forwardMulticast = useAppStore((s) => s.forwardMulticast);
  const fecParityShards = useAppStore((s) => s.fecParityShards);
  const extraRoutes = useAppStore((s) => s.extraRoutes);

  useEffect(() => {
    // Also runs on mount, which is a no-op engine-side when nothing is
    // connected — cheaper than tracking "is this the first run" just to
    // skip a call that does nothing.
    void updateConnectionSettings({
      forwardBroadcast,
      forwardMulticast,
      fecParityShards,
      extraRoutes,
    }).catch(() => {
      // No Tauri runtime (browser preview / tests), or no live link —
      // both expected, and nothing here can act on either.
    });
  }, [forwardBroadcast, forwardMulticast, fecParityShards, extraRoutes]);
}
