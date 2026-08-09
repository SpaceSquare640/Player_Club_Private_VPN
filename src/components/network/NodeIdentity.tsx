import { useTranslation } from "react-i18next";
import { useTelemetryStore } from "../../stores/telemetryStore";
import Button from "../ui/Button";

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
      className="flex items-center gap-3 rounded-xl border border-white/5 bg-surface-2/60 px-4 py-2.5 text-xs"
    >
      <span className="text-ink-muted">{t("nodeIdentity.label")}</span>
      <span
        data-testid="peer-address"
        title={t("nodeIdentity.fingerprintTitle")}
        className="font-mono text-brand-cyan"
      >
        {identity.peerAddress}
      </span>
      <Button
        variant="ghost"
        size="sm"
        data-testid="copy-pubkey"
        title={t("nodeIdentity.copyButtonTitle")}
        onClick={() => void navigator.clipboard?.writeText(identity.publicKeyB64)}
        className="ml-auto"
      >
        {t("nodeIdentity.copyButton")}
      </Button>
    </div>
  );
}
