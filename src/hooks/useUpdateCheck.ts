import { useEffect } from "react";
import { checkForUpdate } from "../lib/updateCheck";
import { useUpdateStore } from "../stores/updateStore";

/**
 * Runs once per app launch: check-and-notify only, no auto-download/install.
 * Best-effort — a failed check (offline, GitHub API hiccup, no Tauri context
 * in a browser preview) is silently ignored, same pattern as the rest of the
 * app's non-critical background checks. Never blocks or delays the UI.
 */
export function useUpdateCheck(): void {
  const setAvailable = useUpdateStore((s) => s.setAvailable);

  useEffect(() => {
    let active = true;
    checkForUpdate()
      .then((info) => {
        if (active) setAvailable(info);
      })
      .catch(() => {
        // Best-effort — see doc comment above.
      });
    return () => {
      active = false;
    };
    // Runs once on mount, like the app-wide telemetry subscription.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);
}
