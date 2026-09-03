/**
 * Vektorskop (Phase 14 Schritt 6, siehe `DECISIONS.md` ADR-0041): "seit
 * mindestens 2012 angefragt … Lightroom hat noch kein Vektorskop- und
 * kein Wellenform-Werkzeug". Reine clientseitige Berechnung über den
 * bereits gerenderten RGBA8-Puffer (`useDevelopRender`s `developFrame`),
 * exakt nach dem in `lib/histogram.ts` etablierten Muster — kein neuer
 * Backend-Command.
 *
 * Ein Vektorskop stellt die Farbverteilung eines Bildes auf der
 * Chrominanz-Ebene (Cb/Cr aus YCbCr, ITU-R BT.601) als Dichte-Heatmap
 * dar: der Ursprung (Bildmitte des Rasters) entspricht Grau/unbunt, der
 * Abstand vom Zentrum entspricht Sättigung, der Winkel dem Farbton.
 * Rec.601-Gewichte statt Rec.709 — dieselbe Wahl wie
 * `apx_ai::color::rgb_to_ycbcr` (das Backend-Pendant aus Phase 7), damit
 * Frontend und Backend "Farbigkeit" konsistent auf derselben
 * Chroma-Definition messen, auch wenn hier direkt auf `0..255`-Bytes
 * statt `0..1`-normierten Kanälen gerechnet wird.
 */

/** Seitenlänge des quadratischen Cb/Cr-Dichte-Rasters — grob genug für
 * eine flüssige Live-Neuberechnung bei jedem Regler-Tick, fein genug für
 * eine erkennbare Farbverteilung (dieselbe Größenordnung wie
 * `lib/histogram.ts`s 256 Werte-Buckets je Kanal, hier aber zweidimensional,
 * deshalb kleiner gewählt). */
const GRID_SIZE = 128;

export interface Vectorscope {
  size: number;
  /** `size * size` Dichte-Zähler, zeilenweise (`grid[y * size + x]`).
   * `x` wächst mit Cb (Blau/Gelb-Achse) nach rechts, `y` wächst mit
   * *fallendem* Cr nach unten — Cr selbst wächst mathematisch Richtung
   * Rot, Canvas-Y wächst aber nach unten, deshalb beim Schreiben
   * gespiegelt, damit Rot beim Zeichnen oben landet (Broadcast-Vektorskop-
   * Konvention). */
  grid: Uint32Array;
  maxCount: number;
}

export function computeVectorscope(pixels: Uint8Array | Uint8ClampedArray, width: number, height: number): Vectorscope {
  const grid = new Uint32Array(GRID_SIZE * GRID_SIZE);
  const pixelCount = width * height;
  const lastIndex = GRID_SIZE - 1;

  for (let i = 0; i < pixelCount; i++) {
    const offset = i * 4;
    const r = pixels[offset] ?? 0;
    const g = pixels[offset + 1] ?? 0;
    const b = pixels[offset + 2] ?? 0;
    // ITU-R BT.601 Cb/Cr, Vollbereich — dieselben Koeffizienten wie
    // `apx_ai::color::rgb_to_ycbcr`, hier auf `0..255`-Bytes angewandt,
    // deshalb der Offset `+128` statt einer `0.5`-Verschiebung.
    const cb = -0.168_736 * r - 0.331_264 * g + 0.5 * b + 128;
    const cr = 0.5 * r - 0.418_688 * g - 0.081_312 * b + 128;
    const gx = Math.min(lastIndex, Math.max(0, Math.round((cb / 255) * lastIndex)));
    const gy = Math.min(lastIndex, Math.max(0, Math.round(((255 - cr) / 255) * lastIndex)));
    const cell = gy * GRID_SIZE + gx;
    grid[cell] = (grid[cell] ?? 0) + 1;
  }

  let maxCount = 0;
  for (let i = 0; i < grid.length; i++) maxCount = Math.max(maxCount, grid[i] ?? 0);

  return { size: GRID_SIZE, grid, maxCount };
}
