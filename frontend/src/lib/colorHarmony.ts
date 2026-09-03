/**
 * Farb-Harmonie-Rad: Harmonie-Berechnung (Phase 14 Schritt 7, siehe
 * `DECISIONS.md` ADR-0041 Nachtrag VII, Recherche-Tabelle Punkt 10) —
 * reine Farbtheorie-Mathematik über die bereits vom Backend extrahierte
 * Palette (`apx_ai::palette`, siehe `lib/tauri.ts`s `extractColorPalette`),
 * kein Bildzugriff mehr nötig. Die Zuordnung einer Palettenfarbe zum
 * nächstgelegenen der acht festen HSL-Bänder nutzt das bereits
 * bestehende `nearestHslBand` aus `edl.ts` (Phase 11 Schritt 6, TAT)
 * wieder, statt eine zweite Zuordnungslogik zu schreiben.
 */

import { clampSliderValue, HSL_BAND_SLIDER_SPECS, nearestHslBand } from "./edl";
import type { HslAdjustment } from "./edl";
import type { PaletteColorDto } from "./tauri";

export type HarmonyType = "complementary" | "triadic" | "splitComplementary" | "analogous";

export const HARMONY_LABELS: Record<HarmonyType, string> = {
  complementary: "Komplementär",
  triadic: "Triade",
  splitComplementary: "Split-Komplementär",
  analogous: "Analog",
};

function wrapDegrees(degrees: number): number {
  return ((degrees % 360) + 360) % 360;
}

/** Die Ziel-Farbtöne (Grad, `0..360`) einer Harmonie relativ zu einem
 * Basis-Farbton — Standard-Farbtheorie-Definitionen. */
export function harmonyTargetHues(baseHueDegrees: number, harmony: HarmonyType): number[] {
  switch (harmony) {
    case "complementary":
      return [wrapDegrees(baseHueDegrees), wrapDegrees(baseHueDegrees + 180)];
    case "triadic":
      return [wrapDegrees(baseHueDegrees), wrapDegrees(baseHueDegrees + 120), wrapDegrees(baseHueDegrees + 240)];
    case "splitComplementary":
      return [wrapDegrees(baseHueDegrees), wrapDegrees(baseHueDegrees + 150), wrapDegrees(baseHueDegrees + 210)];
    case "analogous":
      return [wrapDegrees(baseHueDegrees - 30), wrapDegrees(baseHueDegrees), wrapDegrees(baseHueDegrees + 30)];
  }
}

/** Kürzester VORZEICHENBEHAFTETER Kreisabstand von `from` zu `to` in Grad
 * (`-180..180`) — anders als eine reine Distanzfunktion entscheidet das
 * Vorzeichen, in welche Richtung ein Regler verschoben werden muss. */
export function signedHueDelta(from: number, to: number): number {
  return ((to - from + 540) % 360) - 180;
}

function nearestTargetHue(hueDegrees: number, targets: number[]): number {
  let closest = targets[0] ?? hueDegrees;
  let closestDistance = Infinity;
  for (const target of targets) {
    const distance = Math.abs(signedHueDelta(hueDegrees, target));
    if (distance < closestDistance) {
      closestDistance = distance;
      closest = target;
    }
  }
  return closest;
}

/** Obergrenze des HSL-Band-Farbton-Reglers in tatsächlichen Grad, siehe
 * `crates/apx-pipeline/src/stages/hsl_color_mixer.rs`s
 * `MAX_HUE_SHIFT_DEGREES` — der Regler selbst liegt in `-100..100`,
 * `hue_shift_degrees = (regler / 100) * MAX_HUE_SHIFT_DEGREES`. */
const MAX_HUE_SHIFT_DEGREES = 60;

const HUE_SLIDER_SPEC = HSL_BAND_SLIDER_SPECS.find((spec) => spec.key === "hue")!;

/** Farbtöne mit weniger Buntheit als dieser Schwellenwert (CIE-LCh-
 * `chroma`, praktisch neutrales Grau) werden beim Harmonisieren
 * ignoriert — ihr Farbton ist kaum aussagekräftig und würde nur
 * zufälliges Rauschen in den Vorschlag einbringen. */
export const MIN_CHROMA_FOR_HARMONIZE = 8;

export interface HarmonizeShift {
  band: keyof HslAdjustment;
  /** Regler-Einheiten (`-100..100`), additiv auf den *aktuellen*
   * Bandwert anzuwenden — siehe `store/index.ts`s `harmonizeToTarget`. */
  hueRegler: number;
}

/**
 * Ordnet jede hinreichend bunte Palettenfarbe dem nächstgelegenen der
 * acht festen HSL-Bänder zu und berechnet, wie weit dessen Farbton-
 * Regler verschoben werden müsste, damit die tatsächliche Farbe auf
 * ihren nächstgelegenen Harmonie-Zielfarbton (relativ zu `baseHueDegrees`)
 * einrastet. Landen mehrere Palettenfarben im selben Band, gewinnt die
 * mit dem größten Bildanteil (keine Mittelung unterschiedlicher
 * tatsächlicher Farbtöne — das würde bei zwei deutlich verschiedenen
 * Farben im selben Band einen bedeutungslosen Mittelwert ergeben).
 */
export function computeHarmonizeShifts(palette: readonly PaletteColorDto[], baseHueDegrees: number, harmony: HarmonyType): HarmonizeShift[] {
  const targets = harmonyTargetHues(baseHueDegrees, harmony);
  const byBand = new Map<keyof HslAdjustment, { percentage: number; hueRegler: number }>();

  for (const color of palette) {
    if (color.chroma < MIN_CHROMA_FOR_HARMONIZE) continue;
    const band = nearestHslBand(color.hue_degrees);
    const target = nearestTargetHue(color.hue_degrees, targets);
    const deltaDegrees = signedHueDelta(color.hue_degrees, target);
    const hueRegler = clampSliderValue((deltaDegrees / MAX_HUE_SHIFT_DEGREES) * 100, HUE_SLIDER_SPEC);

    const existing = byBand.get(band);
    if (!existing || color.percentage > existing.percentage) {
      byBand.set(band, { percentage: color.percentage, hueRegler });
    }
  }

  return Array.from(byBand.entries()).map(([band, value]) => ({ band, hueRegler: value.hueRegler }));
}
