import { describe, it, expect } from "vitest";
import {
  buildAreaPath,
  buildLinePath,
  nearestIndex,
  niceMax,
  sampleX,
  sampleY,
} from "./spectrum";

describe("niceMax", () => {
  it("returns a fallback ceiling when every value is zero (idle)", () => {
    expect(niceMax([0, 0, 0])).toBe(10);
    expect(niceMax([])).toBe(10);
  });

  it("adds headroom above the peak rather than exactly matching it", () => {
    const max = niceMax([0, 50, 30]);
    expect(max).toBeGreaterThan(50);
  });

  it("is stable for repeated identical peaks", () => {
    expect(niceMax([100, 100])).toBe(niceMax([100]));
  });
});

describe("sampleX / sampleY", () => {
  it("places a single sample at the origin", () => {
    expect(sampleX(0, 1, 300)).toBe(0);
  });

  it("spreads samples evenly across the width", () => {
    expect(sampleX(0, 3, 300)).toBe(0);
    expect(sampleX(1, 3, 300)).toBe(150);
    expect(sampleX(2, 3, 300)).toBe(300);
  });

  it("maps zero to the baseline and the max to the top", () => {
    expect(sampleY(0, 100, 100)).toBe(100);
    expect(sampleY(100, 100, 100)).toBe(0);
    expect(sampleY(50, 100, 100)).toBe(50);
  });

  it("clamps a value above maxValue to the top rather than overflowing", () => {
    expect(sampleY(150, 100, 100)).toBe(0);
  });

  it("does not divide by zero when maxValue is zero", () => {
    expect(sampleY(0, 0, 100)).toBe(100);
  });
});

describe("buildLinePath / buildAreaPath", () => {
  it("returns an empty path for no samples", () => {
    expect(buildLinePath([], 300, 100, 10)).toBe("");
    expect(buildAreaPath([], 300, 100, 10)).toBe("");
  });

  it("starts with M and continues with L for each subsequent point", () => {
    const path = buildLinePath([1, 2, 3], 300, 100, 10);
    const commands = path.split(" ").map((seg) => seg[0]);
    expect(commands).toEqual(["M", "L", "L"]);
  });

  it("closes the area path down to the baseline and back to the origin", () => {
    const path = buildAreaPath([5, 8], 300, 100, 10);
    expect(path.endsWith("L0,100.00 Z")).toBe(true);
  });
});

describe("nearestIndex", () => {
  it("clamps to the single index when there is only one sample", () => {
    expect(nearestIndex(999, 1, 300)).toBe(0);
  });

  it("rounds a pointer position to the closest sample", () => {
    // 5 samples spread across width 300 -> steps at 0, 75, 150, 225, 300.
    expect(nearestIndex(0, 5, 300)).toBe(0);
    expect(nearestIndex(80, 5, 300)).toBe(1);
    expect(nearestIndex(300, 5, 300)).toBe(4);
  });

  it("clamps out-of-range pointer positions instead of returning an invalid index", () => {
    expect(nearestIndex(-50, 5, 300)).toBe(0);
    expect(nearestIndex(9999, 5, 300)).toBe(4);
  });
});
