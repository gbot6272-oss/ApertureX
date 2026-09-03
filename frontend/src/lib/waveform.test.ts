import { describe, expect, it } from "vitest";

import { computeWaveform } from "./waveform";

function makeUniformRow(value: number, width: number): Uint8ClampedArray {
  const pixels = new Uint8ClampedArray(width * 4);
  for (let x = 0; x < width; x++) {
    pixels[x * 4] = value;
    pixels[x * 4 + 1] = value;
    pixels[x * 4 + 2] = value;
    pixels[x * 4 + 3] = 255;
  }
  return pixels;
}

describe("computeWaveform", () => {
  it("counts a uniform-value row entirely into the matching value bucket, spread across all columns", () => {
    const width = 4;
    const pixels = makeUniformRow(200, width);
    const wave = computeWaveform(pixels, width, 1);

    let totalAtValue200 = 0;
    for (let col = 0; col < wave.columns; col++) {
      totalAtValue200 += wave.r[col * wave.rows + 200] ?? 0;
    }
    expect(totalAtValue200).toBe(width);
    expect(wave.maxCount).toBeGreaterThan(0);
  });

  it("keeps the left half of the image separate from the right half in the column axis", () => {
    // Linke Bildhälfte dunkel, rechte Bildhälfte hell — muss sich in
    // unterschiedlichen Spalten-Buckets niederschlagen, nicht in
    // denselben (das wäre nur ein Gesamt-Histogramm, keine Wellenform).
    const width = 8;
    const pixels = new Uint8ClampedArray(width * 4);
    for (let x = 0; x < width; x++) {
      const value = x < width / 2 ? 20 : 220;
      pixels[x * 4] = value;
      pixels[x * 4 + 1] = value;
      pixels[x * 4 + 2] = value;
      pixels[x * 4 + 3] = 255;
    }
    const wave = computeWaveform(pixels, width, 1);

    // Spalten-Bucket exakt wie `computeWaveform` selbst berechnet, statt
    // den letzten Ausgabe-Bucket anzunehmen — bei einer Bildbreite (8),
    // die viel kleiner als `COLUMN_BUCKETS` (256) ist, erreicht kein `x`
    // je den allerletzten Ausgabe-Bucket (der bräuchte `x === width`).
    const leftColumn = Math.floor((0 / width) * wave.columns);
    const rightColumn = Math.floor(((width - 1) / width) * wave.columns);
    expect(wave.r[leftColumn * wave.rows + 20]).toBeGreaterThan(0);
    expect(wave.r[leftColumn * wave.rows + 220]).toBe(0);
    expect(wave.r[rightColumn * wave.rows + 220]).toBeGreaterThan(0);
    expect(wave.r[rightColumn * wave.rows + 20]).toBe(0);
  });

  it("tracks the three channels independently", () => {
    const pixels = new Uint8ClampedArray(4);
    pixels[0] = 10;
    pixels[1] = 150;
    pixels[2] = 250;
    pixels[3] = 255;
    const wave = computeWaveform(pixels, 1, 1);
    expect(wave.r[10]).toBe(1);
    expect(wave.g[150]).toBe(1);
    expect(wave.b[250]).toBe(1);
    expect(wave.r[150]).toBe(0);
  });
});
