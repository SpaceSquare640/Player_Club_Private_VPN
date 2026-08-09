import { useEffect, useRef, useState } from "react";
import { getRelayStatus, startRelay, stopRelay } from "../lib/engine";
import type { RelayHostStatus } from "../types/telemetry";

const STATUS_POLL_MS = 2000;

export interface RelayHostController {
  status: RelayHostStatus | null;
  port: string;
  error: string | null;
  busy: boolean;
  setPort: (v: string) => void;
  onStart: () => Promise<void>;
  onStop: () => Promise<void>;
}

/**
 * Owns the "host a relay from this app" page (Settings-adjacent, but its own
 * route — see `pages/RelayServer.tsx`) — starting/stopping the local
 * `RelayHost` and polling its live status, same
 * simple-polling-over-push-events tradeoff as `useVirtualNetwork`.
 *
 * This is a separate concern from `appStore.relayServerAddr` (which relay
 * *this* app's own create/join calls route through) — a machine can host a
 * relay for others without using one itself, and vice versa.
 */
export function useRelayHost(): RelayHostController {
  const [status, setStatus] = useState<RelayHostStatus | null>(null);
  const [port, setPort] = useState("9420");
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const pollRef = useRef<ReturnType<typeof setInterval> | null>(null);

  const refreshStatus = async () => {
    try {
      setStatus(await getRelayStatus());
    } catch {
      // No Tauri runtime (browser preview / tests) — leave status blank.
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

  const onStart = async () => {
    setError(null);
    setBusy(true);
    try {
      const parsedPort = Number(port);
      await startRelay(Number.isFinite(parsedPort) ? parsedPort : 0);
      await refreshStatus();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  const onStop = async () => {
    setError(null);
    setBusy(true);
    try {
      await stopRelay();
      await refreshStatus();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  return { status, port, error, busy, setPort, onStart, onStop };
}
