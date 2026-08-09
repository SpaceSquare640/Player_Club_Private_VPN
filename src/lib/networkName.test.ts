import { describe, it, expect } from "vitest";
import { generateNetworkName } from "./networkName";

describe("generateNetworkName", () => {
  it("produces an adjective-noun-NN shaped name", () => {
    expect(generateNetworkName()).toMatch(/^[a-z]+-[a-z]+-\d{2}$/);
  });

  it("varies across calls (not hardcoded to one value)", () => {
    const names = new Set(Array.from({ length: 20 }, () => generateNetworkName()));
    expect(names.size).toBeGreaterThan(1);
  });
});
