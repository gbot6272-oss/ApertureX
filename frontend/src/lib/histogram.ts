/**
 * Live-Histogramm + Clipping-Zählung (Phase 9 Schritt 4, siehe `PLAN.md`/
 * `DECISIONS.md` ADR-0035) — reine Analyse über den bereits gerenderten
 * RGBA8-Puffer (`useDevelopRender`s `developFrame`), kein neuer
 * Rendering-Pfad. Luminanz nutzt dieselben Rec.-709-Gewichte wie
 * `lib/softProof.ts`s Gamut-Warnung, damit beide Module konsistent
 * "wahrgenommene Helligkeit" rechnen.
 */

const BUCKETS = 256;

export interface Histogram {
  r: number[];
  g: number[];
  b: number[];
  luminance: number[];
  /** Größter Einzel-Bucket-Wert über alle vier Kanäle — für die
   * Normierung der Balkenhöhe beim Zeichnen. */
  maxCount: number;
}

/** Zählt Vorkommen jedes Bytewerts (0..255) je Kanal plus Luminanz. */
export function computeHistogram(pixels: Uint8Array | Uint8ClampedArray, width: number, height: number): Histogram {
  const r = new Array<number>(BUCKETS).fill(0);
  const g = new Array<number>(BUCKETS).fill(0);
  const b = new Array<number>(BUCKETS).fill(0);
  const luminance = new Array<number>(BUCKETS).fill(0);
  const pixelCount = width * height;

  for (let i = 0; i < pixelCount; i++) {
    const offset = i * 4;
    const rv = pixels[offset] ?? 0;
    const gv = pixels[offset + 1] ?? 0;
    const bv = pixels[offset + 2] ?? 0;
    r[rv] = (r[rv] ?? 0) + 1;
    g[gv] = (g[gv] ?? 0) + 1;
    b[bv] = (b[bv] ?? 0) + 1;
    const lum = Math.round(0.2126 * rv + 0.7152 * gv + 0.0722 * bv);
    const lumBucket = Math.min(BUCKETS - 1, Math.max(0, lum));
    luminance[lumBucket] = (luminance[lumBucket] ?? 0) + 1;
  }

  let maxCount = 0;
  for (let i = 0; i < BUCKETS; i++) {
    maxCount = Math.max(maxCount, r[i] ?? 0, g[i] ?? 0, b[i] ?? 0, luminance[i] ?? 0);
  }

  return { r, g, b, luminance, maxCount };
}

export interface ClippingCounts {
  /** Pixel, bei denen mindestens ein Kanal <= `shadowThreshold` ist. */
  shadowClipped: number;
  /** Pixel, bei denen mindestens ein Kanal >= `highlightThreshold` ist. */
  highlightClipped: number;
  totalPixels: number;
}

const DEFAULT_SHADOW_THRESHOLD = 0;
const DEFAULT_HIGHLIGHT_THRESHOLD = 255;

/** Zählt Tiefen-/Lichter-Clipping — ein Pixel gilt als geclippt, sobald
 * *ein* Kanal den Schwellwert erreicht (Lightroom-Konvention), nicht erst
 * wenn alle drei Kanäle betroffen sind. */
export function countClipping(
  pixels: Uint8Array | Uint8ClampedArray,
  width: number,
  height: number,
  shadowThreshold: number = DEFAULT_SHADOW_THRESHOLD,
  highlightThreshold: number = DEFAULT_HIGHLIGHT_THRESHOLD,
): ClippingCounts {
  let shadowClipped = 0;
  let highlightClipped = 0;
  const pixelCount = width * height;

  for (let i = 0; i < pixelCount; i++) {
    const offset = i * 4;
    const rv = pixels[offset] ?? 0;
    const gv = pixels[offset + 1] ?? 0;
    const bv = pixels[offset + 2] ?? 0;
    if (rv <= shadowThreshold || gv <= shadowThreshold || bv <= shadowThreshold) shadowClipped++;
    if (rv >= highlightThreshold || gv >= highlightThreshold || bv >= highlightThreshold) highlightClipped++;
  }

  return { shadowClipped, highlightClipped, totalPixels: pixelCount };
}

/** Baut eine RGBA8-Overlay-Maske derselben Auflösung wie `pixels` —
 * transparent überall außer an geclippten Pixeln (Rot = Lichter,
 * Blau = Tiefen, Lightroom-Konvention), zum Zeichnen über das Bild. */
export function buildClippingOverlay(
  pixels: Uint8Array | Uint8ClampedArray,
  width: number,
  height: number,
  shadowThreshold: number = DEFAULT_SHADOW_THRESHOLD,
  highlightThreshold: number = DEFAULT_HIGHLIGHT_THRESHOLD,
): Uint8ClampedArray {
  const overlay = new Uint8ClampedArray(width * height * 4);
  const pixelCount = width * height;

  for (let i = 0; i < pixelCount; i++) {
    const offset = i * 4;
    const rv = pixels[offset] ?? 0;
    const gv = pixels[offset + 1] ?? 0;
    const bv = pixels[offset + 2] ?? 0;
    const shadow = rv <= shadowThreshold || gv <= shadowThreshold || bv <= shadowThreshold;
    const highlight = rv >= highlightThreshold || gv >= highlightThreshold || bv >= highlightThreshold;
    if (highlight) {
      overlay[offset] = 255;
      overlay[offset + 1] = 0;
      overlay[offset + 2] = 0;
      overlay[offset + 3] = 255;
    } else if (shadow) {
      overlay[offset] = 0;
      overlay[offset + 1] = 80;
      overlay[offset + 2] = 255;
      overlay[offset + 3] = 255;
    }
  }

  return overlay;
}
