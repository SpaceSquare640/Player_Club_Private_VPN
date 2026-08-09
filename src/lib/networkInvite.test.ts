import { describe, it, expect } from "vitest";
import { encodeInvite, decodeInvite } from "./networkInvite";

describe("networkInvite", () => {
  it("round-trips name, password, and host address", () => {
    const invite = { networkName: "party", password: "secret", hostAddr: "192.168.1.5:7777" };
    expect(decodeInvite(encodeInvite(invite))).toEqual(invite);
  });

  it("round-trips values containing special characters (colons, unicode)", () => {
    const invite = { networkName: "我的派對", password: "p:a:s:s", hostAddr: "203.0.113.10:9000" };
    expect(decodeInvite(encodeInvite(invite))).toEqual(invite);
  });

  it("returns null for unrelated clipboard content", () => {
    expect(decodeInvite("just some random text")).toBeNull();
    expect(decodeInvite("")).toBeNull();
  });

  it("returns null for a malformed invite (wrong part count)", () => {
    expect(decodeInvite("pcpv-invite:v1:onlyonepart")).toBeNull();
  });

  it("returns null when a field is empty", () => {
    expect(decodeInvite(encodeInvite({ networkName: "", password: "secret", hostAddr: "1.2.3.4:5" }))).toBeNull();
  });

  it("tolerates surrounding whitespace from a pasted clipboard", () => {
    const invite = { networkName: "party", password: "secret", hostAddr: "192.168.1.5:7777" };
    expect(decodeInvite(`  ${encodeInvite(invite)}  \n`)).toEqual(invite);
  });
});
