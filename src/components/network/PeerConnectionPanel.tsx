import { useTranslation } from "react-i18next";
import { useTelemetryStore } from "../../stores/telemetryStore";
import { useConnection } from "../../hooks/useConnection";
import Card from "../ui/Card";
import Button from "../ui/Button";
import Badge from "../ui/Badge";

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
    <Card variant="raised" className="p-4 text-xs" data-testid="peer-connection">
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
              <Badge
                tone={notice.code === "data_plane" ? "violet" : "amber"}
                data-testid="dataplane-badge"
                title={notice.message}
              >
                {notice.code === "data_plane"
                  ? t("peerConnection.dataPlaneBadge")
                  : t("peerConnection.controlOnlyBadge")}
              </Badge>
            )}
          {link === "connected" ? (
            <Button variant="danger" size="sm" data-testid="disconnect-btn" onClick={onDisconnect}>
              {t("peerConnection.disconnect")}
            </Button>
          ) : link === "connecting" ? (
            <Button
              variant="warning"
              size="sm"
              data-testid="connecting-btn"
              onClick={onDisconnect}
              title={t("peerConnection.connectingTitle")}
              className="gap-2"
            >
              <span className="mr-1 inline-block size-2 animate-pulse rounded-full bg-brand-amber align-middle" />
              {t("peerConnection.connecting")}
            </Button>
          ) : (
            <Button
              variant="secondary"
              size="sm"
              data-testid="connect-btn"
              onClick={onConnect}
              disabled={!canConnect}
              title={connectTitle}
            >
              {link === "failed" ? t("peerConnection.retryConnect") : t("peerConnection.connect")}
            </Button>
          )}
        </div>
      </div>

      <div className="mt-4 flex flex-wrap items-start gap-4">
        <div className="min-w-[240px] flex-1">
          <Button variant="secondary" size="sm" data-testid="create-offer-btn" onClick={onCreateOffer}>
            {t("peerConnection.createOffer")}
          </Button>
          {offerBlob && (
            <div className="mt-2">
              <textarea
                data-testid="offer-blob"
                readOnly
                value={offerBlob}
                className="h-16 w-full resize-none rounded-lg bg-black/40 p-2 font-mono text-[11px] text-ink"
              />
              <Button
                variant="ghost"
                size="sm"
                className="mt-1"
                onClick={() => void navigator.clipboard?.writeText(offerBlob)}
              >
                {t("peerConnection.copyOffer")}
              </Button>
            </div>
          )}
        </div>

        <div className="min-w-[240px] flex-1">
          <textarea
            data-testid="peer-input"
            value={peerInput}
            onChange={(e) => setPeerInput(e.target.value)}
            placeholder={t("peerConnection.peerInputPlaceholder")}
            className="h-16 w-full resize-none rounded-lg bg-black/40 p-2 font-mono text-[11px] text-ink placeholder:text-ink-muted"
          />
          <Button
            variant="secondary"
            size="sm"
            className="mt-1"
            data-testid="process-btn"
            onClick={onProcessPeerBlob}
          >
            {t("peerConnection.process")}
          </Button>
          {answerBlob && (
            <div className="mt-2">
              <div className="text-ink-muted">{t("peerConnection.sendAnswerBack")}</div>
              <textarea
                data-testid="answer-blob"
                readOnly
                value={answerBlob}
                className="mt-1 h-16 w-full resize-none rounded-lg bg-black/40 p-2 font-mono text-[11px] text-ink"
              />
              <Button
                variant="ghost"
                size="sm"
                className="mt-1"
                onClick={() => void navigator.clipboard?.writeText(answerBlob)}
              >
                {t("peerConnection.copyAnswer")}
              </Button>
            </div>
          )}
        </div>
      </div>

      {connError && (
        <div data-testid="conn-error" className="mt-2 text-brand-red">
          {connError}
        </div>
      )}
    </Card>
  );
}
