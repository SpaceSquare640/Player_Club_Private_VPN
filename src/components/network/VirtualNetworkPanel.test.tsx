import { describe, it, expect, beforeEach, vi } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";

const { invokeMock } = vi.hoisted(() => ({ invokeMock: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke: invokeMock }));

import VirtualNetworkPanel from "./VirtualNetworkPanel";
import type { NetworkStatus } from "../../types/telemetry";

function wireInvoke(status: NetworkStatus | null = null) {
  invokeMock.mockImplementation((cmd: string) => {
    switch (cmd) {
      case "get_network_status":
        return Promise.resolve(status);
      case "create_network":
        return Promise.resolve("127.0.0.1:54321");
      case "join_network":
        return Promise.resolve(undefined);
      case "leave_network":
        return Promise.resolve(undefined);
      default:
        return Promise.resolve(undefined);
    }
  });
}

beforeEach(() => {
  invokeMock.mockReset();
  wireInvoke(null);
});

describe("VirtualNetworkPanel — not in a network", () => {
  it("shows create and join forms", async () => {
    render(<VirtualNetworkPanel />);
    await waitFor(() => expect(invokeMock).toHaveBeenCalledWith("get_network_status"));
    expect(screen.getByTestId("vn-create-btn")).toBeInTheDocument();
    expect(screen.getByTestId("vn-join-btn")).toBeInTheDocument();
  });

  it("disables Create until a name and password are entered", async () => {
    render(<VirtualNetworkPanel />);
    expect(screen.getByTestId("vn-create-btn")).toBeDisabled();

    fireEvent.change(screen.getByTestId("vn-create-name"), { target: { value: "party" } });
    expect(screen.getByTestId("vn-create-btn")).toBeDisabled();

    fireEvent.change(screen.getByTestId("vn-create-password"), { target: { value: "secret" } });
    expect(screen.getByTestId("vn-create-btn")).not.toBeDisabled();
  });

  it("creates a network with the entered name, password, and bind address", async () => {
    render(<VirtualNetworkPanel />);
    fireEvent.change(screen.getByTestId("vn-create-name"), { target: { value: "party" } });
    fireEvent.change(screen.getByTestId("vn-create-password"), { target: { value: "secret" } });
    fireEvent.change(screen.getByTestId("vn-create-bind-addr"), { target: { value: "0.0.0.0:7777" } });

    fireEvent.click(screen.getByTestId("vn-create-btn"));

    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith("create_network", {
        bindAddr: "0.0.0.0:7777",
        networkName: "party",
        password: "secret",
      }),
    );
  });

  it("disables Join until host address, name, and password are all entered", () => {
    render(<VirtualNetworkPanel />);
    expect(screen.getByTestId("vn-join-btn")).toBeDisabled();

    fireEvent.change(screen.getByTestId("vn-join-host-addr"), { target: { value: "192.168.1.5:7777" } });
    fireEvent.change(screen.getByTestId("vn-join-name"), { target: { value: "party" } });
    expect(screen.getByTestId("vn-join-btn")).toBeDisabled();

    fireEvent.change(screen.getByTestId("vn-join-password"), { target: { value: "secret" } });
    expect(screen.getByTestId("vn-join-btn")).not.toBeDisabled();
  });

  it("joins a network with the entered details", async () => {
    render(<VirtualNetworkPanel />);
    fireEvent.change(screen.getByTestId("vn-join-host-addr"), { target: { value: "192.168.1.5:7777" } });
    fireEvent.change(screen.getByTestId("vn-join-name"), { target: { value: "party" } });
    fireEvent.change(screen.getByTestId("vn-join-password"), { target: { value: "secret" } });

    fireEvent.click(screen.getByTestId("vn-join-btn"));

    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith("join_network", {
        hostAddr: "192.168.1.5:7777",
        networkName: "party",
        password: "secret",
      }),
    );
  });

  it("shows an inline error when create_network rejects", async () => {
    invokeMock.mockImplementation((cmd: string) => {
      if (cmd === "create_network") return Promise.reject(new Error("bind failed"));
      if (cmd === "get_network_status") return Promise.resolve(null);
      return Promise.resolve(undefined);
    });
    render(<VirtualNetworkPanel />);
    fireEvent.change(screen.getByTestId("vn-create-name"), { target: { value: "party" } });
    fireEvent.change(screen.getByTestId("vn-create-password"), { target: { value: "secret" } });

    fireEvent.click(screen.getByTestId("vn-create-btn"));

    expect(await screen.findByTestId("vn-error")).toHaveTextContent("bind failed");
  });
});

describe("VirtualNetworkPanel — in a network", () => {
  const status: NetworkStatus = {
    networkName: "party",
    isHost: true,
    hostAddr: "127.0.0.1:54321",
    members: [
      { pubkey: "pkA", fingerprint: "PC-AAAA-AAAA-AAAA-AAAA", link: "connected" },
      { pubkey: "pkB", fingerprint: "PC-BBBB-BBBB-BBBB-BBBB", link: "connecting" },
    ],
  };

  it("shows the network name, host badge, address, and member list", async () => {
    wireInvoke(status);
    render(<VirtualNetworkPanel />);

    expect(await screen.findByTestId("vn-network-name")).toHaveTextContent("party");
    expect(screen.getByTestId("vn-host-addr")).toHaveTextContent("127.0.0.1:54321");
    expect(screen.getAllByTestId("vn-member")).toHaveLength(2);
    expect(screen.getByText("PC-AAAA-AAAA-AAAA-AAAA")).toBeInTheDocument();
    expect(screen.getByText("PC-BBBB-BBBB-BBBB-BBBB")).toBeInTheDocument();
  });

  it("shows a placeholder when no other members have joined yet", async () => {
    wireInvoke({ ...status, members: [] });
    render(<VirtualNetworkPanel />);
    await waitFor(() => expect(screen.getByTestId("vn-member-list")).toBeInTheDocument());
    expect(screen.queryByTestId("vn-member")).not.toBeInTheDocument();
  });

  it("leaves the network on click", async () => {
    wireInvoke(status);
    render(<VirtualNetworkPanel />);
    await screen.findByTestId("vn-leave-btn");

    fireEvent.click(screen.getByTestId("vn-leave-btn"));

    await waitFor(() => expect(invokeMock).toHaveBeenCalledWith("leave_network"));
  });
});
