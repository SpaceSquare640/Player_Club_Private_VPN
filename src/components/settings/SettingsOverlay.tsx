import { useState } from "react";
import { X } from "lucide-react";
import { useTranslation } from "react-i18next";
import { save, open as openDialog } from "@tauri-apps/plugin-dialog";
import { readTextFile, writeTextFile } from "@tauri-apps/plugin-fs";
import { useAppStore } from "../../stores/appStore";
import { THEMES } from "../../theme/themes";
import { cn } from "../../lib/cn";
import type { SupportedLanguage } from "../../i18n";
import { parseConnectionProfile, serializeConnectionProfile } from "../../lib/profile";

/**
 * Frosted-glass (Mica-style) Settings overlay. Slides in from the right and
 * hosts the theme switcher. Visibility is driven by the store's `settingsOpen`.
 */
const FEC_OPTIONS = [1, 2, 3] as const;

const LANGUAGE_OPTIONS: { id: SupportedLanguage; labelKey: string }[] = [
  { id: "en", labelKey: "settings.languageEnglish" },
  { id: "zh-Hant", labelKey: "settings.languageZhHant" },
];

export default function SettingsOverlay() {
  const { t } = useTranslation();
  const open = useAppStore((s) => s.settingsOpen);
  const toggle = useAppStore((s) => s.toggleSettings);
  const theme = useAppStore((s) => s.theme);
  const setTheme = useAppStore((s) => s.setTheme);
  const language = useAppStore((s) => s.language);
  const setLanguage = useAppStore((s) => s.setLanguage);
  const expertMode = useAppStore((s) => s.expertMode);
  const setExpertMode = useAppStore((s) => s.setExpertMode);
  const forwardBroadcast = useAppStore((s) => s.forwardBroadcast);
  const forwardMulticast = useAppStore((s) => s.forwardMulticast);
  const setForwardBroadcast = useAppStore((s) => s.setForwardBroadcast);
  const setForwardMulticast = useAppStore((s) => s.setForwardMulticast);
  const fecParityShards = useAppStore((s) => s.fecParityShards);
  const setFecParityShards = useAppStore((s) => s.setFecParityShards);
  const [importError, setImportError] = useState<string | null>(null);

  const handleExportProfile = async () => {
    setImportError(null);
    const path = await save({
      title: t("settings.exportProfileDialogTitle"),
      defaultPath: "player-club-vpn-profile.json",
      filters: [{ name: "JSON", extensions: ["json"] }],
    });
    if (!path) return;
    const json = serializeConnectionProfile({ forwardBroadcast, forwardMulticast, fecParityShards });
    await writeTextFile(path, json);
  };

  const handleImportProfile = async () => {
    setImportError(null);
    const path = await openDialog({
      title: t("settings.importProfileDialogTitle"),
      multiple: false,
      filters: [{ name: "JSON", extensions: ["json"] }],
    });
    if (!path || Array.isArray(path)) return;
    const raw = await readTextFile(path);
    const result = parseConnectionProfile(raw);
    if (!result.ok) {
      setImportError(t(`settings.importError.${result.error}`));
      return;
    }
    setForwardBroadcast(result.settings.forwardBroadcast);
    setForwardMulticast(result.settings.forwardMulticast);
    setFecParityShards(result.settings.fecParityShards);
  };

  return (
    <div
      aria-hidden={!open}
      className={cn(
        "fixed inset-0 z-50 transition-opacity duration-200",
        open ? "pointer-events-auto opacity-100" : "pointer-events-none opacity-0",
      )}
    >
      {/* Scrim */}
      <div
        className="absolute inset-0 bg-black/40"
        onClick={() => toggle(false)}
      />

      {/* Frosted-glass panel */}
      <aside
        data-testid="settings-overlay"
        className={cn(
          "absolute right-0 top-0 h-full w-[380px] border-l border-white/10 p-6 shadow-2xl",
          "bg-surface-2/70 backdrop-blur-xl backdrop-saturate-150",
          "transition-transform duration-200",
          open ? "translate-x-0" : "translate-x-full",
        )}
      >
        <div className="flex items-center justify-between">
          <h2 className="text-lg font-semibold text-ink">{t("settings.title")}</h2>
          <button
            type="button"
            aria-label={t("settings.closeAriaLabel")}
            data-testid="settings-close"
            onClick={() => toggle(false)}
            className="flex h-8 w-8 items-center justify-center rounded-lg text-ink-muted transition-colors hover:bg-white/5 hover:text-ink"
          >
            <X size={18} />
          </button>
        </div>

        <section className="mt-6">
          <h3 className="text-xs font-semibold uppercase tracking-wider text-ink-muted">
            {t("settings.themeHeading")}
          </h3>
          <div className="mt-3 grid grid-cols-2 gap-2">
            {THEMES.map((th) => (
              <button
                key={th.id}
                type="button"
                data-testid={`theme-${th.id}`}
                aria-pressed={theme === th.id}
                onClick={() => setTheme(th.id)}
                className={cn(
                  "flex items-center gap-2 rounded-lg border px-3 py-2 text-sm transition-colors",
                  theme === th.id
                    ? "border-brand-violet text-ink"
                    : "border-white/10 text-ink-muted hover:border-white/25 hover:text-ink",
                )}
              >
                <span
                  className="h-4 w-4 rounded-full ring-1 ring-white/20"
                  style={{ background: th.swatch }}
                />
                {t(`theme.${th.id}`)}
              </button>
            ))}
          </div>
        </section>

        <section className="mt-6" data-testid="settings-language">
          <h3 className="text-xs font-semibold uppercase tracking-wider text-ink-muted">
            {t("settings.languageHeading")}
          </h3>
          <div className="mt-3 grid grid-cols-2 gap-2">
            {LANGUAGE_OPTIONS.map((opt) => (
              <button
                key={opt.id}
                type="button"
                data-testid={`language-${opt.id}`}
                aria-pressed={language === opt.id}
                onClick={() => setLanguage(opt.id)}
                className={cn(
                  "rounded-lg border px-3 py-2 text-sm transition-colors",
                  language === opt.id
                    ? "border-brand-violet text-ink"
                    : "border-white/10 text-ink-muted hover:border-white/25 hover:text-ink",
                )}
              >
                {t(opt.labelKey)}
              </button>
            ))}
          </div>
        </section>

        <section className="mt-6">
          <button
            type="button"
            data-testid="settings-expert-mode"
            aria-pressed={expertMode}
            onClick={() => setExpertMode(!expertMode)}
            title={t("settings.expertModeTitle")}
            className={cn(
              "flex w-full items-center justify-between rounded-lg border px-3 py-2 text-sm transition-colors",
              expertMode
                ? "border-brand-violet text-ink"
                : "border-white/10 text-ink-muted hover:border-white/25 hover:text-ink",
            )}
          >
            {t("settings.expertMode")}
            <span className={expertMode ? "text-brand-violet" : "text-ink-muted"}>
              {expertMode ? t("settings.on") : t("settings.off")}
            </span>
          </button>
        </section>

        {/* Advanced settings — display-only filter. Hidden values remain in
            effect (see `appStore.expertMode`'s doc comment); this section is
            never unmounted-and-reset, only conditionally rendered. */}
        {expertMode && (
          <section className="mt-6" data-testid="settings-connection">
            <h3 className="text-xs font-semibold uppercase tracking-wider text-ink-muted">
              {t("settings.connectionHeading")}
            </h3>
            <p className="mt-1 text-xs text-ink-muted">{t("settings.connectionSubtitle")}</p>

            <div className="mt-3 flex flex-col gap-2">
              <button
                type="button"
                data-testid="settings-forward-broadcast"
                aria-pressed={forwardBroadcast}
                onClick={() => setForwardBroadcast(!forwardBroadcast)}
                title={t("settings.forwardBroadcastTitle")}
                className={cn(
                  "flex items-center justify-between rounded-lg border px-3 py-2 text-sm transition-colors",
                  forwardBroadcast
                    ? "border-brand-violet text-ink"
                    : "border-white/10 text-ink-muted hover:border-white/25 hover:text-ink",
                )}
              >
                {t("settings.forwardBroadcast")}
                <span className={forwardBroadcast ? "text-brand-violet" : "text-ink-muted"}>
                  {forwardBroadcast ? t("settings.on") : t("settings.off")}
                </span>
              </button>

              <button
                type="button"
                data-testid="settings-forward-multicast"
                aria-pressed={forwardMulticast}
                onClick={() => setForwardMulticast(!forwardMulticast)}
                title={t("settings.forwardMulticastTitle")}
                className={cn(
                  "flex items-center justify-between rounded-lg border px-3 py-2 text-sm transition-colors",
                  forwardMulticast
                    ? "border-brand-violet text-ink"
                    : "border-white/10 text-ink-muted hover:border-white/25 hover:text-ink",
                )}
              >
                {t("settings.forwardMulticast")}
                <span className={forwardMulticast ? "text-brand-violet" : "text-ink-muted"}>
                  {forwardMulticast ? t("settings.on") : t("settings.off")}
                </span>
              </button>
            </div>

            <div className="mt-4">
              <div className="flex items-center justify-between">
                <span className="text-sm text-ink">{t("settings.fecRedundancyHeading")}</span>
                <span className="text-xs text-ink-muted">
                  {t("settings.fecRedundancyValue", { n: fecParityShards })}
                </span>
              </div>
              <p className="mt-1 text-xs text-ink-muted">{t("settings.fecRedundancySubtitle")}</p>
              <div className="mt-2 grid grid-cols-3 gap-2">
                {FEC_OPTIONS.map((n) => (
                  <button
                    key={n}
                    type="button"
                    data-testid={`settings-fec-${n}`}
                    aria-pressed={fecParityShards === n}
                    onClick={() => setFecParityShards(n)}
                    className={cn(
                      "rounded-lg border px-3 py-2 text-sm transition-colors",
                      fecParityShards === n
                        ? "border-brand-violet text-ink"
                        : "border-white/10 text-ink-muted hover:border-white/25 hover:text-ink",
                    )}
                  >
                    {n}
                  </button>
                ))}
              </div>
            </div>

            <div className="mt-4 flex flex-col gap-2">
              <div className="flex gap-2">
                <button
                  type="button"
                  data-testid="settings-export-profile"
                  onClick={handleExportProfile}
                  className="flex-1 rounded-lg border border-white/10 px-3 py-2 text-sm text-ink-muted transition-colors hover:border-white/25 hover:text-ink"
                >
                  {t("settings.exportProfile")}
                </button>
                <button
                  type="button"
                  data-testid="settings-import-profile"
                  onClick={handleImportProfile}
                  className="flex-1 rounded-lg border border-white/10 px-3 py-2 text-sm text-ink-muted transition-colors hover:border-white/25 hover:text-ink"
                >
                  {t("settings.importProfile")}
                </button>
              </div>
              {importError && (
                <p data-testid="settings-import-error" className="text-xs text-red-400">
                  {importError}
                </p>
              )}
            </div>
          </section>
        )}
      </aside>
    </div>
  );
}
