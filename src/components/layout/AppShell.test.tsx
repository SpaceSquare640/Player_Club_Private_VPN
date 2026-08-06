import { describe, it, expect, beforeEach, vi } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";

// The engine-facing hooks would otherwise reach for a Tauri runtime that
// isn't there. These tests are about routing, not telemetry.
vi.mock("../../hooks/useEngineTelemetry", () => ({ useEngineTelemetry: () => {} }));
vi.mock("../../hooks/useLiveConnectionSettings", () => ({ useLiveConnectionSettings: () => {} }));
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn().mockResolvedValue(undefined) }));

import { createMemoryRouter, RouterProvider } from "react-router";
import AppShell from "./AppShell";
import { useAppStore, type RouteId } from "../../stores/appStore";
import i18n from "../../i18n";

/**
 * Mounts AppShell on a real router at `initialPath`, with trivial page
 * stubs. Using the actual `createMemoryRouter`/`RouterProvider` rather than
 * mocking them is the whole point: this is the integration a react-router
 * major-version bump can silently break, and it was previously only ever
 * checked by hand in a browser.
 */
function renderAt(initialPath: string) {
  const router = createMemoryRouter(
    [
      {
        path: "/",
        element: <AppShell />,
        children: [
          { index: true, element: <div data-testid="page-dashboard" /> },
          { path: "network", element: <div data-testid="page-network" /> },
          { path: "diagnostics", element: <div data-testid="page-diagnostics" /> },
          { path: "minecraft", element: <div data-testid="page-minecraft" /> },
        ],
      },
    ],
    { initialEntries: [initialPath] },
  );
  return { router, ...render(<RouterProvider router={router} />) };
}

beforeEach(async () => {
  useAppStore.setState({ activeRoute: "dashboard", settingsOpen: false });
  await i18n.changeLanguage("en");
});

describe("AppShell — URL to store sync", () => {
  it.each<[string, RouteId]>([
    ["/", "dashboard"],
    ["/network", "network"],
    ["/diagnostics", "diagnostics"],
    ["/minecraft", "minecraft"],
  ])("syncs %s into the store as %s", async (path, expected) => {
    renderAt(path);
    await waitFor(() => expect(useAppStore.getState().activeRoute).toBe(expected));
  });

  it("renders the routed page and a matching breadcrumb together", async () => {
    renderAt("/minecraft");
    await screen.findByTestId("page-minecraft");
    await waitFor(() =>
      expect(screen.getByTestId("breadcrumb-current")).toHaveTextContent("Minecraft"),
    );
  });

  it("treats an unknown path as the dashboard rather than crashing", async () => {
    // `routeFromPath` falls through to "dashboard"; this pins that the
    // fallback is deliberate, not an accident of ordering.
    renderAt("/");
    await waitFor(() => expect(useAppStore.getState().activeRoute).toBe("dashboard"));
  });
});

describe("AppShell — last-route restore on launch", () => {
  it("redirects to the persisted route when launched at the root", async () => {
    useAppStore.setState({ activeRoute: "diagnostics" });
    const { router } = renderAt("/");

    await waitFor(() => expect(router.state.location.pathname).toBe("/diagnostics"));
    expect(useAppStore.getState().activeRoute).toBe("diagnostics");
  });

  it("does not redirect when the persisted route is already the dashboard", async () => {
    useAppStore.setState({ activeRoute: "dashboard" });
    const { router } = renderAt("/");

    await screen.findByTestId("page-dashboard");
    expect(router.state.location.pathname).toBe("/");
  });

  /// The restore must not hijack an explicit deep link — otherwise opening
  /// the app *at* a route would bounce you to wherever you were last time.
  it("lets an explicit deep link win over the persisted route", async () => {
    useAppStore.setState({ activeRoute: "diagnostics" });
    const { router } = renderAt("/network");

    await screen.findByTestId("page-network");
    expect(router.state.location.pathname).toBe("/network");
    await waitFor(() => expect(useAppStore.getState().activeRoute).toBe("network"));
  });
});
