import { describe, it, expect, beforeEach } from "vitest";
import { useSavedNetworksStore } from "./savedNetworksStore";

beforeEach(() => {
  localStorage.clear();
  useSavedNetworksStore.setState({ networks: [] });
});

describe("savedNetworksStore", () => {
  it("remembers a newly created network", () => {
    useSavedNetworksStore
      .getState()
      .remember({ mode: "create", networkName: "party", password: "secret", bindAddr: "0.0.0.0:0", gameTag: null });

    const { networks } = useSavedNetworksStore.getState();
    expect(networks).toHaveLength(1);
    expect(networks[0]).toMatchObject({ mode: "create", networkName: "party", bindAddr: "0.0.0.0:0" });
  });

  it("updates the existing entry in place instead of duplicating on a repeat create with the same name/address", () => {
    const { remember } = useSavedNetworksStore.getState();
    remember({ mode: "create", networkName: "party", password: "old", bindAddr: "0.0.0.0:0", gameTag: null });
    remember({ mode: "create", networkName: "party", password: "new", bindAddr: "0.0.0.0:0", gameTag: null });

    const { networks } = useSavedNetworksStore.getState();
    expect(networks).toHaveLength(1);
    expect(networks[0].password).toBe("new");
  });

  it("treats create and join as distinct even with the same name", () => {
    const { remember } = useSavedNetworksStore.getState();
    remember({ mode: "create", networkName: "party", password: "secret", bindAddr: "0.0.0.0:0", gameTag: null });
    remember({ mode: "join", networkName: "party", password: "secret", hostAddr: "192.168.1.5:7777", gameTag: null });

    expect(useSavedNetworksStore.getState().networks).toHaveLength(2);
  });

  it("forgets an entry by id", () => {
    const { remember, forget } = useSavedNetworksStore.getState();
    remember({ mode: "create", networkName: "party", password: "secret", bindAddr: "0.0.0.0:0", gameTag: null });
    const id = useSavedNetworksStore.getState().networks[0].id;

    forget(id);

    expect(useSavedNetworksStore.getState().networks).toHaveLength(0);
  });
});
