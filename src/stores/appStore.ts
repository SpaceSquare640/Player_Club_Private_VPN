import { create } from "zustand";
import { persist, createJSONStorage } from "zustand/middleware";
import { DEFAULT_THEME, type ThemeId } from "../theme/themes";
import { DEFAULT_CONNECTION_SETTINGS } from "../types/telemetry";
import type { SupportedLanguage } from "../i18n";

export type RouteId = "dashboard" | "network" | "diagnostics";

/**
 * Best-effort default language from the OS/browser locale, used only until the
 * user picks one explicitly (which then persists and wins on every future
 * launch). No detector plugin — this is a three-line check, not a dependency.
 */
function detectDefaultLanguage(): SupportedLanguage {
  return typeof navigator !== "undefined" && navigator.language?.startsWith("zh")
    ? "zh-Hant"
    : "en";
}

interface AppState {
  /** Currently active top-level route (kept in sync with the router). */
  activeRoute: RouteId;
  /** Active visual theme. */
  theme: ThemeId;
  /** Whether the Settings overlay is open. */
  settingsOpen: boolean;
  /** UI language. Synced to i18next by `components/layout/AppShell.tsx`. */
  language: SupportedLanguage;
  /**
   * Connect-time settings (Phase B.3) — split-tunnel broadcast/multicast
   * forwarding and FEC redundancy. Read by `useConnection.onConnect` and applied
   * once per connection; changing them here does not affect an already-live link.
   */
  forwardBroadcast: boolean;
  forwardMulticast: boolean;
  /** FEC parity shards per group of 8 (1 = the historical default). */
  fecParityShards: number;

  setActiveRoute: (route: RouteId) => void;
  setTheme: (theme: ThemeId) => void;
  toggleSettings: (open?: boolean) => void;
  setLanguage: (language: SupportedLanguage) => void;
  setForwardBroadcast: (on: boolean) => void;
  setForwardMulticast: (on: boolean) => void;
  setFecParityShards: (n: number) => void;
}

/** localStorage key under which the persisted UI slice is stored. */
const STORAGE_KEY = "pcpv-app-store";

/**
 * Single source of truth for shell state. The sidebar, breadcrumb, and settings
 * overlay all read from and write to this store, keeping them synchronized.
 *
 * State is persisted to `localStorage` (synchronous, so the theme is hydrated
 * before first paint — no flash of the default palette). Only the serializable
 * UI fields are persisted via `partialize`; action functions are never written.
 * A persisted blob from before a given field existed simply lacks that key —
 * zustand's persist merge fills it from the initial state below, so no
 * migration step is needed.
 */
export const useAppStore = create<AppState>()(
  persist(
    (set) => ({
      activeRoute: "dashboard",
      theme: DEFAULT_THEME,
      settingsOpen: false,
      language: detectDefaultLanguage(),
      forwardBroadcast: DEFAULT_CONNECTION_SETTINGS.forwardBroadcast,
      forwardMulticast: DEFAULT_CONNECTION_SETTINGS.forwardMulticast,
      fecParityShards: DEFAULT_CONNECTION_SETTINGS.fecParityShards,

      setActiveRoute: (route) => set({ activeRoute: route }),
      setTheme: (theme) => set({ theme }),
      toggleSettings: (open) =>
        set((state) => ({ settingsOpen: open ?? !state.settingsOpen })),
      setLanguage: (language) => set({ language }),
      setForwardBroadcast: (on) => set({ forwardBroadcast: on }),
      setForwardMulticast: (on) => set({ forwardMulticast: on }),
      setFecParityShards: (n) => set({ fecParityShards: n }),
    }),
    {
      name: STORAGE_KEY,
      version: 1,
      storage: createJSONStorage(() => localStorage),
      partialize: (state) => ({
        activeRoute: state.activeRoute,
        theme: state.theme,
        settingsOpen: state.settingsOpen,
        language: state.language,
        forwardBroadcast: state.forwardBroadcast,
        forwardMulticast: state.forwardMulticast,
        fecParityShards: state.fecParityShards,
      }),
    },
  ),
);
