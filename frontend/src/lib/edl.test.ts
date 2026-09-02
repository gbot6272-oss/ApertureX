import { describe, expect, it } from "vitest";

import {
  applyArrowStep,
  BASIC_SLIDER_SPECS,
  buildEdlEnvelopeJson,
  clampSliderValue,
  EDL_SCHEMA_VERSION,
  emptyBrushGeometry,
  NEUTRAL_BASIC_ADJUSTMENTS,
  neutralEdlPayload,
  neutralMaskAdjustments,
  newBrushMask,
  parseEdlEnvelopeJson,
  readBasicField,
  visibleMasks,
  WHITE_BALANCE_PRESETS,
  writeBasicField,
  type BasicAdjustments,
  type Mask,
  type MaskGroup,
} from "./edl";

describe("buildEdlEnvelopeJson / parseEdlEnvelopeJson", () => {
  it("roundtrips the neutral payload", () => {
    const payload = neutralEdlPayload();
    const json = buildEdlEnvelopeJson(payload);
    const parsed = parseEdlEnvelopeJson(json);
    expect(parsed).toEqual(payload);
  });

  it("roundtrips a non-neutral value", () => {
    const payload = {
      ...neutralEdlPayload(),
      basic: {
        ...NEUTRAL_BASIC_ADJUSTMENTS,
        exposure_ev: 0.7,
        contrast: -15,
        white_balance: { temp_shift_kelvin: 300, tint_shift: -10 },
      },
    };
    const parsed = parseEdlEnvelopeJson(buildEdlEnvelopeJson(payload));
    expect(parsed).toEqual(payload);
  });

  it("embeds the current schema version", () => {
    const json = buildEdlEnvelopeJson(neutralEdlPayload());
    const raw = JSON.parse(json) as { schema_version: number };
    expect(raw.schema_version).toBe(EDL_SCHEMA_VERSION);
  });

  it("rejects an unknown schema version", () => {
    const json = JSON.stringify({ schema_version: 9999, payload: neutralEdlPayload() });
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
    // white_balance{temp_shift_kelvin,tint_shift} + 11 direkte Felder = 13
    // (siehe `crates/apx-pipeline/src/edl/v2.rs`s `BasicAdjustments` — 12
    // Regler insgesamt, Weißabgleich zählt als einer mit zwei Werten).
    // Die fünf per ADR-0011/ADR-0028 nach Phase 4 verschobenen Felder
    // (Textur/Klarheit/Dunst entfernen/Dynamik/Sättigung) sind seit
    // Phase 4 Schritt 3 mit dabei.
    expect(BASIC_SLIDER_SPECS).toHaveLength(13);
  });

  it("every spec's neutral value is within its own range", () => {
    for (const spec of BASIC_SLIDER_SPECS) {
      expect(spec.neutral).toBeGreaterThanOrEqual(spec.min);
      expect(spec.neutral).toBeLessThanOrEqual(spec.max);
    }
  });
});

describe("WHITE_BALANCE_PRESETS (Phase 4, Schritt 3)", () => {
  it("has unique keys", () => {
    const keys = WHITE_BALANCE_PRESETS.map((preset) => preset.key);
    expect(new Set(keys).size).toBe(keys.length);
  });

  it("includes a neutral 'as-shot' preset", () => {
    const asShot = WHITE_BALANCE_PRESETS.find((preset) => preset.key === "as_shot");
    expect(asShot).toEqual({ key: "as_shot", label: "Wie aufgenommen", temp_shift_kelvin: 0, tint_shift: 0 });
  });

  it("keeps every preset within the slider bounds", () => {
    const tempSpec = BASIC_SLIDER_SPECS.find((spec) => spec.key === "temp_shift_kelvin");
    const tintSpec = BASIC_SLIDER_SPECS.find((spec) => spec.key === "tint_shift");
    if (!tempSpec || !tintSpec) throw new Error("Test-Fixture: WB-Specs fehlen");
    for (const preset of WHITE_BALANCE_PRESETS) {
      expect(preset.temp_shift_kelvin).toBeGreaterThanOrEqual(tempSpec.min);
      expect(preset.temp_shift_kelvin).toBeLessThanOrEqual(tempSpec.max);
      expect(preset.tint_shift).toBeGreaterThanOrEqual(tintSpec.min);
      expect(preset.tint_shift).toBeLessThanOrEqual(tintSpec.max);
    }
  });
});

