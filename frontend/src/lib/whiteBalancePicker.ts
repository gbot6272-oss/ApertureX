import { BASIC_SLIDER_SPECS, clampSliderValue, type WhiteBalanceAdjustment } from "./edl";

/**
 * Weißabgleich-Pipette (`SPEC.md` §3.2 „Weißabgleich ... Pipette") —
 * berechnet aus einem im Viewer angeklickten, im Idealfall neutral-grauen
 * Bildpunkt eine neue Temperatur-/Tint-Verschiebung.
 *
 * **Bewusste Vereinfachung** (konsistent mit `white_balance.rs`s eigener
 * Modul-Doku, die die dortige Temperatur/Tint→Gain-Umrechnung bereits als
 * lineare Näherung statt physikalisch exakter Planckscher-Strahler-
 * Berechnung dokumentiert): diese Funktion arbeitet auf dem fertigen,
 * bereits gamma-kodierten RGBA8-Vorschaubild (das Einzige, was das
 * Frontend zu sehen bekommt), nicht auf den linearen Kamera-RGB-Werten,
 * auf denen die Rust-Pipeline tatsächlich rechnet — eine grobe
 * `^2.2`-Rückrechnung approximiert die sRGB-Gammakurve, und die
 * Umrechnung von Kanal-Korrekturfaktor in Regler-Einheiten ist eine
 * eigene, von `white_balance.rs`s Konstanten unabhängige Näherung (die
 * dortigen Konstanten sind für lineare Kamera-RGB-Werte kalibriert, nicht
 * für gamma-kodierte Anzeigepixel). Das Ergebnis bewegt Temperatur/Tint
 * zuverlässig in die richtige Richtung und um einen plausiblen Betrag,
 * ist aber keine farbmetrisch exakte Umkehrung.
 */
export function srgbByteToApproxLinear(byteValue: number): number {
  return Math.pow(Math.max(0, Math.min(255, byteValue)) / 255, 2.2);
}

/** Korrekturfaktoren (`avg / Kanalwert`) werden auf dieses Intervall
 * begrenzt, bevor sie in eine Regler-Verschiebung umgerechnet werden —
 * verhindert einen absurden Ein-Klick-Sprung bei einem sehr dunklen oder
 * sehr bunten angeklickten Pixel. */
const CORRECTION_CLAMP_MIN = 0.5;
const CORRECTION_CLAMP_MAX = 2.0;
const CORRECTION_SPREAD = CORRECTION_CLAMP_MAX - CORRECTION_CLAMP_MIN;

function clampCorrection(value: number): number {
  return Math.min(CORRECTION_CLAMP_MAX, Math.max(CORRECTION_CLAMP_MIN, value));
}

/** Berechnet die neue Weißabgleich-Verschiebung, addiert auf `current`
 * (die Pipette korrigiert relativ zum bestehenden Wert, nicht absolut —
 * anders als ein Preset, siehe [`WHITE_BALANCE_PRESETS`]). `r`/`g`/`b`
 * sind die am angeklickten Punkt gelesenen RGBA8-Bytewerte (`0..=255`). */
export function computeWhiteBalanceShiftFromSample(r: number, g: number, b: number, current: WhiteBalanceAdjustment): WhiteBalanceAdjustment {
  const linearR = srgbByteToApproxLinear(r);
  const linearG = srgbByteToApproxLinear(g);
  const linearB = srgbByteToApproxLinear(b);
  const average = (linearR + linearG + linearB) / 3;
  if (average < 1e-4) return current; // zu dunkel für eine verlässliche Aussage

  const correctionR = clampCorrection(average / linearR);
  const correctionG = clampCorrection(average / linearG);
  const correctionB = clampCorrection(average / linearB);

  const tempSpec = BASIC_SLIDER_SPECS.find((spec) => spec.key === "temp_shift_kelvin");
  const tintSpec = BASIC_SLIDER_SPECS.find((spec) => spec.key === "tint_shift");
  if (!tempSpec || !tintSpec) return current; // kann nicht eintreten, siehe Test dazu

  // Rot/Blau-Asymmetrie auf den vollen Temperatur-Reglerbereich abgebildet
  // (maximale Asymmetrie = voller Korrekturbereich in beide Richtungen),
  // Grün-Abweichung analog auf den Tint-Bereich — steigendes Tint senkt
  // laut `white_balance.rs::compute_gains` den Grün-Gain, daher das
  // Minuszeichen (zu viel Grün im Sample → Tint muss steigen).
  const deltaTemp = ((correctionR - correctionB) / CORRECTION_SPREAD) * tempSpec.max;
  const deltaTint = -((correctionG - 1) / (CORRECTION_SPREAD / 2)) * tintSpec.max;

  return {
    temp_shift_kelvin: clampSliderValue(current.temp_shift_kelvin + deltaTemp, tempSpec),
    tint_shift: clampSliderValue(current.tint_shift + deltaTint, tintSpec),
  };
}
