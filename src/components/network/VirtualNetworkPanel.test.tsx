import { describe, it, expect, beforeEach, vi } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";

const { invokeMock } = vi.hoisted(() => ({ invokeMock: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke: invokeMock }));

import VirtualNetworkPanel from "./VirtualNetworkPanel";
import { useSavedNetworksStore } from "../../stores/savedNetworksStore";
import { DEFAULT_CONNECTION_SETTINGS, type NetworkStatus } from "../../types/telemetry";

function wireInvoke(networks: NetworkStatus[] = []) {
  invokeMock.mockImplementation((cmd: string) => {
    switch (cmd) {
      case "get_network_statuses":
        return Promise.resolve(networks);
      case "create_network":
        return Promise.resolve("net-created");
      case "join_network":
        return Promise.resolve("net-joined");
      case "leave_network":
        return Promise.resolve(undefined);
      default:
        return Promise.resolve(undefined);
    }
  });
}

beforeEach(() => {
  invokeMock.mockReset();
  wireInvoke([]);
  localStorage.clear();
  useSavedNetworksStore.setState({ networks: [] });
});

describe("VirtualNetworkPanel — not in any network", () => {
  it("shows create and join forms", async () => {
    render(<VirtualNetworkPanel />);
    await waitFor(() => expect(invokeMock).toHaveBeenCalledWith("get_network_statuses"));
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
        gameTag: null,
        settings: DEFAULT_CONNECTION_SETTINGS,
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
        gameTag: null,
        settings: DEFAULT_CONNECTION_SETTINGS,
      }),
    );
  });

  it("passes a fixed gameTag and settings through to create_network", async () => {
    const minecraftSettings = { forwardBroadcast: true, forwardMulticast: true, fecParityShards: 2, extraRoutes: [] };
    render(<VirtualNetworkPanel gameTag="minecraft" settings={minecraftSettings} />);
    fireEvent.change(screen.getByTestId("vn-create-name"), { target: { value: "party" } });
    fireEvent.change(screen.getByTestId("vn-create-password"), { target: { value: "secret" } });

    fireEvent.click(screen.getByTestId("vn-create-btn"));

    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith(
        "create_network",
        expect.objectContaining({ gameTag: "minecraft", settings: minecraftSettings }),
      ),
    );
  });

  it("shows an inline error when create_network rejects", async () => {
    invokeMock.mockImplementation((cmd: string) => {
      if (cmd === "create_network") return Promise.reject(new Error("bind failed"));
      if (cmd === "get_network_statuses") return Promise.resolve([]);
      return Promise.resolve(undefined);
    });
    render(<VirtualNetworkPanel />);
    fireEvent.change(screen.getByTestId("vn-create-name"), { target: { value: "party" } });
    fireEvent.change(screen.getByTestId("vn-create-password"), { target: { value: "secret" } });

    fireEvent.click(screen.getByTestId("vn-create-btn"));

    expect(await screen.findByTestId("vn-error")).toHaveTextContent("bind failed");
  });
});

describe("VirtualNetworkPanel — collapseFormsByDefault", () => {
  it("shows a hint instead of the forms when not in any network", async () => {
    render(<VirtualNetworkPanel collapseFormsByDefault />);
    await waitFor(() => expect(invokeMock).toHaveBeenCalledWith("get_network_statuses"));

    expect(screen.getByTestId("vn-collapsed-hint")).toBeInTheDocument();
    expect(screen.queryByTestId("vn-create-btn")).not.toBeInTheDocument();
    expect(screen.queryByTestId("vn-join-btn")).not.toBeInTheDocument();
  });

  it("reveals the general-purpose forms when expanded", async () => {
    render(<VirtualNetworkPanel collapseFormsByDefault />);
    await waitFor(() => expect(invokeMock).toHaveBeenCalledWith("get_network_statuses"));

    fireEvent.click(screen.getByTestId("vn-expand-general-forms"));

    expect(screen.getByTestId("vn-create-btn")).toBeInTheDocument();
    expect(screen.getByTestId("vn-join-btn")).toBeInTheDocument();
  });

  it("shows the active-network card (not the hint) once in a network", async () => {
    wireInvoke([
      { id: "net-1", networkName: "party", isHost: true, hostAddr: "127.0.0.1:54321", gameTag: null, members: [] },
    ]);
    render(<VirtualNetworkPanel collapseFormsByDefault />);

    expect(await screen.findByTestId("virtual-network-active")).toBeInTheDocument();
    expect(screen.queryByTestId("vn-collapsed-hint")).not.toBeInTheDocument();
  });

  it("does not collapse by default when the prop is omitted", async () => {
    render(<VirtualNetworkPanel />);
    await waitFor(() => expect(invokeMock).toHaveBeenCalledWith("get_network_statuses"));

    expect(screen.getByTestId("vn-create-btn")).toBeInTheDocument();
    expect(screen.queryByTestId("vn-collapsed-hint")).not.toBeInTheDocument();
  });
});

