import { describe, expect, it } from "vitest";

import type { DevelopFrame } from "../hooks/useDevelopRender";
import { applySoftProof, type SoftProofSettings } from "./softProof";

function frameOf(pixels: number[]): DevelopFrame {
  return { width: pixels.length / 4, height: 1, pixels: new Uint8Array(pixels) };
}

const BASE_SETTINGS: SoftProofSettings = {
  profile: "srgb",
  intent: "perceptual",
  gamutWarning: false,
  paperWhite: false,
};

describe("applySoftProof", () => {
  it("sRGB + wahrnehmungsorientiert lässt Pixel unverändert (Identität)", () => {
    const frame = frameOf([10, 200, 50, 255, 0, 0, 0, 255, 255, 255, 255, 128]);
    const out = applySoftProof(frame, BASE_SETTINGS);
    expect(Array.from(out)).toEqual(Array.from(frame.pixels));
  });

  it("Graustufen-Druck (simuliert) + wahrnehmungsorientiert setzt jeden Kanal auf die Rec.601-Luminanz", () => {
    const frame = frameOf([255, 0, 0, 255]); // reines Rot
    const out = applySoftProof(frame, { ...BASE_SETTINGS, profile: "grayscale_sim" });
    const expectedLuma = Math.round(0.299 * 255);
    expect(out[0]).toBe(expectedLuma);
    expect(out[1]).toBe(expectedLuma);
    expect(out[2]).toBe(expectedLuma);
    expect(out[3]).toBe(255); // Alpha bleibt unangetastet
  });

  it("Relativ farbmetrisch lässt schwach gesättigte Pixel unangetastet, komprimiert aber stark gesättigte", () => {
    const mutedPink = frameOf([210, 190, 195, 255]); // niedrige Sättigung
    const pureRed = frameOf([255, 0, 0, 255]); // maximale Sättigung
    const settings: SoftProofSettings = { ...BASE_SETTINGS, profile: "print_sim", intent: "relative_colorimetric" };

    const mutedOut = applySoftProof(mutedPink, settings);
    expect(Array.from(mutedOut)).toEqual(Array.from(mutedPink.pixels));

    const redOut = applySoftProof(pureRed, settings);
    expect(Array.from(redOut)).not.toEqual(Array.from(pureRed.pixels));
    // R=G=B wäre nur bei vollständiger Entsättigung (factor 0) der Fall —
    // "Druck (simuliert)" hat factor 0.7, bleibt also näher an Rot als an Grau.
    expect(redOut[0]).toBeGreaterThan(redOut[1]!);
  });

  it("Farbumfangswarnung färbt außerhalb liegende Pixel magenta statt sie zu komprimieren", () => {
    const pureRed = frameOf([255, 0, 0, 255]);
    const settings: SoftProofSettings = { profile: "print_sim", intent: "relative_colorimetric", gamutWarning: true, paperWhite: false };
    const out = applySoftProof(pureRed, settings);
    expect(Array.from(out)).toEqual([255, 0, 255, 255]);
  });

  it("Papierweiß-Simulation komprimiert reines Schwarz und Weiß Richtung der Bodenwerte", () => {
    const frame = frameOf([0, 0, 0, 255, 255, 255, 255, 255]);
    const out = applySoftProof(frame, { ...BASE_SETTINGS, paperWhite: true });
    expect(out[0]).toBeGreaterThan(0);
    expect(out[0]).toBeLessThan(255);
    expect(out[4]).toBeLessThan(255);
    expect(out[4]).toBeGreaterThan(0);
  });

  it("verändert den übergebenen Frame-Puffer nicht (gibt einen neuen zurück)", () => {
    const frame = frameOf([255, 0, 0, 255]);
    const original = Array.from(frame.pixels);
    applySoftProof(frame, { ...BASE_SETTINGS, profile: "grayscale_sim", paperWhite: true });
    expect(Array.from(frame.pixels)).toEqual(original);
  });
});
