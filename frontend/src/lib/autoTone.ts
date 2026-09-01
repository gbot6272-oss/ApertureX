import type { Histogram } from "./histogram";
import { srgbByteToApproxLinear } from "./whiteBalancePicker";

/**
 * Auto-Ton (Phase 9 Schritt 5, siehe `PLAN.md`/`DECISIONS.md` ADR-0035) —
 * eine reine Histogramm-Perzentil-Heuristik, kein LLM/Modellinferenz
 * (dieselbe Ehrlichkeitslinie wie `apx-ai::reference`s numerische
 * Optimierung, ADR-0033 Punkt 4). **Bewusste Vereinfachung**: setzt nur
 * Belichtung und Kontrast, nicht die volle Sechs-Regler-Bandbreite von
 * Adobes proprietärem Auto-Ton (Lichter/Tiefen/Weiß/Schwarz bleiben
 * unverändert) — eine algebraische Umkehrung von deren tonwertkurven-
 * artiger Wirkung in `apx-pipeline::stages::basic_fused` wäre nur geraten,
 * nicht real hergeleitet. Arbeitet zudem auf dem bereits gamma-kodierten
 * RGBA8-Anzeigepuffer (`Viewer.tsx`s `developFrame`), nicht auf linearen
 * Kamera-RGB-Werten — dieselbe grobe `^2.2`-Rückrechnung wie
 * `whiteBalancePicker.ts`, keine farbmetrisch exakte Umkehrung.
 */

const EXPOSURE_EV_RANGE = 5; // deckt sich mit `BASIC_SLIDER_SPECS`s Belichtung-Regler
const TARGET_MIDTONE_LINEAR = 0.18; // "18 % Grau"

export interface AutoToneResult {
  exposure_ev: number;
  contrast: number;
}

/** Kleinster Bytewert, bei dem die kumulative Häufigkeit `percentile`
 * (0..1) der Gesamtpixelzahl erreicht. */
function percentileByte(luminance: number[], totalPixels: number, percentile: number): number {
  const target = totalPixels * percentile;
  let cumulative = 0;
  for (let i = 0; i < luminance.length; i++) {
    cumulative += luminance[i] ?? 0;
    if (cumulative >= target) return i;
  }
  return luminance.length - 1;
}

export function computeAutoTone(histogram: Histogram): AutoToneResult {
  const totalPixels = histogram.luminance.reduce((sum, count) => sum + count, 0);
  if (totalPixels === 0) return { exposure_ev: 0, contrast: 0 };

  const blackByte = percentileByte(histogram.luminance, totalPixels, 0.01);
  const whiteByte = percentileByte(histogram.luminance, totalPixels, 0.99);
  const medianByte = percentileByte(histogram.luminance, totalPixels, 0.5);

  const linearMedian = srgbByteToApproxLinear(medianByte);
  const exposureEv = linearMedian > 1e-4 ? Math.log2(TARGET_MIDTONE_LINEAR / linearMedian) : 0;

  // Schmale Perzentil-Spanne (flaches/dunstiges Bild) → mehr Kontrast
  // hinzufügen; bereits volle 0..255-Spanne → kein zusätzlicher Kontrast.
  const spread = Math.max(0, whiteByte - blackByte);
  const contrast = (1 - spread / 255) * 100;

  return {
    exposure_ev: Math.max(-EXPOSURE_EV_RANGE, Math.min(EXPOSURE_EV_RANGE, exposureEv)),
    contrast: Math.max(-100, Math.min(100, contrast)),
  };
}
