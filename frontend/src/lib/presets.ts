import { neutralEdlPayload } from "./edl";
import type { EdlPayload } from "./edl";

/**
 * Presets (Phase 5, siehe `DECISIONS.md` ADR-0031) — ein Preset speichert
 * eine *Teilmenge* der EDL-Sektionen, nicht das komplette `EdlPayload`.
 * Reparatur und Masken sind bewusst nie Teil eines Presets (bildspezifische
 * Klon-/Reparatur-Striche und lokale Masken-Geometrie — z. B. ein Pinsel-
 * Strich an einer bestimmten Bildposition — sind kein „Look", der auf ein
 * anderes Foto übertragbar wäre; siehe auch `DECISIONS.md` ADR-0032).
 */
export type PresetSectionKey = Exclude<keyof EdlPayload, "repair" | "masks" | "mask_groups">;

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
 * immer UND-verknüpft (kein ODER, keine Verschachtelung). `section: null`
 * bedeutet „gilt für das ganze Preset" (schlägt die Regel fehl, wird das
 * gesamte Preset nicht angewendet); eine gesetzte Sektion grenzt die
 * Wirkung eines Fehlschlags auf genau diese Sektion ein (`PLAN.md` Phase
 * 5 Schritt 7: „Eine fehlgeschlagene Regel schließt nur die betroffene
 * Sektion aus, nicht das ganze Preset"). */
export interface PresetCondition {
  field: PresetConditionField;
  op: PresetConditionOperator;
  value: string;
  section: PresetSectionKey | null;
}

/** Die für die Regel-Auswertung relevante Teilmenge der Fotometadaten
 * (`PhotoDto`) — als eigenes, schlankes Interface statt eines Imports aus
 * `lib/tauri.ts`, damit dieses Modul unabhängig von den Tauri-DTOs bleibt
 * und einfach zu testen ist. */
export interface PresetConditionPhotoMeta {
  iso: number | null;
  aperture: number | null;
  focal_length: number | null;
  camera_model: string | null;
  lens: string | null;
}

/** Prüft eine einzelne Regel gegen die Fotometadaten. Ein `null`-Metadatum
 * (z. B. kein EXIF-ISO-Wert vorhanden) lässt die Regel fehlschlagen —
 * konservativ: eine Bedingung, die sich nicht auswerten lässt, gilt als
 * nicht erfüllt, statt sie stillschweigend zu ignorieren. `contains` ist
 * nur für die String-Felder (Kameramodell/Objektiv) sinnvoll und
 * vergleicht Kleinbuchstaben-Teilstrings; numerische Operatoren (`>`, `<`,
 * `=`) sind nur für die Zahlenfelder definiert. Ein nicht passender
 * Feld/Operator-Kombination (z. B. `>` auf `camera_model`) gilt ebenfalls
 * als nicht erfüllt. */
export function evaluateCondition(condition: PresetCondition, photo: PresetConditionPhotoMeta): boolean {
  switch (condition.field) {
    case "iso":
    case "aperture":
    case "focal_length": {
      const actual = photo[condition.field];
      if (actual === null) return false;
      const expected = Number(condition.value);
      if (Number.isNaN(expected)) return false;
      switch (condition.op) {
        case ">":
          return actual > expected;
        case "<":
          return actual < expected;
        case "=":
          return actual === expected;
        default:
          return false;
      }
    }
    case "camera_model":
    case "lens": {
      const actual = photo[condition.field];
      if (actual === null) return false;
      switch (condition.op) {
        case "contains":
          return actual.toLowerCase().includes(condition.value.toLowerCase());
        case "=":
          return actual.toLowerCase() === condition.value.toLowerCase();
        default:
          return false;
      }
    }
  }
}

/** Wendet alle Regeln eines Presets auf eine EDL-Teilmenge an: Regeln ohne
 * `section` (gelten fürs ganze Preset) — schlägt eine davon fehl, wird
 * `null` zurückgegeben (Preset komplett übersprungen); Regeln mit
 * `section` entfernen bei einem Fehlschlag nur ihre eine Sektion aus der
 * Teilmenge (mehrere Regeln je Sektion sind UND-verknüpft, siehe
 * `PresetCondition`-Doku oben). Presets ohne jede Bedingung (`conditions`
 * leer) geben `subset` unverändert zurück. */
export function applyConditionsToSubset(
  subset: PresetEdlSubset,
  conditions: readonly PresetCondition[],
  photo: PresetConditionPhotoMeta | null,
): PresetEdlSubset | null {
  if (conditions.length === 0) return subset;
  // Ohne bekanntes Foto (z. B. noch keine Auswahl) werden Bedingungen
  // konservativ als nicht erfüllt behandelt — siehe `evaluateCondition`s
  // Dokumentation zu fehlenden Metadaten.
  const meta: PresetConditionPhotoMeta = photo ?? { iso: null, aperture: null, focal_length: null, camera_model: null, lens: null };

  for (const condition of conditions) {
    if (condition.section === null && !evaluateCondition(condition, meta)) {
      return null;
    }
  }

  const excludedSections = new Set<PresetSectionKey>();
  for (const condition of conditions) {
    if (condition.section !== null && !evaluateCondition(condition, meta)) {
      excludedSections.add(condition.section);
    }
  }
  if (excludedSections.size === 0) return subset;

  const result: Record<string, unknown> = { ...subset };
  for (const section of excludedSections) {
    delete result[section];
  }
  return result as PresetEdlSubset;
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

// ---- Versionierung + Diff-Ansicht (Phase 5 Schritt 8) -----------------------

function isPlainObject(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

/** Eine einzelne Abweichung zwischen zwei Preset-Versionen — `path` ist der
 * Feld-Pfad (z. B. `"basic.exposure_ev"`), `a`/`b` die jeweiligen Werte
 * (`undefined`, wenn das Feld in dieser Version fehlt, z. B. weil die
 * Sektion damals nicht ausgewählt war). */
export interface EdlSubsetDiffEntry {
  path: string;
  a: unknown;
  b: unknown;
}

/** Vergleicht zwei EDL-Teilmengen feldweise und liefert jeden abweichenden
 * Blattwert mit Pfad — die Grundlage der kleinen Diff-Ansicht beim
 * Vergleichen zweier `PresetVersionDto`s. Steigt rekursiv in verschachtelte
 * Objekte ab (HSL-Bänder, Color-Grading-Farbräder, Weißabgleich-
 * Unterobjekt …), behandelt Arrays aber als atomaren Wert (Kurvenpunkte,
 * Farbmischer-Regionen, Objektivkorrektur-Hilfslinien sind strukturierte
 * Listen, kein sinnvoll feldweise vergleichbarer Wert) — dieselbe
 * Konvention wie `interpolateValue`s Umgang mit nicht-skalaren
 * Preset-Bestandteilen. Ein Feld, das nur in einer der beiden Versionen
 * existiert (z. B. weil eine Sektion damals nicht ausgewählt war), zählt
 * als Abweichung gegen `undefined`. */
export function diffEdlSubsets(a: PresetEdlSubset, b: PresetEdlSubset): EdlSubsetDiffEntry[] {
  const entries: EdlSubsetDiffEntry[] = [];

  function walk(path: string, va: unknown, vb: unknown) {
    const aIsObject = isPlainObject(va);
    const bIsObject = isPlainObject(vb);
    // Auch wenn nur eine Seite ein Objekt ist (die andere `undefined`,
    // weil eine Sektion in dieser Version gar nicht ausgewählt war) wird
    // bis auf Blattebene abgestiegen — sonst würde eine ganze fehlende
    // Sektion nur als ein einziger grober Eintrag erscheinen statt als
    // die tatsächlich betroffenen Einzelfelder.
    if (aIsObject || bIsObject) {
      const keys = new Set([...(aIsObject ? Object.keys(va) : []), ...(bIsObject ? Object.keys(vb) : [])]);
      for (const key of keys) {
        walk(`${path}.${key}`, aIsObject ? va[key] : undefined, bIsObject ? vb[key] : undefined);
      }
      return;
    }
    if (JSON.stringify(va) !== JSON.stringify(vb)) {
      entries.push({ path, a: va, b: vb });
    }
  }

  const topLevelKeys = new Set([...Object.keys(a), ...Object.keys(b)]);
  for (const key of topLevelKeys) {
    walk(key, (a as Record<string, unknown>)[key], (b as Record<string, unknown>)[key]);
  }
  return entries;
}

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
