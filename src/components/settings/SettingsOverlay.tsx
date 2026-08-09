import { useState } from "react";
import { ExternalLink, X } from "lucide-react";
import { useTranslation } from "react-i18next";
import { save, open as openDialog } from "@tauri-apps/plugin-dialog";
import { readTextFile, writeTextFile } from "@tauri-apps/plugin-fs";
import { openUrl } from "@tauri-apps/plugin-opener";
import { useAppStore } from "../../stores/appStore";
import { THEMES } from "../../theme/themes";
import { cn } from "../../lib/cn";
import type { SupportedLanguage } from "../../i18n";
import { parseConnectionProfile, serializeConnectionProfile } from "../../lib/profile";
import { repoDoc, wikiPage } from "../../lib/externalDocs";
import Button from "../ui/Button";
import Toggle from "../ui/Toggle";

function legalLinks(language: SupportedLanguage) {
  return {
    userManual: wikiPage(language, "User-Manual", "User-Manual-zh-Hant"),
    termsOfService: repoDoc(language, "TERMS_OF_SERVICE.md", "TERMS_OF_SERVICE.zh-Hant.md"),
    privacyPolicy: repoDoc(language, "PRIVACY_POLICY.md", "PRIVACY_POLICY.zh-Hant.md"),
    wiki: wikiPage(language, "", "Home-zh-Hant"),
  };
}

/**
 * Frosted-glass (Mica-style) Settings overlay. Slides in from the right and
 * hosts the theme switcher. Visibility is driven by the store's `settingsOpen`.
 */
const FEC_OPTIONS = [1, 2, 3] as const;

