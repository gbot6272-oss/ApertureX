import { describe, expect, it } from "vitest";

import { computeVectorscope } from "./vectorscope";

function makePixels(entries: Array<[number, number, number]>): Uint8ClampedArray {
  const pixels = new Uint8ClampedArray(entries.length * 4);
  entries.forEach(([r, g, b], i) => {
    pixels[i * 4] = r;
    pixels[i * 4 + 1] = g;
    pixels[i * 4 + 2] = b;
    pixels[i * 4 + 3] = 255;
  });
  return pixels;
}

describe("computeVectorscope", () => {
  it("puts every neutral gray pixel into the same cell, near the center of the grid", () => {
    // Drei unterschiedlich helle, aber allesamt unbunte (R=G=B) Pixel
    // haben alle Cb=Cr=128 (exakter Chroma-Nullpunkt) und müssen deshalb
    // in derselben Rasterzelle landen — unabhängig von ihrer Helligkeit,
    // die das Vektorskop bewusst nicht abbildet.
    const pixels = makePixels([
      [128, 128, 128],
      [40, 40, 40],
      [220, 220, 220],
    ]);
    const scope = computeVectorscope(pixels, 3, 1);
    let hitCell = -1;
    let hitCount = 0;
    for (let i = 0; i < scope.grid.length; i++) {
      if ((scope.grid[i] ?? 0) > 0) {
        hitCell = i;
        hitCount++;
      }
    }
    expect(hitCount).toBe(1);
    expect(scope.grid[hitCell] ?? 0).toBe(3);
    expect(scope.maxCount).toBe(3);
    // "Nahe der Mitte" statt exakt mittig: `128/255` ist kein exaktes
    // Vielfaches von `1/(size-1)`, die gerundete Rasterzelle kann daher
    // um ein bis zwei Zellen von der rechnerischen Mitte abweichen.
    const center = (scope.size - 1) / 2;
    const x = hitCell % scope.size;
    const y = Math.floor(hitCell / scope.size);
    expect(Math.abs(x - center)).toBeLessThanOrEqual(2);
    expect(Math.abs(y - center)).toBeLessThanOrEqual(2);
  });

  it("places a saturated red pixel away from the center, on the red half of the plane", () => {
    const pixels = makePixels([[255, 0, 0]]);
    const scope = computeVectorscope(pixels, 1, 1);
    const center = (scope.size - 1) / 2;
    let redX = -1;
    let redY = -1;
    for (let y = 0; y < scope.size; y++) {
      for (let x = 0; x < scope.size; x++) {
        if ((scope.grid[y * scope.size + x] ?? 0) > 0) {
          redX = x;
          redY = y;
        }
      }
    }
    const distance = Math.hypot(redX - center, redY - center);
    expect(distance).toBeGreaterThan(scope.size / 4);
  });

  it("places saturated red and saturated cyan-ish blue on opposite sides of the center", () => {
    // Rot (hohes Cr, niedriges Cb) und ein blaues Pixel (niedriges Cr,
    // hohes Cb) müssen auf gegenüberliegenden Seiten des Ursprungs
    // landen — der Kern der Farbton/Winkel-Semantik eines Vektorskops.
    const pixels = makePixels([
      [255, 0, 0],
      [0, 0, 255],
    ]);
    const scope = computeVectorscope(pixels, 2, 1);
    const nonZero: Array<{ x: number; y: number }> = [];
    for (let y = 0; y < scope.size; y++) {
      for (let x = 0; x < scope.size; x++) {
        if ((scope.grid[y * scope.size + x] ?? 0) > 0) nonZero.push({ x, y });
      }
    }
    expect(nonZero).toHaveLength(2);
    const [a, b] = nonZero as [{ x: number; y: number }, { x: number; y: number }];
    // Deutlich unterschiedliche X-Position (Cb-Achse) für Rot vs. Blau.
    expect(Math.abs(a.x - b.x)).toBeGreaterThan(scope.size / 4);
  });
});
