import { describe, expect, it } from "vitest";

import { neutralEdlPayload } from "./edl";
import {
  applyConditionsToSubset,
  applyRulesToSubset,
  buildPresetEdlSubset,
  diffEdlSubsets,
  evaluateCondition,
  mergeEdlSubset,
  parseConditions,
  parseEdlSubset,
  parseRules,
  scalePresetEdlSubset,
  serializeConditions,
  serializeEdlSubset,
  serializeRules,
} from "./presets";
import type { PresetCondition, PresetConditionPhotoMeta, PresetEdlSubset, PresetLeafCondition, PresetRules } from "./presets";
import type { RuleNode } from "./ruleTree";
import { evaluateRuleNode } from "./ruleTree";

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
      { field: "iso", op: ">", value: "3200", section: null },
      { field: "lens", op: "contains", value: "35mm", section: "basic" },
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

describe("evaluateCondition", () => {
  const photo: PresetConditionPhotoMeta = { iso: 800, aperture: 2.8, focal_length: 85, camera_model: "EOS R5", lens: "RF 85mm f/1.2L" };

  it.each([
    [{ field: "iso", op: ">", value: "400" }, true],
    [{ field: "iso", op: ">", value: "1600" }, false],
    [{ field: "iso", op: "<", value: "1600" }, true],
    [{ field: "iso", op: "=", value: "800" }, true],
    [{ field: "aperture", op: "=", value: "2.8" }, true],
    [{ field: "focal_length", op: ">", value: "50" }, true],
  ] as const)("numeric field %o -> %s", (partial, expected) => {
    expect(evaluateCondition({ ...partial, section: null }, photo)).toBe(expected);
  });

  it("evaluates 'contains' case-insensitively on string fields", () => {
    expect(evaluateCondition({ field: "lens", op: "contains", value: "85mm", section: null }, photo)).toBe(true);
    expect(evaluateCondition({ field: "lens", op: "contains", value: "24-70", section: null }, photo)).toBe(false);
    expect(evaluateCondition({ field: "camera_model", op: "contains", value: "r5", section: null }, photo)).toBe(true);
  });

  it("evaluates '=' case-insensitively on string fields", () => {
    expect(evaluateCondition({ field: "camera_model", op: "=", value: "eos r5", section: null }, photo)).toBe(true);
  });

  it("treats a missing metadata value as not satisfied", () => {
    const noIso: PresetConditionPhotoMeta = { ...photo, iso: null };
    expect(evaluateCondition({ field: "iso", op: ">", value: "0", section: null }, noIso)).toBe(false);
  });

  it("treats a non-numeric operator on a numeric field as not satisfied", () => {
    expect(evaluateCondition({ field: "iso", op: "contains", value: "800", section: null }, photo)).toBe(false);
  });

  it("treats a numeric operator on a string field as not satisfied", () => {
    expect(evaluateCondition({ field: "camera_model", op: ">", value: "A", section: null }, photo)).toBe(false);
  });

  it("treats an unparseable numeric value as not satisfied", () => {
    expect(evaluateCondition({ field: "iso", op: ">", value: "not-a-number", section: null }, photo)).toBe(false);
  });
});

