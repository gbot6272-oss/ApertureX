import { describe, expect, it } from "vitest";

import { buildClippingOverlay, computeHistogram, countClipping } from "./histogram";

function makePixels(entries: Array<[number, number, number]>): Uint8ClampedArray {
  const pixels = new Uint8ClampedArray(entries.length * 4);
  entries.forEach(([r, g, b], i) => {
    pixels[i * 4] = r;
    pixels[i * 4 + 1] = g;
    pixels[i * 4 + 2] = b;
    pixels[i * 4 + 3] = 255;
  });
  return pixels;
}

describe("computeHistogram", () => {
  it("counts each channel byte value into the matching bucket", () => {
    const pixels = makePixels([
      [10, 20, 30],
      [10, 200, 30],
    ]);
    const hist = computeHistogram(pixels, 2, 1);
    expect(hist.r[10]).toBe(2);
    expect(hist.g[20]).toBe(1);
    expect(hist.g[200]).toBe(1);
    expect(hist.b[30]).toBe(2);
    expect(hist.maxCount).toBeGreaterThanOrEqual(2);
  });

  it("computes luminance using rec-709 weights", () => {
    const pixels = makePixels([[255, 255, 255]]);
    const hist = computeHistogram(pixels, 1, 1);
    expect(hist.luminance[255]).toBe(1);
  });
});

describe("countClipping", () => {
  it("counts a pixel as shadow-clipped when any channel hits the threshold", () => {
    const pixels = makePixels([
      [0, 128, 128],
      [50, 50, 50],
    ]);
    const counts = countClipping(pixels, 2, 1);
    expect(counts.shadowClipped).toBe(1);
    expect(counts.highlightClipped).toBe(0);
    expect(counts.totalPixels).toBe(2);
  });

  it("counts a pixel as highlight-clipped when any channel hits the threshold", () => {
    const pixels = makePixels([[255, 10, 10]]);
    const counts = countClipping(pixels, 1, 1);
    expect(counts.highlightClipped).toBe(1);
  });

  it("respects custom thresholds", () => {
    const pixels = makePixels([[250, 10, 10]]);
    expect(countClipping(pixels, 1, 1, 0, 255).highlightClipped).toBe(0);
    expect(countClipping(pixels, 1, 1, 0, 250).highlightClipped).toBe(1);
  });
});

describe("buildClippingOverlay", () => {
  it("marks highlight-clipped pixels red and leaves others transparent", () => {
    const pixels = makePixels([
      [255, 10, 10],
      [10, 10, 10],
    ]);
    const overlay = buildClippingOverlay(pixels, 2, 1);
    expect([overlay[0], overlay[1], overlay[2], overlay[3]]).toEqual([255, 0, 0, 255]);
    expect(overlay[7]).toBe(0); // zweites Pixel bleibt transparent (Alpha=0)
  });

  it("marks shadow-clipped pixels blue", () => {
    const pixels = makePixels([[0, 10, 10]]);
    const overlay = buildClippingOverlay(pixels, 1, 1);
    expect([overlay[0], overlay[1], overlay[2], overlay[3]]).toEqual([0, 80, 255, 255]);
  });
});
