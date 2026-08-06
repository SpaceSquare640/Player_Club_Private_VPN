import { describe, it, expect, beforeEach, vi } from "vitest";
import { renderHook, act, waitFor } from "@testing-library/react";

const { invokeMock } = vi.hoisted(() => ({ invokeMock: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke: invokeMock }));

import { useLiveConnectionSettings } from "./useLiveConnectionSettings";
import { useAppStore } from "../stores/appStore";
import { DEFAULT_CONNECTION_SETTINGS } from "../types/telemetry";

/** Every `update_connection_settings` call's `settings` payload, in order. */
function pushedSettings() {
  return invokeMock.mock.calls
    .filter(([cmd]) => cmd === "update_connection_settings")
    .map(([, args]) => args.settings);
}

beforeEach(() => {
  invokeMock.mockReset();
  invokeMock.mockResolvedValue(undefined);
  useAppStore.setState({
    forwardBroadcast: DEFAULT_CONNECTION_SETTINGS.forwardBroadcast,
    forwardMulticast: DEFAULT_CONNECTION_SETTINGS.forwardMulticast,
    fecParityShards: DEFAULT_CONNECTION_SETTINGS.fecParityShards,
    extraRoutes: DEFAULT_CONNECTION_SETTINGS.extraRoutes,
  });
});

describe("useLiveConnectionSettings", () => {
  it("pushes the current settings on mount", async () => {
    renderHook(() => useLiveConnectionSettings());
    await waitFor(() => expect(pushedSettings()).toHaveLength(1));
    expect(pushedSettings()[0]).toEqual(DEFAULT_CONNECTION_SETTINGS);
  });

  it("pushes again when a toggle changes", async () => {
    renderHook(() => useLiveConnectionSettings());
    await waitFor(() => expect(pushedSettings()).toHaveLength(1));

    act(() => useAppStore.getState().setForwardBroadcast(false));

    await waitFor(() => expect(pushedSettings()).toHaveLength(2));
    expect(pushedSettings()[1].forwardBroadcast).toBe(false);
  });

  /// The regression this guards: an earlier draft wired this to the Settings
  /// overlay's toggle handlers. That would have silently missed the Minecraft
  /// preset button and JSON profile import, which write the same store fields
  /// directly — so a live link would keep the stale policy after either.
  it("pushes for a store write that did not come from the Settings toggles", async () => {
    renderHook(() => useLiveConnectionSettings());
    await waitFor(() => expect(pushedSettings()).toHaveLength(1));

    // Exactly what the Minecraft preset button does.
    act(() => {
      useAppStore.getState().setForwardBroadcast(true);
      useAppStore.getState().setForwardMulticast(true);
      useAppStore.getState().setFecParityShards(2);
    });

    await waitFor(() => expect(pushedSettings().length).toBeGreaterThan(1));
    const pushed = pushedSettings();
    expect(pushed[pushed.length - 1]).toEqual({
      forwardBroadcast: true,
      forwardMulticast: true,
      fecParityShards: 2,
      extraRoutes: [],
    });
  });

  it("does not push again when nothing actually changed", async () => {
    const { rerender } = renderHook(() => useLiveConnectionSettings());
    await waitFor(() => expect(pushedSettings()).toHaveLength(1));

    rerender();
    rerender();

    expect(pushedSettings()).toHaveLength(1);
  });

  it("swallows a rejected push (no Tauri runtime, or no live link)", async () => {
    invokeMock.mockRejectedValue(new Error("no engine"));
    // An unhandled rejection here would fail the test run, so reaching the
    // assertion at all is the assertion.
    renderHook(() => useLiveConnectionSettings());
    await waitFor(() => expect(pushedSettings()).toHaveLength(1));
  });
});
