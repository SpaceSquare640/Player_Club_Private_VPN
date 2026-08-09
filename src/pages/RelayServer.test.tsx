import { describe, it, expect, beforeEach, vi } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";

const { invokeMock, getPublicIpMock } = vi.hoisted(() => ({ invokeMock: vi.fn(), getPublicIpMock: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke: invokeMock }));
vi.mock("../lib/publicIp", () => ({ getPublicIp: getPublicIpMock }));

import RelayServer from "./RelayServer";
import { useAppStore } from "../stores/appStore";
import type { RelayHostStatus } from "../types/telemetry";

function wireInvoke(status: RelayHostStatus | null = null) {
  invokeMock.mockImplementation((cmd: string) => {
    switch (cmd) {
      case "get_relay_status":
        return Promise.resolve(status);
      case "start_relay":
        return Promise.resolve(9420);
      case "stop_relay":
        return Promise.resolve(undefined);
      default:
        return Promise.resolve(undefined);
    }
  });
}

beforeEach(() => {
  invokeMock.mockReset();
  wireInvoke(null);
  getPublicIpMock.mockReset();
  getPublicIpMock.mockResolvedValue(null);
  useAppStore.setState({ relayServerAddr: "" });
});

describe("RelayServer page — not hosting", () => {
  it("shows the port field and start button", async () => {
    render(<RelayServer />);
    await waitFor(() => expect(invokeMock).toHaveBeenCalledWith("get_relay_status"));
    expect(screen.getByTestId("relay-server-port-input")).toBeInTheDocument();
    expect(screen.getByTestId("relay-server-start-btn")).toBeInTheDocument();
  });

  it("starts hosting with the entered port", async () => {
    render(<RelayServer />);
    fireEvent.change(screen.getByTestId("relay-server-port-input"), { target: { value: "9420" } });
    fireEvent.click(screen.getByTestId("relay-server-start-btn"));

    await waitFor(() => expect(invokeMock).toHaveBeenCalledWith("start_relay", { port: 9420 }));
  });

  it("shows an inline error when start_relay rejects", async () => {
    invokeMock.mockImplementation((cmd: string) => {
      if (cmd === "start_relay") return Promise.reject(new Error("already hosting a relay — stop it first"));
      if (cmd === "get_relay_status") return Promise.resolve(null);
      return Promise.resolve(undefined);
    });
    render(<RelayServer />);
    fireEvent.click(screen.getByTestId("relay-server-start-btn"));

    expect(await screen.findByTestId("relay-server-error")).toHaveTextContent("already hosting a relay");
  });
});

describe("RelayServer page — hosting", () => {
  const status: RelayHostStatus = { port: 9420, registeredNetworks: ["party"] };

  it("shows the bound port and registered networks", async () => {
    wireInvoke(status);
    render(<RelayServer />);

    expect(await screen.findByTestId("relay-server-port")).toHaveTextContent(":9420");
    expect(screen.getByTestId("relay-server-registered-item")).toHaveTextContent("party");
  });

  it("shows a placeholder when no networks are registered", async () => {
    wireInvoke({ ...status, registeredNetworks: [] });
    render(<RelayServer />);

    await waitFor(() => expect(screen.getByTestId("relay-server-registered-list")).toBeInTheDocument());
    expect(screen.queryByTestId("relay-server-registered-item")).not.toBeInTheDocument();
  });

  it("stops hosting on click", async () => {
    wireInvoke(status);
    render(<RelayServer />);
    await screen.findByTestId("relay-server-stop-btn");

    fireEvent.click(screen.getByTestId("relay-server-stop-btn"));

    await waitFor(() => expect(invokeMock).toHaveBeenCalledWith("stop_relay"));
  });

  it("shows a fallback when the public IP can't be detected", async () => {
    wireInvoke(status);
    render(<RelayServer />);

    expect(await screen.findByTestId("relay-server-reachable-addr")).toHaveTextContent(
      "Couldn't detect your public IP",
    );
  });
});

describe("RelayServer page — public IP detected", () => {
  const status: RelayHostStatus = { port: 9420, registeredNetworks: [] };

  it("shows the reachable address combining the public IP and bound port", async () => {
    wireInvoke(status);
    getPublicIpMock.mockResolvedValue("203.0.113.10");
    render(<RelayServer />);

    expect(await screen.findByTestId("relay-server-reachable-addr")).toHaveTextContent("203.0.113.10:9420");
  });

  it("sets it as the app's own Relay Server setting on click", async () => {
    wireInvoke(status);
    getPublicIpMock.mockResolvedValue("203.0.113.10");
    render(<RelayServer />);
    await screen.findByTestId("relay-server-reachable-addr");

    fireEvent.click(screen.getByTestId("relay-server-use-for-self-btn"));

    expect(useAppStore.getState().relayServerAddr).toBe("203.0.113.10:9420");
  });
});
