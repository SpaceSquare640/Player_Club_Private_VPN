import { ExternalLink, Router } from "lucide-react";
import { useTranslation } from "react-i18next";
import { openUrl } from "@tauri-apps/plugin-opener";
import { useRelayHost } from "../hooks/useRelayHost";
import { useAppStore } from "../stores/appStore";
import { wikiPage } from "../lib/externalDocs";
import Card from "../components/ui/Card";
import Button from "../components/ui/Button";
import Badge from "../components/ui/Badge";

/**
 * Lets this machine host a relay (`engine::relay`) for other people's
 * cross-internet Virtual Network create/join calls, without needing the
 * separate standalone `relay` binary. Whoever wants to use it points their
 * own Settings → Connection → "Relay Server" field at this machine's
 * reachable address:port — reachability (port forwarding if hosting across
 * the internet) is still on the host to arrange, this page only starts the
 * process.
 */
export default function RelayServer() {
  const { t } = useTranslation();
  const language = useAppStore((s) => s.language);
  const relayServerAddr = useAppStore((s) => s.relayServerAddr);
  const setRelayServerAddr = useAppStore((s) => s.setRelayServerAddr);
  const { status, publicIp, port, error, busy, setPort, onStart, onStop } = useRelayHost();
  const reachableAddr = publicIp && status ? `${publicIp}:${status.port}` : null;

  return (
    <div className="space-y-6" data-testid="page-relay-server">
      <div className="flex items-start justify-between gap-3">
        <div className="flex items-center gap-3">
          <Router size={28} className="text-brand-violet" />
          <div>
            <h1 className="text-2xl font-semibold text-balance text-ink">{t("relayServer.title")}</h1>
            <p className="text-sm text-pretty text-ink-muted">{t("relayServer.subtitle")}</p>
          </div>
        </div>
        <button
          type="button"
          data-testid="relay-server-guide-link"
          onClick={() => openUrl(wikiPage(language, "Running-a-Relay-Server", "Running-a-Relay-Server-zh-Hant"))}
          className="flex shrink-0 items-center gap-1 text-sm text-ink-muted transition-colors duration-150 hover:text-ink"
        >
          {t("relayServer.guideLink")}
          <ExternalLink size={12} />
        </button>
      </div>

      <Card className="p-4 text-sm" data-testid="relay-server-status">
        {status ? (
          <>
            <div className="flex items-center justify-between gap-3">
              <div className="flex items-center gap-2">
                <Badge tone="cyan">{t("relayServer.running")}</Badge>
                <span data-testid="relay-server-port" className="font-mono text-ink">
                  :{status.port}
                </span>
              </div>
              <Button variant="danger" size="sm" data-testid="relay-server-stop-btn" onClick={() => void onStop()} disabled={busy}>
                {t("relayServer.stop")}
              </Button>
            </div>

            <div className="mt-3 flex items-center gap-2 text-ink-muted">
              <span>{t("relayServer.reachableAddrLabel")}:</span>
              <span data-testid="relay-server-reachable-addr" className="font-mono text-ink">
                {reachableAddr ?? t("relayServer.publicIpUnavailable")}
              </span>
              {reachableAddr && (
                <>
                  <Button variant="ghost" size="sm" onClick={() => void navigator.clipboard?.writeText(reachableAddr)}>
                    {t("network.virtualCopyAddr")}
                  </Button>
                  <Button
                    variant="ghost"
                    size="sm"
                    data-testid="relay-server-use-for-self-btn"
                    onClick={() => setRelayServerAddr(reachableAddr)}
                    disabled={relayServerAddr === reachableAddr}
                  >
                    {relayServerAddr === reachableAddr
                      ? t("relayServer.useForSelfDone")
                      : t("relayServer.useForSelf")}
                  </Button>
                </>
              )}
            </div>
            <p className="mt-2 text-xs text-pretty text-ink-muted">{t("relayServer.reachabilityHint")}</p>

            <div className="mt-3">
              <span className="text-xs font-semibold uppercase tracking-wider text-ink-muted">
                {t("relayServer.registeredHeading")}
              </span>
              <ul data-testid="relay-server-registered-list" className="mt-1.5 space-y-1">
                {status.registeredNetworks.length === 0 ? (
                  <li className="text-xs text-ink-muted">{t("relayServer.noneRegistered")}</li>
                ) : (
                  status.registeredNetworks.map((name) => (
                    <li key={name} data-testid="relay-server-registered-item" className="font-mono text-xs text-ink">
                      {name}
                    </li>
                  ))
                )}
              </ul>
            </div>
          </>
        ) : (
          <div className="flex flex-wrap items-end gap-2">
            <div className="flex-1">
              <label htmlFor="relay-server-port-input" className="text-xs text-ink-muted">
                {t("relayServer.portLabel")}
              </label>
              <input
                id="relay-server-port-input"
                data-testid="relay-server-port-input"
                value={port}
                onChange={(e) => setPort(e.target.value)}
                title={t("relayServer.portTitle")}
                className="mt-1 w-full rounded-lg bg-black/40 p-2 font-mono text-ink placeholder:text-ink-muted"
              />
            </div>
            <Button variant="primary" size="sm" data-testid="relay-server-start-btn" onClick={() => void onStart()} disabled={busy}>
              {t("relayServer.start")}
            </Button>
          </div>
        )}

        {error && (
          <div data-testid="relay-server-error" className="mt-2 text-brand-red">
            {error}
          </div>
        )}
      </Card>
    </div>
  );
}
