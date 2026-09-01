/**
 * Reine Geometrie-Funktionen für Zoom/Pan im Viewer (Schritt 9). Getrennt
 * von der Canvas-Komponente, damit die Logik unabhängig von React/DOM
 * nachvollziehbar bleibt.
 */

export type FitMode = "fit" | "fill" | "manual";

/** Feste Zoom-Stufen aus `PHASE1_PROMPT.md` Abschnitt 7 (1:1 bis 16:1).
 * "Einpassen"/"Füllen" sind eigene Modi, keine Punkte auf dieser Leiter —
 * ihr tatsächlicher Skalierungsfaktor hängt von Container- und
 * Bildgröße ab und wird über `computeBaseScale` berechnet. */
export const ZOOM_STEPS = [1, 2, 4, 8, 16];

export function clampZoom(zoom: number): number {
  return Math.min(32, Math.max(0.02, zoom));
}

/** Skalierungsfaktor für "Einpassen" (ganzes Bild sichtbar) bzw.
 * "Füllen" (Container komplett bedeckt, Bild kann überstehen). */
export function computeBaseScale(mode: "fit" | "fill", containerW: number, containerH: number, imgW: number, imgH: number): number {
  if (imgW <= 0 || imgH <= 0 || containerW <= 0 || containerH <= 0) return 1;
  const scaleX = containerW / imgW;
  const scaleY = containerH / imgH;
  return mode === "fill" ? Math.max(scaleX, scaleY) : Math.min(scaleX, scaleY);
}

/** Nächsthöhere/-niedrigere Stufe auf der Zoom-Leiter, inklusive der
 * aktuellen "Einpassen"-Skalierung als zusätzlichem Sprungpunkt. */
export function nextZoomStep(current: number, direction: 1 | -1, fitScale: number): number {
  const ladder = Array.from(new Set([fitScale, ...ZOOM_STEPS])).sort((a, b) => a - b);
  // `ladder` enthält immer mindestens `fitScale`, daher ist ein Fallback
  // auf `current` (statt einer nie eintretenden Ausnahme) nur zur
  // Typsicherheit für den (unerreichbaren) Leer-Fall nötig.
  if (direction === 1) {
    return ladder.find((v) => v > current + 1e-6) ?? ladder[ladder.length - 1] ?? current;
  }
  const descending = [...ladder].reverse();
  return descending.find((v) => v < current - 1e-6) ?? ladder[0] ?? current;
}

export interface Offset {
  x: number;
  y: number;
}

/** Position der linken oberen Bildecke im Container, bei Skalierung
 * `scale` und zentriertem Bild plus Pan-Versatz. */
export function imageOrigin(containerW: number, containerH: number, imgW: number, imgH: number, scale: number, pan: Offset): Offset {
  return {
    x: (containerW - imgW * scale) / 2 + pan.x,
    y: (containerH - imgH * scale) / 2 + pan.y,
  };
}

/** Neuer Pan-Versatz, damit der Bildpunkt unter der Maus beim Zoomen an
 * derselben Bildschirmposition bleibt ("zum Cursor zoomen"). */
export function panForZoomAtCursor(cursor: Offset, containerW: number, containerH: number, imgW: number, imgH: number, oldScale: number, newScale: number, oldPan: Offset): Offset {
  const oldOrigin = imageOrigin(containerW, containerH, imgW, imgH, oldScale, oldPan);
  const imagePointX = (cursor.x - oldOrigin.x) / oldScale;
  const imagePointY = (cursor.y - oldOrigin.y) / oldScale;

  const newOriginX = cursor.x - imagePointX * newScale;
  const newOriginY = cursor.y - imagePointY * newScale;

  return {
    x: newOriginX - (containerW - imgW * newScale) / 2,
    y: newOriginY - (containerH - imgH * newScale) / 2,
  };
}
