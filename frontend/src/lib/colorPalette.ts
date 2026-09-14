/**
 * Dominante Farben eines Fotos (Phase 31 Schritt 5, siehe
 * `DECISIONS.md`).
 *
 * **Warum im Frontend und nicht in Rust.** Wie beim Fokus-Peaking geht
 * es um eine Beobachtung am fertig entwickelten Vorschaubild, nicht um
 * eine Bildveränderung. Die Daten liegen hier bereits (`developFrame`),
 * ein Tauri-Rundlauf brächte nur Latenz und einen weiteren Befehl.
 *
 * **Warum k-Means und kein Histogramm-Bucketing.** Farb-Bucketing (RGB
 * in ein grobes Raster werfen, die vollsten Zellen nehmen) ist schneller,
 * liefert aber Rasterkanten statt echter Farben: zwei nahezu gleiche
 * Töne landen in verschiedenen Zellen und tauchen beide auf, während
 * eine Farbe genau auf einer Zellgrenze halbiert wird und
 * verschwindet. k-Means findet die Häufungen dort, wo sie wirklich
 * liegen.
 *
 * **Warum im Opponent-Raum gemessen wird.** Euklidischer Abstand in RGB
 * entspricht nicht dem, was das Auge als "ähnliche Farbe" sieht — Blau
 * wirkt dort viel zu weit weg von Schwarz. Der Abstand wird deshalb in
 * einem Helligkeits-/Gegenfarben-Raum genommen, wie ihn auch
 * `stages::creative`s Farbabgleich verwendet.
 */

export interface PaletteSwatch {
  r: number;
  g: number;
  b: number;
  /** Anteil der Bildpixel, die zu dieser Farbe gehören (0…1). */
  share: number;
  /** `#rrggbb`, fertig für einen Farbwähler. */
  hex: string;
}

/** RGB → (Helligkeit, Rot-Grün, Gelb-Blau). */
function toOpponent(r: number, g: number, b: number): [number, number, number] {
  return [(r + g + b) / 3, r - g, (r + g) / 2 - b];
}

function hex(value: number): string {
  return Math.round(Math.max(0, Math.min(255, value)))
    .toString(16)
    .padStart(2, "0");
}

export function toHex(r: number, g: number, b: number): string {
  return `#${hex(r)}${hex(g)}${hex(b)}`;
}

/**
 * Extrahiert `count` dominante Farben.
 *
 * `stride` überspringt Pixel: bei einem Vorschaubild mit einigen
 * hunderttausend Pixeln reicht jedes n-te vollkommen, und k-Means über
 * alle wäre für eine Palette unverhältnismäßig. Die Startpunkte werden
 * NICHT zufällig gewählt, sondern gleichmäßig über die Stichprobe
 * verteilt — sonst käme bei jedem Aufruf eine leicht andere Palette
 * heraus, und der Nutzer könnte sich auf nichts verlassen.
 */
export function extractPalette(
  pixels: Uint8Array | Uint8ClampedArray,
  width: number,
  height: number,
  count = 6,
  stride = 4,
): PaletteSwatch[] {
  const samples: Array<[number, number, number]> = [];
  const total = width * height;
  for (let i = 0; i < total; i += Math.max(1, stride)) {
    const base = i * 4;
    // Durchsichtige Pixel überspringen: ein freigestelltes Foto hätte
    // sonst "Schwarz" als dominante Farbe, obwohl dort nichts ist.
    if ((pixels[base + 3] ?? 255) < 16) continue;
    samples.push([pixels[base] ?? 0, pixels[base + 1] ?? 0, pixels[base + 2] ?? 0]);
  }
  if (samples.length === 0) return [];

  const k = Math.max(1, Math.min(count, samples.length));
  let centers: Array<[number, number, number]> = Array.from({ length: k }, (_, index) => {
    const pick = Math.floor((index * samples.length) / k);
    return [...(samples[pick] as [number, number, number])];
  });

  const assignment = new Int32Array(samples.length);
  // Feste Rundenzahl statt eines Konvergenzkriteriums: die Palette ist
  // eine Anzeige, keine Optimierungsaufgabe, und zwölf Runden reichen
  // für ein stabiles Ergebnis, ohne dass die Laufzeit vom Bild abhängt.
  for (let round = 0; round < 12; round += 1) {
    for (let i = 0; i < samples.length; i += 1) {
      const [r, g, b] = samples[i] as [number, number, number];
      const [l, rg, yb] = toOpponent(r, g, b);
      let best = 0;
      let bestDistance = Infinity;
      for (let c = 0; c < centers.length; c += 1) {
        const [cr, cg, cb] = centers[c] as [number, number, number];
        const [cl, crg, cyb] = toOpponent(cr, cg, cb);
        const distance = (l - cl) ** 2 + (rg - crg) ** 2 + (yb - cyb) ** 2;
        if (distance < bestDistance) {
          bestDistance = distance;
          best = c;
        }
      }
      assignment[i] = best;
    }

    const sums = Array.from({ length: k }, () => [0, 0, 0, 0]);
    for (let i = 0; i < samples.length; i += 1) {
      const slot = sums[assignment[i] ?? 0] as number[];
      const [r, g, b] = samples[i] as [number, number, number];
      slot[0] = (slot[0] ?? 0) + r;
      slot[1] = (slot[1] ?? 0) + g;
      slot[2] = (slot[2] ?? 0) + b;
      slot[3] = (slot[3] ?? 0) + 1;
    }
    centers = centers.map((center, index) => {
      const slot = sums[index] as number[];
      const n = slot[3] ?? 0;
      // Ein leerer Cluster behält seinen Mittelpunkt, statt auf (0,0,0)
      // zu springen — ein Sprung nach Schwarz hätte eine Farbe erfunden,
      // die im Bild gar nicht vorkommt.
      return n === 0
        ? center
        : ([(slot[0] ?? 0) / n, (slot[1] ?? 0) / n, (slot[2] ?? 0) / n] as [number, number, number]);
    });
  }

  const counts = new Array<number>(k).fill(0);
  for (let i = 0; i < assignment.length; i += 1) {
    counts[assignment[i] ?? 0] = (counts[assignment[i] ?? 0] ?? 0) + 1;
  }

  return centers
    .map((center, index) => {
      const [r, g, b] = center;
      return {
        r: Math.round(r),
        g: Math.round(g),
        b: Math.round(b),
        share: (counts[index] ?? 0) / samples.length,
        hex: toHex(r, g, b),
      };
    })
    .filter((swatch) => swatch.share > 0)
    .sort((a, b) => b.share - a.share);
}
