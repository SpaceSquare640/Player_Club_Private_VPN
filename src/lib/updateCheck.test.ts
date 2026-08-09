import { describe, it, expect } from "vitest";
import { isNewerVersion } from "./updateCheck";

describe("isNewerVersion", () => {
  it("reports a higher patch version as newer", () => {
    expect(isNewerVersion("0.43.1", "0.43.0")).toBe(true);
  });

  it("reports a higher minor version as newer even with a lower patch", () => {
    expect(isNewerVersion("0.44.0", "0.43.9")).toBe(true);
  });

  it("reports a higher major version as newer even with lower minor/patch", () => {
    expect(isNewerVersion("1.0.0", "0.99.99")).toBe(true);
  });

  it("reports an identical version as not newer", () => {
    expect(isNewerVersion("0.43.1", "0.43.1")).toBe(false);
  });

  it("reports an older version as not newer", () => {
    expect(isNewerVersion("0.43.0", "0.43.1")).toBe(false);
  });

  it("treats a missing patch component as 0", () => {
    expect(isNewerVersion("0.44", "0.43.9")).toBe(true);
    expect(isNewerVersion("0.43", "0.43.0")).toBe(false);
  });
});
