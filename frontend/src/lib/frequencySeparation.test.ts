import { describe, expect, it } from "vitest";

import { applyFrequencyView } from "./frequencySeparation";

function flatGray(width: number, height: number, value: number): Uint8Array {
  const pixels = new Uint8Array(width * height * 4);
  for (let i = 0; i < width * height; i++) {
    pixels[i * 4] = value;
    pixels[i * 4 + 1] = value;
    pixels[i * 4 + 2] = value;
    pixels[i * 4 + 3] = 255;
  }
  return pixels;
}

describe("applyFrequencyView", () => {
  it("returns the input unchanged in Normal mode", () => {
    const pixels = flatGray(8, 8, 120);
    expect(applyFrequencyView(pixels, 8, 8, "Normal")).toBe(pixels);
  });

  it("keeps a flat image's low-frequency view at the same value", () => {
    const pixels = flatGray(16, 16, 100);
    const low = applyFrequencyView(pixels, 16, 16, "LowFrequency");
    expect(low[0]).toBe(100);
    expect(low[1]).toBe(100);
    expect(low[2]).toBe(100);
  });

  it("shows a flat image's high-frequency view as neutral mid-gray", () => {
    const pixels = flatGray(16, 16, 200);
    const high = applyFrequencyView(pixels, 16, 16, "HighFrequency");
    expect(high[0]).toBe(128);
    expect(high[1]).toBe(128);
    expect(high[2]).toBe(128);
  });

  it("smooths a sharp single-pixel spike in the low-frequency view", () => {
    const size = 20;
    const pixels = flatGray(size, size, 50);
    const centerIdx = (10 * size + 10) * 4;
    pixels[centerIdx] = 250;
    pixels[centerIdx + 1] = 250;
    pixels[centerIdx + 2] = 250;

    const low = applyFrequencyView(pixels, size, size, "LowFrequency");
    expect(low[centerIdx]).toBeLessThan(250);
    expect(low[centerIdx]).toBeGreaterThan(50);
  });
});
