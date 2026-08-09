import { useEffect, useRef, useState } from "react";
import { createNetwork, getNetworkStatuses, joinNetwork, leaveNetwork } from "../lib/engine";
import { DEFAULT_CONNECTION_SETTINGS, type ConnectionSettings, type NetworkStatus } from "../types/telemetry";

const STATUS_POLL_MS = 2000;

export interface VirtualNetworkController {
  networks: NetworkStatus[];
  createName: string;
  createPassword: string;
  createBindAddr: string;
  joinHostAddr: string;
  joinName: string;
  joinPassword: string;
  error: string | null;
  busy: boolean;
  setCreateName: (v: string) => void;
  setCreatePassword: (v: string) => void;
  setCreateBindAddr: (v: string) => void;
  setJoinHostAddr: (v: string) => void;
  setJoinName: (v: string) => void;
  setJoinPassword: (v: string) => void;
  onCreate: () => Promise<void>;
  onJoin: () => Promise<void>;
  onLeave: (networkId: string) => Promise<void>;
}

/**
 * Owns the virtual-network (Phase G.1–G.4+) create/join/leave forms and the
 * live status readout for every currently active network — a session can
 * host and/or join several networks at once. No push event exists for
 * roster/link changes yet (unlike the manual-signaling flow's
 * `engine://state`), so this polls `get_network_statuses` on a short
 * interval while mounted — the same accepted tradeoff as the Basic/Expert
 * Settings display filter: simple and correct, not the most efficient
 * possible.
 *
 * `gameTag` (display metadata, e.g. `"minecraft"`) and `settings` (applied
 * to every auto-connected peer) are fixed for the lifetime of this hook
 * instance — the Network page's general panel omits both (`null` /
 * defaults), while the Minecraft page's panel fixes them so its "create"
 * button needs no separate settings step. Both panel instances read/write
 * the same underlying `MeshSession`, so networks created via one instance
 * show up in the other's `networks` list too.
 */
export function useVirtualNetwork(
  gameTag: string | null = null,
  settings: ConnectionSettings = DEFAULT_CONNECTION_SETTINGS,
): VirtualNetworkController {
  const [networks, setNetworks] = useState<NetworkStatus[]>([]);
  const [createName, setCreateName] = useState("");
  const [createPassword, setCreatePassword] = useState("");
  const [createBindAddr, setCreateBindAddr] = useState("0.0.0.0:0");
  const [joinHostAddr, setJoinHostAddr] = useState("");
  const [joinName, setJoinName] = useState("");
  const [joinPassword, setJoinPassword] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const pollRef = useRef<ReturnType<typeof setInterval> | null>(null);

  const refreshStatus = async () => {
    try {
      setNetworks((await getNetworkStatuses()) ?? []);
    } catch {
      // No Tauri runtime (browser preview / tests) — leave the list empty.
    }
  };

  useEffect(() => {
    void refreshStatus();
    pollRef.current = setInterval(() => void refreshStatus(), STATUS_POLL_MS);
    return () => {
      if (pollRef.current) clearInterval(pollRef.current);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const onCreate = async () => {
    setError(null);
    setBusy(true);
    try {
      await createNetwork(createBindAddr, createName, createPassword, gameTag, settings);
      await refreshStatus();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  const onJoin = async () => {
    setError(null);
    setBusy(true);
    try {
      await joinNetwork(joinHostAddr, joinName, joinPassword, gameTag, settings);
      await refreshStatus();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  const onLeave = async (networkId: string) => {
    setError(null);
    setBusy(true);
    try {
      await leaveNetwork(networkId);
      await refreshStatus();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  return {
    networks,
    createName,
    createPassword,
    createBindAddr,
    joinHostAddr,
    joinName,
    joinPassword,
    error,
    busy,
    setCreateName,
    setCreatePassword,
    setCreateBindAddr,
    setJoinHostAddr,
    setJoinName,
    setJoinPassword,
    onCreate,
    onJoin,
    onLeave,
  };
}
