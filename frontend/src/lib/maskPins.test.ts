import { describe, expect, it } from "vitest";

import type { Mask, MaskGeometry } from "./edl";
import { newMask } from "./edl";
import { computeMaskPinPosition } from "./maskPins";

function maskWith(geometry: MaskGeometry): Mask {
  return newMask("m1", "Test", geometry);
}

describe("computeMaskPinPosition", () => {
  it("uses the midpoint for a linear gradient", () => {
    const mask = maskWith({ kind: "LinearGradient", x1: 0.2, y1: 0.4, x2: 0.6, y2: 0.8 });
    const position = computeMaskPinPosition(mask);
    expect(position?.x).toBeCloseTo(0.4);
    expect(position?.y).toBeCloseTo(0.6);
  });

  it("uses the center for a radial gradient", () => {
    const mask = maskWith({ kind: "RadialGradient", center_x: 0.3, center_y: 0.7, radius_x: 0.1, radius_y: 0.1, angle_degrees: 0, feather: 0.5 });
    expect(computeMaskPinPosition(mask)).toEqual({ x: 0.3, y: 0.7 });
  });

  it("averages all stroke points for a brush mask", () => {
    const mask = maskWith({
      kind: "Brush",
      strokes: [
        { points: [{ x: 0, y: 0 }, { x: 1, y: 0 }], radius: 0.1, feather: 0.5 },
        { points: [{ x: 0, y: 1 }], radius: 0.1, feather: 0.5 },
      ],
    });
    expect(computeMaskPinPosition(mask)).toEqual({ x: 1 / 3, y: 1 / 3 });
  });

  it("returns null for a color-range mask (no spatial anchor)", () => {
    const mask = maskWith({ kind: "ColorRange", target_r: 200, target_g: 100, target_b: 50, tolerance: 20, feather: 0.5 });
    expect(computeMaskPinPosition(mask)).toBeNull();
  });

  it("returns null for a brush mask with no points", () => {
    const mask = maskWith({ kind: "Brush", strokes: [] });
    expect(computeMaskPinPosition(mask)).toBeNull();
  });
});
