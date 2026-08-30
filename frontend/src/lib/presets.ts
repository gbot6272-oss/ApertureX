import { neutralEdlPayload } from "./edl";
import type { EdlPayload } from "./edl";

/**
 * Presets (Phase 5, siehe `DECISIONS.md` ADR-0031) — ein Preset speichert
 * eine *Teilmenge* der EDL-Sektionen, nicht das komplette `EdlPayload`.
 * Reparatur ist bewusst nie Teil eines Presets (bildspezifische
 * Klon-/Reparatur-Striche sind kein „Look", der auf ein anderes Foto
 * übertragbar wäre).
 */
export type PresetSectionKey = Exclude<keyof EdlPayload, "repair">;

export const PRESET_SECTION_KEYS: readonly PresetSectionKey[] = [
  "basic",
  "curves",
  "hsl",
  "color_mixer",
  "color_grading",
  "details",
  "lens_corrections",
  "effects",
  "calibration",
  "geometry",
];

export const PRESET_SECTION_LABELS: Record<PresetSectionKey, string> = {
  basic: "Grundeinstellungen",
  curves: "Kurven",
  hsl: "HSL",
  color_mixer: "Farbmischer",
  color_grading: "Color Grading",
  details: "Details",
  lens_corrections: "Objektivkorrekturen",
  effects: "Effekte",
  calibration: "Kalibrierung",
  geometry: "Geometrie",
};

/** Die eigentliche gespeicherte EDL-Teilmenge — für `apx-catalog`/
 * `apx-app` ein opaker JSON-String (`PresetVersionDto.edl_subset_json`),
 * hier auf der Frontend-Seite typisiert. Nur die beim Speichern
 * ausgewählten Sektionen sind gesetzt. */
export type PresetEdlSubset = Partial<Pick<EdlPayload, PresetSectionKey>>;

/** Extrahiert genau die ausgewählten Sektionen aus einem vollständigen
 * `EdlPayload` — der Checkbox-Dialog beim Speichern eines Presets
 * (`SavePresetDialog.tsx`, `SPEC.md` §3.5: „Beim Speichern zeigt ein
 * Dialog jede einzelne Einstellungsgruppe mit Checkbox"). */
export function buildPresetEdlSubset(edl: EdlPayload, sections: readonly PresetSectionKey[]): PresetEdlSubset {
  const subset: Record<string, unknown> = {};
  for (const key of sections) {
    subset[key] = edl[key];
  }
  return subset as PresetEdlSubset;
}

export function parseEdlSubset(json: string): PresetEdlSubset {
  try {
    const parsed: unknown = JSON.parse(json);
    if (parsed && typeof parsed === "object") return parsed as PresetEdlSubset;
    return {};
  } catch {
    return {};
  }
}

export function serializeEdlSubset(subset: PresetEdlSubset): string {
  return JSON.stringify(subset);
}

// ---- Preset-Stärke (0-200 %, siehe SPEC.md §3.5) ---------------------------

/** Interpoliert einen einzelnen Wert zwischen seiner Neutralstellung und
 * dem im Preset gespeicherten Zielwert. Nur numerische Blattwerte werden
 * skaliert (Regler-Werte im eigentlichen Sinn); verschachtelte Objekte
 * (z. B. `basic.white_balance`, HSL-Bänder, Color-Grading-Farbräder)
 * werden rekursiv abgestiegen. Arrays (Kurven-Punkte, Farbmischer-
 * Regionen, Objektivkorrektur-Hilfslinien) sind strukturierte Listen,
 * kein linear interpolierbarer „Wert" — sie werden unskaliert
 * übernommen, ebenso Strings/Booleans/Enums (Kurventyp, Upright-Modus,
 * Kameraprofil-Auswahl usw.). Dieselbe Einschränkung hat auch Lightroom
 * bei nicht-skalaren Preset-Bestandteilen. */
function interpolateValue(neutral: unknown, target: unknown, t: number): unknown {
  if (typeof target === "number" && typeof neutral === "number") {
    return neutral + (target - neutral) * t;
  }
  if (Array.isArray(target)) {
    return target;
  }
  if (target && typeof target === "object" && neutral && typeof neutral === "object") {
    const result: Record<string, unknown> = {};
    for (const key of Object.keys(target as Record<string, unknown>)) {
      result[key] = interpolateValue((neutral as Record<string, unknown>)[key], (target as Record<string, unknown>)[key], t);
    }
    return result;
  }
  return target;
}

/** Skaliert jede Sektion einer EDL-Teilmenge auf `strengthPercent` (0–200)
 * ihres Wegs von der jeweiligen Neutralstellung zum gespeicherten
 * Zielwert — 100 % ist der Preset-Wert unverändert, 0 % ist neutral,
 * 200 % verdoppelt den Abstand zur Neutralstellung. */
export function scalePresetEdlSubset(subset: PresetEdlSubset, strengthPercent: number): PresetEdlSubset {
  const neutral = neutralEdlPayload() as unknown as Record<string, unknown>;
  const t = strengthPercent / 100;
  const scaled: Record<string, unknown> = {};
  for (const key of Object.keys(subset)) {
    scaled[key] = interpolateValue(neutral[key], (subset as Record<string, unknown>)[key], t);
  }
  return scaled as PresetEdlSubset;
}

/** Ersetzt in `base` genau die in `subset` enthaltenen Sektionen — jede
 * ausgewählte Sektion wird als Ganzes übernommen (siehe
 * `buildPresetEdlSubset`), nicht feldweise gemischt. */
export function mergeEdlSubset(base: EdlPayload, subset: PresetEdlSubset): EdlPayload {
  return { ...base, ...subset };
}

// ---- Bedingte Presets (vereinfacht, siehe DECISIONS.md ADR-0031 Punkt 4) ---

export type PresetConditionField = "iso" | "aperture" | "focal_length" | "camera_model" | "lens";
export type PresetConditionOperator = ">" | "<" | "=" | "contains";

/** Eine einzelne Bedingungsregel — mehrere Regeln in einem Preset sind
 * immer UND-verknüpft (kein ODER, keine Verschachtelung). */
export interface PresetCondition {
  field: PresetConditionField;
  op: PresetConditionOperator;
  value: string;
}

export const PRESET_CONDITION_FIELD_OPTIONS: ReadonlyArray<{ value: PresetConditionField; label: string }> = [
  { value: "iso", label: "ISO" },
  { value: "aperture", label: "Blende" },
  { value: "focal_length", label: "Brennweite" },
  { value: "camera_model", label: "Kameramodell" },
  { value: "lens", label: "Objektiv" },
];

export const PRESET_CONDITION_OPERATOR_OPTIONS: ReadonlyArray<{ value: PresetConditionOperator; label: string }> = [
  { value: ">", label: ">" },
  { value: "<", label: "<" },
  { value: "=", label: "=" },
  { value: "contains", label: "enthält" },
];

export function parseConditions(json: string): PresetCondition[] {
  try {
    const parsed: unknown = JSON.parse(json);
    return Array.isArray(parsed) ? (parsed as PresetCondition[]) : [];
  } catch {
    return [];
  }
}

export function serializeConditions(conditions: PresetCondition[]): string {
  return JSON.stringify(conditions);
}
