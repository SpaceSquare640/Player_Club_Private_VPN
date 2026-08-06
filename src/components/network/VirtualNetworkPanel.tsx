import { useState } from "react";
import { useTranslation } from "react-i18next";
import { useVirtualNetwork } from "../../hooks/useVirtualNetwork";
import { cn } from "../../lib/cn";
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
 * `MeshSession` on the Rust side — there is only ever one active network no
 * matter which panel instance you used to create or join it.
 */
export default function VirtualNetworkPanel({ gameTag, settings, collapseFormsByDefault }: VirtualNetworkPanelProps) {
  const { t } = useTranslation();
  const [formsExpanded, setFormsExpanded] = useState(!collapseFormsByDefault);
  const {
    status,
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
  } = useVirtualNetwork(gameTag, settings);

  if (status) {
    return (
      <div
        data-testid="virtual-network-active"
        className="rounded-xl border border-white/10 bg-surface-2/40 p-4 text-xs"
      >
        <div className="flex items-center justify-between gap-3">
          <div>
            <span data-testid="vn-network-name" className="font-medium text-ink">
              {status.networkName}
            </span>
            {status.isHost && (
              <span className="ml-2 rounded bg-brand-violet/15 px-1.5 py-0.5 text-[10px] font-medium text-brand-violet">
                {t("network.virtualHostBadge")}
              </span>
            )}
            {status.gameTag && (
              <span
                data-testid="vn-game-tag"
                className="ml-2 rounded bg-brand-cyan/15 px-1.5 py-0.5 text-[10px] font-medium text-brand-cyan"
              >
                {status.gameTag in GAME_TAG_LABEL_KEYS ? t(GAME_TAG_LABEL_KEYS[status.gameTag]) : status.gameTag}
              </span>
            )}
          </div>
          <button
            type="button"
            data-testid="vn-leave-btn"
            onClick={() => void onLeave()}
            disabled={busy}
            className="rounded-lg border border-brand-red/40 px-3 py-1.5 text-brand-red transition-colors hover:bg-brand-red/10 disabled:cursor-not-allowed disabled:opacity-50"
          >
            {t("network.virtualLeave")}
          </button>
        </div>

        <div className="mt-2 flex items-center gap-2 text-ink-muted">
          <span>{t("network.virtualHostAddr")}:</span>
          <span data-testid="vn-host-addr" className="font-mono text-ink">
            {status.hostAddr}
          </span>
          <button
            type="button"
            onClick={() => void navigator.clipboard?.writeText(status.hostAddr)}
            className="rounded border border-white/15 px-1.5 py-0.5 text-ink-muted transition-colors hover:text-ink"
          >
            {t("network.virtualCopyAddr")}
          </button>
        </div>

        <ul data-testid="vn-member-list" className="mt-3 space-y-1.5">
          {status.members.length === 0 ? (
            <li className="text-ink-muted">{t("network.virtualNoMembers")}</li>
          ) : (
            status.members.map((m) => (
              <li key={m.pubkey} data-testid="vn-member" className="flex items-center gap-2">
                <span className={cn("h-2 w-2 rounded-full", LINK_DOT[m.link])} />
                <span className="font-mono text-ink">{m.fingerprint}</span>
                <span className="text-ink-muted">{m.link}</span>
              </li>
            ))
          )}
        </ul>

        {error && (
          <div data-testid="vn-error" className="mt-2 text-brand-red">
            {error}
          </div>
        )}
      </div>
    );
  }

  if (!formsExpanded) {
    return (
      <div
        data-testid="vn-collapsed-hint"
        className="rounded-xl border border-white/10 bg-surface-2/40 p-4 text-xs text-ink-muted"
      >
        <p>{t("network.virtualCollapsedHint")}</p>
        <button
          type="button"
          data-testid="vn-expand-general-forms"
          onClick={() => setFormsExpanded(true)}
          className="mt-2 text-brand-cyan transition-colors hover:text-ink"
        >
          {t("network.virtualExpandGeneralForms")}
        </button>
      </div>
    );
  }

  return (
    <div className="flex flex-wrap items-start gap-4">
      <div className="min-w-[240px] flex-1 rounded-xl border border-white/10 bg-surface-2/40 p-4 text-xs">
        <h3 className="text-xs font-semibold uppercase tracking-wider text-ink-muted">
          {t("network.virtualCreateHeading")}
        </h3>
        <div className="mt-3 flex flex-col gap-2">
          <input
            data-testid="vn-create-name"
            value={createName}
            onChange={(e) => setCreateName(e.target.value)}
            placeholder={t("network.virtualNamePlaceholder")}
            className="rounded bg-black/40 p-2 text-ink placeholder:text-ink-muted"
          />
          <input
            data-testid="vn-create-password"
            type="password"
            value={createPassword}
            onChange={(e) => setCreatePassword(e.target.value)}
            placeholder={t("network.virtualPasswordPlaceholder")}
            className="rounded bg-black/40 p-2 text-ink placeholder:text-ink-muted"
          />
          <input
            data-testid="vn-create-bind-addr"
            value={createBindAddr}
            onChange={(e) => setCreateBindAddr(e.target.value)}
            title={t("network.virtualCreateBindAddrTitle")}
            className="rounded bg-black/40 p-2 font-mono text-ink placeholder:text-ink-muted"
          />
          <button
            type="button"
            data-testid="vn-create-btn"
            onClick={() => void onCreate()}
            disabled={busy || !createName || !createPassword}
            className="rounded-lg border border-brand-violet/40 px-3 py-1.5 text-brand-violet transition-colors hover:bg-brand-violet/10 disabled:cursor-not-allowed disabled:opacity-50"
          >
            {t("network.virtualCreateButton")}
          </button>
        </div>
      </div>

      <div className="min-w-[240px] flex-1 rounded-xl border border-white/10 bg-surface-2/40 p-4 text-xs">
        <h3 className="text-xs font-semibold uppercase tracking-wider text-ink-muted">
          {t("network.virtualJoinHeading")}
        </h3>
        <div className="mt-3 flex flex-col gap-2">
          <input
            data-testid="vn-join-host-addr"
            value={joinHostAddr}
            onChange={(e) => setJoinHostAddr(e.target.value)}
            placeholder={t("network.virtualHostAddrPlaceholder")}
            className="rounded bg-black/40 p-2 font-mono text-ink placeholder:text-ink-muted"
          />
          <input
            data-testid="vn-join-name"
            value={joinName}
            onChange={(e) => setJoinName(e.target.value)}
            placeholder={t("network.virtualNamePlaceholder")}
            className="rounded bg-black/40 p-2 text-ink placeholder:text-ink-muted"
          />
          <input
            data-testid="vn-join-password"
            type="password"
            value={joinPassword}
            onChange={(e) => setJoinPassword(e.target.value)}
            placeholder={t("network.virtualPasswordPlaceholder")}
            className="rounded bg-black/40 p-2 text-ink placeholder:text-ink-muted"
          />
          <button
            type="button"
            data-testid="vn-join-btn"
            onClick={() => void onJoin()}
            disabled={busy || !joinHostAddr || !joinName || !joinPassword}
            className="rounded-lg border border-brand-cyan/40 px-3 py-1.5 text-brand-cyan transition-colors hover:bg-brand-cyan/10 disabled:cursor-not-allowed disabled:opacity-50"
          >
            {t("network.virtualJoinButton")}
          </button>
        </div>
      </div>

      {error && (
        <div data-testid="vn-error" className="w-full text-brand-red">
          {error}
        </div>
      )}
    </div>
  );
}
