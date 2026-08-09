/**
 * Links out to the project's GitHub Wiki / repo docs, kept in one place so
 * every in-app entry point (Settings' About & Legal section, the Virtual
 * Network panel's guide links, ...) resolves the same way. Traditional
 * Chinese has translated docs; everything else falls back to English rather
 * than link to a page that doesn't exist.
 */
import type { SupportedLanguage } from "../i18n";

const WIKI_BASE = "https://github.com/SpaceSquare640/Player_Club_Private_VPN/wiki";
const REPO_BASE = "https://github.com/SpaceSquare640/Player_Club_Private_VPN/blob/main";

function hasZhHantDoc(language: SupportedLanguage) {
  return language === "zh-Hant";
}

/** A Wiki page, given its English and zh-Hant slugs (no `.md`, no leading
 * slash). Pass `""` for the wiki's own root/home ("Home" in English isn't a
 * real slug — GitHub just serves the wiki root). */
export function wikiPage(language: SupportedLanguage, enSlug: string, zhHantSlug: string): string {
  const slug = hasZhHantDoc(language) ? zhHantSlug : enSlug;
  return slug ? `${WIKI_BASE}/${slug}` : WIKI_BASE;
}

/** A root-level repo doc (e.g. `TERMS_OF_SERVICE.md`), given its English and zh-Hant filenames. */
export function repoDoc(language: SupportedLanguage, enFile: string, zhHantFile: string): string {
  return `${REPO_BASE}/${hasZhHantDoc(language) ? zhHantFile : enFile}`;
}
