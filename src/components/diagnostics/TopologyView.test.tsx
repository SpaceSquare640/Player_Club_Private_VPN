import { describe, it, expect, beforeEach, vi } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";

const { invokeMock } = vi.hoisted(() => ({ invokeMock: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke: invokeMock }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn(async () => () => {}) }));

import TopologyView from "./TopologyView";
import { useTelemetryStore } from "../../stores/telemetryStore";
import type { ConnectionInfo } from "../../types/telemetry";

const pristineTelemetry = useTelemetryStore.getState();

function wireConnection(conn: ConnectionInfo) {
  invokeMock.mockImplementation((cmd: string) =>
    cmd === "get_connection" ? Promise.resolve(conn) : Promise.resolve(undefined),
  );
}

beforeEach(() => {
  invokeMock.mockReset();
  useTelemetryStore.setState(pristineTelemetry, true);
});

describe("TopologyView", () => {
  it("shows placeholders before an identity or a peer exists", async () => {
    wireConnection({ role: "idle", localCandidateCount: 0, link: "idle", peer: null });
    render(<TopologyView />);
    await waitFor(() => expect(invokeMock).toHaveBeenCalledWith("get_connection"));

    expect(screen.getByTestId("topology-this-node")).toHaveTextContent("—");
    expect(screen.getByTestId("topology-peer")).toHaveTextContent("No peer negotiated");
    expect(screen.getByTestId("topology-link-label")).toHaveTextContent("Idle");
  });

  it("shows the peer address and the live RTT once connected", async () => {
    wireConnection({
      role: "initiator",
      localCandidateCount: 2,
      link: "connected",
      peer: { peerAddress: "PC-AAAA-BBBB-CCCC-DDDD", candidateCount: 3 },
    });
    useTelemetryStore.setState({
      identity: { peerAddress: "PC-1234-5678-9ABC-DEF0", publicKeyB64: "cHVia2V5" },
      snapshot: {
        state: "connected",
        rttMs: 42,
        jitterMs: 1,
        lossPct: 0,
        txKbps: 0,
        rxKbps: 0,
        peers: 1,
        fecRecovered: 0,
        policyBlocked: 0,
      },
    });

    render(<TopologyView />);
    await waitFor(() =>
      expect(screen.getByTestId("topology-link-label")).toHaveTextContent("Connected"),
    );

    expect(screen.getByTestId("topology-this-node")).toHaveTextContent("PC-1234-5678-9ABC-DEF0");
    expect(screen.getByTestId("topology-peer")).toHaveTextContent("PC-AAAA-BBBB-CCCC-DDDD");
    expect(screen.getByText("42 ms")).toBeInTheDocument();
  });

  it("labels a failed link distinctly from idle or connecting", async () => {
    wireConnection({
      role: "initiator",
      localCandidateCount: 2,
      link: "failed",
      peer: { peerAddress: "PC-AAAA-BBBB-CCCC-DDDD", candidateCount: 3 },
    });
    render(<TopologyView />);
    await waitFor(() =>
      expect(screen.getByTestId("topology-link-label")).toHaveTextContent("Failed"),
    );
  });
});
