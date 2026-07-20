import { create } from "zustand";
import { persist, createJSONStorage } from "zustand/middleware";
import { DEFAULT_THEME, type ThemeId } from "../theme/themes";

export type RouteId = "dashboard" | "network" | "diagnostics";

interface AppState {
  /** Currently active top-level route (kept in sync with the router). */
  activeRoute: RouteId;
  /** Active visual theme. */
  theme: ThemeId;
  /** Whether the Settings overlay is open. */
  settingsOpen: boolean;

  setActiveRoute: (route: RouteId) => void;
  setTheme: (theme: ThemeId) => void;
  toggleSettings: (open?: boolean) => void;
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
 */
export const useAppStore = create<AppState>()(
  persist(
    (set) => ({
      activeRoute: "dashboard",
      theme: DEFAULT_THEME,
      settingsOpen: false,

      setActiveRoute: (route) => set({ activeRoute: route }),
      setTheme: (theme) => set({ theme }),
      toggleSettings: (open) =>
        set((state) => ({ settingsOpen: open ?? !state.settingsOpen })),
    }),
    {
      name: STORAGE_KEY,
      version: 1,
      storage: createJSONStorage(() => localStorage),
      partialize: (state) => ({
        activeRoute: state.activeRoute,
        theme: state.theme,
        settingsOpen: state.settingsOpen,
      }),
    },
  ),
);
