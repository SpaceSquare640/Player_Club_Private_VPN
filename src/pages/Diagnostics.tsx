import { useEffect, useState } from "react";
import { useTelemetryStore } from "../stores/telemetryStore";
import { useEngineTelemetry } from "../hooks/useEngineTelemetry";
import {
  acceptAnswer,
  acceptOffer,
  configForMode,
  connectPeer,
  createOffer,
  disconnectPeer,
  getConnection,
  requestElevation,
  startEngine,
  stopEngine,
} from "../lib/engine";
import { cn } from "../lib/cn";
import type { ConnectionInfo, EngineMode, EngineState } from "../types/telemetry";

const MODE_LABELS: Record<EngineMode, string> = {
  simulated: "Simulated",
  probe: "Transport probe",
  real: "Real adapter",
};

/** Semantic styling per lifecycle state (Cyan idle, Amber transient, …). */
const STATE_STYLES: Record<EngineState, { label: string; dot: string; text: string }> = {
  idle: { label: "Idle", dot: "bg-brand-cyan", text: "text-brand-cyan" },
  connecting: {
    label: "Connecting",
    dot: "bg-brand-amber animate-pulse",
    text: "text-brand-amber",
  },
  starting: {
    label: "Starting",
    dot: "bg-brand-amber animate-pulse",
    text: "text-brand-amber",
  },
  connected: { label: "Connected", dot: "bg-brand-violet", text: "text-brand-violet" },
  "needs-elevation": {
    label: "Needs Admin",
    dot: "bg-brand-amber",
    text: "text-brand-amber",
  },
  error: { label: "Error", dot: "bg-brand-red", text: "text-brand-red" },
};

function StatTile({
  label,
  value,
  unit,
  testid,
}: {
  label: string;
  value: string;
  unit: string;
  testid: string;
}) {
  return (
    <div
      data-testid={testid}
      className="rounded-xl border border-white/10 bg-surface-2/60 p-4"
    >
      <div className="text-xs uppercase tracking-wider text-ink-muted">{label}</div>
      <div className="mt-1 flex items-baseline gap-1">
        <span className="text-2xl font-semibold tabular-nums text-ink">{value}</span>
        <span className="text-xs text-ink-muted">{unit}</span>
      </div>
    </div>
  );
}

/**
 * Diagnostics — live readout of the engine telemetry. Phase B adds an Expert
 * "real adapter" toggle (Wintun) with graceful elevation handling; the default
 * remains the simulated path. Topology map and spectrum monitor arrive with the
 * dedicated Diagnostics module.
 */