describe("applyConditionsToSubset", () => {
  const photo: PresetConditionPhotoMeta = { iso: 200, aperture: 4, focal_length: 50, camera_model: "EOS R5", lens: "RF 24-70mm" };
  const subset: PresetEdlSubset = {
    basic: { ...neutralEdlPayload().basic, exposure_ev: 0.5 },
    curves: neutralEdlPayload().curves,
  };

  it("returns the subset unchanged when there are no conditions", () => {
    expect(applyConditionsToSubset(subset, [], photo)).toBe(subset);
  });

  it("returns the full subset when a whole-preset condition (section: null) is satisfied", () => {
    const conditions: PresetCondition[] = [{ field: "iso", op: "<", value: "400", section: null }];
    expect(applyConditionsToSubset(subset, conditions, photo)).toEqual(subset);
  });

  it("returns null when a whole-preset condition fails, excluding the entire preset", () => {
    const conditions: PresetCondition[] = [{ field: "iso", op: ">", value: "400", section: null }];
    expect(applyConditionsToSubset(subset, conditions, photo)).toBeNull();
  });

  it("excludes only the affected section when a section-scoped condition fails", () => {
    const conditions: PresetCondition[] = [{ field: "iso", op: ">", value: "400", section: "curves" }];
    const result = applyConditionsToSubset(subset, conditions, photo);
    expect(result?.basic).toEqual(subset.basic);
    expect(result?.curves).toBeUndefined();
  });

  it("keeps a section whose condition is satisfied", () => {
    const conditions: PresetCondition[] = [{ field: "iso", op: "<", value: "400", section: "curves" }];
    expect(applyConditionsToSubset(subset, conditions, photo)).toEqual(subset);
  });

  it("ANDs multiple rules on the same section — one failure excludes it", () => {
    const conditions: PresetCondition[] = [
      { field: "iso", op: "<", value: "400", section: "curves" },
      { field: "aperture", op: ">", value: "8", section: "curves" },
    ];
    const result = applyConditionsToSubset(subset, conditions, photo);
    expect(result?.curves).toBeUndefined();
    expect(result?.basic).toEqual(subset.basic);
  });

  it("treats a null photo (no selection) conservatively — every condition fails", () => {
    const conditions: PresetCondition[] = [{ field: "iso", op: ">", value: "0", section: null }];
    expect(applyConditionsToSubset(subset, conditions, null)).toBeNull();
  });
});

describe("evaluateRuleNode + applyRulesToSubset (Phase 13 Schritt 7, echter UND/ODER-Baum)", () => {
  const photo: PresetConditionPhotoMeta = { iso: 200, aperture: 4, focal_length: 50, camera_model: "EOS R5", lens: "RF 24-70mm" };
  const subset: PresetEdlSubset = {
    basic: { ...neutralEdlPayload().basic, exposure_ev: 0.5 },
    curves: neutralEdlPayload().curves,
  };

  it("evaluates an OR group as true when only one branch matches", () => {
    const node: RuleNode<PresetLeafCondition> = {
      type: "group",
      operator: "or",
      children: [
        { type: "condition", condition: { field: "iso", op: ">", value: "10000" } }, // falsch
        { type: "condition", condition: { field: "camera_model", op: "contains", value: "r5" } }, // wahr
      ],
    };
    const rules: PresetRules = [{ section: null, node }];
    expect(applyRulesToSubset(subset, rules, photo)).toEqual(subset);
  });

  it("evaluates a nested AND-inside-OR group correctly", () => {
    // (iso > 10000) ODER (Blende = 4 UND Brennweite = 50) — nur der
    // verschachtelte UND-Zweig trifft zu.
    const node: RuleNode<PresetLeafCondition> = {
      type: "group",
      operator: "or",
      children: [
        { type: "condition", condition: { field: "iso", op: ">", value: "10000" } },
        {
          type: "group",
          operator: "and",
          children: [
            { type: "condition", condition: { field: "aperture", op: "=", value: "4" } },
            { type: "condition", condition: { field: "focal_length", op: "=", value: "50" } },
          ],
        },
      ],
    };
    const rules: PresetRules = [{ section: "curves", node }];
    const result = applyRulesToSubset(subset, rules, photo);
    expect(result?.curves).toEqual(subset.curves);
  });

  it("excludes a section when its OR group has no matching branch", () => {
    const node: RuleNode<PresetLeafCondition> = {
      type: "group",
      operator: "or",
      children: [
        { type: "condition", condition: { field: "iso", op: ">", value: "10000" } },
        { type: "condition", condition: { field: "aperture", op: "=", value: "22" } },
      ],
    };
    const rules: PresetRules = [{ section: "curves", node }];
    const result = applyRulesToSubset(subset, rules, photo);
    expect(result?.curves).toBeUndefined();
    expect(result?.basic).toEqual(subset.basic);
  });

  it("an empty AND group is vacuously true, an empty OR group is vacuously false", () => {
    expect(evaluateRuleNode({ type: "group", operator: "and", children: [] }, () => false)).toBe(true);
    expect(evaluateRuleNode({ type: "group", operator: "or", children: [] }, () => true)).toBe(false);
  });
});

