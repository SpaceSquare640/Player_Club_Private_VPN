/**
 * i18next setup. Both locales are bundled statically (no runtime fetch and no
 * `i18next-http-backend`) — this is a desktop app, so shipping every language
 * costs nothing and avoids a loading flash. Language *detection* is likewise
 * handled without a plugin: `stores/appStore.ts` picks a default from
 * `navigator.language` once and persists the user's choice from then on,
 * mirroring how the theme is stored. `components/layout/AppShell.tsx`
 * subscribes to that store field and calls `i18n.changeLanguage`.
 */
import i18n from "i18next";
import { initReactI18next } from "react-i18next";
import en from "./locales/en/common.json";
import zhHant from "./locales/zh-Hant/common.json";

export type SupportedLanguage = "en" | "zh-Hant";

void i18n.use(initReactI18next).init({
  resources: {
    en: { common: en },
    "zh-Hant": { common: zhHant },
  },
  lng: "en", // AppShell syncs the persisted/detected choice immediately on mount.
  fallbackLng: "en",
  defaultNS: "common",
  interpolation: { escapeValue: false }, // React already escapes.
  react: { useSuspense: false },
});

export default i18n;
