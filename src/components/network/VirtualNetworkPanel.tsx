import { useState } from "react";
import { ExternalLink } from "lucide-react";
import { useTranslation } from "react-i18next";
import { openUrl } from "@tauri-apps/plugin-opener";
import { useVirtualNetwork } from "../../hooks/useVirtualNetwork";
import { useAppStore } from "../../stores/appStore";
import { cn } from "../../lib/cn";
import { wikiPage } from "../../lib/externalDocs";
import { encodeInvite } from "../../lib/networkInvite";
import Card from "../ui/Card";
import Button from "../ui/Button";
import Badge from "../ui/Badge";
import type { ConnectionSettings, LinkState } from "../../types/telemetry";

const LINK_DOT: Record<LinkState, string> = {
  idle: "bg-ink-muted",
  connecting: "bg-brand-amber animate-pulse",
  connected: "bg-brand-cyan",
  failed: "bg-brand-red",
};

/** Known game tags get a friendly label; an unrecognized future tag falls back to itself. */
const GAME_TAG_LABEL_KEYS: Record<string, string> = {
  minecraft: "network.gameTagMinecraft",
};

export interface VirtualNetworkPanelProps {
  /** Fixed game tag (display metadata, e.g. `"minecraft"`) applied on create/join. */
  gameTag?: string;
  /** Fixed connection settings applied to every auto-connected peer. */
  settings?: ConnectionSettings;
  /**
   * When not in a network, show a one-line hint pointing at game-specific
   * pages instead of the create/join forms — an explicit "or create a
   * general-purpose network" toggle still reveals them. Used by the Network
   * page's general management panel so it doesn't compete with e.g. the
   * Minecraft page's own quick-create panel as a second, redundant entry
   * point for the common case. Once in a network, this has no effect — the
   * status view (name, game tag, member list, leave) is the same either way,
   * which is the actual "management" part.
   */
  collapseFormsByDefault?: boolean;
}

/**
 * Hamachi/Radmin-style virtual networking (Phase G.1–G.4): create or join a
 * named, password-gated network and let every member auto-connect — no
 * manual offer/answer paste. User-hosted, not centrally hosted: whoever
 * creates a network runs its signaling server themselves (see
 * `network.virtualCreateBindAddrTitle`); a host behind NAT without port
 * forwarding is a known limitation, not something this UI can paper over.
 *
 * Used both as the Network page's general management panel (no fixed
 * `gameTag`/`settings` — a real independent instance, its own create/join
 * state) and, with a fixed `gameTag`, as a game-specific quick-create panel
 * (e.g. the Minecraft page). Both read/write the *same* underlying
 * `MeshSession` on the Rust side, which supports any number of
 * simultaneously active networks (hosted and/or joined) — a network created
 * via one panel instance shows up in the other's list too.
 */
