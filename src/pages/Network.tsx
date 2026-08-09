import { useState } from "react";
import { Trans, useTranslation } from "react-i18next";
import NodeIdentity from "../components/network/NodeIdentity";
import PeerConnectionPanel from "../components/network/PeerConnectionPanel";
import VirtualNetworkPanel from "../components/network/VirtualNetworkPanel";
import { cn } from "../lib/cn";

type NetworkMode = "manual" | "virtual";

/**
 * Network — manage peer connectivity via either mode:
 * - Manual: this node's identity, offer/answer blob exchange, Connect/Disconnect.
 * - Virtual network (Phase G.1–G.4): create/join a Hamachi-style named,
 *   password-gated network where every member auto-connects.
 * Both modes share the same underlying engine (`ConnectionManager`) — live
 * telemetry for an established link is shown on the Diagnostics page either way.
 */
export default function Network() {
  const { t } = useTranslation();
  const [mode, setMode] = useState<NetworkMode>("manual");

  return (
    <section data-testid="page-network" className="flex h-full flex-col gap-5">
      <header>
        <h1 className="text-2xl font-semibold text-balance text-ink">{t("network.title")}</h1>
        <p className="text-sm text-pretty text-ink-muted">{t("network.subtitle")}</p>
      </header>

      <NodeIdentity />

      <div className="flex gap-2 border-b border-white/10 text-sm">
        {(["manual", "virtual"] as const).map((m) => (
          <button
            key={m}
            type="button"
            data-testid={`network-mode-${m}`}
            aria-pressed={mode === m}
            onClick={() => setMode(m)}
            className={cn(
              "border-b-2 px-3 py-2 transition-colors",
              mode === m
                ? "border-brand-violet text-ink"
                : "border-transparent text-ink-muted hover:text-ink",
            )}
          >
            {t(m === "manual" ? "network.modeManual" : "network.modeVirtual")}
          </button>
        ))}
      </div>

      {mode === "manual" ? (
        <>
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
        </>
      ) : (
        <VirtualNetworkPanel collapseFormsByDefault />
      )}
    </section>
  );
}
