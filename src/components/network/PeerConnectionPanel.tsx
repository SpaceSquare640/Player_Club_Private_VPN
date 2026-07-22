import { useTelemetryStore } from "../../stores/telemetryStore";
import { useConnection } from "../../hooks/useConnection";
import { cn } from "../../lib/cn";

/**
 * Manual-signaling + peer-link management: create an offer, exchange blobs with a
 * peer, then Connect / Disconnect. The live link state and the data-plane badge
 * come from the app-wide telemetry subscription via the connection hook.
 */
export default function PeerConnectionPanel() {
  const notice = useTelemetryStore((s) => s.notice);
  const {
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
  } = useConnection();

  return (
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
  );
}
