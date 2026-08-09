import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { createNetwork, getNetworkStatuses, joinNetwork, leaveNetwork } from "../lib/engine";
import { useAppStore } from "../stores/appStore";
import { useSavedNetworksStore, type SavedNetwork } from "../stores/savedNetworksStore";
import { generateNetworkName } from "../lib/networkName";
import { decodeInvite } from "../lib/networkInvite";
import { DEFAULT_CONNECTION_SETTINGS, type ConnectionSettings, type NetworkStatus } from "../types/telemetry";

const STATUS_POLL_MS = 2000;

export interface VirtualNetworkController {
  networks: NetworkStatus[];
  savedNetworks: SavedNetwork[];
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
  /** Recreate/rejoin a remembered network with one click, filling nothing in first. */
  onQuickStart: (saved: SavedNetwork) => Promise<void>;
  onForgetSaved: (id: string) => void;
  /** Reads the clipboard and, if it holds a valid invite (see `lib/networkInvite.ts`), fills all three join fields at once. */
  onPasteInvite: () => Promise<void>;
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
 * Every successful create/join is also remembered in `savedNetworksStore`
 * (persisted to `localStorage`, so it survives an app restart) — the live
 * `MeshSession` on the Rust side is purely in-memory and forgets everything
 * when the app closes, so this is what lets a previously created/joined
 * network be recreated/rejoined with one click (`onQuickStart`) instead of
 * retyping its name/password/address.
 *
 * If a Relay Server address is set in Settings → Connection, every
 * create/join automatically routes through it instead of binding/dialing
 * directly — this is what makes cross-internet virtual networks reachable
 * without port forwarding (see `engine::relay`'s doc comment on the Rust
 * side for why direct binding alone can't do that).
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
  const { t } = useTranslation();
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
  const savedNetworks = useSavedNetworksStore((s) => s.networks);
  const remember = useSavedNetworksStore((s) => s.remember);
  const forget = useSavedNetworksStore((s) => s.forget);
  // Settings → Connection → "Relay Server (optional)" — applied to every
  // create/join automatically when set, so cross-internet reachability
  // doesn't need per-network configuration. Empty means direct-bind/dial,
  // unchanged from before relay support existed.
  const relayServerAddr = useAppStore((s) => s.relayServerAddr);
  const relayAddr = relayServerAddr.trim() || null;

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

  const runCreate = async (bindAddr: string, networkName: string, password: string) => {
    setError(null);
    setBusy(true);
    try {
      await createNetwork(bindAddr, networkName, password, gameTag, settings, relayAddr);
      remember({ mode: "create", networkName, password, bindAddr, gameTag });
      await refreshStatus();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  const runJoin = async (hostAddr: string, networkName: string, password: string) => {
    setError(null);
    setBusy(true);
    try {
      await joinNetwork(hostAddr, networkName, password, gameTag, settings, relayAddr);
      remember({ mode: "join", networkName, password, hostAddr, gameTag });
      await refreshStatus();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  // A blank name is filled with a generated default rather than blocked —
  // the host doesn't have to think one up, mirroring the old project (where
  // there was no "network name" concept to fill in at all). The *generated*
  // name is what's actually created/shown/shared, not the empty field.
  const onCreate = () => runCreate(createBindAddr, createName.trim() || generateNetworkName(), createPassword);
  const onJoin = () => runJoin(joinHostAddr, joinName, joinPassword);

  const onPasteInvite = async () => {
    setError(null);
    try {
      const text = await navigator.clipboard.readText();
      const invite = decodeInvite(text);
      if (!invite) {
        setError(t("network.pasteInviteInvalid"));
        return;
      }
      setJoinHostAddr(invite.hostAddr);
      setJoinName(invite.networkName);
      setJoinPassword(invite.password);
    } catch (e) {
      setError(String(e));
    }
  };

  const onQuickStart = (saved: SavedNetwork) =>
    saved.mode === "create"
      ? runCreate(saved.bindAddr ?? "0.0.0.0:0", saved.networkName, saved.password)
      : runJoin(saved.hostAddr ?? "", saved.networkName, saved.password);

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
    savedNetworks,
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
    onQuickStart,
    onForgetSaved: forget,
    onPasteInvite,
  };
}
