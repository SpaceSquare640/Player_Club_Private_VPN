import { Gamepad2 } from "lucide-react";
import { useTranslation } from "react-i18next";
import { useAppStore } from "../stores/appStore";
import VirtualNetworkPanel from "../components/network/VirtualNetworkPanel";
import { cn } from "../lib/cn";

/**
 * Manually-triggered preset, not a background process scan — deliberately
 * lighter-weight than real "game detection". LAN-world discovery on
 * Minecraft's Java and Bedrock editions relies on broadcast/multicast, so
 * both stay on; FEC is nudged one step above the default for a bit more
 * resilience on typical home connections without maxing it out.
 */
const MINECRAFT_PRESET = { forwardBroadcast: true, forwardMulticast: true, fecParityShards: 2, extraRoutes: [] as string[] };

export default function Minecraft() {
  const { t } = useTranslation();
  const forwardBroadcast = useAppStore((s) => s.forwardBroadcast);
  const forwardMulticast = useAppStore((s) => s.forwardMulticast);
  const fecParityShards = useAppStore((s) => s.fecParityShards);
  const setForwardBroadcast = useAppStore((s) => s.setForwardBroadcast);
  const setForwardMulticast = useAppStore((s) => s.setForwardMulticast);
  const setFecParityShards = useAppStore((s) => s.setFecParityShards);

  const isApplied =
    forwardBroadcast === MINECRAFT_PRESET.forwardBroadcast &&
    forwardMulticast === MINECRAFT_PRESET.forwardMulticast &&
    fecParityShards === MINECRAFT_PRESET.fecParityShards;

  return (
    <div className="space-y-6" data-testid="page-minecraft">
      <div className="flex items-center gap-3">
        {/* Neutral placeholder icon — pending resolution of Minecraft-branded
            artwork licensing before it can ship in this repo. */}
        <Gamepad2 size={28} className="text-brand-violet" />
        <div>
          <h1 className="text-2xl font-semibold text-ink">{t("minecraft.title")}</h1>
          <p className="text-sm text-ink-muted">{t("minecraft.subtitle")}</p>
        </div>
      </div>

      <section className="rounded-2xl bg-surface-2 p-6 ring-1 ring-white/5" data-testid="minecraft-summary">
        <h2 className="text-sm font-medium text-ink-muted">{t("minecraft.currentSettingsHeading")}</h2>
        <ul className="mt-3 space-y-2 text-sm">
          <li className="text-ink">
            {t("settings.forwardBroadcast")}:{" "}
            <span className={forwardBroadcast ? "text-brand-cyan" : "text-ink-muted"}>
              {forwardBroadcast ? t("settings.on") : t("settings.off")}
            </span>
          </li>
          <li className="text-ink">
            {t("settings.forwardMulticast")}:{" "}
            <span className={forwardMulticast ? "text-brand-cyan" : "text-ink-muted"}>
              {forwardMulticast ? t("settings.on") : t("settings.off")}
            </span>
          </li>
          <li className="text-ink">
            {t("settings.fecRedundancyHeading")}:{" "}
            <span className="text-brand-cyan">{t("settings.fecRedundancyValue", { n: fecParityShards })}</span>
          </li>
        </ul>
      </section>

      <section className="rounded-2xl bg-surface-2 p-6 ring-1 ring-white/5">
        <h2 className="text-sm font-medium text-ink-muted">{t("minecraft.presetHeading")}</h2>
        <p className="mt-1 text-xs text-ink-muted">{t("minecraft.presetSubtitle")}</p>
        <button
          type="button"
          data-testid="minecraft-apply-preset"
          disabled={isApplied}
          onClick={() => {
            setForwardBroadcast(MINECRAFT_PRESET.forwardBroadcast);
            setForwardMulticast(MINECRAFT_PRESET.forwardMulticast);
            setFecParityShards(MINECRAFT_PRESET.fecParityShards);
          }}
          className={cn(
            "mt-4 rounded-lg px-4 py-2 text-sm font-medium transition",
            isApplied
              ? "cursor-default bg-white/5 text-ink-muted"
              : "bg-brand-violet text-white hover:opacity-90 focus:outline-none focus:ring-2 focus:ring-brand-violet/60",
          )}
        >
          {isApplied ? t("minecraft.presetApplied") : t("minecraft.presetApply")}
        </button>
      </section>

      <section>
        <h2 className="mb-3 text-sm font-medium text-ink-muted">{t("minecraft.networkHeading")}</h2>
        <p className="mb-3 text-xs text-ink-muted">{t("minecraft.networkSubtitle")}</p>
        {/* Same MeshSession as the Network page's general panel — creating or
            joining here just pre-fills the game tag and connection settings,
            it isn't a separate network system. */}
        <VirtualNetworkPanel gameTag="minecraft" settings={MINECRAFT_PRESET} />
      </section>
    </div>
  );
}
