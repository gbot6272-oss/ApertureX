import type { Mask, MaskGeometry } from "./edl";

/**
 * Bearbeitungs-Pins (Phase 9 Schritt 5, siehe `PLAN.md`/`DECISIONS.md`
 * ADR-0035) — reine Frontend-Überlagerung über die bestehende
 * Maskengeometrie, kein neues Backend-Feld. Berechnet nur für Masken mit
 * einer eindeutigen räumlichen Position (Verlauf/Radial/Pinsel); Farb-/
 * Luminanzbereich-Masken (`ColorRange`/`LuminanceRange`, global über das
 * ganze Bild) und KI-generierte Rasterflächen (`AiGenerated`, ein Pin
 * müsste den Alpha-Kanal nach dessen Schwerpunkt durchsuchen — nicht
 * gerechtfertigter Mehraufwand für einen reinen Fokussier-Marker) haben
 * bewusst keinen Pin.
 */
function positionForGeometry(geometry: MaskGeometry): { x: number; y: number } | null {
  switch (geometry.kind) {
    case "LinearGradient":
      return { x: (geometry.x1 + geometry.x2) / 2, y: (geometry.y1 + geometry.y2) / 2 };
    case "RadialGradient":
      return { x: geometry.center_x, y: geometry.center_y };
    case "Brush": {
      const points = geometry.strokes.flatMap((stroke) => stroke.points);
      if (points.length === 0) return null;
      const sum = points.reduce((acc, p) => ({ x: acc.x + p.x, y: acc.y + p.y }), { x: 0, y: 0 });
      return { x: sum.x / points.length, y: sum.y / points.length };
    }
    default:
      return null;
  }
}

/** Position (0..1, normiert auf Bildmaße) für den ersten Komponenten
 * einer Maske mit eindeutiger Position — `null`, wenn keine ihrer
 * Komponenten eine hat. */
export function computeMaskPinPosition(mask: Mask): { x: number; y: number } | null {
  for (const component of mask.components) {
    const position = positionForGeometry(component.geometry);
    if (position) return position;
  }
  return null;
}
