/**
 * TypeScript-Gegenstück zu `crates/apx-pipeline/src/edl/v1.rs` und
 * `crates/apx-core/src/edl.rs` — von Hand synchron gehalten (vertretbar
 * bei sieben Feldern, siehe `PLAN.md` Phase 2, Abschnitt "Risiken":
 * dieser Punkt sollte bei einem deutlich größeren EDL in Phase 4 neu
 * bewertet werden).
 *
 * Die JSON-Form muss exakt der `serde`-Serialisierung von
 * `apx_pipeline::EdlV1` entsprechen (Feldnamen, Verschachtelung), da
 * `crate::edl::migrate::from_envelope` sie strikt gegen die Struktur
 * validiert statt fehlende Felder mit Defaults aufzufüllen.
 */

export const EDL_SCHEMA_VERSION = 1;

export interface WhiteBalanceAdjustment {
  temp_shift_kelvin: number;
  tint_shift: number;
}

export const NEUTRAL_WHITE_BALANCE: WhiteBalanceAdjustment = {
  temp_shift_kelvin: 0,
  tint_shift: 0,
};

export interface BasicAdjustments {
  white_balance: WhiteBalanceAdjustment;
  exposure_ev: number;
  contrast: number;
  highlights: number;
  shadows: number;
  whites: number;
  blacks: number;
}

export const NEUTRAL_BASIC_ADJUSTMENTS: BasicAdjustments = {
  white_balance: NEUTRAL_WHITE_BALANCE,
  exposure_ev: 0,
  contrast: 0,
  highlights: 0,
  shadows: 0,
  whites: 0,
  blacks: 0,
};

/** Baut die JSON-Serialisierung eines `EdlEnvelope` (siehe
 * `apx_core::EdlEnvelope`), wie sie sowohl die `develop/...`-Protokoll-
 * Route als auch `apply_develop_edit` erwarten. */
export function buildEdlEnvelopeJson(basic: BasicAdjustments): string {
  return JSON.stringify({
    schema_version: EDL_SCHEMA_VERSION,
    payload: { basic },
  });
}

/** Liest `BasicAdjustments` aus einem `EdlEnvelope`-JSON-String (z. B. aus
 * `current_develop_edit`/`undo_develop_edit`/`redo_develop_edit`). Gibt
 * bei unbekannter Schema-Version oder unlesbarer Nutzlast `null` zurück,
 * statt einen Absturz zu riskieren — der Aufrufer entscheidet dann, ob er
 * auf `NEUTRAL_BASIC_ADJUSTMENTS` zurückfällt. */
export function parseEdlEnvelopeJson(json: string): BasicAdjustments | null {
  try {
    const parsed: unknown = JSON.parse(json);
    if (typeof parsed !== "object" || parsed === null) return null;
    const envelope = parsed as { schema_version?: unknown; payload?: unknown };
    if (envelope.schema_version !== EDL_SCHEMA_VERSION) return null;
    const payload = envelope.payload as { basic?: unknown } | undefined;
    const basic = payload?.basic;
    if (typeof basic !== "object" || basic === null) return null;
    // Keine tiefe Struktur-Validierung (Feld für Feld) — anders als die
    // Rust-Seite (die `serde` strukturell prüfen lässt) reicht hier ein
    // grober Plausibilitätscheck, da diese Funktion nur auf Antworten
    // angewendet wird, die dasselbe Backend gerade erst geschrieben hat.
    return basic as BasicAdjustments;
  } catch {
    return null;
  }
}

/** Ein einzelner Regler-Eintrag für `DevelopPanel` — Wertebereich und
 * Schrittweiten nach `SPEC.md` §4 (Pfeiltasten = Feinschritt, Umschalt +
 * Pfeiltasten = Grobschritt). */
export interface SliderSpec {
  key: string;
  label: string;
  min: number;
  max: number;
  /** Schrittweite bei einfachem Pfeiltasten-Druck. */
  fineStep: number;
  /** Schrittweite bei Umschalt+Pfeiltasten. */
  coarseStep: number;
  neutral: number;
}

export const BASIC_SLIDER_SPECS: readonly SliderSpec[] = [
  { key: "temp_shift_kelvin", label: "Temperatur", min: -2000, max: 2000, fineStep: 10, coarseStep: 100, neutral: 0 },
  { key: "tint_shift", label: "Tint", min: -100, max: 100, fineStep: 1, coarseStep: 10, neutral: 0 },
  { key: "exposure_ev", label: "Belichtung", min: -5, max: 5, fineStep: 0.01, coarseStep: 0.1, neutral: 0 },
  { key: "contrast", label: "Kontrast", min: -100, max: 100, fineStep: 1, coarseStep: 10, neutral: 0 },
  { key: "highlights", label: "Lichter", min: -100, max: 100, fineStep: 1, coarseStep: 10, neutral: 0 },
  { key: "shadows", label: "Tiefen", min: -100, max: 100, fineStep: 1, coarseStep: 10, neutral: 0 },
  { key: "whites", label: "Weiß", min: -100, max: 100, fineStep: 1, coarseStep: 10, neutral: 0 },
  { key: "blacks", label: "Schwarz", min: -100, max: 100, fineStep: 1, coarseStep: 10, neutral: 0 },
] as const;

export function clampSliderValue(value: number, spec: Pick<SliderSpec, "min" | "max">): number {
  return Math.min(spec.max, Math.max(spec.min, value));
}

/** Wert nach einem Pfeiltasten-Druck (siehe `SPEC.md` §4: „Pfeiltasten =
 * Feinschritt, Umschalt = Grobschritt"). */
export function applyArrowStep(value: number, direction: 1 | -1, spec: SliderSpec, coarse: boolean): number {
  const step = coarse ? spec.coarseStep : spec.fineStep;
  return clampSliderValue(value + direction * step, spec);
}

/** Liest ein verschachteltes `BasicAdjustments`-Feld über den
 * `SliderSpec`-Schlüssel (`"temp_shift_kelvin"`/`"tint_shift"` liegen
 * unter `white_balance`, alle anderen direkt unter `basic`). */
export function readBasicField(basic: BasicAdjustments, key: string): number {
  if (key === "temp_shift_kelvin" || key === "tint_shift") {
    return basic.white_balance[key];
  }
  return basic[key as Exclude<keyof BasicAdjustments, "white_balance">];
}

/** Schreibt ein Feld über denselben Schlüssel wie [`readBasicField`] —
 * mutiert `basic` direkt (zum Aufruf innerhalb eines Immer-`set()`-Blocks
 * im Store gedacht, siehe `store/index.ts`). */
export function writeBasicField(basic: BasicAdjustments, key: string, value: number): void {
  if (key === "temp_shift_kelvin" || key === "tint_shift") {
    basic.white_balance[key] = value;
    return;
  }
  basic[key as Exclude<keyof BasicAdjustments, "white_balance">] = value;
}
