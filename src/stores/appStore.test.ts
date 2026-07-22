import { describe, it, expect, beforeEach } from "vitest";
import { useAppStore } from "./appStore";
import { DEFAULT_THEME } from "../theme/themes";

const STORAGE_KEY = "pcpv-app-store";

beforeEach(() => {
  localStorage.clear();
  useAppStore.setState({
    activeRoute: "dashboard",
    theme: DEFAULT_THEME,
    settingsOpen: false,
  });
});

describe("appStore", () => {
  it("defaults to the dashboard route and the default theme", () => {
    const s = useAppStore.getState();
    expect(s.activeRoute).toBe("dashboard");
    expect(s.theme).toBe(DEFAULT_THEME);
    expect(s.settingsOpen).toBe(false);
  });

  it("setActiveRoute and setTheme update state", () => {
    useAppStore.getState().setActiveRoute("network");
    useAppStore.getState().setTheme("ember");
    expect(useAppStore.getState().activeRoute).toBe("network");
    expect(useAppStore.getState().theme).toBe("ember");
  });

  it("toggleSettings flips, and accepts an explicit value", () => {
    useAppStore.getState().toggleSettings();
    expect(useAppStore.getState().settingsOpen).toBe(true);
    useAppStore.getState().toggleSettings();
    expect(useAppStore.getState().settingsOpen).toBe(false);
    useAppStore.getState().toggleSettings(true);
    expect(useAppStore.getState().settingsOpen).toBe(true);
  });

  it("persists the UI slice to localStorage", () => {
    useAppStore.getState().setTheme("aurora");
    useAppStore.getState().setActiveRoute("diagnostics");
    const raw = localStorage.getItem(STORAGE_KEY);
    expect(raw).toBeTruthy();
    const parsed = JSON.parse(raw as string);
    expect(parsed.state.theme).toBe("aurora");
    expect(parsed.state.activeRoute).toBe("diagnostics");
  });
});
