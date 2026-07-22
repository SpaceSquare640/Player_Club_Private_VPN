import { describe, it, expect, beforeEach } from "vitest";
import { useAppStore } from "./appStore";
import { DEFAULT_THEME } from "../theme/themes";
import { DEFAULT_CONNECTION_SETTINGS } from "../types/telemetry";

const STORAGE_KEY = "pcpv-app-store";

beforeEach(() => {
  localStorage.clear();
  useAppStore.setState({
    activeRoute: "dashboard",
    theme: DEFAULT_THEME,
    settingsOpen: false,
    forwardBroadcast: DEFAULT_CONNECTION_SETTINGS.forwardBroadcast,
    forwardMulticast: DEFAULT_CONNECTION_SETTINGS.forwardMulticast,
    fecParityShards: DEFAULT_CONNECTION_SETTINGS.fecParityShards,
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

  // Phase B.3 — connection settings (split-tunnel forwarding + FEC redundancy).
  it("defaults connection settings to the historical hardcoded behaviour", () => {
    const s = useAppStore.getState();
    expect(s.forwardBroadcast).toBe(true);
    expect(s.forwardMulticast).toBe(true);
    expect(s.fecParityShards).toBe(1);
  });

  it("setForwardBroadcast, setForwardMulticast and setFecParityShards update state", () => {
    useAppStore.getState().setForwardBroadcast(false);
    useAppStore.getState().setForwardMulticast(false);
    useAppStore.getState().setFecParityShards(2);
    const s = useAppStore.getState();
    expect(s.forwardBroadcast).toBe(false);
    expect(s.forwardMulticast).toBe(false);
    expect(s.fecParityShards).toBe(2);
  });

  it("persists connection settings to localStorage", () => {
    useAppStore.getState().setForwardBroadcast(false);
    useAppStore.getState().setFecParityShards(3);
    const parsed = JSON.parse(localStorage.getItem(STORAGE_KEY) as string);
    expect(parsed.state.forwardBroadcast).toBe(false);
    expect(parsed.state.forwardMulticast).toBe(true);
    expect(parsed.state.fecParityShards).toBe(3);
  });
});
