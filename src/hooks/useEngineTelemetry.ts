import { useEffect } from "react";
import {
  getIdentity,
  getPacketLog,
  getPrivilegeStatus,
  getSnapshot,
  getStatus,
  isEngineActive,
  onNotice,
  onPackets,
  onState,
  onStats,
} from "../lib/engine";
import { useTelemetryStore } from "../stores/telemetryStore";

/**
 * Subscribes the telemetry store to the engine's IPC event streams for the
 * lifetime of the calling component, and pulls an initial snapshot so a
 * late-mounting view paints immediately instead of waiting for the next tick.
 */
export function useEngineTelemetry(): void {
  const setState = useTelemetryStore((s) => s.setState);
  const setSnapshot = useTelemetryStore((s) => s.setSnapshot);
  const appendPackets = useTelemetryStore((s) => s.appendPackets);
  const setRunning = useTelemetryStore((s) => s.setRunning);
  const setPrivilege = useTelemetryStore((s) => s.setPrivilege);
  const setNotice = useTelemetryStore((s) => s.setNotice);
  const setIdentity = useTelemetryStore((s) => s.setIdentity);

  useEffect(() => {
    let active = true;
    const unlisteners: UnlistenLike[] = [];

    (async () => {
      try {
        const [u1, u2, u3, u4] = await Promise.all([
          onStats((s) => {
            setSnapshot(s);
            setState(s.state);
          }),
          onPackets((b) => appendPackets(b)),
          onState((st) => {
            setState(st);
            setRunning(isEngineActive(st));
            if (st === "idle") setNotice(null);
          }),
          onNotice((n) => setNotice(n)),
        ]);

        // If the component unmounted while we were awaiting, clean up now.
        if (!active) {
          u1();
          u2();
          u3();
          u4();
          return;
        }
        unlisteners.push(u1, u2, u3, u4);
      } catch {
        // Not in a Tauri context (e.g. a plain browser preview) — no events.
        return;
      }

      // Initial pull so the view isn't blank before the first pushed event.
      try {
        setIdentity(await getIdentity());
        setPrivilege(await getPrivilegeStatus());
        const status = await getStatus();
        setState(status.state);
        setRunning(isEngineActive(status.state));
        if (status.state !== "idle") {
          setSnapshot(await getSnapshot());
          appendPackets(await getPacketLog());
        }
      } catch {
        // Non-fatal — pushed events will populate shortly.
      }
    })();

    return () => {
      active = false;
      for (const u of unlisteners) u();
    };
    // Zustand setters are stable — subscribe once on mount.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);
}

type UnlistenLike = () => void;
