import { useEffect, useState } from "react";
import { useTelemetryStore } from "../stores/telemetryStore";
import { useAppStore } from "../stores/appStore";
import {
  acceptAnswer,
  acceptOffer,
  connectPeer,
  createOffer,
  disconnectPeer,
  getConnection,
} from "../lib/engine";
import type { ConnectionInfo, ConnectionSettings, LinkState } from "../types/telemetry";

export interface ConnectionController {
  conn: ConnectionInfo | null;
  offerBlob: string;
  peerInput: string;
  answerBlob: string;
  connError: string | null;
  /** Live-link state (`idle` until the first `getConnection`). */
  link: LinkState;
  /** True only with a negotiated peer, an idle/failed link, and no telemetry session. */
  canConnect: boolean;
  /** A telemetry-source session (simulator/probe/real) is running. */
  running: boolean;
  setPeerInput: (v: string) => void;
  onConnect: () => Promise<void>;
  onDisconnect: () => Promise<void>;
  onCreateOffer: () => Promise<void>;
  onProcessPeerBlob: () => Promise<void>;
}

/**
 * Owns the manual-signaling + peer-link state and actions, shared by the Network
 * page's connection panel. The live link state (`connecting → connected/error`)
 * is driven by the app-wide telemetry subscription (see `useEngineTelemetry`),
 * so this hook re-reads `getConnection` on every lifecycle transition.
 */
export function useConnection(): ConnectionController {
  const running = useTelemetryStore((s) => s.running);
  const state = useTelemetryStore((s) => s.state);

  const [offerBlob, setOfferBlob] = useState("");
  const [peerInput, setPeerInput] = useState("");
  const [answerBlob, setAnswerBlob] = useState("");
  const [conn, setConn] = useState<ConnectionInfo | null>(null);
  const [connError, setConnError] = useState<string | null>(null);

  const refreshConn = async () => {
    try {
      setConn(await getConnection());
    } catch {
      // No Tauri runtime (browser preview / tests) — leave status blank.
    }
  };

  // Refresh on mount and on every lifecycle transition — the peer link drives
  // `state` through connecting → connected/error as the handshake progresses.
  useEffect(() => {
    void refreshConn();
  }, [state]);

  const onConnect = async () => {
    setConnError(null);
    try {
      // Read fresh at connect time (not subscribed) — these apply once, to this
      // connection, not retroactively, so there is nothing to react to mid-link.
      const { forwardBroadcast, forwardMulticast, fecParityShards } = useAppStore.getState();
      const settings: ConnectionSettings = { forwardBroadcast, forwardMulticast, fecParityShards };
      await connectPeer(settings);
      await refreshConn();
    } catch (e) {
      setConnError(String(e));
    }
  };

  const onDisconnect = async () => {
    setConnError(null);
    try {
      await disconnectPeer();
      await refreshConn();
    } catch (e) {
      setConnError(String(e));
    }
  };

  const onCreateOffer = async () => {
    setConnError(null);
    try {
      setOfferBlob(await createOffer());
      await refreshConn();
    } catch (e) {
      setConnError(String(e));
    }
  };

  const onProcessPeerBlob = async () => {
    setConnError(null);
    const blob = peerInput.trim();
    try {
      if (blob.includes(".OFFER.")) {
        setAnswerBlob(await acceptOffer(blob));
      } else if (blob.includes(".ANSWER.")) {
        await acceptAnswer(blob);
        setAnswerBlob("");
      } else {
        throw new Error("Unrecognized blob (expected a PCPV1 offer or answer)");
      }
      await refreshConn();
    } catch (e) {
      setConnError(String(e));
    }
  };

  const link: LinkState = conn?.link ?? "idle";
  const canConnect = !!conn?.peer && !running && (link === "idle" || link === "failed");

  return {
    conn,
    offerBlob,
    peerInput,
    answerBlob,
    connError,
    link,
    canConnect,
    running,
    setPeerInput,
    onConnect,
    onDisconnect,
    onCreateOffer,
    onProcessPeerBlob,
  };
}