const LANGUAGE_OPTIONS: { id: SupportedLanguage; labelKey: string }[] = [
  { id: "en", labelKey: "settings.languageEnglish" },
  { id: "zh-Hant", labelKey: "settings.languageZhHant" },
  { id: "zh-Hans", labelKey: "settings.languageZhHans" },
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
  const extraRoutes = useAppStore((s) => s.extraRoutes);
  const setExtraRoutes = useAppStore((s) => s.setExtraRoutes);
  const [extraRoutesText, setExtraRoutesText] = useState(() => extraRoutes.join(", "));
  const [importError, setImportError] = useState<string | null>(null);

  const handleExportProfile = async () => {
    setImportError(null);
    const path = await save({
      title: t("settings.exportProfileDialogTitle"),
      defaultPath: "player-club-vpn-profile.json",
      filters: [{ name: "JSON", extensions: ["json"] }],
    });
    if (!path) return;
    const json = serializeConnectionProfile({ forwardBroadcast, forwardMulticast, fecParityShards, extraRoutes });
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
    setExtraRoutes(result.settings.extraRoutes);
    setExtraRoutesText(result.settings.extraRoutes.join(", "));
  };

  /** Commit the free-text field as a parsed route list on blur, not on
   * every keystroke — typing "192.168.50.0/2" mid-edit is not yet a valid
   * entry, and the store shouldn't churn through invalid intermediate states. */
  const commitExtraRoutesText = () => {
    const routes = extraRoutesText
      .split(",")
      .map((s) => s.trim())
      .filter((s) => s.length > 0);
    setExtraRoutes(routes);
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
      <div className="absolute inset-0 bg-black/50" onClick={() => toggle(false)} />

      {/* Frosted-glass panel */}
      <aside
        data-testid="settings-overlay"
        className={cn(
          "absolute right-0 top-0 h-full w-[380px] overflow-y-auto border-l border-white/10 p-6 shadow-2xl",
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
            className="flex size-8 items-center justify-center rounded-lg text-ink-muted transition-colors duration-150 hover:bg-white/5 hover:text-ink"
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
                  "flex items-center gap-2 rounded-lg border px-3 py-2 text-sm transition-colors duration-150",
                  theme === th.id
                    ? "border-brand-violet bg-brand-violet/10 text-ink"
                    : "border-white/10 text-ink-muted hover:border-white/25 hover:text-ink",
                )}
              >
                <span
                  className="size-4 rounded-full ring-1 ring-white/20"
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
          <div className="mt-3 grid grid-cols-3 gap-2">
            {LANGUAGE_OPTIONS.map((opt) => (
              <button
                key={opt.id}
                type="button"
                data-testid={`language-${opt.id}`}
                aria-pressed={language === opt.id}
                onClick={() => setLanguage(opt.id)}
                className={cn(
                  "rounded-lg border px-3 py-2 text-sm transition-colors duration-150",
                  language === opt.id
                    ? "border-brand-violet bg-brand-violet/10 text-ink"
                    : "border-white/10 text-ink-muted hover:border-white/25 hover:text-ink",
                )}
              >
                {t(opt.labelKey)}
              </button>
            ))}
          </div>
        </section>

        <section className="mt-6">
          <label className="flex w-full items-center justify-between rounded-lg border border-white/10 px-3 py-2.5 text-sm">
            <span className="text-ink" title={t("settings.expertModeTitle")}>
              {t("settings.expertMode")}
            </span>
            <Toggle
              checked={expertMode}
              onChange={setExpertMode}
              label={t("settings.expertMode")}
              data-testid="settings-expert-mode"
            />
          </label>
        </section>

        {/* Advanced settings — display-only filter. Hidden values remain in
            effect (see `appStore.expertMode`'s doc comment); this section is
            never unmounted-and-reset, only conditionally rendered. */}
        {expertMode && (
          <section className="mt-6" data-testid="settings-connection">
            <h3 className="text-xs font-semibold uppercase tracking-wider text-ink-muted">
              {t("settings.connectionHeading")}
            </h3>
            <p className="mt-1 text-xs text-pretty text-ink-muted">{t("settings.connectionSubtitle")}</p>

            <div className="mt-3 flex flex-col gap-2">
              <label className="flex items-center justify-between rounded-lg border border-white/10 px-3 py-2.5 text-sm">
                <span className="text-ink" title={t("settings.forwardBroadcastTitle")}>
                  {t("settings.forwardBroadcast")}
                </span>
                <Toggle
                  checked={forwardBroadcast}
                  onChange={setForwardBroadcast}
                  label={t("settings.forwardBroadcast")}
                  data-testid="settings-forward-broadcast"
                />
              </label>

              <label className="flex items-center justify-between rounded-lg border border-white/10 px-3 py-2.5 text-sm">
                <span className="text-ink" title={t("settings.forwardMulticastTitle")}>
                  {t("settings.forwardMulticast")}
                </span>
                <Toggle
                  checked={forwardMulticast}
                  onChange={setForwardMulticast}
                  label={t("settings.forwardMulticast")}
                  data-testid="settings-forward-multicast"
                />
              </label>
            </div>

            <div className="mt-4">
              <div className="flex items-center justify-between">
                <span className="text-sm text-ink">{t("settings.fecRedundancyHeading")}</span>
                <span className="text-xs tabular-nums text-ink-muted">
                  {t("settings.fecRedundancyValue", { n: fecParityShards })}
                </span>
              </div>
              <p className="mt-1 text-xs text-pretty text-ink-muted">{t("settings.fecRedundancySubtitle")}</p>
              <div className="mt-2 grid grid-cols-3 gap-2">
                {FEC_OPTIONS.map((n) => (
                  <button
                    key={n}
                    type="button"
                    data-testid={`settings-fec-${n}`}
                    aria-pressed={fecParityShards === n}
                    onClick={() => setFecParityShards(n)}
                    className={cn(
                      "rounded-lg border px-3 py-2 text-sm tabular-nums transition-colors duration-150",
                      fecParityShards === n
                        ? "border-brand-violet bg-brand-violet/10 text-ink"
                        : "border-white/10 text-ink-muted hover:border-white/25 hover:text-ink",
                    )}
                  >
                    {n}
                  </button>
                ))}
              </div>
            </div>

            <div className="mt-4">
              <span className="text-sm text-ink">{t("settings.extraRoutesHeading")}</span>
              <p className="mt-1 text-xs text-pretty text-ink-muted">{t("settings.extraRoutesSubtitle")}</p>
              <input
                data-testid="settings-extra-routes"
                value={extraRoutesText}
                onChange={(e) => setExtraRoutesText(e.target.value)}
                onBlur={commitExtraRoutesText}
                placeholder={t("settings.extraRoutesPlaceholder")}
                className="mt-2 w-full rounded-lg bg-black/40 p-2 font-mono text-xs text-ink placeholder:text-ink-muted"
              />
            </div>

            <div className="mt-4 flex flex-col gap-2">
              <div className="flex gap-2">
                <Button
                  variant="ghost"
                  size="sm"
                  data-testid="settings-export-profile"
                  onClick={handleExportProfile}
                  className="flex-1 border border-white/10"
                >
                  {t("settings.exportProfile")}
                </Button>
                <Button
                  variant="ghost"
                  size="sm"
                  data-testid="settings-import-profile"
                  onClick={handleImportProfile}
                  className="flex-1 border border-white/10"
                >
                  {t("settings.importProfile")}
                </Button>
              </div>
              {importError && (
                <p data-testid="settings-import-error" className="text-xs text-brand-red">
                  {importError}
                </p>
              )}
            </div>
          </section>
        )}

        <section className="mt-6" data-testid="settings-about">
          <h3 className="text-xs font-semibold uppercase tracking-wider text-ink-muted">
            {t("settings.aboutHeading")}
          </h3>
          <div className="mt-3 flex flex-col gap-1">
            {(
              [
                ["userManual", "userManualLink"],
                ["termsOfService", "termsOfServiceLink"],
                ["privacyPolicy", "privacyPolicyLink"],
                ["wiki", "wikiLink"],
              ] as const
            ).map(([linkKey, labelKey]) => (
              <button
                key={linkKey}
                type="button"
                data-testid={`settings-about-${linkKey}`}
                onClick={() => openUrl(legalLinks(language)[linkKey])}
                className="flex items-center justify-between rounded-lg px-3 py-2 text-sm text-ink-muted transition-colors duration-150 hover:bg-white/5 hover:text-ink"
              >
                {t(`settings.${labelKey}`)}
                <ExternalLink size={14} />
              </button>
            ))}
          </div>
        </section>
      </aside>
    </div>
  );
}
