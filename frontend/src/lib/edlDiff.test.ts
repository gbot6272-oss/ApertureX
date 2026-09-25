import { describe, expect, it } from "vitest";

import { neutralEdlPayload } from "./edl";
import { diffEdlPayloads, labelForPath } from "./edlDiff";

describe("diffEdlPayloads", () => {
  it("meldet nichts, wenn sich nichts geaendert hat", () => {
    expect(diffEdlPayloads(neutralEdlPayload(), neutralEdlPayload())).toEqual([]);
  });

  it("findet einen geaenderten Regler und beschriftet ihn lesbar", () => {
    const after = neutralEdlPayload();
    after.basic.exposure_ev = 0.75;
    const changes = diffEdlPayloads(neutralEdlPayload(), after);
    expect(changes).toHaveLength(1);
    expect(changes[0]!.path).toBe("basic.exposure_ev");
    expect(changes[0]!.label).toBe("Belichtung");
    // Ganzzahlen bleiben ohne Nachkommastellen — "0" liest sich besser
    // als "0.00", und der Regler zeigt es genauso.
    expect(changes[0]!.before).toBe("0");
    expect(changes[0]!.after).toBe("0.75");
  });

  it("liefert bei zwei frisch gebauten Neutralstaenden wirklich zwei unabhaengige Objekte", () => {
    // Regressionstest zum Fund aus Phase 34 F6: `neutralEdlPayload`
    // gab einige Gruppen als Referenz auf die modulweite Konstante
    // zurueck. Eine Aenderung an einem Stand veraenderte damit still
    // auch jeden anderen — und dieser Vergleich meldete "nichts
    // geaendert".
    const a = neutralEdlPayload();
    const b = neutralEdlPayload();
    b.basic.exposure_ev = 1.5;
    expect(a.basic.exposure_ev).not.toBe(1.5);
  });

  it("findet auch Aenderungen tief in einer Gruppe", () => {
    const after = neutralEdlPayload();
    after.details.sharpen_amount = 42;
    const changes = diffEdlPayloads(neutralEdlPayload(), after);
    expect(changes.map((c) => c.path)).toContain("details.sharpen_amount");
  });

  it("zaehlt Listen, statt sie Eintrag fuer Eintrag aufzuzaehlen", () => {
    const after = neutralEdlPayload();
    after.interactive.point_lights.lights = [
      { x: 0.5, y: 0.5, radius: 0.3, intensity: 0.5, falloff: 2, color_rgb: [1, 1, 1] },
      { x: 0.2, y: 0.2, radius: 0.3, intensity: 0.5, falloff: 2, color_rgb: [1, 1, 1] },
    ];
    const changes = diffEdlPayloads(neutralEdlPayload(), after);
    const lights = changes.find((c) => c.path === "interactive.point_lights.lights");
    expect(lights?.before).toBe("0 Einträge");
    expect(lights?.after).toBe("2 Einträge");
  });

  it("uebersetzt Wahrheitswerte in an/aus", () => {
    const before = neutralEdlPayload();
    const after = neutralEdlPayload();
    after.interactive.color_replace.preserve_luma = !before.interactive.color_replace.preserve_luma;
    const change = diffEdlPayloads(before, after)[0]!;
    expect([change.before, change.after].sort()).toEqual(["an", "aus"]);
  });

  it("haelt Rundungsrauschen nicht fuer eine Aenderung", () => {
    // Ein Wert, der durch JSON-Hin-und-Her minimal abweicht, ist keine
    // Bearbeitung, die jemand vorgenommen hat.
    const before = neutralEdlPayload();
    const after = neutralEdlPayload();
    after.basic.exposure_ev = before.basic.exposure_ev + 1e-9;
    expect(diffEdlPayloads(before, after)).toEqual([]);
  });

  it("meldet ein Feld, das nur auf einer Seite vorkommt", () => {
    // Ein Altstand ohne ein spaeter dazugekommenes Feld (siehe
    // ADR-0069) darf nicht stillschweigend als "gleich" gelten.
    const before = neutralEdlPayload() as unknown as Record<string, unknown>;
    delete (before.basic as Record<string, unknown>).exposure_ev;
    const changes = diffEdlPayloads(before as never, neutralEdlPayload());
    expect(changes.map((c) => c.path)).toContain("basic.exposure_ev");
    expect(changes.find((c) => c.path === "basic.exposure_ev")?.before).toBe("—");
  });

  it("liefert die Aenderungen nach Pfad sortiert", () => {
    const after = neutralEdlPayload();
    after.basic.contrast = 10;
    after.basic.exposure_ev = 1;
    const paths = diffEdlPayloads(neutralEdlPayload(), after).map((c) => c.path);
    expect(paths).toEqual([...paths].sort());
  });
});

describe("labelForPath", () => {
  it("faellt auf den Pfad zurueck, wenn es keinen Regler dazu gibt", () => {
    expect(labelForPath("irgendwas.unbekannt")).toBe("irgendwas.unbekannt");
  });
});
