import { describe, expect, it } from "vitest";

import { binPointsIntoGrid, heatScaleColor, lerpHexColor, normalizeHeatIntensity, rgbaCss } from "./photoHeatmap";

describe("binPointsIntoGrid", () => {
  it("groups nearby points into the same cell", () => {
    const cells = binPointsIntoGrid(
      [
        { lat: 52.5, lon: 13.4 },
        { lat: 52.6, lon: 13.45 },
      ],
      2,
    );
    expect(cells).toHaveLength(1);
    expect(cells[0]?.count).toBe(2);
  });

  it("separates points in different cells", () => {
    const cells = binPointsIntoGrid(
      [
        { lat: 52.5, lon: 13.4 },
        { lat: -33.9, lon: 151.2 }, // Sydney — far away
      ],
      2,
    );
    expect(cells).toHaveLength(2);
    expect(cells.every((c) => c.count === 1)).toBe(true);
  });

  it("ignores non-finite coordinates instead of throwing", () => {
    const cells = binPointsIntoGrid([{ lat: NaN, lon: 13.4 }], 2);
    expect(cells).toHaveLength(0);
  });

  it("returns no cells for an empty input", () => {
    expect(binPointsIntoGrid([], 2)).toHaveLength(0);
  });
});

describe("lerpHexColor", () => {
  it("returns the first color at t = 0", () => {
    expect(lerpHexColor("#000000", "#ffffff", 0)).toEqual([0, 0, 0]);
  });

  it("returns the second color at t = 1", () => {
    expect(lerpHexColor("#000000", "#ffffff", 1)).toEqual([255, 255, 255]);
  });

  it("interpolates at the midpoint", () => {
    expect(lerpHexColor("#000000", "#ffffff", 0.5)).toEqual([128, 128, 128]);
  });

  it("clamps t outside [0, 1]", () => {
    expect(lerpHexColor("#000000", "#ffffff", -1)).toEqual([0, 0, 0]);
    expect(lerpHexColor("#000000", "#ffffff", 2)).toEqual([255, 255, 255]);
  });
});

describe("heatScaleColor", () => {
  const cool = "#111111";
  const mid = "#5b9bd5";
  const hot = "#e07a5f";

  it("is the cool color at t = 0", () => {
    expect(heatScaleColor(0, cool, mid, hot)).toEqual(lerpHexColor(cool, cool, 0));
  });

  it("is exactly the mid color at t = 0.5", () => {
    expect(heatScaleColor(0.5, cool, mid, hot)).toEqual(lerpHexColor(cool, mid, 1));
  });

  it("is the hot color at t = 1", () => {
    expect(heatScaleColor(1, cool, mid, hot)).toEqual(lerpHexColor(mid, hot, 1));
  });
});

describe("normalizeHeatIntensity", () => {
  it("is 0 when maxCount is 0 (no division by zero)", () => {
    expect(normalizeHeatIntensity(0, 0)).toBe(0);
  });

  it("is 1 at the maximum count", () => {
    expect(normalizeHeatIntensity(10, 10)).toBeCloseTo(1);
  });

  it("compresses low counts upward via a square root (not linear)", () => {
    // sqrt(0.25) = 0.5 — a quarter of the max count reads as half
    // intensity, not a quarter, so a single hotspot doesn't crush
    // every other location toward invisibility.
    expect(normalizeHeatIntensity(1, 4)).toBeCloseTo(0.5);
  });
});

describe("rgbaCss", () => {
  it("formats a CSS rgba() string", () => {
    expect(rgbaCss([91, 155, 213], 0.5)).toBe("rgba(91, 155, 213, 0.5)");
  });
});
