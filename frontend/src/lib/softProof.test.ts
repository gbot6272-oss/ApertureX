import { describe, expect, it } from "vitest";

import type { DevelopFrame } from "../hooks/useDevelopRender";
import { applyPaperWhite, encodeSoftProofSegment, type SoftProofSettings } from "./softProof";

function frameOf(pixels: number[]): DevelopFrame {
  return { width: pixels.length / 4, height: 1, pixels: new Uint8Array(pixels) };
}

const BASE_SETTINGS: SoftProofSettings = {
  profile: "srgb",
  customIccPath: "",
  intent: "perceptual",
  gamutWarning: false,
  paperWhite: false,
};

// Die eigentliche Farb-/Gamut-Transformation lief bis Phase 12 Schritt 6
// clientseitig (siehe die vorherige Fassung dieser Datei) und läuft seither
// als echter `lcms2`-Transform serverseitig über die `develop/...`-Route
// (`crates/apx-export/src/icc.rs::soft_proof_rgba8`, dort bereits mit
// eigenem Test abgedeckt). Hier bleiben nur die beiden clientseitigen
// Bausteine übrig: die Papierweiß-Tonwertkompression und die
// URL-Segment-Kodierung, die den Server-Aufruf überhaupt erst auslöst.

describe("applyPaperWhite", () => {
  it("komprimiert reines Schwarz und Weiß Richtung der Bodenwerte, lässt Alpha unangetastet", () => {
    const frame = frameOf([0, 0, 0, 255, 255, 255, 255, 128]);
    const out = applyPaperWhite(frame);
    expect(out[0]).toBeGreaterThan(0);
    expect(out[0]).toBeLessThan(255);
    expect(out[3]).toBe(255);
    expect(out[4]).toBeLessThan(255);
    expect(out[4]).toBeGreaterThan(0);
    expect(out[7]).toBe(128);
  });

  it("verändert den übergebenen Frame-Puffer nicht (gibt einen neuen zurück)", () => {
    const frame = frameOf([0, 0, 0, 255]);
    const original = Array.from(frame.pixels);
    applyPaperWhite(frame);
    expect(Array.from(frame.pixels)).toEqual(original);
  });
});

describe("encodeSoftProofSegment", () => {
  it("liefert 'none' ohne Einstellungen und ein dekodierbares base64url-JSON mit Einstellungen", () => {
    expect(encodeSoftProofSegment(null)).toBe("none");

    const settings: SoftProofSettings = { ...BASE_SETTINGS, profile: "adobe_rgb", gamutWarning: true };
    const segment = encodeSoftProofSegment(settings);
    expect(segment).not.toBe("none");
    expect(segment).not.toContain("/"); // muss die bestehende URL-Segmentierung nicht stören.

    const decoded = JSON.parse(atob(segment.replaceAll("-", "+").replaceAll("_", "/"))) as { target: string; gamut_warning: boolean };
    expect(decoded.target).toBe("adobe_rgb");
    expect(decoded.gamut_warning).toBe(true);
  });
});
