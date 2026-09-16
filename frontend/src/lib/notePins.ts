/**
 * Umrechnung zwischen Notiz-Koordinaten und dem, was im Viewer zu sehen
 * ist (Phase 32 F6).
 *
 * **Notizen hängen am unbeschnittenen Original.** Gespeichert wird
 * normiert 0..1 bezogen auf das Originalbild (siehe
 * `migrations/0013_photo_notes.sql`). Der Viewer zeigt aber den
 * *beschnittenen* Stand. Ohne Umrechnung säße jeder Pin nach einem
 * Zuschnitt an der falschen Stelle — und zwar umso falscher, je stärker
 * beschnitten wurde.
 *
 * Die Alternative wäre gewesen, die Notiz in Ausschnitts-Koordinaten zu
 * speichern. Dann wandert sie aber mit jedem späteren Zuschnitt übers
 * Motiv, obwohl sie eine feste Stelle darin meint („Staubfleck hier").
 * Original-Koordinaten + Umrechnung ist der Weg, der die Aussage der
 * Notiz erhält.
 */

export interface UnitPoint {
  x: number;
  y: number;
}

export interface CropWindow {
  x: number;
  y: number;
  width: number;
  height: number;
}

export const FULL_CROP: CropWindow = { x: 0, y: 0, width: 1, height: 1 };

/**
 * Original-Koordinate → Koordinate im angezeigten Ausschnitt.
 *
 * `null`, wenn der Punkt außerhalb des Ausschnitts liegt: die Notiz
 * zeigt dann auf etwas, das gerade nicht im Bild ist. Sie am Rand
 * klebend anzuzeigen wäre eine Lüge über ihre Position — die Liste
 * neben dem Bild führt sie trotzdem auf, mit Hinweis.
 */
export function originalToView(point: UnitPoint, crop: CropWindow = FULL_CROP): UnitPoint | null {
  if (crop.width <= 0 || crop.height <= 0) return null;
  const x = (point.x - crop.x) / crop.width;
  const y = (point.y - crop.y) / crop.height;
  if (x < 0 || x > 1 || y < 0 || y > 1) return null;
  return { x, y };
}

/** Koordinate im angezeigten Ausschnitt → Original-Koordinate. */
export function viewToOriginal(point: UnitPoint, crop: CropWindow = FULL_CROP): UnitPoint {
  return {
    x: clampUnit(crop.x + point.x * crop.width),
    y: clampUnit(crop.y + point.y * crop.height),
  };
}

/** Auf 0..1 begrenzen — ein Klick knapp neben das Bild (Rundung,
 * Rahmenbreite) soll eine Notiz am Rand ergeben, keinen Fehler. */
export function clampUnit(value: number): number {
  if (Number.isNaN(value)) return 0;
  return Math.min(1, Math.max(0, value));
}

/**
 * Position eines Klicks innerhalb eines Elements, normiert auf dessen
 * Fläche. Gemeinsame Hilfe für Setzen und Verschieben eines Pins.
 */
export function pointFromEvent(rect: DOMRect, clientX: number, clientY: number): UnitPoint {
  if (rect.width <= 0 || rect.height <= 0) return { x: 0, y: 0 };
  return {
    x: clampUnit((clientX - rect.left) / rect.width),
    y: clampUnit((clientY - rect.top) / rect.height),
  };
}

/** Kurzfassung für die Pin-Beschriftung — der volle Text steht im
 * Popover, auf dem Pin ist nur Platz für den Anfang. */
export function shortLabel(body: string, maxLength = 28): string {
  const single = body.replace(/\s+/g, " ").trim();
  if (single.length <= maxLength) return single;
  return `${single.slice(0, maxLength - 1)}…`;
}
