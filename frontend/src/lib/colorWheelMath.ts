/**
 * Reine Geometrie für [`ColorWheel`](../components/ColorWheel.tsx) —
 * getrennt von der Komponente, damit die Winkel-/Abstandsrechnung
 * unabhängig von React/DOM testbar ist (analog zu `viewerMath.ts`).
 * Konvention: Farbton 0° zeigt nach oben, wächst im Uhrzeigersinn.
 */

function clamp01(value: number): number {
  return Math.min(1, Math.max(0, value));
}

/** Pixel-Versatz vom Mittelpunkt (`dx`/`dy`, `dy` nach unten positiv, wie
 * DOM-Koordinaten) → Farbton/Sättigung. */
export function pixelOffsetToHueSaturation(dx: number, dy: number, radius: number): { hue_degrees: number; saturation: number } {
  const distance = Math.sqrt(dx * dx + dy * dy);
  const saturation = radius > 0 ? clamp01(distance / radius) : 0;
  let hue = (Math.atan2(dx, -dy) * 180) / Math.PI;
  if (hue < 0) hue += 360;
  return { hue_degrees: hue, saturation };
}

/** Kehrfunktion: Farbton/Sättigung → Pixel-Versatz vom Mittelpunkt. */
export function hueSaturationToPixelOffset(hueDegrees: number, saturation: number, radius: number): { dx: number; dy: number } {
  const angleRad = (hueDegrees * Math.PI) / 180;
  return {
    dx: radius * saturation * Math.sin(angleRad),
    dy: -radius * saturation * Math.cos(angleRad),
  };
}
