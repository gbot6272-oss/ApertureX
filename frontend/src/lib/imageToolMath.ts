/**
 * Die Umrechnung zwischen normierten Bildkoordinaten (`0..1`) und
 * Bildschirmpixeln für die am Bild bedienten Werkzeuge aus Phase 30
 * (siehe `DECISIONS.md` ADR-0060).
 *
 * **Warum ein eigenes Modul:** sieben Werkzeuge greifen auf dieselbe
 * Umrechnung zu. Läge sie in der Overlay-Komponente, wäre sie nur über
 * den gerenderten Viewer testbar — und ein Vorzeichen- oder
 * Zoom-Fehler zeigt sich dort erst, wenn jemand hineinzoomt und zieht.
 * Hier ist sie eine reine Funktion mit Einheitentests.
 *
 * Die Bezugsgröße ist immer das *Bild*, nicht der Container: `origin`
 * ist die Bildschirmposition der linken oberen Bildecke (aus
 * `viewerMath.imageOrigin`), `scale` der wirksame Zoom.
 */

export interface Point {
  x: number;
  y: number;
}

export interface ViewTransform {
  /** Bildschirmposition der linken oberen Bildecke. */
  origin: Point;
  /** Wirksamer Zoomfaktor. */
  scale: number;
  /** Bildgröße in Bildpixeln. */
  imgW: number;
  imgH: number;
}

/** Normierte Bildkoordinate → Bildschirmpixel (relativ zum Container). */
export function normalizedToScreen(u: number, v: number, view: ViewTransform): Point {
  return {
    x: view.origin.x + u * view.imgW * view.scale,
    y: view.origin.y + v * view.imgH * view.scale,
  };
}

/** Bildschirmpixel → normierte Bildkoordinate, auf `0..1` geklemmt.
 *
 * Geklemmt statt abgewiesen: beim Ziehen rutscht der Zeiger regelmäßig
 * über den Bildrand hinaus, und ein Griff, der dabei einfach stehen
 * bleibt statt am Rand entlangzulaufen, fühlt sich kaputt an. */
export function screenToNormalized(x: number, y: number, view: ViewTransform): Point {
  const w = Math.max(1e-6, view.imgW * view.scale);
  const h = Math.max(1e-6, view.imgH * view.scale);
  return {
    x: Math.min(1, Math.max(0, (x - view.origin.x) / w)),
    y: Math.min(1, Math.max(0, (y - view.origin.y) / h)),
  };
}

/** Ein normierter Radius (Anteil der KÜRZEREN Bildkante, dieselbe
 * Konvention wie `stages::interactive::Frame`) in Bildschirmpixeln. */
export function normalizedRadiusToScreen(radius: number, view: ViewTransform): number {
  return radius * Math.min(view.imgW, view.imgH) * view.scale;
}

/** Liegt ein Bildschirmpunkt innerhalb von `tolerance` Pixeln um einen
 * normierten Griff? Für die Treffererkennung beim Anfassen. */
export function isNearHandle(
  screen: Point,
  handleU: number,
  handleV: number,
  view: ViewTransform,
  tolerance = 12,
): boolean {
  const p = normalizedToScreen(handleU, handleV, view);
  return Math.hypot(screen.x - p.x, screen.y - p.y) <= tolerance;
}
