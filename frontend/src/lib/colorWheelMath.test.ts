import { describe, expect, it } from "vitest";

import { hueSaturationToPixelOffset, pixelOffsetToHueSaturation } from "./colorWheelMath";

describe("pixelOffsetToHueSaturation", () => {
  it("returns hue 0 and full saturation straight above the center", () => {
    const { hue_degrees, saturation } = pixelOffsetToHueSaturation(0, -50, 50);
    expect(hue_degrees).toBeCloseTo(0);
    expect(saturation).toBeCloseTo(1);
  });

  it("returns hue 90 to the right of the center", () => {
    const { hue_degrees } = pixelOffsetToHueSaturation(50, 0, 50);
    expect(hue_degrees).toBeCloseTo(90);
  });

  it("returns hue 180 below the center", () => {
    const { hue_degrees } = pixelOffsetToHueSaturation(0, 50, 50);
    expect(hue_degrees).toBeCloseTo(180);
  });

  it("returns saturation 0 at the exact center", () => {
    const { saturation } = pixelOffsetToHueSaturation(0, 0, 50);
    expect(saturation).toBe(0);
  });

  it("clamps saturation to 1 beyond the wheel's radius", () => {
    const { saturation } = pixelOffsetToHueSaturation(0, -200, 50);
    expect(saturation).toBe(1);
  });
});

describe("hueSaturationToPixelOffset", () => {
  it("round-trips with pixelOffsetToHueSaturation", () => {
    for (const hue of [0, 45, 90, 180, 270, 350]) {
      for (const saturation of [0.2, 0.6, 1.0]) {
        const { dx, dy } = hueSaturationToPixelOffset(hue, saturation, 50);
        const back = pixelOffsetToHueSaturation(dx, dy, 50);
        expect(back.hue_degrees).toBeCloseTo(hue, 1);
        expect(back.saturation).toBeCloseTo(saturation, 5);
      }
    }
  });

  it("places a zero-saturation point exactly at the center", () => {
    const { dx, dy } = hueSaturationToPixelOffset(123, 0, 50);
    expect(dx).toBeCloseTo(0);
    expect(dy).toBeCloseTo(0);
  });
});