describe("VirtualNetworkPanel — game tag badge", () => {
  it("shows a friendly label for a known game tag", async () => {
    wireInvoke([
      {
        id: "net-1",
        networkName: "party",
        isHost: true,
        hostAddr: "127.0.0.1:54321",
        gameTag: "minecraft",
        members: [],
      },
    ]);
    render(<VirtualNetworkPanel />);
    expect(await screen.findByTestId("vn-game-tag")).toHaveTextContent("Minecraft");
  });

  it("falls back to the raw tag for an unrecognized value", async () => {
    wireInvoke([
      {
        id: "net-1",
        networkName: "party",
        isHost: true,
        hostAddr: "127.0.0.1:54321",
        gameTag: "some-future-game",
        members: [],
      },
    ]);
    render(<VirtualNetworkPanel />);
    expect(await screen.findByTestId("vn-game-tag")).toHaveTextContent("some-future-game");
  });

  it("shows no badge when the network has no game tag", async () => {
    wireInvoke([
      { id: "net-1", networkName: "party", isHost: true, hostAddr: "127.0.0.1:54321", gameTag: null, members: [] },
    ]);
    render(<VirtualNetworkPanel />);
    await screen.findByTestId("vn-network-name");
    expect(screen.queryByTestId("vn-game-tag")).not.toBeInTheDocument();
  });
});

describe("VirtualNetworkPanel — in a network", () => {
  const status: NetworkStatus = {
    id: "net-1",
    networkName: "party",
    isHost: true,
    hostAddr: "127.0.0.1:54321",
    gameTag: null,
    members: [
      { pubkey: "pkA", fingerprint: "PC-AAAA-AAAA-AAAA-AAAA", link: "connected" },
      { pubkey: "pkB", fingerprint: "PC-BBBB-BBBB-BBBB-BBBB", link: "connecting" },
    ],
  };

  it("shows the network name, host badge, address, and member list", async () => {
    wireInvoke([status]);
    render(<VirtualNetworkPanel />);

    expect(await screen.findByTestId("vn-network-name")).toHaveTextContent("party");
    expect(screen.getByTestId("vn-host-addr")).toHaveTextContent("127.0.0.1:54321");
    expect(screen.getAllByTestId("vn-member")).toHaveLength(2);
    expect(screen.getByText("PC-AAAA-AAAA-AAAA-AAAA")).toBeInTheDocument();
    expect(screen.getByText("PC-BBBB-BBBB-BBBB-BBBB")).toBeInTheDocument();
  });

  it("shows a placeholder when no other members have joined yet", async () => {
    wireInvoke([{ ...status, members: [] }]);
    render(<VirtualNetworkPanel />);
    await waitFor(() => expect(screen.getByTestId("vn-member-list")).toBeInTheDocument());
    expect(screen.queryByTestId("vn-member")).not.toBeInTheDocument();
  });

  it("leaves the network on click", async () => {
    wireInvoke([status]);
    render(<VirtualNetworkPanel />);
    await screen.findByTestId("vn-leave-btn");

    fireEvent.click(screen.getByTestId("vn-leave-btn"));

    await waitFor(() => expect(invokeMock).toHaveBeenCalledWith("leave_network", { networkId: "net-1" }));
  });

  it("still shows the create/join forms alongside an active network", async () => {
    wireInvoke([status]);
    render(<VirtualNetworkPanel />);

    await screen.findByTestId("virtual-network-active");
    expect(screen.getByTestId("vn-create-btn")).toBeInTheDocument();
    expect(screen.getByTestId("vn-join-btn")).toBeInTheDocument();
  });
});

