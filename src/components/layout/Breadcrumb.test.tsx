import { describe, it, expect, beforeEach } from "vitest";
import { render, screen } from "@testing-library/react";
import Breadcrumb from "./Breadcrumb";
import { useAppStore, type RouteId } from "../../stores/appStore";
import i18n from "../../i18n";

beforeEach(async () => {
  useAppStore.setState({ activeRoute: "dashboard" });
  await i18n.changeLanguage("en");
});

describe("Breadcrumb", () => {
  it("shows the active route's label", () => {
    render(<Breadcrumb />);
    expect(screen.getByTestId("breadcrumb-current")).toHaveTextContent("Dashboard");
  });

  /// Every `RouteId` must have a label — a new route added to the union
  /// without a `LABEL_KEYS` entry is a TypeScript error, but a route whose
  /// key has no *translation* only shows up at runtime as the raw key
  /// string. This catches that.
  it.each<[RouteId, string]>([
    ["dashboard", "Dashboard"],
    ["network", "Network"],
    ["diagnostics", "Diagnostics"],
    ["minecraft", "Minecraft"],
  ])("renders a real translated label for %s", (route, expected) => {
    useAppStore.setState({ activeRoute: route });
    render(<Breadcrumb />);
    const crumb = screen.getByTestId("breadcrumb-current");
    expect(crumb).toHaveTextContent(expected);
    // A missing translation renders the key itself ("nav.network").
    expect(crumb.textContent).not.toContain("nav.");
  });

  it("follows a store route change", () => {
    const { rerender } = render(<Breadcrumb />);
    expect(screen.getByTestId("breadcrumb-current")).toHaveTextContent("Dashboard");

    useAppStore.setState({ activeRoute: "minecraft" });
    rerender(<Breadcrumb />);

    expect(screen.getByTestId("breadcrumb-current")).toHaveTextContent("Minecraft");
  });
});
