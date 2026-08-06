import { describe, it, expect, beforeEach, vi } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";

// Mock the Tauri IPC bridge so the page can be driven without a backend.
const { invokeMock } = vi.hoisted(() => ({ invokeMock: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke: invokeMock }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn(async () => () => {}) }));

import Network from "./Network";
import { useTelemetryStore } from "../stores/telemetryStore";
import type { ConnectionPeer } from "../types/telemetry";

/** Route `invoke` by command name, optionally advertising a negotiated peer. */
function wireInvoke(peer: ConnectionPeer | null = null) {
  invokeMock.mockImplementation((cmd: string) => {
    switch (cmd) {
      case "get_connection":
        return Promise.resolve({
          role: peer ? "responder" : "idle",
          localCandidateCount: 2,
          link: "idle",
          peer,
        });
      case "create_offer":
        return Promise.resolve("PCPV1.OFFER.abc.123");
      case "accept_offer":
        return Promise.resolve("PCPV1.ANSWER.def.456");
      default:
        return Promise.resolve(undefined);
    }
  });
}

beforeEach(() => {
  invokeMock.mockReset();
  wireInvoke();
  useTelemetryStore.setState({
    running: false,
    state: "idle",
    notice: null,
    identity: { peerAddress: "PC-1234-5678-9ABC-DEF0", publicKeyB64: "cHVia2V5" },
  });
});

describe("Network page", () => {
  it("renders the node identity and the peer-connection panel", () => {
    render(<Network />);
    expect(screen.getByTestId("page-network")).toBeInTheDocument();
    expect(screen.getByTestId("peer-address")).toHaveTextContent("PC-1234-5678-9ABC-DEF0");
    expect(screen.getByTestId("peer-connection")).toBeInTheDocument();
  });

  it("creates an offer and shows the blob", async () => {
    render(<Network />);
    fireEvent.click(screen.getByTestId("create-offer-btn"));
    await waitFor(() => expect(invokeMock).toHaveBeenCalledWith("create_offer"));
    const blob = await screen.findByTestId("offer-blob");
    expect(blob).toHaveValue("PCPV1.OFFER.abc.123");
  });

  it("processes a pasted offer into an answer, mapping blob -> blobStr", async () => {
    render(<Network />);
    fireEvent.change(screen.getByTestId("peer-input"), {
      target: { value: "PCPV1.OFFER.xyz.999" },
    });
    fireEvent.click(screen.getByTestId("process-btn"));
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith("accept_offer", {
        blobStr: "PCPV1.OFFER.xyz.999",
      }),
    );
    expect(await screen.findByTestId("answer-blob")).toHaveValue("PCPV1.ANSWER.def.456");
  });

  it("disables Connect until a peer is negotiated", async () => {
    render(<Network />);
    // get_connection runs on mount; with no peer, Connect stays disabled.
    await waitFor(() => expect(invokeMock).toHaveBeenCalledWith("get_connection"));
    expect(screen.getByTestId("connect-btn")).toBeDisabled();
  });

  it("enables Connect once a peer has been negotiated", async () => {
    wireInvoke({ peerAddress: "PC-AAAA-BBBB-CCCC-DDDD", candidateCount: 3 });
    render(<Network />);
    await waitFor(() =>
      expect(screen.getByTestId("connect-btn")).not.toBeDisabled(),
    );
    expect(screen.getByTestId("conn-status")).toHaveTextContent("PC-AAAA-BBBB-CCCC-DDDD");
  });

  describe("mode tabs", () => {
    it("shows the manual panel by default and switches to the virtual-network panel", async () => {
      render(<Network />);
      expect(screen.getByTestId("peer-connection")).toBeInTheDocument();
      expect(screen.queryByTestId("vn-create-btn")).not.toBeInTheDocument();

      fireEvent.click(screen.getByTestId("network-mode-virtual"));

      expect(screen.queryByTestId("peer-connection")).not.toBeInTheDocument();
      await waitFor(() => expect(invokeMock).toHaveBeenCalledWith("get_network_status"));
      // Network page's panel collapses its forms by default (see
      // VirtualNetworkPanel's "collapseFormsByDefault" behaviour tests) —
      // the hint is what should show up here, not the forms themselves.
      expect(screen.getByTestId("vn-collapsed-hint")).toBeInTheDocument();
    });

    it("switching back to manual restores the peer-connection panel", () => {
      render(<Network />);
      fireEvent.click(screen.getByTestId("network-mode-virtual"));
      fireEvent.click(screen.getByTestId("network-mode-manual"));
      expect(screen.getByTestId("peer-connection")).toBeInTheDocument();
    });
  });
});
