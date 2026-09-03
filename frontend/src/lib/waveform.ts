/**
 * Wellenform-Monitor (Phase 14 Schritt 6, siehe `DECISIONS.md` ADR-0041):
 * "seit mindestens 2012 angefragt … Lightroom hat noch kein Vektorskop-
 * und kein Wellenform-Werkzeug". Reine clientseitige Berechnung über
 * denselben RGBA8-Puffer wie `lib/histogram.ts`/`lib/vectorscope.ts` —
 * kein neuer Backend-Command.
 *
 * Anders als das aggregierte Gesamt-Histogramm zeigt eine Wellenform je
 * Bildspalte (X-Achse) die Werteverteilung des jeweiligen Kanals
 * (Y-Achse, `0..255`) als Dichte-Heatmap — macht z. B. einen
 * Belichtungsverlauf über die Bildbreite (Vignettierung, ein
 * schrittweiser Übergang, ein degradierter Verlaufsfilter) sichtbar, den
 * das Gesamt-Histogramm allein nicht zeigen kann.
 */

/** Anzahl der Spalten-Buckets im Ausgaberaster — die Bildbreite selbst
 * kann beliebig groß sein (bis zu `ANALYSIS_MAX_EDGE`), mehrere
 * Bildspalten werden deshalb je Ausgabespalte zusammengefasst, exakt wie
 * `lib/vectorscope.ts`s `GRID_SIZE`-Rasterung der Chrominanz-Ebene. */
const COLUMN_BUCKETS = 256;
/** Werte-Buckets je Spalte — dieselbe Auflösung wie `lib/histogram.ts`s
 * 256 Byte-Werte je Kanal. */
const VALUE_BUCKETS = 256;

export interface Waveform {
  columns: number;
  rows: number;
  /** `columns * rows` Dichte-Zähler je Kanal, spaltenweise
   * (`r[column * rows + value]`). */
  r: Uint32Array;
  g: Uint32Array;
  b: Uint32Array;
  maxCount: number;
}

export function computeWaveform(pixels: Uint8Array | Uint8ClampedArray, width: number, height: number): Waveform {
  const r = new Uint32Array(COLUMN_BUCKETS * VALUE_BUCKETS);
  const g = new Uint32Array(COLUMN_BUCKETS * VALUE_BUCKETS);
  const b = new Uint32Array(COLUMN_BUCKETS * VALUE_BUCKETS);
  const lastColumn = COLUMN_BUCKETS - 1;

  for (let y = 0; y < height; y++) {
    for (let x = 0; x < width; x++) {
      const offset = (y * width + x) * 4;
      const col = width > 0 ? Math.min(lastColumn, Math.floor((x / width) * COLUMN_BUCKETS)) : 0;
      const rv = pixels[offset] ?? 0;
      const gv = pixels[offset + 1] ?? 0;
      const bv = pixels[offset + 2] ?? 0;
      const rIdx = col * VALUE_BUCKETS + rv;
      const gIdx = col * VALUE_BUCKETS + gv;
      const bIdx = col * VALUE_BUCKETS + bv;
      r[rIdx] = (r[rIdx] ?? 0) + 1;
      g[gIdx] = (g[gIdx] ?? 0) + 1;
      b[bIdx] = (b[bIdx] ?? 0) + 1;
    }
  }

  let maxCount = 0;
  for (let i = 0; i < r.length; i++) maxCount = Math.max(maxCount, r[i] ?? 0, g[i] ?? 0, b[i] ?? 0);

  return { columns: COLUMN_BUCKETS, rows: VALUE_BUCKETS, r, g, b, maxCount };
}
