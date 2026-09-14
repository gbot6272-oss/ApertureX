import { describe, expect, it } from "vitest";

import { buildZoneOverlay, ZONE_COLORS, zoneIndexFor, zoneLuminance } from "./zoneOverlay";

describe("Zonen-Überlagerung (Phase 30)", () => {
  it("teilt die Luminanz in genau zehn Zonen von Schwarz bis Weiß", () => {
    expect(zoneIndexFor(0)).toBe(0);
    expect(zoneIndexFor(1)).toBe(9);
    expect(zoneIndexFor(0.5)).toBe(5);
    expect(ZONE_COLORS).toHaveLength(10);
  });

  it("klemmt Werte ausserhalb 0..1 statt aus dem Farbfeld zu laufen", () => {
    expect(zoneIndexFor(-3)).toBe(0);
    expect(zoneIndexFor(7)).toBe(9);
  });

  /** Die Einteilung MUSS der Rust-Seite entsprechen: Zone `z` liegt bei
   * Luminanz `z/9`. Zeigt die Überlagerung auf eine andere Zone als der
   * Regler, ist sie schlimmer als keine. */
  it("trifft für jede Zonenmitte genau ihre eigene Zone", () => {
    for (let zone = 0; zone < 10; zone += 1) {
      expect(zoneIndexFor(zone / 9)).toBe(zone);
    }
  });

  it("nutzt dieselbe Luminanz-Gewichtung wie die Pipeline", () => {
    expect(zoneLuminance(255, 255, 255)).toBeCloseTo(1, 6);
    expect(zoneLuminance(0, 0, 0)).toBe(0);
    // 0.3 R + 0.59 G + 0.11 B
    expect(zoneLuminance(255, 0, 0)).toBeCloseTo(0.3, 6);
    expect(zoneLuminance(0, 255, 0)).toBeCloseTo(0.59, 6);
  });

  it("färbt jedes Pixel nach seiner Zone ein", () => {
    // Zwei Pixel: schwarz und weiß.
    const pixels = new Uint8ClampedArray([0, 0, 0, 255, 255, 255, 255, 255]);
    const overlay = buildZoneOverlay(pixels, 2, 1);
    expect([overlay[0], overlay[1], overlay[2]]).toEqual([...ZONE_COLORS[0]!]);
    expect([overlay[4], overlay[5], overlay[6]]).toEqual([...ZONE_COLORS[9]!]);
  });

  it("hebt bei gewählter Zone genau diese hervor und blendet den Rest zurück", () => {
    const pixels = new Uint8ClampedArray([0, 0, 0, 255, 255, 255, 255, 255]);
    const overlay = buildZoneOverlay(pixels, 2, 1, 9);
    expect(overlay[7]).toBe(255); // die hervorgehobene Zone
    expect(overlay[3]).toBeLessThan(80); // alles andere deutlich schwächer
  });
});