describe("neutralEdlPayload (Phase 4, Schritt 1)", () => {
  it("round-trips through JSON unchanged", () => {
    const payload = neutralEdlPayload();
    const json = buildEdlEnvelopeJson(payload);
    expect(parseEdlEnvelopeJson(json)).toEqual(payload);
  });

  it("has all twelve Grundeinstellungen neutral, including the five Phase-4 fields", () => {
    const { basic } = neutralEdlPayload();
    expect(basic.texture).toBe(0);
    expect(basic.clarity).toBe(0);
    expect(basic.dehaze).toBe(0);
    expect(basic.vibrance).toBe(0);
    expect(basic.saturation).toBe(0);
  });

  it("gives every curve channel the identity (0,0)-(1,1) point curve", () => {
    const { curves } = neutralEdlPayload();
    for (const channel of [curves.rgb, curves.red, curves.green, curves.blue, curves.luminance]) {
      expect(channel).toEqual({
        kind: "Points",
        points: [
          { input: 0, output: 0 },
          { input: 1, output: 1 },
        ],
      });
    }
  });

  it("starts with no color-mixer regions and no repair strokes", () => {
    const payload = neutralEdlPayload();
    expect(payload.color_mixer.regions).toEqual([]);
    expect(payload.repair).toEqual([]);
  });

  it("defaults lens corrections to no profile and upright mode Off", () => {
    const { lens_corrections } = neutralEdlPayload();
    expect(lens_corrections.profile_id).toBeNull();
    expect(lens_corrections.upright_mode).toBe("Off");
  });

  it("starts with no masks and no mask groups", () => {
    const payload = neutralEdlPayload();
    expect(payload.masks).toEqual([]);
    expect(payload.mask_groups).toEqual([]);
  });
});

describe("Masken (Phase 6)", () => {
  it("newBrushMask() has one empty brush component and neutral adjustments", () => {
    const mask = newBrushMask("mask-1", "Neue Maske");
    expect(mask.components).toEqual([{ geometry: { kind: "Brush", strokes: [] }, combine: "Add", invert: false }]);
    expect(mask.adjustments).toEqual(neutralMaskAdjustments());
    expect(mask.visible).toBe(true);
    expect(mask.blend_mode).toBe("Normal");
  });

  it("roundtrips a mask with multiple component types through JSON", () => {
    const mask: Mask = {
      ...newBrushMask("mask-2", "Himmel"),
      components: [
        { geometry: { kind: "LinearGradient", x1: 0, y1: 0, x2: 1, y2: 1 }, combine: "Add", invert: false },
        {
          geometry: { kind: "ColorRange", target_r: 0.8, target_g: 0.2, target_b: 0.2, tolerance: 0.1, feather: 0.2 },
          combine: "Intersect",
          invert: true,
        },
      ],
    };
    const json = JSON.stringify(mask);
    expect(JSON.parse(json)).toEqual(mask);
  });

  it("emptyBrushGeometry() tags itself with kind 'Brush'", () => {
    expect(emptyBrushGeometry()).toEqual({ kind: "Brush", strokes: [] });
  });

  // Phase 12 Schritt 1 (siehe DECISIONS.md ADR-0039): `visibleMasks` ist
  // das clientseitige Spiegelbild von
  // `apx_pipeline::stages::masks::visible_masks` — hier für die
  // Masken-Farbüberlagerung im Viewer statt für die Pipeline genutzt.
  it("visibleMasks() hides invisible masks and masks in hidden groups, but keeps ungrouped/visible ones", () => {
    const groups: MaskGroup[] = [
      { id: "g-visible", name: "Himmel", visible: true },
      { id: "g-hidden", name: "Vordergrund", visible: false },
    ];
    const visible = newBrushMask("m-visible", "Sichtbar");
    const hidden = { ...newBrushMask("m-hidden", "Ausgeblendet"), visible: false };
    const inHiddenGroup = { ...newBrushMask("m-in-hidden-group", "In ausgeblendeter Gruppe"), group_id: "g-hidden" };
    const inVisibleGroup = { ...newBrushMask("m-in-visible-group", "In sichtbarer Gruppe"), group_id: "g-visible" };

    const result = visibleMasks([visible, hidden, inHiddenGroup, inVisibleGroup], groups);

    expect(result.map((m) => m.id)).toEqual(["m-visible", "m-in-visible-group"]);
  });
});
