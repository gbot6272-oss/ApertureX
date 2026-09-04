/**
 * Foto-Dichte-Heatmap (siehe `DECISIONS.md` ADR-0044) — reine Logik,
 * geteilt zwischen dem Globus (`GlobeView.tsx`, Kugel-Zellen) und der
 * flachen Karte (`MapView.tsx`, Leaflet-Canvas-Overlay), damit beide
 * exakt dieselbe Dichte-Einteilung und Farbskala verwenden statt zweier
 * unabhängiger, potenziell inkonsistenter Implementierungen.
 */

export interface GeoPoint {
  lat: number;
  lon: number;
}

export interface HeatCell {
  /** Mittelpunkt der Rasterzelle. */
  lat: number;
  lon: number;
  /** Anzahl der Fotos in dieser Zelle. */
  count: number;
}

/** Bündelt `points` in ein Breiten-/Längengrad-Raster fester
 * Zellgröße (`cellSizeDeg`) — dieselbe simple, gut nachvollziehbare
 * Rasterung wie die Signatur-"Buckets" der Personenansicht-
 * Vorsortierung (Phase 11 Schritt 5) statt eines dichtebasierten
 * Clustering-Algorithmus (KDE o. Ä.) — für eine Foto-Dichte-Übersicht
 * (nicht wissenschaftliche Genauigkeit) reicht das, und bleibt in
 * O(n) statt O(n²)/O(n log n). Zellen mit `count === 0` werden nicht
 * zurückgegeben. */
export function binPointsIntoGrid(points: GeoPoint[], cellSizeDeg: number): HeatCell[] {
  const cells = new Map<string, HeatCell>();
  for (const { lat, lon } of points) {
    if (!Number.isFinite(lat) || !Number.isFinite(lon)) continue;
    const cellLat = Math.floor(lat / cellSizeDeg) * cellSizeDeg + cellSizeDeg / 2;
    const cellLon = Math.floor(lon / cellSizeDeg) * cellSizeDeg + cellSizeDeg / 2;
    const key = `${cellLat.toFixed(4)}:${cellLon.toFixed(4)}`;
    const existing = cells.get(key);
    if (existing) {
      existing.count += 1;
    } else {
      cells.set(key, { lat: cellLat, lon: cellLon, count: 1 });
    }
  }
  return Array.from(cells.values());
}

export type RgbColor = [number, number, number];

function parseHexColor(hex: string): RgbColor {
  const clean = hex.replace("#", "");
  const r = parseInt(clean.slice(0, 2), 16);
  const g = parseInt(clean.slice(2, 4), 16);
  const b = parseInt(clean.slice(4, 6), 16);
  return [r, g, b];
}

function lerp(a: number, b: number, t: number): number {
  return a + (b - a) * t;
}

/** Interpoliert zwischen zwei Hex-Farben (`#rrggbb`) — `t` wird auf
 * [0, 1] geklemmt, das Ergebnis ist ein `[r, g, b]`-Tupel statt eines
 * fertigen CSS-Strings, damit Aufrufer (Canvas-Radialverläufe im
 * Globus/der Karten-Heatmap) die Alpha-Stufe selbst anhängen können,
 * ohne einen CSS-Farbstring parsen zu müssen. Reine Farbmathematik,
 * kein Farbraum-Umweg nötig (anders als die Entwickeln-Pipeline
 * arbeitet eine UI-Heatmap direkt in sRGB, keine physikalisch
 * korrekte Farbmischung gefordert). */
export function lerpHexColor(fromHex: string, toHex: string, t: number): RgbColor {
  const clamped = Math.min(1, Math.max(0, t));
  const [r1, g1, b1] = parseHexColor(fromHex);
  const [r2, g2, b2] = parseHexColor(toHex);
  return [Math.round(lerp(r1, r2, clamped)), Math.round(lerp(g1, g2, clamped)), Math.round(lerp(b1, b2, clamped))];
}

/** Zwei-Stufen-Heat-Skala: kühl (wenig Fotos) → `midHex` (Akzentfarbe
 * des Themes) → warm (viele Fotos, `hotHex` — die Themes-eigene
 * Gefahren-/Warnfarbe). Bewusst aus den beiden bereits im Theme
 * vorhandenen semantischen Farben abgeleitet (siehe `index.css`s
 * `--color-accent`/`--color-danger`) statt einer generischen
 * Regenbogen-Skala — folgt automatisch Akzentfarbe/Theme-Wechsel
 * (Phase 10 Schritt 7), bleibt "technisch/dunkel" statt bunt-verspielt. */
export function heatScaleColor(t: number, coolHex: string, midHex: string, hotHex: string): RgbColor {
  const clamped = Math.min(1, Math.max(0, t));
  return clamped < 0.5 ? lerpHexColor(coolHex, midHex, clamped * 2) : lerpHexColor(midHex, hotHex, (clamped - 0.5) * 2);
}

/** `[r, g, b]` + Alpha → CSS-`rgba(...)`-String, der letzte Schritt vor
 * `ctx.fillStyle`/`gradient.addColorStop`. */
export function rgbaCss([r, g, b]: RgbColor, alpha: number): string {
  return `rgba(${r}, ${g}, ${b}, ${alpha})`;
}

/** Normiert einen rohen Zellen-Zähler auf [0, 1] per Quadratwurzel-
 * Kompression statt linear — verhindert, dass ein einzelner sehr
 * fotoreicher Ort (z. B. der Heimatordner) fast alle anderen Orte auf
 * "praktisch null" staucht; dieselbe Kompressions-Idee wie ein
 * Histogramm-Tone-Curve-Kompromiss, hier auf Zähldichte angewandt. */
export function normalizeHeatIntensity(count: number, maxCount: number): number {
  if (maxCount <= 0) return 0;
  return Math.sqrt(count / maxCount);
}
