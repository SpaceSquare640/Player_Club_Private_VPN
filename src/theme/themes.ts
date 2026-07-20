/**
 * Theme registry. The actual color values live in `src/styles/index.css` under
 * `:root[data-theme="<id>"]` blocks; `ThemeProvider` sets the `data-theme`
 * attribute and the CSS custom properties cascade app-wide. This module holds
 * only the metadata the theme switcher UI needs.
 */
export type ThemeId =
  | "midnight"
  | "carbon"
  | "nebula"
  | "abyss"
  | "aurora"
  | "ember";

export interface ThemeMeta {
  id: ThemeId;
  name: string;
  /** Primary accent, used for the switcher swatch. */
  swatch: string;
}

export const THEMES: ThemeMeta[] = [
  { id: "midnight", name: "Midnight", swatch: "#8b5cf6" },
  { id: "carbon", name: "Carbon", swatch: "#a78bfa" },
  { id: "nebula", name: "Nebula", swatch: "#a855f7" },
  { id: "abyss", name: "Abyss", swatch: "#818cf8" },
  { id: "aurora", name: "Aurora", swatch: "#60a5fa" },
  { id: "ember", name: "Ember", swatch: "#f97316" },
];

export const DEFAULT_THEME: ThemeId = "midnight";
