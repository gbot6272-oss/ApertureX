import { describe, expect, it } from "vitest";

import {
  applyArrowStep,
  BASIC_SLIDER_SPECS,
  buildEdlEnvelopeJson,
  clampSliderValue,
  EDL_SCHEMA_VERSION,
  NEUTRAL_BASIC_ADJUSTMENTS,
  parseEdlEnvelopeJson,
  readBasicField,
  writeBasicField,
  type BasicAdjustments,
} from "./edl";

describe("buildEdlEnvelopeJson / parseEdlEnvelopeJson", () => {
  it("roundtrips neutral adjustments", () => {
    const json = buildEdlEnvelopeJson(NEUTRAL_BASIC_ADJUSTMENTS);
    const parsed = parseEdlEnvelopeJson(json);
    expect(parsed).toEqual(NEUTRAL_BASIC_ADJUSTMENTS);
  });

  it("roundtrips a non-neutral value", () => {
    const basic: BasicAdjustments = {
      ...NEUTRAL_BASIC_ADJUSTMENTS,
      exposure_ev: 0.7,
      contrast: -15,
      white_balance: { temp_shift_kelvin: 300, tint_shift: -10 },
    };
    const parsed = parseEdlEnvelopeJson(buildEdlEnvelopeJson(basic));
    expect(parsed).toEqual(basic);
  });

  it("embeds the current schema version", () => {
    const json = buildEdlEnvelopeJson(NEUTRAL_BASIC_ADJUSTMENTS);
    const raw = JSON.parse(json) as { schema_version: number };
    expect(raw.schema_version).toBe(EDL_SCHEMA_VERSION);
  });

  it("rejects an unknown schema version", () => {
    const json = JSON.stringify({ schema_version: 9999, payload: { basic: NEUTRAL_BASIC_ADJUSTMENTS } });
    expect(parseEdlEnvelopeJson(json)).toBeNull();
  });

  it("rejects malformed json without throwing", () => {
    expect(parseEdlEnvelopeJson("not json")).toBeNull();
    expect(parseEdlEnvelopeJson("{}")).toBeNull();
  });
});

describe("clampSliderValue", () => {
  it("clamps to the slider's range", () => {
    const spec = { min: -10, max: 10 };
    expect(clampSliderValue(-100, spec)).toBe(-10);
    expect(clampSliderValue(100, spec)).toBe(10);
    expect(clampSliderValue(3, spec)).toBe(3);
  });
});

describe("applyArrowStep", () => {
  const exposureSpec = BASIC_SLIDER_SPECS.find((s) => s.key === "exposure_ev");
  if (!exposureSpec) throw new Error("Test-Fixture: exposure_ev-Spec fehlt");

  it("moves by the fine step for a plain arrow press", () => {
    expect(applyArrowStep(0, 1, exposureSpec, false)).toBeCloseTo(exposureSpec.fineStep);
    expect(applyArrowStep(0, -1, exposureSpec, false)).toBeCloseTo(-exposureSpec.fineStep);
  });

  it("moves by the coarse step when shift is held", () => {
    expect(applyArrowStep(0, 1, exposureSpec, true)).toBeCloseTo(exposureSpec.coarseStep);
  });

  it("never leaves the slider's range", () => {
    expect(applyArrowStep(exposureSpec.max, 1, exposureSpec, true)).toBe(exposureSpec.max);
    expect(applyArrowStep(exposureSpec.min, -1, exposureSpec, true)).toBe(exposureSpec.min);
  });
});

describe("readBasicField", () => {
  it("reads white-balance fields from the nested object", () => {
    const basic: BasicAdjustments = {
      ...NEUTRAL_BASIC_ADJUSTMENTS,
      white_balance: { temp_shift_kelvin: 42, tint_shift: -7 },
    };
    expect(readBasicField(basic, "temp_shift_kelvin")).toBe(42);
    expect(readBasicField(basic, "tint_shift")).toBe(-7);
  });

  it("reads top-level fields directly", () => {
    const basic: BasicAdjustments = { ...NEUTRAL_BASIC_ADJUSTMENTS, contrast: 33 };
    expect(readBasicField(basic, "contrast")).toBe(33);
  });
});

describe("writeBasicField", () => {
  it("writes white-balance fields into the nested object", () => {
    const basic: BasicAdjustments = { ...NEUTRAL_BASIC_ADJUSTMENTS, white_balance: { ...NEUTRAL_BASIC_ADJUSTMENTS.white_balance } };
    writeBasicField(basic, "temp_shift_kelvin", 55);
    expect(basic.white_balance.temp_shift_kelvin).toBe(55);
    expect(readBasicField(basic, "temp_shift_kelvin")).toBe(55);
  });

  it("writes top-level fields directly", () => {
    const basic: BasicAdjustments = { ...NEUTRAL_BASIC_ADJUSTMENTS };
    writeBasicField(basic, "shadows", -20);
    expect(basic.shadows).toBe(-20);
  });
});

describe("BASIC_SLIDER_SPECS", () => {
  it("has one entry per BasicAdjustments field (temp/tint counted separately)", () => {
    // white_balance{temp_shift_kelvin,tint_shift} + 6 direkte Felder = 8.
    expect(BASIC_SLIDER_SPECS).toHaveLength(8);
  });

  it("every spec's neutral value is within its own range", () => {
    for (const spec of BASIC_SLIDER_SPECS) {
      expect(spec.neutral).toBeGreaterThanOrEqual(spec.min);
      expect(spec.neutral).toBeLessThanOrEqual(spec.max);
    }
  });
});
