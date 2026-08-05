import { describe, it, expect, beforeEach } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import Minecraft from "./Minecraft";
import { useAppStore } from "../stores/appStore";
import { DEFAULT_CONNECTION_SETTINGS } from "../types/telemetry";

beforeEach(() => {
  useAppStore.setState({
    forwardBroadcast: DEFAULT_CONNECTION_SETTINGS.forwardBroadcast,
    forwardMulticast: DEFAULT_CONNECTION_SETTINGS.forwardMulticast,
    fecParityShards: DEFAULT_CONNECTION_SETTINGS.fecParityShards,
  });
});

describe("Minecraft page", () => {
  it("shows the current connection settings", () => {
    useAppStore.setState({ forwardBroadcast: false, forwardMulticast: true, fecParityShards: 3 });
    render(<Minecraft />);

    expect(screen.getByTestId("minecraft-summary")).toBeInTheDocument();
    expect(screen.getByText("r = 3")).toBeInTheDocument();
  });

  it("applying the preset updates all three settings in one click", () => {
    useAppStore.setState({ forwardBroadcast: false, forwardMulticast: false, fecParityShards: 1 });
    render(<Minecraft />);

    fireEvent.click(screen.getByTestId("minecraft-apply-preset"));

    expect(useAppStore.getState().forwardBroadcast).toBe(true);
    expect(useAppStore.getState().forwardMulticast).toBe(true);
    expect(useAppStore.getState().fecParityShards).toBe(2);
  });

  it("disables the button once the preset is already applied", () => {
    useAppStore.setState({ forwardBroadcast: true, forwardMulticast: true, fecParityShards: 2 });
    render(<Minecraft />);

    expect(screen.getByTestId("minecraft-apply-preset")).toBeDisabled();
  });
});
