import { create } from "zustand";
import { persist, createJSONStorage } from "zustand/middleware";
import { DEFAULT_THEME, type ThemeId } from "../theme/themes";
import { DEFAULT_CONNECTION_SETTINGS } from "../types/telemetry";
import type { SupportedLanguage } from "../i18n";

export type RouteId = "dashboard" | "network" | "diagnostics" | "minecraft" | "relayServer";

/**
 * Best-effort default language from the OS/browser locale, used only until the
 * user picks one explicitly (which then persists and wins on every future
 * launch). No detector plugin — this is a short check, not a dependency.
 * Traditional-script regions (Taiwan, Hong Kong, Macau) and an explicit
 * "Hant" subtag map to zh-Hant; every other "zh" locale (zh-CN, zh-SG, or a
 * bare "zh") maps to zh-Hans, the more common default.
 */
function detectDefaultLanguage(): SupportedLanguage {
  if (typeof navigator === "undefined" || !navigator.language?.startsWith("zh")) return "en";
  const locale = navigator.language.toLowerCase();
  const isTraditional = ["-tw", "-hk", "-mo", "-hant"].some((suffix) => locale.includes(suffix));
  return isTraditional ? "zh-Hant" : "zh-Hans";
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
   * Show advanced settings (currently: the Connection section) in the Settings
   * overlay. Purely a *display* filter — hidden settings remain in effect and
   * are still applied at the next Connect; toggling this off never resets or
   * ignores them.
   */
  expertMode: boolean;
  /**
   * Connect-time settings (Phase B.3) — split-tunnel broadcast/multicast
   * forwarding and FEC redundancy. Read by `useConnection.onConnect` and applied
   * once per connection; changing them here does not affect an already-live link.
   */
  forwardBroadcast: boolean;
  forwardMulticast: boolean;
  /** FEC parity shards per group of 8 (1 = the historical default). */
  fecParityShards: number;
  /** Additional "address/prefix" networks routed into the tunnel (Phase E.2). */
  extraRoutes: string[];
  /**
   * `ip:port` of a relay server (see `engine::relay`), or `""` for none.
   * Empty (the default) preserves direct-bind/direct-dial virtual-network
   * behavior exactly as before — only reachable on the same LAN, or a
   * manually port-forwarded address. Non-empty routes every
   * `createNetwork`/`joinNetwork` call through that relay instead, making
   * virtual networks reachable across the internet without any port
   * forwarding of the user's own. One setting, applied automatically to
   * every create/join — not a per-network choice.
   */
  relayServerAddr: string;

  setActiveRoute: (route: RouteId) => void;
  setTheme: (theme: ThemeId) => void;
  toggleSettings: (open?: boolean) => void;
  setLanguage: (language: SupportedLanguage) => void;
  setExpertMode: (on: boolean) => void;
  setForwardBroadcast: (on: boolean) => void;
  setForwardMulticast: (on: boolean) => void;
  setFecParityShards: (n: number) => void;
  setExtraRoutes: (routes: string[]) => void;
  setRelayServerAddr: (addr: string) => void;
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
      expertMode: false,
      forwardBroadcast: DEFAULT_CONNECTION_SETTINGS.forwardBroadcast,
      forwardMulticast: DEFAULT_CONNECTION_SETTINGS.forwardMulticast,
      fecParityShards: DEFAULT_CONNECTION_SETTINGS.fecParityShards,
      extraRoutes: DEFAULT_CONNECTION_SETTINGS.extraRoutes,
      relayServerAddr: "",

      setActiveRoute: (route) => set({ activeRoute: route }),
      setTheme: (theme) => set({ theme }),
      toggleSettings: (open) =>
        set((state) => ({ settingsOpen: open ?? !state.settingsOpen })),
      setLanguage: (language) => set({ language }),
      setExpertMode: (on) => set({ expertMode: on }),
      setForwardBroadcast: (on) => set({ forwardBroadcast: on }),
      setForwardMulticast: (on) => set({ forwardMulticast: on }),
      setFecParityShards: (n) => set({ fecParityShards: n }),
      setExtraRoutes: (routes) => set({ extraRoutes: routes }),
      setRelayServerAddr: (addr) => set({ relayServerAddr: addr }),
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
        expertMode: state.expertMode,
        forwardBroadcast: state.forwardBroadcast,
        forwardMulticast: state.forwardMulticast,
        fecParityShards: state.fecParityShards,
        extraRoutes: state.extraRoutes,
        relayServerAddr: state.relayServerAddr,
      }),
    },
  ),
);
