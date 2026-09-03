import { describe, expect, it } from "vitest";

import { computeHarmonizeShifts, harmonyTargetHues, signedHueDelta } from "./colorHarmony";
import type { PaletteColorDto } from "./tauri";

function color(hue: number, chroma: number, percentage: number): PaletteColorDto {
  return { r: 128, g: 128, b: 128, hue_degrees: hue, chroma, lightness: 50, percentage };
}

describe("harmonyTargetHues", () => {
  it("complementary is exactly opposite the base hue", () => {
    expect(harmonyTargetHues(30, "complementary")).toEqual([30, 210]);
  });

  it("triadic splits the wheel into three equal thirds", () => {
    expect(harmonyTargetHues(0, "triadic")).toEqual([0, 120, 240]);
  });

  it("splitComplementary sits on either side of the pure complement", () => {
    expect(harmonyTargetHues(0, "splitComplementary")).toEqual([0, 150, 210]);
  });

  it("analogous stays close to the base hue on both sides", () => {
    expect(harmonyTargetHues(10, "analogous")).toEqual([340, 10, 40]);
  });

  it("wraps hues into 0..360", () => {
    const targets = harmonyTargetHues(350, "complementary");
    expect(targets[0]).toBeCloseTo(350);
    expect(targets[1]).toBeCloseTo(170);
  });
});

describe("signedHueDelta", () => {
  it("is positive when the shortest path goes clockwise (increasing degrees)", () => {
    expect(signedHueDelta(10, 40)).toBeCloseTo(30);
  });

  it("is negative when the shortest path goes counter-clockwise", () => {
    expect(signedHueDelta(40, 10)).toBeCloseTo(-30);
  });

  it("takes the short way around the 0/360 wraparound", () => {
    expect(signedHueDelta(350, 10)).toBeCloseTo(20);
    expect(signedHueDelta(10, 350)).toBeCloseTo(-20);
  });
});

describe("computeHarmonizeShifts", () => {
  it("ignores low-chroma (near-gray) colors", () => {
    const palette = [color(0, 2, 1.0)];
    expect(computeHarmonizeShifts(palette, 0, "complementary")).toEqual([]);
  });

  it("proposes no shift for a color that already sits on its harmony target", () => {
    const palette = [color(0, 30, 1.0)];
    const shifts = computeHarmonizeShifts(palette, 0, "complementary");
    expect(shifts).toHaveLength(1);
    expect(shifts[0]!.hueRegler).toBeCloseTo(0, 1);
  });

  it("proposes a positive shift toward the nearest harmony target", () => {
    // Ein Grünton (~120°) bei einer Komplementär-Harmonie mit Basis 0°
    // (Ziele 0°/180°) liegt näher an 180° und muss deshalb Richtung
    // größerer Gradzahl (positiv) verschoben werden.
    const palette = [color(120, 40, 1.0)];
    const shifts = computeHarmonizeShifts(palette, 0, "complementary");
    expect(shifts).toHaveLength(1);
    expect(shifts[0]!.band).toBe("green");
    expect(shifts[0]!.hueRegler).toBeGreaterThan(0);
  });

  it("keeps only the highest-percentage color when two colors land in the same band", () => {
    const palette = [color(0, 40, 0.2), color(5, 40, 0.8)];
    const shifts = computeHarmonizeShifts(palette, 0, "complementary");
    expect(shifts).toHaveLength(1);
    // Die 5°-Farbe (80% Anteil) gewinnt gegenüber der exakt auf dem Ziel
    // liegenden 0°-Farbe (20% Anteil) — deshalb ein winziger, aber
    // nicht exakt neutraler Regler-Wert.
    expect(shifts[0]!.hueRegler).not.toBeCloseTo(0, 3);
  });

  it("clamps extreme deltas to the slider's -100..100 range", () => {
    // 90° Abstand zum nächsten Ziel wäre eine 90°-Verschiebung, weit
    // über der 60°-Obergrenze des Reglers.
    const palette = [color(90, 40, 1.0)];
    const shifts = computeHarmonizeShifts(palette, 0, "complementary");
    expect(Math.abs(shifts[0]!.hueRegler)).toBeLessThanOrEqual(100);
  });
});
