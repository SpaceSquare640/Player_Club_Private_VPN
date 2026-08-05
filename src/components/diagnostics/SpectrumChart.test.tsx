import { describe, it, expect, beforeEach, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import SpectrumChart from "./SpectrumChart";
import { useTelemetryStore } from "../../stores/telemetryStore";

const pristine = useTelemetryStore.getState();

beforeEach(() => {
  useTelemetryStore.setState(pristine, true);
});

describe("SpectrumChart", () => {
  it("shows the empty state with no samples yet", () => {
    render(<SpectrumChart />);
    expect(screen.getByTestId("spectrum-chart")).toBeInTheDocument();
    expect(screen.getByText("No traffic yet")).toBeInTheDocument();
    expect(document.querySelector("svg")).not.toBeInTheDocument();
  });

  it("renders the legend and a chart once samples exist", () => {
    useTelemetryStore.setState({
      spectrumHistory: [
        { txKbps: 10, rxKbps: 5 },
        { txKbps: 20, rxKbps: 15 },
      ],
    });
    render(<SpectrumChart />);
    expect(screen.getByText("TX")).toBeInTheDocument();
    expect(screen.getByText("RX")).toBeInTheDocument();
    expect(document.querySelector("svg")).toBeInTheDocument();
    // Both series must actually have drawn a non-empty path.
    const paths = document.querySelectorAll("path");
    expect(paths.length).toBeGreaterThanOrEqual(2);
    paths.forEach((p) => expect(p.getAttribute("d")).not.toBe(""));
  });

  it("shows a tooltip with the nearest sample's values on hover", () => {
    useTelemetryStore.setState({
      spectrumHistory: [
        { txKbps: 100, rxKbps: 50 },
        { txKbps: 200, rxKbps: 150 },
      ],
    });
    render(<SpectrumChart />);
    const svg = document.querySelector("svg") as SVGSVGElement;

    // happy-dom's getBoundingClientRect is zeroed by default; stub a real one
    // so the pointer -> sample-index math has something to divide against.
    vi.spyOn(svg, "getBoundingClientRect").mockReturnValue({
      left: 0,
      top: 0,
      width: 300,
      height: 100,
      right: 300,
      bottom: 100,
      x: 0,
      y: 0,
      toJSON: () => "",
    });

    fireEvent.mouseMove(svg, { clientX: 299, clientY: 50 });
    const tooltip = screen.getByTestId("spectrum-tooltip");
    expect(tooltip).toHaveTextContent("200");
    expect(tooltip).toHaveTextContent("150");

    fireEvent.mouseLeave(svg);
    expect(screen.queryByTestId("spectrum-tooltip")).not.toBeInTheDocument();
  });
});