describe("parseRules — Migration alter, flacher Presets (Phase 13 Schritt 7)", () => {
  it("migrates a legacy flat PresetCondition[] into one AND-group rule per condition", () => {
    const legacy: PresetCondition[] = [
      { field: "iso", op: "<", value: "400", section: null },
      { field: "aperture", op: ">", value: "8", section: "curves" },
    ];
    const rules = parseRules(JSON.stringify(legacy));
    expect(rules).toHaveLength(2);
    expect(rules[0]).toEqual({ section: null, node: { type: "condition", condition: { field: "iso", op: "<", value: "400" } } });
    expect(rules[1]).toEqual({ section: "curves", node: { type: "condition", condition: { field: "aperture", op: ">", value: "8" } } });
  });

  it("round-trips a new-format tree unchanged through serializeRules/parseRules", () => {
    const rules: PresetRules = [
      {
        section: null,
        node: { type: "group", operator: "or", children: [{ type: "condition", condition: { field: "iso", op: ">", value: "100" } }] },
      },
    ];
    expect(parseRules(serializeRules(rules))).toEqual(rules);
  });

  it("treats unparseable JSON the same conservative way as parseConditions (empty)", () => {
    expect(parseRules("{not valid json")).toEqual([]);
  });
});

describe("diffEdlSubsets", () => {
  it("returns no entries for identical subsets", () => {
    const a = edlWithBasic({ exposure_ev: 0.5 });
    expect(diffEdlSubsets({ basic: a.basic }, { basic: { ...a.basic } })).toEqual([]);
  });

  it("reports a top-level scalar field that differs", () => {
    const a = edlWithBasic({ exposure_ev: 0.5 });
    const b = edlWithBasic({ exposure_ev: 0.8 });
    const diff = diffEdlSubsets({ basic: a.basic }, { basic: b.basic });
    expect(diff).toContainEqual({ path: "basic.exposure_ev", a: 0.5, b: 0.8 });
  });

  it("reports a nested field that differs (e.g. white_balance)", () => {
    const a = edlWithBasic({ white_balance: { temp_shift_kelvin: 100, tint_shift: 0 } });
    const b = edlWithBasic({ white_balance: { temp_shift_kelvin: 200, tint_shift: 0 } });
    const diff = diffEdlSubsets({ basic: a.basic }, { basic: b.basic });
    expect(diff).toContainEqual({ path: "basic.white_balance.temp_shift_kelvin", a: 100, b: 200 });
    expect(diff.some((entry) => entry.path === "basic.white_balance.tint_shift")).toBe(false);
  });

  it("treats an array field as an atomic value instead of diffing elements", () => {
    const regionA = { target_hue_degrees: 30, bandwidth_degrees: 40, feather: 15, hue_shift: 10, saturation_shift: 10, luminance_shift: 0 };
    const regionB = { ...regionA, hue_shift: 20 };
    const diff = diffEdlSubsets({ color_mixer: { regions: [regionA] } }, { color_mixer: { regions: [regionB] } });
    expect(diff).toEqual([{ path: "color_mixer.regions", a: [regionA], b: [regionB] }]);
  });

  it("reports a section present in only one of the two subsets as undefined on the other side", () => {
    const a = edlWithBasic({ exposure_ev: 0.5 });
    const diff = diffEdlSubsets({ basic: a.basic }, {});
    expect(diff.some((entry) => entry.path === "basic.exposure_ev" && entry.b === undefined)).toBe(true);
  });
});
