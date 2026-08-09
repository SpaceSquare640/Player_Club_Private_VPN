import { create } from "zustand";
import type { UpdateInfo } from "../lib/updateCheck";

interface UpdateStore {
  /** `null` until checked, or once checked with nothing newer found. */
  available: UpdateInfo | null;
  setAvailable: (info: UpdateInfo | null) => void;
}

/** Ephemeral, like `telemetryStore` — re-checked fresh each launch, not persisted. */
export const useUpdateStore = create<UpdateStore>((set) => ({
  available: null,
  setAvailable: (available) => set({ available }),
}));
