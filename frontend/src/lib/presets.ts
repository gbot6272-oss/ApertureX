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
