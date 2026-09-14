/**
 * Fokus-Peaking: markiert farbig, welche Kanten tatsächlich scharf sind
 * (Phase 31 Schritt 4, siehe `DECISIONS.md`).
 *
 * **Warum das keine Pipeline-Stufe ist.** Peaking verändert das Foto
 * nicht — es ist eine Sichthilfe beim Beurteilen, wie das
 * Clipping-Overlay (Phase 9 Schritt 4) oder die Masken-Überlagerung.
 * Käme es ins EDL, würde es exportiert und in Presets weitergereicht
 * werden, was beides niemand will. Es rechnet deshalb hier im Frontend
 * auf dem bereits fertig entwickelten Vorschaubild, genau wie
 * `buildClippingOverlay`.
 *
 * **Warum die Gradientenstärke und nicht der Laplace-Operator.** Der
 * Laplace-Operator ist die zweite Ableitung und reagiert deshalb stark
 * auf Rauschen — bei hohen ISO-Werten leuchtet dann das ganze Bild.
 * Der Sobel-Gradient ist die erste Ableitung, dämpft durch seine
 * eingebaute 1-2-1-Glättung quer zur Ableitungsrichtung, und genau
 * darum benutzen ihn Kamera- und Videomonitore für Peaking.
 */

/** Ein Überlagerungsbild als RGBA8, überall durchsichtig außer auf den
 * als scharf erkannten Kanten. */
export interface PeakingOverlay {
  pixels: Uint8ClampedArray;
  /** Anteil der als scharf markierten Pixel (0…1) — speist die Anzeige
   * neben dem Schalter, damit man den Schwellwert sinnvoll einstellen
   * kann, statt blind zu schieben. */
  coverage: number;
}

/** Die drei wählbaren Markierungsfarben, kräftig und im Foto selten. */
export const PEAKING_COLORS = {
  red: [255, 40, 40],
  green: [40, 255, 90],
  blue: [70, 150, 255],
} as const;

export type PeakingColor = keyof typeof PEAKING_COLORS;

/** Luminanz nach Rec. 709 — dieselbe Gewichtung wie im Histogramm. */
function luminanceAt(pixels: Uint8Array | Uint8ClampedArray, index: number): number {
  return (
    0.2126 * (pixels[index] ?? 0) +
    0.7152 * (pixels[index + 1] ?? 0) +
    0.0722 * (pixels[index + 2] ?? 0)
  );
}

/**
 * Sobel-Gradientenstärke je Pixel, auf 0…1 normiert.
 *
 * Der Rand (ein Pixel ringsum) bleibt 0: dort fehlt der volle
 * 3×3-Nachbarschaftsblock. Ihn zu extrapolieren würde am Bildrand eine
 * Scheinkante erzeugen — beim Peaking wäre das ein leuchtender Rahmen
 * um jedes Foto.
 */
export function sobelMagnitude(
  pixels: Uint8Array | Uint8ClampedArray,
  width: number,
  height: number,
): Float32Array {
  const out = new Float32Array(width * height);
  if (width < 3 || height < 3) return out;

  for (let y = 1; y < height - 1; y += 1) {
    for (let x = 1; x < width - 1; x += 1) {
      const at = (dx: number, dy: number) => luminanceAt(pixels, ((y + dy) * width + (x + dx)) * 4);

      const tl = at(-1, -1);
      const tc = at(0, -1);
      const tr = at(1, -1);
      const ml = at(-1, 0);
      const mr = at(1, 0);
      const bl = at(-1, 1);
      const bc = at(0, 1);
      const br = at(1, 1);

      const gx = tl + 2 * ml + bl - (tr + 2 * mr + br);
      const gy = tl + 2 * tc + tr - (bl + 2 * bc + br);

      // Der größtmögliche Betrag eines Sobel-Kerns auf 0…255-Werten ist
      // 4 * 255 = 1020 je Richtung; die Diagonale davon normiert auf 1.
      out[y * width + x] = Math.min(1, Math.hypot(gx, gy) / 1020);
    }
  }
  return out;
}

/**
 * Baut das Überlagerungsbild.
 *
 * `threshold` ist die Gradientenstärke ab der markiert wird (0…1).
 * Kleiner = mehr leuchtet. Der Wert ist bewusst der Rohschwellwert und
 * nicht "Empfindlichkeit 0–100": das Ergebnis hängt so nur von einer
 * Zahl ab, und `coverage` sagt dem Nutzer unmittelbar, was sie bewirkt.
 */
export function buildPeakingOverlay(
  pixels: Uint8Array | Uint8ClampedArray,
  width: number,
  height: number,
  threshold: number,
  color: PeakingColor = "red",
): PeakingOverlay {
  const out = new Uint8ClampedArray(width * height * 4);
  const magnitude = sobelMagnitude(pixels, width, height);
  const [r, g, b] = PEAKING_COLORS[color];
  let marked = 0;

  for (let i = 0; i < magnitude.length; i += 1) {
    const strength = magnitude[i] ?? 0;
    if (strength < threshold) continue;
    marked += 1;
    const base = i * 4;
    out[base] = r;
    out[base + 1] = g;
    out[base + 2] = b;
    // Je deutlicher die Kante, desto kräftiger die Markierung — eine
    // harte 0/1-Maske liesse eine knapp überschrittene Kante genauso
    // aussehen wie eine perfekt sitzende Schärfeebene.
    out[base + 3] = Math.round(140 + 115 * Math.min(1, (strength - threshold) / 0.15));
  }

  return { pixels: out, coverage: magnitude.length === 0 ? 0 : marked / magnitude.length };
}
