import "@testing-library/jest-dom/vitest";
import i18n from "../i18n";

// Initialize once for every test file. Forcing "en" here means every existing
// component test's string assertions keep matching the exact copy already in
// the codebase — the English JSON values are the same literals those tests
// were written against, so this migration is a non-visual-regression refactor
// by construction, not something that needed rewriting every assertion.
await i18n.changeLanguage("en");

// happy-dom v20 ships a file-backed `localStorage` that is not a functional
// `Storage` in this environment (its methods are undefined). The persisted app
// store needs a real one, so install a deterministic in-memory implementation.
class MemoryStorage implements Storage {
  private map = new Map<string, string>();
  get length() {
    return this.map.size;
  }
  clear() {
    this.map.clear();
  }
  getItem(key: string) {
    return this.map.has(key) ? this.map.get(key)! : null;
  }
  key(index: number) {
    return Array.from(this.map.keys())[index] ?? null;
  }
  removeItem(key: string) {
    this.map.delete(key);
  }
  setItem(key: string, value: string) {
    this.map.set(key, String(value));
  }
}

Object.defineProperty(globalThis, "localStorage", {
  value: new MemoryStorage(),
  writable: true,
  configurable: true,
});