export default function VirtualNetworkPanel({ gameTag, settings, collapseFormsByDefault }: VirtualNetworkPanelProps) {
  const { t } = useTranslation();
  const language = useAppStore((s) => s.language);
  const [formsExpanded, setFormsExpanded] = useState(!collapseFormsByDefault);
  const {
    networks,
    savedNetworks,
    createName,
    createPassword,
    createBindAddr,
    joinHostAddr,
    joinName,
    joinPassword,
    error,
    busy,
    setCreateName,
    setCreatePassword,
    setCreateBindAddr,
    setJoinHostAddr,
    setJoinName,
    setJoinPassword,
    onCreate,
    onJoin,
    onLeave,
    onQuickStart,
    onForgetSaved,
    onPasteInvite,
  } = useVirtualNetwork(gameTag, settings);

  const activeNetworksList = networks.length > 0 && (
    <div className="flex flex-col gap-3">
      {networks.map((status) => {
        // The invite needs the password, which `NetworkStatus` never carries
        // (the backend doesn't echo it back) — `savedNetworksStore` already
        // has it from the create/join that produced this network, so that's
        // the source of truth here rather than tracking it separately.
        const savedForInvite = savedNetworks.find(
          (s) => s.networkName === status.networkName && s.mode === (status.isHost ? "create" : "join"),
        );
        return (
          <Card
            key={status.id}
            variant="raised"
            className="p-4 text-xs"
            data-testid="virtual-network-active"
            data-network-id={status.id}
          >
            <div className="flex items-center justify-between gap-3">
              <div>
                <span data-testid="vn-network-name" className="font-medium text-ink">
                  {status.networkName}
                </span>
                {status.isHost && (
                  <Badge tone="violet" className="ml-2">
                    {t("network.virtualHostBadge")}
                  </Badge>
                )}
                {status.gameTag && (
                  <Badge tone="cyan" className="ml-2" data-testid="vn-game-tag">
                    {status.gameTag in GAME_TAG_LABEL_KEYS ? t(GAME_TAG_LABEL_KEYS[status.gameTag]) : status.gameTag}
                  </Badge>
                )}
                {status.reconnecting && (
                  <Badge tone="amber" className="ml-2" data-testid="vn-reconnecting">
                    {t("network.virtualReconnecting")}
                  </Badge>
                )}
              </div>
              <Button
                variant="danger"
                size="sm"
                data-testid="vn-leave-btn"
                onClick={() => void onLeave(status.id)}
                disabled={busy}
              >
                {t("network.virtualLeave")}
              </Button>
            </div>

            <div className="mt-3 flex items-center gap-2 text-ink-muted">
              <span>{t("network.virtualHostAddr")}:</span>
              <span data-testid="vn-host-addr" className="font-mono text-ink">
                {status.hostAddr}
              </span>
              <Button variant="ghost" size="sm" onClick={() => void navigator.clipboard?.writeText(status.hostAddr)}>
                {t("network.virtualCopyAddr")}
              </Button>
              {savedForInvite && (
                <Button
                  variant="ghost"
                  size="sm"
                  data-testid="vn-copy-invite"
                  onClick={() =>
                    void navigator.clipboard?.writeText(
                      encodeInvite({
                        networkName: status.networkName,
                        password: savedForInvite.password,
                        hostAddr: status.hostAddr,
                      }),
                    )
                  }
                >
                  {t("network.virtualCopyInvite")}
                </Button>
              )}
            </div>

            <ul data-testid="vn-member-list" className="mt-3 space-y-1.5">
              {status.members.length === 0 ? (
                <li className="text-ink-muted">{t("network.virtualNoMembers")}</li>
              ) : (
                status.members.map((m) => (
                  <li key={m.pubkey} data-testid="vn-member" className="flex items-center gap-2">
                    <span className={cn("size-2 rounded-full", LINK_DOT[m.link])} />
                    <span className="font-mono text-ink">{m.fingerprint}</span>
                    <span className="text-ink-muted">{m.link}</span>
                  </li>
                ))
              )}
            </ul>
          </Card>
        );
      })}
    </div>
  );

  // A saved entry whose name matches a currently active network would just
  // fail to (re-)bind/join if started again — most concretely, re-creating
  // with the same specific bind address a moment ago hit "os error 10048"
  // (address already in use) because the earlier instance was still live.
  // Disabling Start for those (rather than letting the click reach the
  // backend and surface a raw OS error) prevents that class of failure
  // outright instead of just wording the error better.
  const activeNetworkNames = new Set(networks.map((n) => n.networkName));

  const savedNetworksList = savedNetworks.length > 0 && (
    <Card className="p-4 text-xs" data-testid="vn-saved-list">
      <h3 className="text-xs font-semibold uppercase tracking-wider text-ink-muted">{t("network.savedHeading")}</h3>
      <ul className="mt-3 space-y-1.5">
        {savedNetworks.map((saved) => {
          const isRunning = activeNetworkNames.has(saved.networkName);
          return (
            <li key={saved.id} data-testid="vn-saved-item" className="flex items-center justify-between gap-2">
              <div className="flex items-center gap-2">
                <span className="font-medium text-ink">{saved.networkName}</span>
                <Badge tone={saved.mode === "create" ? "violet" : "cyan"}>
                  {t(saved.mode === "create" ? "network.savedModeHost" : "network.savedModeJoin")}
                </Badge>
                <span className="font-mono text-ink-muted">{saved.mode === "create" ? saved.bindAddr : saved.hostAddr}</span>
              </div>
              <div className="flex items-center gap-1">
                <Button
                  variant="secondary"
                  size="sm"
                  data-testid="vn-saved-start-btn"
                  onClick={() => void onQuickStart(saved)}
                  disabled={busy || isRunning}
                  title={isRunning ? t("network.savedAlreadyRunning") : undefined}
                >
                  {isRunning ? t("network.savedRunning") : busy ? t("network.virtualConnecting") : t("network.savedStart")}
                </Button>
                <Button variant="ghost" size="sm" data-testid="vn-saved-forget-btn" onClick={() => onForgetSaved(saved.id)}>
                  {t("network.savedForget")}
                </Button>
              </div>
            </li>
          );
        })}
      </ul>
    </Card>
  );

  if (networks.length === 0 && !formsExpanded) {
    return (
      <div className="flex flex-col gap-4">
        {savedNetworksList}
        <Card className="p-4 text-xs text-ink-muted" data-testid="vn-collapsed-hint">
          <p className="text-pretty">{t("network.virtualCollapsedHint")}</p>
          <Button
            variant="ghost"
            size="sm"
            data-testid="vn-expand-general-forms"
            onClick={() => setFormsExpanded(true)}
            className="mt-2 -ml-3"
          >
            {t("network.virtualExpandGeneralForms")}
          </Button>
        </Card>
      </div>
    );
  }

  return (
    <div className="flex flex-col gap-4">
      {activeNetworksList}
      {savedNetworksList}
      <div className="flex flex-wrap items-start gap-4">
        <Card className="min-w-[240px] flex-1 p-4 text-xs">
        <div className="flex items-center justify-between gap-2">
          <h3 className="text-xs font-semibold uppercase tracking-wider text-ink-muted">
            {t("network.virtualCreateHeading")}
          </h3>
          <button
            type="button"
            data-testid="vn-create-guide-link"
            onClick={() => openUrl(wikiPage(language, "Create-a-Virtual-Network", "Create-a-Virtual-Network-zh-Hant"))}
            className="flex shrink-0 items-center gap-1 text-ink-muted transition-colors duration-150 hover:text-ink"
          >
            {t("network.createGuideLink")}
            <ExternalLink size={12} />
          </button>
        </div>
        <div className="mt-3 flex flex-col gap-2">
          <input
            data-testid="vn-create-name"
            value={createName}
            onChange={(e) => setCreateName(e.target.value)}
            placeholder={t("network.virtualNamePlaceholderOptional")}
            title={t("network.virtualNameOptionalTitle")}
            className="rounded-lg bg-black/40 p-2 text-ink placeholder:text-ink-muted"
          />
          <input
            data-testid="vn-create-password"
            type="password"
            value={createPassword}
            onChange={(e) => setCreatePassword(e.target.value)}
            placeholder={t("network.virtualPasswordPlaceholder")}
            className="rounded-lg bg-black/40 p-2 text-ink placeholder:text-ink-muted"
          />
          <input
            data-testid="vn-create-bind-addr"
            value={createBindAddr}
            onChange={(e) => setCreateBindAddr(e.target.value)}
            title={t("network.virtualCreateBindAddrTitle")}
            className="rounded-lg bg-black/40 p-2 font-mono text-ink placeholder:text-ink-muted"
          />
          <Button
            variant="secondary"
            size="sm"
            data-testid="vn-create-btn"
            onClick={() => void onCreate()}
            disabled={busy || !createPassword}
          >
            {busy ? t("network.virtualConnecting") : t("network.virtualCreateButton")}
          </Button>
        </div>
      </Card>

      <Card className="min-w-[240px] flex-1 p-4 text-xs">
        <div className="flex items-center justify-between gap-2">
          <h3 className="text-xs font-semibold uppercase tracking-wider text-ink-muted">
            {t("network.virtualJoinHeading")}
          </h3>
          <button
            type="button"
            data-testid="vn-join-guide-link"
            onClick={() => openUrl(wikiPage(language, "Join-a-Virtual-Network", "Join-a-Virtual-Network-zh-Hant"))}
            className="flex shrink-0 items-center gap-1 text-ink-muted transition-colors duration-150 hover:text-ink"
          >
            {t("network.joinGuideLink")}
            <ExternalLink size={12} />
          </button>
        </div>
        <div className="mt-3 flex flex-col gap-2">
          <Button variant="ghost" size="sm" data-testid="vn-paste-invite" onClick={() => void onPasteInvite()} className="self-start -ml-3">
            {t("network.virtualPasteInvite")}
          </Button>
          <input
            data-testid="vn-join-host-addr"
            value={joinHostAddr}
            onChange={(e) => setJoinHostAddr(e.target.value)}
            placeholder={t("network.virtualHostAddrPlaceholder")}
            className="rounded-lg bg-black/40 p-2 font-mono text-ink placeholder:text-ink-muted"
          />
          <input
            data-testid="vn-join-name"
            value={joinName}
            onChange={(e) => setJoinName(e.target.value)}
            placeholder={t("network.virtualNamePlaceholder")}
            className="rounded-lg bg-black/40 p-2 text-ink placeholder:text-ink-muted"
          />
          <input
            data-testid="vn-join-password"
            type="password"
            value={joinPassword}
            onChange={(e) => setJoinPassword(e.target.value)}
            placeholder={t("network.virtualPasswordPlaceholder")}
            className="rounded-lg bg-black/40 p-2 text-ink placeholder:text-ink-muted"
          />
          <Button
            variant="secondary"
            size="sm"
            data-testid="vn-join-btn"
            onClick={() => void onJoin()}
            disabled={busy || !joinHostAddr || !joinName || !joinPassword}
          >
            {busy ? t("network.virtualConnecting") : t("network.virtualJoinButton")}
          </Button>
        </div>
      </Card>

        {error && (
          <div data-testid="vn-error" className="w-full text-brand-red">
            {error}
          </div>
        )}
      </div>
    </div>
  );
}
