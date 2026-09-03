/**
 * Frequenztrennungs-Ansichtsmodus (Phase 14 Schritt 2, siehe `PLAN.md`/
 * `DECISIONS.md` ADR-0041) — reine Anzeige-Berechnung über den bereits
 * gerenderten RGBA8-Puffer (`useDevelopRender`s `developFrame`), kein
 * neuer Backend-Command, dasselbe clientseitige Muster wie
 * `lib/histogram.ts`. Dieselbe Box-Tiefpass-Näherung (kein echter
 * Gauß-Weichzeichner) und Mittelgrau-Verschiebung wie das Rust-Gegenstück
 * `apx_pipeline::stages::frequency_separation`, die die eigentliche
 * Retusche ausführt (`RepairStroke::layer`) — hier geht es nur um die
 * *Anzeige*, es wird nichts an `developEdl` verändert.
 */

export type FrequencyViewMode = "Normal" | "LowFrequency" | "HighFrequency";

/** Bruchteil der Bildbreite für den Tiefpass-Radius — identisch zu
 * `apx_pipeline::stages::frequency_separation::SPLIT_RADIUS_FRACTION`. */
const SPLIT_RADIUS_FRACTION = 0.02;
/** Mittelgrau-Verschiebung der Hochfrequenz-Ansicht (128/255). */
const HIGH_FREQUENCY_OFFSET = 128;

function boxBlur1d(src: Float32Array, width: number, height: number, radius: number, horizontal: boolean): Float32Array {
  const out = new Float32Array(src.length);
  for (let y = 0; y < height; y++) {
    for (let x = 0; x < width; x++) {
      let sumR = 0;
      let sumG = 0;
      let sumB = 0;
      let count = 0;
      for (let offset = -radius; offset <= radius; offset++) {
        const sx = horizontal ? x + offset : x;
        const sy = horizontal ? y : y + offset;
        if (sx < 0 || sy < 0 || sx >= width || sy >= height) continue;
        const idx = (sy * width + sx) * 3;
        sumR += src[idx] ?? 0;
        sumG += src[idx + 1] ?? 0;
        sumB += src[idx + 2] ?? 0;
        count++;
      }
      const dst = (y * width + x) * 3;
      out[dst] = count > 0 ? sumR / count : 0;
      out[dst + 1] = count > 0 ? sumG / count : 0;
      out[dst + 2] = count > 0 ? sumB / count : 0;
    }
  }
  return out;
}

/** Rechnet `pixels` (RGBA8, wie `developFrame.pixels`) in die gewählte
 * Ansicht um — `"Normal"` gibt `pixels` unverändert zurück (kein
 * Kopieren nötig). `"LowFrequency"` zeigt den Box-Tiefpass (Ton/Farbe,
 * unscharf), `"HighFrequency"` die um Mittelgrau verschobene Differenz
 * (Textur/Poren/Kanten, sichtbar wie ein Photoshop-Hochpass). Alpha
 * bleibt in beiden Fällen unverändert (voll deckend). */
export function applyFrequencyView(pixels: Uint8Array, width: number, height: number, mode: FrequencyViewMode): Uint8Array {
  if (mode === "Normal" || width <= 0 || height <= 0) return pixels;

  const pixelCount = width * height;
  const rgb = new Float32Array(pixelCount * 3);
  for (let i = 0; i < pixelCount; i++) {
    rgb[i * 3] = pixels[i * 4] ?? 0;
    rgb[i * 3 + 1] = pixels[i * 4 + 1] ?? 0;
    rgb[i * 3 + 2] = pixels[i * 4 + 2] ?? 0;
  }

  const radius = Math.max(1, Math.round(SPLIT_RADIUS_FRACTION * width));
  const horizontal = boxBlur1d(rgb, width, height, radius, true);
  const low = boxBlur1d(horizontal, width, height, radius, false);

  const out = new Uint8Array(pixelCount * 4);
  for (let i = 0; i < pixelCount; i++) {
    const src = i * 3;
    const dst = i * 4;
    if (mode === "LowFrequency") {
      out[dst] = Math.min(255, Math.max(0, Math.round(low[src] ?? 0)));
      out[dst + 1] = Math.min(255, Math.max(0, Math.round(low[src + 1] ?? 0)));
      out[dst + 2] = Math.min(255, Math.max(0, Math.round(low[src + 2] ?? 0)));
    } else {
      out[dst] = Math.min(255, Math.max(0, Math.round((rgb[src] ?? 0) - (low[src] ?? 0) + HIGH_FREQUENCY_OFFSET)));
      out[dst + 1] = Math.min(255, Math.max(0, Math.round((rgb[src + 1] ?? 0) - (low[src + 1] ?? 0) + HIGH_FREQUENCY_OFFSET)));
      out[dst + 2] = Math.min(255, Math.max(0, Math.round((rgb[src + 2] ?? 0) - (low[src + 2] ?? 0) + HIGH_FREQUENCY_OFFSET)));
    }
    out[dst + 3] = 255;
  }
  return out;
}