export default function Diagnostics() {
  useEngineTelemetry();
  const running = useTelemetryStore((s) => s.running);
  const state = useTelemetryStore((s) => s.state);
  const snapshot = useTelemetryStore((s) => s.snapshot);
  const packets = useTelemetryStore((s) => s.packets);
  const notice = useTelemetryStore((s) => s.notice);
  const identity = useTelemetryStore((s) => s.identity);

  const [mode, setMode] = useState<EngineMode>("simulated");

  // Manual-signaling panel state.
  const [offerBlob, setOfferBlob] = useState("");
  const [peerInput, setPeerInput] = useState("");
  const [answerBlob, setAnswerBlob] = useState("");
  const [conn, setConn] = useState<ConnectionInfo | null>(null);
  const [connError, setConnError] = useState<string | null>(null);

  const refreshConn = async () => {
    try {
      setConn(await getConnection());
    } catch {
      // No Tauri runtime (browser preview) — leave status blank.
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
      await connectPeer();
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

  const sm = STATE_STYLES[state];
  const fmt = (n: number | undefined, d = 1) => (n == null ? "—" : n.toFixed(d));

  // Peer-link controls: connect only with a negotiated peer, an idle/failed link,
  // and no telemetry-source session running (they share the event channels).
  const link = conn?.link ?? "idle";
  const canConnect = !!conn?.peer && !running && (link === "idle" || link === "failed");

  // Swallow expected rejections (no Tauri context in a browser preview, or an
  // AlreadyRunning/NotRunning race) so they don't surface as unhandled.
  const start = () => void startEngine(configForMode(mode)).catch(() => {});
  const stop = () => void stopEngine().catch(() => {});
  const relaunch = () => void requestElevation().catch(() => {});

  const needsElevation = state === "needs-elevation";
  // Informational notices (e.g. discovered candidates) shown as a readout when
  // not surfaced by the elevation/error banners.
  const infoNotice =
    notice && state !== "needs-elevation" && state !== "error" ? notice : null;

  return (
    <section data-testid="page-diagnostics" className="flex h-full flex-col gap-5">
      <header className="flex items-center justify-between">
        <div>
          <h1 className="text-xl font-semibold text-ink">Diagnostics</h1>
          <p className="text-sm text-ink-muted">
            Live engine telemetry — {MODE_LABELS[mode].toLowerCase()}.
          </p>
        </div>
        <div className="flex items-center gap-3">
          <span className="flex items-center gap-2 text-sm" data-testid="engine-state">
            <span className={cn("h-2.5 w-2.5 rounded-full", sm.dot)} />
            <span className={sm.text}>{sm.label}</span>
          </span>
          {!running && (
            <select
              data-testid="mode-select"
              value={mode}
              onChange={(e) => setMode(e.target.value as EngineMode)}
              title="Telemetry source"
              className="rounded-lg border border-white/15 bg-surface-2 px-2 py-1.5 text-xs text-ink-muted"
            >
              <option value="simulated">{MODE_LABELS.simulated}</option>
              <option value="probe">{MODE_LABELS.probe}</option>
              <option value="real">{MODE_LABELS.real} (Admin)</option>
            </select>
          )}
          {running ? (
            <button
              type="button"
              data-testid="engine-stop"
              onClick={stop}
              className="rounded-lg border border-brand-red/40 px-3 py-1.5 text-sm text-brand-red transition-colors hover:bg-brand-red/10"
            >
              Stop
            </button>
          ) : (
            <button
              type="button"
              data-testid="engine-start"
              onClick={start}
              className="rounded-lg border border-brand-violet/40 px-3 py-1.5 text-sm text-brand-violet transition-colors hover:bg-brand-violet/10"
            >
              Start
            </button>
          )}
        </div>
      </header>

      {identity && (
        <div
          data-testid="node-identity"
          className="flex items-center gap-3 rounded-lg border border-white/10 bg-surface-2/40 px-3 py-2 text-xs"
        >
          <span className="text-ink-muted">This node · fingerprint</span>
          <span
            data-testid="peer-address"
            title="A short fingerprint for voice/chat verification. To connect, share your full public key (Copy public key) — that is the authoritative identifier."
            className="font-mono text-brand-cyan"
          >
            {identity.peerAddress}
          </span>
          <button
            type="button"
            data-testid="copy-pubkey"
            title="Copy the full public key — the authoritative identifier to share with a peer"
            onClick={() => void navigator.clipboard?.writeText(identity.publicKeyB64)}
            className="ml-auto rounded border border-white/15 px-2 py-1 text-ink-muted transition-colors hover:text-ink"
          >
            Copy public key
          </button>
        </div>
      )}

      {needsElevation && (
        <div
          data-testid="elevation-banner"
          className="flex items-center justify-between gap-4 rounded-xl border border-brand-amber/40 bg-brand-amber/10 px-4 py-3"
        >
          <p className="text-sm text-brand-amber">
            {notice?.message ??
              "Administrator privileges are required to create the virtual network adapter."}
          </p>
          <button
            type="button"
            data-testid="relaunch-admin"
            onClick={relaunch}
            className="shrink-0 rounded-lg border border-brand-amber/50 px-3 py-1.5 text-sm text-brand-amber transition-colors hover:bg-brand-amber/15"
          >
            Relaunch as Administrator
          </button>
        </div>
      )}

      {state === "error" && notice && (
        <div
          data-testid="error-banner"
          className="rounded-xl border border-brand-red/40 bg-brand-red/10 px-4 py-3 text-sm text-brand-red"
        >
          {notice.message}
        </div>
      )}

      {infoNotice && (
        <div
          data-testid="info-notice"
          className="rounded-lg border border-white/10 bg-surface-2/40 px-3 py-2 font-mono text-xs text-ink-muted"
        >
          {infoNotice.message}
        </div>
      )}

      <div className="grid grid-cols-2 gap-3 sm:grid-cols-4" data-testid="stat-grid">
        <StatTile testid="stat-rtt" label="Ping (RTT)" value={fmt(snapshot?.rttMs)} unit="ms" />
        <StatTile testid="stat-jitter" label="Jitter" value={fmt(snapshot?.jitterMs)} unit="ms" />
        <StatTile testid="stat-loss" label="Loss" value={fmt(snapshot?.lossPct, 2)} unit="%" />
        <StatTile
          testid="stat-throughput"
          label="Throughput ↑/↓"
          value={`${fmt(snapshot?.txKbps, 0)}/${fmt(snapshot?.rxKbps, 0)}`}
          unit="kbps"
        />
      </div>

      <div className="flex min-h-0 flex-1 flex-col rounded-xl border border-white/10 bg-black/40">
        <div className="flex items-center justify-between border-b border-white/10 px-4 py-2">
          <span className="text-xs font-semibold uppercase tracking-wider text-ink-muted">
            Packet Log
          </span>
          <span className="text-xs tabular-nums text-ink-muted">
            {packets.length} entries
          </span>
        </div>
        <div
          data-testid="packet-log"
          className="min-h-0 flex-1 overflow-auto p-3 font-mono text-xs leading-relaxed"
        >
          {packets.length === 0 ? (
            <div className="text-ink-muted">
              No packets yet. Press Start to begin a session.
            </div>
          ) : (
            packets.map((p, i) => (
              <div key={`${p.tMs}-${i}`} className="flex gap-3 whitespace-nowrap">
                <span className="tabular-nums text-ink-muted">
                  {(p.tMs / 1000).toFixed(2)}s
                </span>
                <span className={p.dir === "tx" ? "text-brand-violet" : "text-brand-cyan"}>
                  {p.dir.toUpperCase()}
                </span>
                <span className="text-ink">{p.proto}</span>
                <span className="tabular-nums text-ink-muted">{p.len}B</span>
                <span className="text-ink-muted">{p.note}</span>
              </div>
            ))
          )}
        </div>
      </div>

      <div
        data-testid="peer-connection"
        className="rounded-xl border border-white/10 bg-surface-2/40 p-4 text-xs"
      >
        <div className="flex items-center justify-between gap-3">
          <span className="text-xs font-semibold uppercase tracking-wider text-ink-muted">
            Peer Connection · manual signaling
          </span>
          <div className="flex items-center gap-3">
            <span data-testid="conn-status" className="text-ink-muted">
              {conn
                ? `link: ${conn.link} · role: ${conn.role}${
                    conn.peer
                      ? ` · peer ${conn.peer.peerAddress} (${conn.peer.candidateCount} cand)`
                      : ""
                  }`
                : "—"}
            </span>
            {link === "connected" &&
              (notice?.code === "data_plane" || notice?.code === "data_plane_off") && (
                <span
                  data-testid="dataplane-badge"
                  title={notice.message}
                  className={cn(
                    "rounded px-1.5 py-0.5 text-[10px] font-medium",
                    notice.code === "data_plane"
                      ? "bg-brand-violet/15 text-brand-violet"
                      : "bg-brand-amber/15 text-brand-amber",
                  )}
                >
                  {notice.code === "data_plane" ? "data plane" : "control-only"}
                </span>
              )}
            {link === "connected" ? (
              <button
                type="button"
                data-testid="disconnect-btn"
                onClick={onDisconnect}
                className="rounded-lg border border-brand-red/40 px-3 py-1.5 text-brand-red transition-colors hover:bg-brand-red/10"
              >
                Disconnect
              </button>
            ) : link === "connecting" ? (
              <button
                type="button"
                data-testid="connecting-btn"
                onClick={onDisconnect}
                title="Handshake in progress (≤8s). Click to cancel."
                className="flex items-center gap-2 rounded-lg border border-brand-amber/40 px-3 py-1.5 text-brand-amber transition-colors hover:bg-brand-amber/10"
              >
                <span className="h-2 w-2 animate-pulse rounded-full bg-brand-amber" />
                Connecting…
              </button>
            ) : (
              <button
                type="button"
                data-testid="connect-btn"
                onClick={onConnect}
                disabled={!canConnect}
                title={
                  !conn?.peer
                    ? "Exchange an offer/answer with a peer first"
                    : running
                      ? "Stop the telemetry-source session before connecting to a peer"
                      : link === "failed"
                        ? "Previous handshake failed — retry"
                        : "Punch through to the peer and establish the encrypted link"
                }
                className={cn(
                  "rounded-lg border px-3 py-1.5 transition-colors",
                  canConnect
                    ? "border-brand-violet/40 text-brand-violet hover:bg-brand-violet/10"
                    : "cursor-not-allowed border-white/10 text-ink-muted opacity-50",
                )}
              >
                {link === "failed" ? "Retry connect" : "Connect"}
              </button>
            )}
          </div>
        </div>

        <div className="mt-3 flex flex-wrap items-start gap-4">
          <div className="min-w-[240px] flex-1">
            <button
              type="button"
              data-testid="create-offer-btn"
              onClick={onCreateOffer}
              className="rounded-lg border border-brand-violet/40 px-3 py-1.5 text-brand-violet transition-colors hover:bg-brand-violet/10"
            >
              Create offer
            </button>
            {offerBlob && (
              <div className="mt-2">
                <textarea
                  data-testid="offer-blob"
                  readOnly
                  value={offerBlob}
                  className="h-16 w-full resize-none rounded bg-black/40 p-2 font-mono text-[11px] text-ink"
                />
                <button
                  type="button"
                  onClick={() => void navigator.clipboard?.writeText(offerBlob)}
                  className="mt-1 rounded border border-white/15 px-2 py-1 text-ink-muted transition-colors hover:text-ink"
                >
                  Copy offer
                </button>
              </div>
            )}
          </div>

          <div className="min-w-[240px] flex-1">
            <textarea
              data-testid="peer-input"
              value={peerInput}
              onChange={(e) => setPeerInput(e.target.value)}
              placeholder="Paste your peer's offer or answer blob…"
              className="h-16 w-full resize-none rounded bg-black/40 p-2 font-mono text-[11px] text-ink placeholder:text-ink-muted"
            />
            <button
              type="button"
              data-testid="process-btn"
              onClick={onProcessPeerBlob}
              className="mt-1 rounded-lg border border-brand-cyan/40 px-3 py-1.5 text-brand-cyan transition-colors hover:bg-brand-cyan/10"
            >
              Process
            </button>
            {answerBlob && (
              <div className="mt-2">
                <div className="text-ink-muted">Send this answer back to your peer:</div>
                <textarea
                  data-testid="answer-blob"
                  readOnly
                  value={answerBlob}
                  className="mt-1 h-16 w-full resize-none rounded bg-black/40 p-2 font-mono text-[11px] text-ink"
                />
                <button
                  type="button"
                  onClick={() => void navigator.clipboard?.writeText(answerBlob)}
                  className="mt-1 rounded border border-white/15 px-2 py-1 text-ink-muted transition-colors hover:text-ink"
                >
                  Copy answer
                </button>
              </div>
            )}
          </div>
        </div>

        {connError && (
          <div data-testid="conn-error" className="mt-2 text-brand-red">
            {connError}
          </div>
        )}
      </div>
    </section>
  );
}
