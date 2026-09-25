import { describe, expect, it } from "vitest";

import {
  EDL_SCHEMA_VERSION,
  neutralEdlPayload,
  normalizeEdlPayload,
  parseEdlEnvelopeJson,
} from "./edl";

/**
 * Regressionstests zum gemeldeten Absturz (siehe `DECISIONS.md`
 * ADR-0069):
 *
 *     TypeError: Cannot read properties of undefined (reading 'point_lights')
 *         at ImageToolsPanel
 *
 * `apx-core`s `EdlEnvelope.payload` ist ein opakes `serde_json::Value` —
 * das Backend reicht gespeicherte EDLs wortwoertlich durch. Ein von
 * einer aelteren App-Version geschriebenes EDL kennt die seither
 * additiv dazugekommenen Felder nicht; das Frontend castete blind und
 * bekam `undefined`.
 */

function envelope(payload: unknown): string {
  return JSON.stringify({ schema_version: EDL_SCHEMA_VERSION, payload });
}

describe("normalizeEdlPayload", () => {
  it("ergaenzt ein vor Phase 30 geschriebenes EDL um `interactive`", () => {
    const alt = neutralEdlPayload() as unknown as Record<string, unknown>;
    delete alt.interactive;

    const result = normalizeEdlPayload(alt);
    expect(result.interactive).toBeDefined();
    expect(result.interactive.point_lights.lights).toEqual([]);
  });

  it("ergaenzt jedes andere additiv dazugekommene Feld ebenso", () => {
    const alt = neutralEdlPayload() as unknown as Record<string, unknown>;
    for (const key of ["creative", "light_optics", "frame", "virtual_aperture"]) {
      delete alt[key];
    }

    const result = normalizeEdlPayload(alt) as unknown as Record<string, unknown>;
    for (const key of ["creative", "light_optics", "frame", "virtual_aperture"]) {
      expect(result[key], key).toBeDefined();
    }
  });

  it("fuellt auch ein Feld auf, das INNERHALB einer bekannten Gruppe fehlt", () => {
    // Der haeufigere Fall in der Praxis: die Gruppe gab es schon, ein
    // einzelnes Unterfeld kam spaeter dazu.
    const alt = neutralEdlPayload() as unknown as Record<string, unknown>;
    const interactive = { ...(alt.interactive as Record<string, unknown>) };
    delete interactive.point_lights;
    alt.interactive = interactive;

    const result = normalizeEdlPayload(alt);
    expect(result.interactive.point_lights).toBeDefined();
    expect(result.interactive.spotlight).toBeDefined();
  });

  it("behaelt gespeicherte Werte und ueberschreibt sie nicht mit Neutralwerten", () => {
    const alt = neutralEdlPayload();
    alt.basic.exposure_ev = 1.75;

    const result = normalizeEdlPayload(alt);
    expect(result.basic.exposure_ev).toBe(1.75);
  });

  it("ersetzt Listen vollstaendig, statt sie elementweise zu mischen", () => {
    // Eine bewusst geleerte Liste darf nicht aus den Neutralwerten
    // wieder aufgefuellt werden — und eine gefuellte nicht mit
    // Neutralwerten vermischt.
    const withLight = neutralEdlPayload();
    withLight.interactive.point_lights.lights = [
      { x: 0.5, y: 0.5, radius: 0.3, intensity: 0.5, falloff: 2, color_rgb: [1, 1, 1] },
    ];

    const result = normalizeEdlPayload(withLight);
    expect(result.interactive.point_lights.lights).toHaveLength(1);
    expect(result.interactive.point_lights.lights[0]!.x).toBe(0.5);
  });

  it("parseEdlEnvelopeJson liefert ein vollstaendiges EDL fuer einen Altstand", () => {
    const alt = neutralEdlPayload() as unknown as Record<string, unknown>;
    delete alt.interactive;

    const parsed = parseEdlEnvelopeJson(envelope(alt));
    expect(parsed).not.toBeNull();
    // Genau der Zugriff, an dem `ImageToolsPanel` abgestuerzt ist.
    expect(parsed!.interactive.point_lights.lights).toEqual([]);
  });

  it("parseEdlEnvelopeJson weist eine fremde Schema-Version weiterhin ab", () => {
    expect(parseEdlEnvelopeJson(JSON.stringify({ schema_version: 1, payload: {} }))).toBeNull();
    expect(parseEdlEnvelopeJson("kein json")).toBeNull();
  });
});
