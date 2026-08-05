import { describe, it, expect } from "vitest";
import en from "./locales/en/common.json";
import zhHant from "./locales/zh-Hant/common.json";
import zhHans from "./locales/zh-Hans/common.json";

const LOCALES: Record<string, unknown> = { en, "zh-Hant": zhHant, "zh-Hans": zhHans };

/** Recursively collect every leaf key path, e.g. "diagnostics.state.idle". */
function keyPaths(obj: unknown, prefix = ""): string[] {
  if (typeof obj !== "object" || obj === null) return [prefix];
  return Object.entries(obj as Record<string, unknown>).flatMap(([k, v]) =>
    keyPaths(v, prefix ? `${prefix}.${k}` : k),
  );
}

describe("i18n locale key parity", () => {
  // The real regression this guards against: a new UI string gets added to
  // one locale and used with `t()`, but another locale file is never
  // updated — that locale's UI would silently fall back to the raw key
  // string at runtime. Checked pairwise across every bundled locale so
  // adding a new one automatically gets covered.
  const names = Object.keys(LOCALES);
  for (let i = 0; i < names.length; i++) {
    for (let j = i + 1; j < names.length; j++) {
      const [nameA, nameB] = [names[i], names[j]];
      it(`${nameA} and ${nameB} expose exactly the same set of keys`, () => {
        const keysA = new Set(keyPaths(LOCALES[nameA]));
        const keysB = new Set(keyPaths(LOCALES[nameB]));

        const missingInB = [...keysA].filter((k) => !keysB.has(k));
        const missingInA = [...keysB].filter((k) => !keysA.has(k));

        expect(missingInB, `keys present in ${nameA} but missing from ${nameB}`).toEqual([]);
        expect(missingInA, `keys present in ${nameB} but missing from ${nameA}`).toEqual([]);
      });
    }
  }

  it("no locale has an empty string value", () => {
    const check = (obj: unknown, path = ""): string[] => {
      if (typeof obj === "string") return obj.trim() === "" ? [path] : [];
      if (typeof obj !== "object" || obj === null) return [];
      return Object.entries(obj as Record<string, unknown>).flatMap(([k, v]) =>
        check(v, path ? `${path}.${k}` : k),
      );
    };
    for (const [name, locale] of Object.entries(LOCALES)) {
      expect(check(locale), `empty values in ${name}`).toEqual([]);
    }
  });
});
