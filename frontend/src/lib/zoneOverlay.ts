/**
 * Falschfarben-Überlagerung der zehn Luminanzzonen (Phase 30 Punkt 10,
 * siehe `DECISIONS.md` ADR-0060).
 *
 * Das Zonensystem aus Phase 28 hat zehn Regler, aber es war nirgends zu
 * sehen, welcher Regler welchen Bildteil trifft. Diese Überlagerung
 * färbt jedes Pixel nach seiner Zone ein — erst damit wird aus zehn
 * abstrakten Zahlen ein Werkzeug.
 *
 * Die Zoneneinteilung ist dieselbe wie in `stages::light_optics`:
 * Zone `z` liegt bei Luminanz `z/9`, ein Pixel gehört zur nächsten.
 * Eine abweichende Einteilung hier wäre schlimmer als keine Anzeige —
 * sie würde auf den falschen Regler zeigen.
 */

/** Zehn gut unterscheidbare Farben, von Dunkelblau (Zone 0) über Grün
 * und Gelb bis Weiß (Zone 9) — die klassische Falschfarben-Leiter, wie
 * sie Kamera-Monitore für Belichtungskontrolle verwenden. */
export const ZONE_COLORS: readonly (readonly [number, number, number])[] = [
  [20, 24, 80],
  [30, 60, 160],
  [30, 120, 190],
  [30, 165, 150],
  [60, 180, 80],
  [150, 195, 50],
  [225, 205, 45],
  [240, 150, 40],
  [235, 95, 60],
  [245, 245, 245],
];

/** Dieselbe Luminanz-Gewichtung wie die Pipeline (`pixel_util::luminance`). */
export function zoneLuminance(r: number, g: number, b: number): number {
  return (0.3 * r + 0.59 * g + 0.11 * b) / 255;
}

/** Zu welcher der zehn Zonen gehört diese Luminanz (`0..1`)? */
export function zoneIndexFor(luminance: number): number {
  const clamped = Math.min(1, Math.max(0, luminance));
  return Math.min(9, Math.max(0, Math.round(clamped * 9)));
}

/**
 * Baut die RGBA-Überlagerung. `highlightZone` hebt genau eine Zone
 * hervor und blendet die übrigen stark zurück — das ist der Modus, in
 * dem man eine einzelne Zone am Bild wiederfindet, statt nur ein buntes
 * Gesamtbild zu sehen.
 */
export function buildZoneOverlay(
  pixels: Uint8Array | Uint8ClampedArray,
  width: number,
  height: number,
  highlightZone: number | null = null,
  opacity = 0.75,
): Uint8ClampedArray {
  const overlay = new Uint8ClampedArray(width * height * 4);
  const count = width * height;
  for (let i = 0; i < count; i += 1) {
    const offset = i * 4;
    const zone = zoneIndexFor(
      zoneLuminance(pixels[offset] ?? 0, pixels[offset + 1] ?? 0, pixels[offset + 2] ?? 0),
    );
    const color = ZONE_COLORS[zone] ?? ZONE_COLORS[0]!;
    overlay[offset] = color[0];
    overlay[offset + 1] = color[1];
    overlay[offset + 2] = color[2];
    overlay[offset + 3] =
      highlightZone === null
        ? Math.round(opacity * 255)
        : zone === highlightZone
          ? 255
          : Math.round(opacity * 60);
  }
  return overlay;
}
