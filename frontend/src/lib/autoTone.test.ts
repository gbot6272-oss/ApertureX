import { describe, expect, it } from "vitest";

import { computeAutoTone } from "./autoTone";
import { computeHistogram } from "./histogram";

function makeUniformPixels(byteValue: number, count: number): Uint8ClampedArray {
  const pixels = new Uint8ClampedArray(count * 4);
  for (let i = 0; i < count; i++) {
    pixels[i * 4] = byteValue;
    pixels[i * 4 + 1] = byteValue;
    pixels[i * 4 + 2] = byteValue;
    pixels[i * 4 + 3] = 255;
  }
  return pixels;
}

describe("computeAutoTone", () => {
  it("returns neutral values for an empty histogram", () => {
    const histogram = computeHistogram(new Uint8ClampedArray(0), 0, 0);
    expect(computeAutoTone(histogram)).toEqual({ exposure_ev: 0, contrast: 0 });
  });

  it("brightens a uniformly dark image (raises exposure)", () => {
    const pixels = makeUniformPixels(20, 100);
    const histogram = computeHistogram(pixels, 100, 1);
    const result = computeAutoTone(histogram);
    expect(result.exposure_ev).toBeGreaterThan(0);
  });

  it("darkens a uniformly bright image (lowers exposure)", () => {
    const pixels = makeUniformPixels(230, 100);
    const histogram = computeHistogram(pixels, 100, 1);
    const result = computeAutoTone(histogram);
    expect(result.exposure_ev).toBeLessThan(0);
  });

  it("adds contrast to a flat, narrow-range image", () => {
    // Alle Pixel im schmalen Band 100..110 — eine flache Aufnahme.
    const pixels = new Uint8ClampedArray(20 * 4);
    for (let i = 0; i < 20; i++) {
      const value = 100 + (i % 10);
      pixels[i * 4] = value;
      pixels[i * 4 + 1] = value;
      pixels[i * 4 + 2] = value;
      pixels[i * 4 + 3] = 255;
    }
    const histogram = computeHistogram(pixels, 20, 1);
    const result = computeAutoTone(histogram);
    expect(result.contrast).toBeGreaterThan(50);
  });

  it("adds little contrast to an image that already spans the full range", () => {
    const pixels = new Uint8ClampedArray(4 * 4);
    const values = [0, 85, 170, 255];
    values.forEach((v, i) => {
      pixels[i * 4] = v;
      pixels[i * 4 + 1] = v;
      pixels[i * 4 + 2] = v;
      pixels[i * 4 + 3] = 255;
    });
    const histogram = computeHistogram(pixels, 4, 1);
    const result = computeAutoTone(histogram);
    expect(result.contrast).toBeLessThan(10);
  });

  it("clamps exposure to the slider range for an extreme image", () => {
    const pixels = makeUniformPixels(1, 10);
    const histogram = computeHistogram(pixels, 10, 1);
    const result = computeAutoTone(histogram);
    expect(result.exposure_ev).toBeLessThanOrEqual(5);
    expect(result.exposure_ev).toBeGreaterThanOrEqual(-5);
  });
});
