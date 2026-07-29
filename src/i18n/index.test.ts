import { describe, it, expect } from "vitest";
import en from "./locales/en/common.json";
import zhHant from "./locales/zh-Hant/common.json";

/** Recursively collect every leaf key path, e.g. "diagnostics.state.idle". */
function keyPaths(obj: unknown, prefix = ""): string[] {
  if (typeof obj !== "object" || obj === null) return [prefix];
  return Object.entries(obj as Record<string, unknown>).flatMap(([k, v]) =>
    keyPaths(v, prefix ? `${prefix}.${k}` : k),
  );
}

describe("i18n locale key parity", () => {
  // The real regression this guards against: a new UI string gets added to
  // English and used with `t()`, but the zh-Hant file is never updated — the
  // Chinese UI would silently fall back to the raw key string at runtime.
  it("en and zh-Hant expose exactly the same set of keys", () => {
    const enKeys = new Set(keyPaths(en));
    const zhKeys = new Set(keyPaths(zhHant));

    const missingInZh = [...enKeys].filter((k) => !zhKeys.has(k));
    const missingInEn = [...zhKeys].filter((k) => !enKeys.has(k));

    expect(missingInZh, "keys present in en but missing from zh-Hant").toEqual([]);
    expect(missingInEn, "keys present in zh-Hant but missing from en").toEqual([]);
  });

  it("neither locale has an empty string value", () => {
    const check = (obj: unknown, path = ""): string[] => {
      if (typeof obj === "string") return obj.trim() === "" ? [path] : [];
      if (typeof obj !== "object" || obj === null) return [];
      return Object.entries(obj as Record<string, unknown>).flatMap(([k, v]) =>
        check(v, path ? `${path}.${k}` : k),
      );
    };
    expect(check(en)).toEqual([]);
    expect(check(zhHant)).toEqual([]);
  });
});
