import { describe, expect, it } from "vitest";

import { neutralEdlPayload } from "./edl";
import {
  buildPresetEdlSubset,
  mergeEdlSubset,
  parseConditions,
  parseEdlSubset,
  scalePresetEdlSubset,
  serializeConditions,
  serializeEdlSubset,
} from "./presets";
import type { PresetCondition, PresetEdlSubset } from "./presets";

// `neutralEdlPayload()` teilt sich für einige Sektionen (`basic`, `hsl`,
// `color_grading`, `effects`, `geometry`) eine gemeinsame Konstante statt
// bei jedem Aufruf frisch zu klonen (siehe `edl.ts`s `neutralEdlPayload`)
// — im echten Store ist das unbedenklich, weil jede Änderung über einen
// Immer-`set()`-Producer läuft (der bei einer Mutation automatisch eine
// Kopie anlegt). Hier in reinen Funktionstests OHNE Immer würde eine
// direkte Feldzuweisung wie `edl.basic.exposure_ev = 0.5` diese geteilte
// Konstante dauerhaft verändern und alle nachfolgenden Tests in dieser
// Datei verfälschen — deshalb wird jede Sektion vor dem Ändern per
// Spread geklont.
function edlWithBasic(overrides: Partial<ReturnType<typeof neutralEdlPayload>["basic"]>) {
  const edl = neutralEdlPayload();
  edl.basic = { ...edl.basic, ...overrides };
  return edl;
}

describe("buildPresetEdlSubset", () => {
  it("copies only the selected sections from a full EdlPayload", () => {
    const edl = edlWithBasic({ exposure_ev: 0.5 });

    const subset = buildPresetEdlSubset(edl, ["basic"]);

    expect(subset.basic).toEqual(edl.basic);
    expect(subset.curves).toBeUndefined();
    expect(subset.hsl).toBeUndefined();
  });

  it("returns an empty object when no sections are selected", () => {
    expect(buildPresetEdlSubset(neutralEdlPayload(), [])).toEqual({});
  });
});

describe("parseEdlSubset/serializeEdlSubset", () => {
  it("roundtrips a subset with only the selected sections", () => {
    const withData = { hsl: { red: { hue: 10, saturation: 5, luminance: 0 } } } as PresetEdlSubset;
    const json = serializeEdlSubset(withData);
    const parsed = parseEdlSubset(json);
    expect(parsed).toEqual(withData);
    expect(parsed.basic).toBeUndefined();
  });

  it("returns an empty object for invalid JSON instead of throwing", () => {
    expect(parseEdlSubset("{invalid")).toEqual({});
  });

  it("returns an empty object for JSON that isn't an object", () => {
    expect(parseEdlSubset("42")).toEqual({});
    expect(parseEdlSubset("null")).toEqual({});
  });
});

describe("scalePresetEdlSubset", () => {
  it("returns the preset value unchanged at 100%", () => {
    const subset = buildPresetEdlSubset(edlWithBasic({ exposure_ev: 0.8 }), ["basic"]);

    const scaled = scalePresetEdlSubset(subset, 100);

    expect(scaled.basic?.exposure_ev).toBeCloseTo(0.8);
  });

  it("returns the neutral value at 0%", () => {
    const subset = buildPresetEdlSubset(edlWithBasic({ exposure_ev: 0.8, contrast: 20 }), ["basic"]);

    const scaled = scalePresetEdlSubset(subset, 0);

    expect(scaled.basic?.exposure_ev).toBeCloseTo(0);
    expect(scaled.basic?.contrast).toBeCloseTo(0);
  });

  it("doubles the distance from neutral at 200%", () => {
    const subset = buildPresetEdlSubset(edlWithBasic({ exposure_ev: 0.5 }), ["basic"]);

    const scaled = scalePresetEdlSubset(subset, 200);

    expect(scaled.basic?.exposure_ev).toBeCloseTo(1.0);
  });

  it("scales nested numeric fields (e.g. white_balance) recursively", () => {
    const subset = buildPresetEdlSubset(
      edlWithBasic({ white_balance: { temp_shift_kelvin: 400, tint_shift: -20 } }),
      ["basic"],
    );

    const scaled = scalePresetEdlSubset(subset, 50);

    expect(scaled.basic?.white_balance.temp_shift_kelvin).toBeCloseTo(200);
    expect(scaled.basic?.white_balance.tint_shift).toBeCloseTo(-10);
  });

  it("leaves non-numeric fields (arrays, enums) unscaled at any strength", () => {
    const edl = neutralEdlPayload();
    const region = {
      target_hue_degrees: 30,
      bandwidth_degrees: 40,
      feather: 15,
      hue_shift: 10,
      saturation_shift: 10,
      luminance_shift: 0,
    };
    edl.color_mixer = { regions: [region] };
    const subset = buildPresetEdlSubset(edl, ["color_mixer"]);

    const scaled = scalePresetEdlSubset(subset, 30);

    expect(scaled.color_mixer?.regions).toEqual([region]);
  });
});

describe("mergeEdlSubset", () => {
  it("replaces only the sections present in the subset, keeping the rest of the base untouched", () => {
    const base = neutralEdlPayload();
    base.curves = { ...base.curves, rgb: { kind: "Parametric", shadows: 5, darks: 0, lights: 0, highlights: 0 } };
    const subset: PresetEdlSubset = { basic: { ...neutralEdlPayload().basic, exposure_ev: 0.4 } };

    const merged = mergeEdlSubset(base, subset);

    expect(merged.basic.exposure_ev).toBeCloseTo(0.4);
    expect(merged.curves).toBe(base.curves);
  });
});

describe("parseConditions/serializeConditions", () => {
  it("roundtrips a list of AND-combined rules", () => {
    const conditions: PresetCondition[] = [
      { field: "iso", op: ">", value: "3200" },
      { field: "lens", op: "contains", value: "35mm" },
    ];
    const json = serializeConditions(conditions);
    expect(parseConditions(json)).toEqual(conditions);
  });

  it("returns an empty array for invalid JSON instead of throwing", () => {
    expect(parseConditions("{not an array")).toEqual([]);
  });

  it("returns an empty array when the JSON is valid but not an array", () => {
    expect(parseConditions('{"field":"iso"}')).toEqual([]);
  });
});
