import { useTranslation } from "react-i18next";
import { useTelemetryStore } from "../../stores/telemetryStore";

/**
 * This node's identity: the short fingerprint (for voice/chat verification) plus
 * a button to copy the authoritative full public key to share with a peer.
 * Populated by the app-wide telemetry subscription.
 */
export default function NodeIdentity() {
  const { t } = useTranslation();
  const identity = useTelemetryStore((s) => s.identity);
  if (!identity) return null;

  return (
    <div
      data-testid="node-identity"
      className="flex items-center gap-3 rounded-lg border border-white/10 bg-surface-2/40 px-3 py-2 text-xs"
    >
      <span className="text-ink-muted">{t("nodeIdentity.label")}</span>
      <span
        data-testid="peer-address"
        title={t("nodeIdentity.fingerprintTitle")}
        className="font-mono text-brand-cyan"
      >
        {identity.peerAddress}
      </span>
      <button
        type="button"
        data-testid="copy-pubkey"
        title={t("nodeIdentity.copyButtonTitle")}
        onClick={() => void navigator.clipboard?.writeText(identity.publicKeyB64)}
        className="ml-auto rounded border border-white/15 px-2 py-1 text-ink-muted transition-colors hover:text-ink"
      >
        {t("nodeIdentity.copyButton")}
      </button>
    </div>
  );
}
