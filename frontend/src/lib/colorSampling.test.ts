import { describe, expect, it } from "vitest";

import { hueDegreesFromRgbByte } from "./colorSampling";

describe("hueDegreesFromRgbByte", () => {
  it("returns 0 for a neutral gray sample", () => {
    expect(hueDegreesFromRgbByte(128, 128, 128)).toBe(0);
  });

  it("returns 0 for a pure red pixel", () => {
    expect(hueDegreesFromRgbByte(255, 0, 0)).toBeCloseTo(0);
  });

  it("returns ~120 for a pure green pixel", () => {
    expect(hueDegreesFromRgbByte(0, 255, 0)).toBeCloseTo(120);
  });

  it("returns ~240 for a pure blue pixel", () => {
    expect(hueDegreesFromRgbByte(0, 0, 255)).toBeCloseTo(240);
  });

  it("returns ~60 for yellow (red+green)", () => {
    expect(hueDegreesFromRgbByte(255, 255, 0)).toBeCloseTo(60);
  });
});
