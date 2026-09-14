import { describe, expect, it } from "vitest";

import { dragDelta, HISTOGRAM_ZONES, zoneAt } from "./histogramZones";

describe("HISTOGRAM_ZONES", () => {
  it("deckt die ganze Breite lückenlos ab", () => {
    expect(HISTOGRAM_ZONES[0]!.start).toBe(0);
    expect(HISTOGRAM_ZONES[HISTOGRAM_ZONES.length - 1]!.end).toBe(1);
    for (let i = 1; i < HISTOGRAM_ZONES.length; i += 1) {
      expect(HISTOGRAM_ZONES[i]!.start).toBe(HISTOGRAM_ZONES[i - 1]!.end);
    }
  });

  it("belegt jedes der fünf Tonwertfelder genau einmal", () => {
    const fields = HISTOGRAM_ZONES.map((zone) => zone.field);
    expect(new Set(fields).size).toBe(fields.length);
    expect(fields).toEqual(["blacks", "shadows", "exposure_ev", "highlights", "whites"]);
  });

  it("gibt der Belichtung die breiteste Zone", () => {
    // Die Zonen sind bewusst NICHT gleich breit: der Weisspunkt betrifft
    // nur die obersten Werte, die Belichtung die ganze Mitte.
    const width = (field: string) => {
      const zone = HISTOGRAM_ZONES.find((z) => z.field === field)!;
      return zone.end - zone.start;
    };
    expect(width("exposure_ev")).toBeGreaterThan(width("whites"));
    expect(width("exposure_ev")).toBeGreaterThan(width("blacks"));
  });
});

describe("zoneAt", () => {
  it("trifft an den Rändern Schwarz und Weiß", () => {
    expect(zoneAt(0).field).toBe("blacks");
    expect(zoneAt(1).field).toBe("whites");
  });

  it("trifft in der Mitte die Belichtung", () => {
    expect(zoneAt(0.5).field).toBe("exposure_ev");
  });

  it("klemmt außerhalb liegende Positionen, statt ins Leere zu greifen", () => {
    // Wer knapp neben das Histogramm greift, meint erkennbar die
    // Randzone — ein wirkungsloser Griff wäre nur ärgerlich.
    expect(zoneAt(-0.4).field).toBe("blacks");
    expect(zoneAt(1.7).field).toBe("whites");
  });

  it("ordnet jede Position im Bereich einer Zone zu", () => {
    for (let i = 0; i <= 100; i += 1) {
      expect(zoneAt(i / 100)).toBeDefined();
    }
  });
});

describe("dragDelta", () => {
  it("hellt beim Ziehen nach rechts auf", () => {
    const zone = HISTOGRAM_ZONES.find((z) => z.field === "exposure_ev")!;
    expect(dragDelta(zone, 40, 200)).toBeGreaterThan(0);
    expect(dragDelta(zone, -40, 200)).toBeLessThan(0);
  });

  it("rechnet die Belichtung in Blendenstufen, nicht in Prozent", () => {
    // Ohne eigene Skala wäre die Belichtung beim Ziehen entweder
    // unbrauchbar träge oder unsteuerbar sprunghaft.
    const exposure = HISTOGRAM_ZONES.find((z) => z.field === "exposure_ev")!;
    const shadows = HISTOGRAM_ZONES.find((z) => z.field === "shadows")!;
    const halfWidth = dragDelta(exposure, 100, 200);
    expect(halfWidth).toBeCloseTo(4, 6);
    expect(dragDelta(shadows, 100, 200)).toBeCloseTo(100, 6);
  });

  it("liefert bei Breite null keine Division durch null", () => {
    expect(dragDelta(HISTOGRAM_ZONES[0]!, 50, 0)).toBe(0);
  });
});
