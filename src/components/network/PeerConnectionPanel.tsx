import { useTranslation } from "react-i18next";
import { useTelemetryStore } from "../../stores/telemetryStore";
import { useConnection } from "../../hooks/useConnection";
import { cn } from "../../lib/cn";

/**
 * Manual-signaling + peer-link management: create an offer, exchange blobs with a
 * peer, then Connect / Disconnect. The live link state and the data-plane badge
 * come from the app-wide telemetry subscription via the connection hook.
 *
 * `conn.link` / `conn.role` are the raw engine state-machine identifiers (e.g.
 * "connected", "initiator") — left untranslated, like the engine notices, since
 * they are technical identifiers rather than prose.
 */
export default function PeerConnectionPanel() {
  const { t } = useTranslation();
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

  const connStatus = conn
    ? conn.peer
      ? t("peerConnection.statusWithPeer", {
          link: conn.link,
          role: conn.role,
          peer: conn.peer.peerAddress,
          count: conn.peer.candidateCount,
        })
      : t("peerConnection.statusNoPeer", { link: conn.link, role: conn.role })
    : t("peerConnection.statusEmpty");

  const connectTitle = !conn?.peer
    ? t("peerConnection.connectTitleNoPeer")
    : running
      ? t("peerConnection.connectTitleRunning")
      : link === "failed"
        ? t("peerConnection.connectTitleFailed")
        : t("peerConnection.connectTitleReady");

  return (
    <div
      data-testid="peer-connection"
      className="rounded-xl border border-white/10 bg-surface-2/40 p-4 text-xs"
    >
      <div className="flex items-center justify-between gap-3">
        <span className="text-xs font-semibold uppercase tracking-wider text-ink-muted">
          {t("peerConnection.heading")}
        </span>
        <div className="flex items-center gap-3">
          <span data-testid="conn-status" className="text-ink-muted">
            {connStatus}
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
                {notice.code === "data_plane"
                  ? t("peerConnection.dataPlaneBadge")
                  : t("peerConnection.controlOnlyBadge")}
              </span>
            )}
          {link === "connected" ? (
            <button
              type="button"
              data-testid="disconnect-btn"
              onClick={onDisconnect}
              className="rounded-lg border border-brand-red/40 px-3 py-1.5 text-brand-red transition-colors hover:bg-brand-red/10"
            >
              {t("peerConnection.disconnect")}
            </button>
          ) : link === "connecting" ? (
            <button
              type="button"
              data-testid="connecting-btn"
              onClick={onDisconnect}
              title={t("peerConnection.connectingTitle")}
              className="flex items-center gap-2 rounded-lg border border-brand-amber/40 px-3 py-1.5 text-brand-amber transition-colors hover:bg-brand-amber/10"
            >
              <span className="h-2 w-2 animate-pulse rounded-full bg-brand-amber" />
              {t("peerConnection.connecting")}
            </button>
          ) : (
            <button
              type="button"
              data-testid="connect-btn"
              onClick={onConnect}
              disabled={!canConnect}
              title={connectTitle}
              className={cn(
                "rounded-lg border px-3 py-1.5 transition-colors",
                canConnect
                  ? "border-brand-violet/40 text-brand-violet hover:bg-brand-violet/10"
                  : "cursor-not-allowed border-white/10 text-ink-muted opacity-50",
              )}
            >
              {link === "failed" ? t("peerConnection.retryConnect") : t("peerConnection.connect")}
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
            {t("peerConnection.createOffer")}
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
                {t("peerConnection.copyOffer")}
              </button>
            </div>
          )}
        </div>

        <div className="min-w-[240px] flex-1">
          <textarea
            data-testid="peer-input"
            value={peerInput}
            onChange={(e) => setPeerInput(e.target.value)}
            placeholder={t("peerConnection.peerInputPlaceholder")}
            className="h-16 w-full resize-none rounded bg-black/40 p-2 font-mono text-[11px] text-ink placeholder:text-ink-muted"
          />
          <button
            type="button"
            data-testid="process-btn"
            onClick={onProcessPeerBlob}
            className="mt-1 rounded-lg border border-brand-cyan/40 px-3 py-1.5 text-brand-cyan transition-colors hover:bg-brand-cyan/10"
          >
            {t("peerConnection.process")}
          </button>
          {answerBlob && (
            <div className="mt-2">
              <div className="text-ink-muted">{t("peerConnection.sendAnswerBack")}</div>
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
                {t("peerConnection.copyAnswer")}
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
