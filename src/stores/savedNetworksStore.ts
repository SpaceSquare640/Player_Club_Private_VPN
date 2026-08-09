import { create } from "zustand";
import { persist, createJSONStorage } from "zustand/middleware";

/**
 * A remembered create/join form, so a network you've hosted or joined before
 * can be recreated/rejoined with one click after the app restarts — the
 * in-memory `MeshSession` on the Rust side holds no state across a relaunch,
 * so nothing here reconnects automatically; it only refills and resubmits
 * the same form the user filled in originally.
 */
export interface SavedNetwork {
  /** Stable local id (independent of any live `NetworkId` — this outlives any particular session). */
  id: string;
  mode: "create" | "join";
  networkName: string;
  password: string;
  /** Bind address, `mode: "create"` only. */
  bindAddr?: string;
  /** Host address, `mode: "join"` only. */
  hostAddr?: string;
  gameTag: string | null;
  /** `Date.now()` of the most recent successful create/join with these details — newest first in the UI. */
  savedAt: number;
}

interface SavedNetworksState {
  networks: SavedNetwork[];
  /** Insert, or update in place (by `mode` + `networkName` + address) and bump `savedAt`. */
  remember: (entry: Omit<SavedNetwork, "id" | "savedAt">) => void;
  forget: (id: string) => void;
}

const STORAGE_KEY = "pcpv-saved-networks";

function matches(a: SavedNetwork, b: Omit<SavedNetwork, "id" | "savedAt">): boolean {
  if (a.mode !== b.mode || a.networkName !== b.networkName) return false;
  return a.mode === "create" ? a.bindAddr === b.bindAddr : a.hostAddr === b.hostAddr;
}

export const useSavedNetworksStore = create<SavedNetworksState>()(
  persist(
    (set) => ({
      networks: [],
      remember: (entry) =>
        set((state) => {
          const existing = state.networks.find((n) => matches(n, entry));
          const saved: SavedNetwork = { ...entry, id: existing?.id ?? crypto.randomUUID(), savedAt: Date.now() };
          const rest = state.networks.filter((n) => n.id !== saved.id);
          return { networks: [saved, ...rest] };
        }),
      forget: (id) => set((state) => ({ networks: state.networks.filter((n) => n.id !== id) })),
    }),
    {
      name: STORAGE_KEY,
      version: 1,
      storage: createJSONStorage(() => localStorage),
    },
  ),
);
