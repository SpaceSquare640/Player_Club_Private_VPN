import { Trans, useTranslation } from "react-i18next";
import NodeIdentity from "../components/network/NodeIdentity";
import PeerConnectionPanel from "../components/network/PeerConnectionPanel";

/**
 * Network — manage the peer connection: this node's identity, manual-signaling
 * blob exchange, and Connect / Disconnect. Live telemetry for an established
 * link is shown on the Diagnostics page.
 */
export default function Network() {
  const { t } = useTranslation();

  return (
    <section data-testid="page-network" className="flex h-full flex-col gap-5">
      <header>
        <h1 className="text-xl font-semibold text-ink">{t("network.title")}</h1>
        <p className="text-sm text-ink-muted">{t("network.subtitle")}</p>
      </header>

      <NodeIdentity />
      <PeerConnectionPanel />

      <p className="text-xs text-ink-muted">
        {/* The <1> placeholder lets each locale keep its own word order around
            the styled span, rather than gluing three separately-translated
            fragments together. */}
        <Trans i18nKey="network.footer">
          Exchange the offer/answer blobs out of band, then press Connect on both
          ends. Once connected, live RTT, throughput and the packet log appear on
          the <span className="text-ink">Diagnostics</span> page.
        </Trans>
      </p>
    </section>
  );
}
