import { describe, it, expect, beforeEach, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";

// `useNavigate` is the one thing here that needs the router. Mocking it
// keeps these tests about the sidebar's own behaviour — which path it asks
// for, and how it renders active state — rather than about routing itself
// (AppShell.test.tsx covers the real router integration).
const { navigateMock } = vi.hoisted(() => ({ navigateMock: vi.fn() }));
vi.mock("react-router", () => ({ useNavigate: () => navigateMock }));

import Sidebar from "./Sidebar";
import { useAppStore } from "../../stores/appStore";
import i18n from "../../i18n";

beforeEach(async () => {
  navigateMock.mockReset();
  useAppStore.setState({ activeRoute: "dashboard", settingsOpen: false });
  await i18n.changeLanguage("en");
});

describe("Sidebar", () => {
  it("renders every nav item plus the settings button", () => {
    render(<Sidebar />);
    for (const id of ["dashboard", "network", "diagnostics", "minecraft"]) {
      expect(screen.getByTestId(`nav-${id}`)).toBeInTheDocument();
    }
    expect(screen.getByTestId("nav-settings")).toBeInTheDocument();
  });

  /// The route→path mapping is the thing a router migration can silently
  /// break: the component still renders, the click still fires, but the
  /// path it asks for is wrong. Asserting the exact argument is what makes
  /// this a real guard rather than a smoke test.
  it.each([
    ["dashboard", "/"],
    ["network", "/network"],
    ["diagnostics", "/diagnostics"],
    ["minecraft", "/minecraft"],
  ])("navigates to %s's path", (id, path) => {
    render(<Sidebar />);
    fireEvent.click(screen.getByTestId(`nav-${id}`));
    expect(navigateMock).toHaveBeenCalledWith(path);
  });

  it("marks only the active route, via both aria-current and data-active", () => {
    useAppStore.setState({ activeRoute: "diagnostics" });
    render(<Sidebar />);

    const active = screen.getByTestId("nav-diagnostics");
    expect(active).toHaveAttribute("aria-current", "page");
    expect(active).toHaveAttribute("data-active", "true");

    const inactive = screen.getByTestId("nav-network");
    expect(inactive).not.toHaveAttribute("aria-current");
    expect(inactive).toHaveAttribute("data-active", "false");
  });

  it("opens the settings overlay without navigating", () => {
    render(<Sidebar />);
    fireEvent.click(screen.getByTestId("nav-settings"));
    expect(useAppStore.getState().settingsOpen).toBe(true);
    expect(navigateMock).not.toHaveBeenCalled();
  });

  it("labels every button for screen readers", () => {
    render(<Sidebar />);
    // Translated, not the raw i18n key.
    expect(screen.getByTestId("nav-network")).toHaveAttribute("aria-label", "Network");
    expect(screen.getByTestId("nav-settings")).toHaveAttribute("aria-label", "Settings");
  });
});
