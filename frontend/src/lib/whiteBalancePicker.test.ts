import { describe, expect, it } from "vitest";

import { NEUTRAL_WHITE_BALANCE } from "./edl";
import { computeWhiteBalanceShiftFromSample, srgbByteToApproxLinear } from "./whiteBalancePicker";

describe("srgbByteToApproxLinear", () => {
  it("maps the extremes to 0 and 1", () => {
    expect(srgbByteToApproxLinear(0)).toBeCloseTo(0);
    expect(srgbByteToApproxLinear(255)).toBeCloseTo(1);
  });

  it("is monotonically increasing", () => {
    expect(srgbByteToApproxLinear(200)).toBeGreaterThan(srgbByteToApproxLinear(100));
  });
});

describe("computeWhiteBalanceShiftFromSample", () => {
  it("leaves the shift unchanged for a perfectly neutral gray sample", () => {
    const result = computeWhiteBalanceShiftFromSample(128, 128, 128, NEUTRAL_WHITE_BALANCE);
    expect(result.temp_shift_kelvin).toBeCloseTo(0);
    expect(result.tint_shift).toBeCloseTo(0);
  });

  it("cools the temperature down for a warm (orange) cast", () => {
    // Rot deutlich vor Grün, Blau am schwächsten — typischer Kunstlicht-
    // /Sonnenuntergangs-Farbstich.
    const result = computeWhiteBalanceShiftFromSample(180, 140, 100, NEUTRAL_WHITE_BALANCE);
    expect(result.temp_shift_kelvin).toBeLessThan(0);
  });

  it("warms the temperature up for a cool (blue) cast", () => {
    const result = computeWhiteBalanceShiftFromSample(100, 140, 180, NEUTRAL_WHITE_BALANCE);
    expect(result.temp_shift_kelvin).toBeGreaterThan(0);
  });

  it("raises tint when green is in excess relative to red/blue", () => {
    const result = computeWhiteBalanceShiftFromSample(120, 180, 120, NEUTRAL_WHITE_BALANCE);
    expect(result.tint_shift).toBeGreaterThan(0);
  });

  it("lowers tint when green is deficient relative to red/blue", () => {
    const result = computeWhiteBalanceShiftFromSample(150, 90, 150, NEUTRAL_WHITE_BALANCE);
    expect(result.tint_shift).toBeLessThan(0);
  });

  it("stays within the slider bounds even for an extreme, heavily clamped sample", () => {
    const result = computeWhiteBalanceShiftFromSample(255, 128, 0, NEUTRAL_WHITE_BALANCE);
    expect(result.temp_shift_kelvin).toBeGreaterThanOrEqual(-2000);
    expect(result.temp_shift_kelvin).toBeLessThanOrEqual(2000);
    expect(result.tint_shift).toBeGreaterThanOrEqual(-100);
    expect(result.tint_shift).toBeLessThanOrEqual(100);
  });

  it("returns the current shift unchanged for a near-black sample (too dark to judge)", () => {
    const current = { temp_shift_kelvin: 300, tint_shift: -20 };
    const result = computeWhiteBalanceShiftFromSample(1, 1, 1, current);
    expect(result).toEqual(current);
  });

  it("corrects additively on top of an already-shifted current value", () => {
    const current = { temp_shift_kelvin: 500, tint_shift: 0 };
    const result = computeWhiteBalanceShiftFromSample(128, 128, 128, current);
    // Ein neutrales Sample ändert nichts an einem bereits gesetzten Shift.
    expect(result).toEqual(current);
  });
});
