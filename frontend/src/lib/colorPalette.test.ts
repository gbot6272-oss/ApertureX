import { describe, expect, it } from "vitest";

import { extractPalette, toHex } from "./colorPalette";

function imageFrom(colors: Array<[number, number, number, number?]>): {
  pixels: Uint8ClampedArray;
  width: number;
  height: number;
} {
  const pixels = new Uint8ClampedArray(colors.length * 4);
  colors.forEach((color, index) => {
    pixels[index * 4] = color[0];
    pixels[index * 4 + 1] = color[1];
    pixels[index * 4 + 2] = color[2];
    pixels[index * 4 + 3] = color[3] ?? 255;
  });
  return { pixels, width: colors.length, height: 1 };
}

function repeat(color: [number, number, number], times: number) {
  return Array.from({ length: times }, () => color);
}

describe("extractPalette", () => {
  it("findet in einem einfarbigen Bild genau diese eine Farbe", () => {
    const { pixels, width, height } = imageFrom(repeat([200, 100, 50], 64));
    const palette = extractPalette(pixels, width, height, 4, 1);
    expect(palette[0]?.r).toBeCloseTo(200, 0);
    expect(palette[0]?.g).toBeCloseTo(100, 0);
    expect(palette[0]?.b).toBeCloseTo(50, 0);
    // Die übrigen Cluster sind leer und fallen heraus.
    expect(palette).toHaveLength(1);
    expect(palette[0]?.share).toBeCloseTo(1, 5);
  });

  it("trennt zwei klar verschiedene Farben und gewichtet sie nach Häufigkeit", () => {
    const { pixels, width, height } = imageFrom([
      ...repeat([255, 0, 0], 48),
      ...repeat([0, 0, 255], 16),
    ]);
    const palette = extractPalette(pixels, width, height, 2, 1);
    expect(palette).toHaveLength(2);
    // Häufigste zuerst: Rot mit 75 %.
    expect(palette[0]?.r).toBeGreaterThan(200);
    expect(palette[0]?.share).toBeCloseTo(0.75, 2);
    expect(palette[1]?.b).toBeGreaterThan(200);
    expect(palette[1]?.share).toBeCloseTo(0.25, 2);
  });

  it("liefert bei zweimaligem Aufruf dasselbe Ergebnis", () => {
    // Ohne feste Startpunkte käme bei jedem Aufruf eine andere Palette
    // heraus — auf so etwas kann sich niemand verlassen.
    const { pixels, width, height } = imageFrom([
      ...repeat([12, 200, 60], 20),
      ...repeat([210, 40, 90], 20),
      ...repeat([30, 60, 220], 20),
    ]);
    const first = extractPalette(pixels, width, height, 3, 1);
    const second = extractPalette(pixels, width, height, 3, 1);
    expect(second).toEqual(first);
  });

  it("überspringt durchsichtige Pixel, statt sie als Schwarz zu zählen", () => {
    const { pixels, width, height } = imageFrom([
      ...repeat([240, 240, 240], 8),
      ...Array.from({ length: 56 }, () => [0, 0, 0, 0] as [number, number, number, number]),
    ]);
    const palette = extractPalette(pixels, width, height, 3, 1);
    expect(palette).toHaveLength(1);
    expect(palette[0]?.r).toBeCloseTo(240, 0);
  });

  it("kommt mit einem leeren Bild klar", () => {
    expect(extractPalette(new Uint8ClampedArray(0), 0, 0, 5, 1)).toEqual([]);
  });

  it("fordert nie mehr Farben, als es Stichproben gibt", () => {
    const { pixels, width, height } = imageFrom(repeat([10, 20, 30], 2));
    expect(extractPalette(pixels, width, height, 8, 1).length).toBeLessThanOrEqual(2);
  });

  it("erfindet keine Farbe, wenn ein Cluster leer bleibt", () => {
    // Alle Stichproben sind hell; ein leerer Cluster darf NICHT nach
    // Schwarz springen und damit eine Farbe behaupten, die es nicht gibt.
    const { pixels, width, height } = imageFrom(repeat([230, 225, 220], 32));
    const palette = extractPalette(pixels, width, height, 5, 1);
    for (const swatch of palette) {
      expect(swatch.r).toBeGreaterThan(200);
    }
  });
});

describe("toHex", () => {
  it("schreibt zweistellig und klemmt an den Rändern", () => {
    expect(toHex(0, 0, 0)).toBe("#000000");
    expect(toHex(255, 255, 255)).toBe("#ffffff");
    expect(toHex(9, 16, 300)).toBe("#0910ff");
    expect(toHex(-5, 128, 64)).toBe("#008040");
  });
});