describe("VirtualNetworkPanel — multiple simultaneous networks", () => {
  it("renders one card per active network, each with its own leave button", async () => {
    wireInvoke([
      { id: "net-1", networkName: "party-a", isHost: true, hostAddr: "127.0.0.1:1111", gameTag: null, members: [] },
      { id: "net-2", networkName: "party-b", isHost: false, hostAddr: "127.0.0.1:2222", gameTag: null, members: [] },
    ]);
    render(<VirtualNetworkPanel />);

    const cards = await screen.findAllByTestId("virtual-network-active");
    expect(cards).toHaveLength(2);
    expect(screen.getByText("party-a")).toBeInTheDocument();
    expect(screen.getByText("party-b")).toBeInTheDocument();
    expect(screen.getAllByTestId("vn-leave-btn")).toHaveLength(2);
  });

  it("leaves only the targeted network", async () => {
    wireInvoke([
      { id: "net-1", networkName: "party-a", isHost: true, hostAddr: "127.0.0.1:1111", gameTag: null, members: [] },
      { id: "net-2", networkName: "party-b", isHost: false, hostAddr: "127.0.0.1:2222", gameTag: null, members: [] },
    ]);
    render(<VirtualNetworkPanel />);

    await screen.findAllByTestId("virtual-network-active");
    fireEvent.click(screen.getAllByTestId("vn-leave-btn")[0]);

    await waitFor(() => expect(invokeMock).toHaveBeenCalledWith("leave_network", { networkId: "net-1" }));
  });
});

describe("VirtualNetworkPanel — saved networks", () => {
  it("remembers a network after creating it, and it survives a remount", async () => {
    const { unmount } = render(<VirtualNetworkPanel />);
    fireEvent.change(screen.getByTestId("vn-create-name"), { target: { value: "party" } });
    fireEvent.change(screen.getByTestId("vn-create-password"), { target: { value: "secret" } });
    fireEvent.click(screen.getByTestId("vn-create-btn"));
    await waitFor(() => expect(invokeMock).toHaveBeenCalledWith("create_network", expect.anything()));
    unmount();

    render(<VirtualNetworkPanel />);
    expect(await screen.findByTestId("vn-saved-list")).toBeInTheDocument();
    expect(screen.getByText("party")).toBeInTheDocument();
  });

  it("quick-starts a saved network without needing the form filled in", async () => {
    useSavedNetworksStore.getState().remember({
      mode: "join",
      networkName: "old-party",
      password: "secret",
      hostAddr: "192.168.1.5:7777",
      gameTag: null,
    });
    render(<VirtualNetworkPanel />);

    fireEvent.click(await screen.findByTestId("vn-saved-start-btn"));

    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith("join_network", {
        hostAddr: "192.168.1.5:7777",
        networkName: "old-party",
        password: "secret",
        gameTag: null,
        settings: DEFAULT_CONNECTION_SETTINGS,
      }),
    );
  });

  it("forgets a saved network on click", async () => {
    useSavedNetworksStore.getState().remember({
      mode: "create",
      networkName: "old-party",
      password: "secret",
      bindAddr: "0.0.0.0:0",
      gameTag: null,
    });
    render(<VirtualNetworkPanel />);

    fireEvent.click(await screen.findByTestId("vn-saved-forget-btn"));

    expect(screen.queryByTestId("vn-saved-list")).not.toBeInTheDocument();
    expect(useSavedNetworksStore.getState().networks).toHaveLength(0);
  });
});
