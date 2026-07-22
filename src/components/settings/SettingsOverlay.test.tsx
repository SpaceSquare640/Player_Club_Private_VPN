import { describe, it, expect, beforeEach } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import SettingsOverlay from "./SettingsOverlay";
import { useAppStore } from "../../stores/appStore";
import { DEFAULT_THEME } from "../../theme/themes";
import { DEFAULT_CONNECTION_SETTINGS } from "../../types/telemetry";

beforeEach(() => {
  localStorage.clear();
  useAppStore.setState({
    activeRoute: "dashboard",
    theme: DEFAULT_THEME,
    settingsOpen: true,
    forwardBroadcast: DEFAULT_CONNECTION_SETTINGS.forwardBroadcast,
    forwardMulticast: DEFAULT_CONNECTION_SETTINGS.forwardMulticast,
    fecParityShards: DEFAULT_CONNECTION_SETTINGS.fecParityShards,
  });
});

describe("SettingsOverlay — Connection section (Phase B.3)", () => {
  it("renders the connection settings with the historical defaults", () => {
    render(<SettingsOverlay />);
    expect(screen.getByTestId("settings-connection")).toBeInTheDocument();
    expect(screen.getByTestId("settings-forward-broadcast")).toHaveAttribute(
      "aria-pressed",
      "true",
    );
    expect(screen.getByTestId("settings-forward-multicast")).toHaveAttribute(
      "aria-pressed",
      "true",
    );
    expect(screen.getByTestId("settings-fec-1")).toHaveAttribute("aria-pressed", "true");
  });

  it("toggling forward broadcast updates the store", () => {
    render(<SettingsOverlay />);
    fireEvent.click(screen.getByTestId("settings-forward-broadcast"));
    expect(useAppStore.getState().forwardBroadcast).toBe(false);
    expect(screen.getByTestId("settings-forward-broadcast")).toHaveAttribute(
      "aria-pressed",
      "false",
    );
  });

  it("toggling forward multicast updates the store independently", () => {
    render(<SettingsOverlay />);
    fireEvent.click(screen.getByTestId("settings-forward-multicast"));
    expect(useAppStore.getState().forwardMulticast).toBe(false);
    expect(useAppStore.getState().forwardBroadcast).toBe(true); // unaffected
  });

  it("selecting an FEC redundancy option updates the store", () => {
    render(<SettingsOverlay />);
    fireEvent.click(screen.getByTestId("settings-fec-2"));
    expect(useAppStore.getState().fecParityShards).toBe(2);
    expect(screen.getByTestId("settings-fec-2")).toHaveAttribute("aria-pressed", "true");
    expect(screen.getByTestId("settings-fec-1")).toHaveAttribute("aria-pressed", "false");
  });
});
