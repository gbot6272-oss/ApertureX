import { describe, expect, it } from "vitest";

import { parseConditions, parseEdlSubset, serializeConditions, serializeEdlSubset } from "./presets";
import type { PresetCondition, PresetEdlSubset } from "./presets";

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
