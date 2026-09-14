import { describe, expect, it } from "vitest";

import { buildPeakingOverlay, sobelMagnitude } from "./focusPeaking";

/** RGBA-Bild aus einer Graustufen-Matrix. */
function gray(rows: number[][]): { pixels: Uint8ClampedArray; width: number; height: number } {
  const height = rows.length;
  const width = rows[0]?.length ?? 0;
  const pixels = new Uint8ClampedArray(width * height * 4);
  for (let y = 0; y < height; y += 1) {
    const row = rows[y] ?? [];
    for (let x = 0; x < width; x += 1) {
      const value = row[x] ?? 0;
      const base = (y * width + x) * 4;
      pixels[base] = value;
      pixels[base + 1] = value;
      pixels[base + 2] = value;
      pixels[base + 3] = 255;
    }
  }
  return { pixels, width, height };
}

function uniform(width: number, height: number, value: number) {
  return gray(Array.from({ length: height }, () => Array.from({ length: width }, () => value)));
}

/** Linke Hälfte schwarz, rechte weiß — eine senkrechte Kante in der Mitte. */
function verticalEdge(width: number, height: number) {
  return gray(
    Array.from({ length: height }, () =>
      Array.from({ length: width }, (_, x) => (x < width / 2 ? 0 : 255)),
    ),
  );
}

describe("sobelMagnitude", () => {
  it("findet in einer gleichmäßigen Fläche überhaupt keine Kante", () => {
    const { pixels, width, height } = uniform(8, 8, 128);
    const magnitude = sobelMagnitude(pixels, width, height);
    expect(Math.max(...magnitude)).toBe(0);
  });

  it("findet eine senkrechte Kante genau dort, wo sie liegt", () => {
    const { pixels, width, height } = verticalEdge(9, 9);
    const magnitude = sobelMagnitude(pixels, width, height);

    // Zeile in der Bildmitte absuchen: das Maximum muss an der
    // Helligkeitsstufe liegen (Spalte 4 oder 5), nicht irgendwo sonst.
    const row = Array.from({ length: width }, (_, x) => magnitude[4 * width + x] ?? 0);
    const peak = row.indexOf(Math.max(...row));
    expect([4, 5]).toContain(peak);
  });

  it("lässt den Bildrand unberührt, statt dort eine Scheinkante zu erfinden", () => {
    // Genau das wäre beim Peaking ein leuchtender Rahmen um jedes Foto.
    const { pixels, width, height } = verticalEdge(9, 9);
    const magnitude = sobelMagnitude(pixels, width, height);
    for (let x = 0; x < width; x += 1) {
      expect(magnitude[x]).toBe(0);
      expect(magnitude[(height - 1) * width + x]).toBe(0);
    }
    for (let y = 0; y < height; y += 1) {
      expect(magnitude[y * width]).toBe(0);
      expect(magnitude[y * width + width - 1]).toBe(0);
    }
  });

  it("kommt mit einem Bild klar, das kleiner als der Kern ist", () => {
    const { pixels, width, height } = uniform(2, 2, 200);
    expect(() => sobelMagnitude(pixels, width, height)).not.toThrow();
    expect(sobelMagnitude(pixels, width, height)).toHaveLength(4);
  });
});

describe("buildPeakingOverlay", () => {
  it("markiert in einer gleichmäßigen Fläche nichts — die Überlagerung bleibt ganz durchsichtig", () => {
    const { pixels, width, height } = uniform(8, 8, 90);
    const overlay = buildPeakingOverlay(pixels, width, height, 0.2);
    expect(overlay.coverage).toBe(0);
    for (let i = 3; i < overlay.pixels.length; i += 4) {
      expect(overlay.pixels[i]).toBe(0);
    }
  });

  it("markiert die Kante und nur die Kante", () => {
    const { pixels, width, height } = verticalEdge(9, 9);
    const overlay = buildPeakingOverlay(pixels, width, height, 0.2);
    expect(overlay.coverage).toBeGreaterThan(0);

    // Ein Pixel weit links liegt in der schwarzen Fläche — dort darf
    // nichts leuchten, sonst wäre die Markierung wertlos.
    expect(overlay.pixels[(4 * width + 1) * 4 + 3]).toBe(0);
  });

  it("markiert bei kleinerem Schwellwert mehr, nie weniger", () => {
    const { pixels, width, height } = verticalEdge(16, 16);
    const streng = buildPeakingOverlay(pixels, width, height, 0.4).coverage;
    const locker = buildPeakingOverlay(pixels, width, height, 0.1).coverage;
    expect(locker).toBeGreaterThanOrEqual(streng);
  });

  it("benutzt die gewählte Farbe", () => {
    const { pixels, width, height } = verticalEdge(9, 9);
    const overlay = buildPeakingOverlay(pixels, width, height, 0.2, "green");
    const markedIndex = [...Array(width * height).keys()].find((i) => (overlay.pixels[i * 4 + 3] ?? 0) > 0);
    expect(markedIndex).toBeDefined();
    expect(overlay.pixels[markedIndex! * 4]).toBe(40);
    expect(overlay.pixels[markedIndex! * 4 + 1]).toBe(255);
  });
});
