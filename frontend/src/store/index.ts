import { create } from "zustand";
import { immer } from "zustand/middleware/immer";

import {
  AI_MASK_KIND_LABELS,
  base64ToByteArray,
  buildEdlEnvelopeJson,
  defaultColorRangeGeometry,
  defaultLinearGradientGeometry,
  defaultLuminanceRangeGeometry,
  defaultRadialGradientGeometry,
  emptyBrushGeometry,
  MAX_COLOR_MIXER_REGIONS,
  neutralEdlPayload,
  newColorMixerRegion,
  newMask,
  parseEdlEnvelopeJson,
  WHITE_BALANCE_PRESETS,
  writeBasicField,
} from "../lib/edl";
import type { AiMaskKind, BlendMode, CalibrationAdjustment, ColorGradingAdjustment, ColorGradingWheel, ColorMixerRegion, CropRect, CurveChannel, CurvesAdjustment, DetailsAdjustment, EdlPayload, EffectsAdjustment, GridOverlay, GuidedLine, HslAdjustment, HslBand, LensCorrectionAdjustment, ManualTransform, Mask, MaskCombine, MaskGeometry, MaskPoint, PrimaryColorAdjustment, RepairMode, RepairPoint, UprightMode } from "../lib/edl";
import { hueDegreesFromRgbByte } from "../lib/colorSampling";
import {
  applyConditionsToSubset,
  buildPresetEdlSubset,
  mergeEdlSubset,
  parseConditions,
  parseEdlSubset,
  PRESET_SECTION_KEYS,
  scalePresetEdlSubset,
  serializeConditions,
  serializeEdlSubset,
} from "../lib/presets";
import type { PresetCondition, PresetConditionPhotoMeta, PresetEdlSubset, PresetSectionKey } from "../lib/presets";
import { sortPhotos } from "../lib/sortPhotos";
import type { SortDirection, SortField } from "../lib/sortPhotos";
import type { SoftProofIntent, SoftProofProfile } from "../lib/softProof";
import * as api from "../lib/tauri";
import type {
  AiSettingsDto,
  BookOptions,
  BookOutcomeDto,
  CatalogStatusDto,
  CollectionDto,
  ExportOutcomeDto,
  ExportPhotoOptions,
  FilterCriteriaDto,
  FolderDto,
  HistoryPositionDto,
  ImportModeDto,
  ImportPresetDto,
  KeywordDto,
  PhotoDto,
  PresetDto,
  PresetFolderDto,
  PrintLayoutOptions,
  SlideshowVideoOptions,
  SlideshowVideoOutcomeDto,
  SnapshotDto,
  ColorLabelDefinitionDto,
  CollectionFolderDto,
  GpxTrackPointDto,
  StackDto,
  TemplateDto,
  TemplateKind,
  WebGalleryOptions,
  WebGalleryOutcomeDto,
  WorkflowTemplatePayload,
  SpotCandidateDto,
} from "../lib/tauri";
import * as undoStackLib from "../lib/undoStack";
import type { UndoEntry } from "../lib/undoStack";
import { computeWhiteBalanceShiftFromSample } from "../lib/whiteBalancePicker";

/** Wandelt eine `HistoryPositionDto` (siehe `lib/tauri.ts`) in ein volles
 * `EdlPayload` um — `Neutral` bedeutet "wie aufgenommen", ein unlesbares
 * `edl_json` fällt (mit einer Konsolen-Warnung) ebenfalls auf neutral
 * zurück statt abzustürzen. Exportiert seit Phase 6 Schritt 10: die
 * Referenzansicht (`ReferenceView.tsx`) lädt damit den Stand eines
 * *anderen* Fotos, genau wie `applyPreviousSettings`/`syncSettingsToSelection`
 * es hier bereits für andere Fotos tun. */
export function edlFromHistoryPosition(position: HistoryPositionDto): EdlPayload {
  if (position.kind === "Neutral") return neutralEdlPayload();
  const parsed = parseEdlEnvelopeJson(position.edl_json);
  if (!parsed) {
    console.error("Unlesbares EDL vom Backend erhalten, falle auf neutral zurück:", position.edl_json);
    return neutralEdlPayload();
  }
  return parsed;
}

/** In welcher Reihenfolge Foto-ID `photoId` in der aktuell angezeigten
 * Liste gemeint ist, wenn ein Bereich per Umschalt-Klick markiert wird —
 * siehe [`selectActivePhotos`]. */
export type SelectionMode = "replace" | "toggle" | "range";

/** Liest aus einem Klick-Event, welcher Auswahlmodus gemeint ist (Strg/Cmd
 * = einzelnes Umschalten, Umschalt = Bereich, sonst Ersetzen) — von Raster
 * und Filmstreifen gemeinsam genutzt (siehe `DECISIONS.md` ADR-0024). */
export function resolveSelectionMode(event: { ctrlKey: boolean; metaKey: boolean; shiftKey: boolean }): SelectionMode {
  if (event.shiftKey) return "range";
  if (event.ctrlKey || event.metaKey) return "toggle";
  return "replace";
}

/** Patcht ein Foto an jeder Stelle, wo es aktuell im Zustand zwischen-
 * gespeichert sein könnte (Ordner-Cache, Sammlungs-Cache, Such-/Filter-
 * Ergebnis) — hält Raster/Filmstreifen nach einer Bewertungs-/Flaggen-/
 * Farb-Änderung sofort konsistent, ohne jedes Mal neu vom Backend zu
 * laden. Muss innerhalb eines Immer-`set()`-Producers aufgerufen werden. */
function patchPhotoEverywhere(state: AppStore, photoId: string, patch: Partial<PhotoDto>) {
  for (const list of Object.values(state.photosByFolder)) {
    const target = list.find((p) => p.id === photoId);
    if (target) Object.assign(target, patch);
  }
  for (const list of Object.values(state.collectionPhotos)) {
    const target = list.find((p) => p.id === photoId);
    if (target) Object.assign(target, patch);
  }
  if (state.libraryResults) {
    const target = state.libraryResults.find((p) => p.id === photoId);
    if (target) Object.assign(target, patch);
  }
}

/** Liest eine Kopie des aktuell bekannten Zustands eines Fotos, egal in
 * welchem Zwischenspeicher es gerade liegt (Ordner-Cache, Sammlungs-Cache,
 * Such-/Filter-Ergebnis) — Grundlage dafür, vor einer Bewertungs-/Flaggen-/
 * Farb-Änderung den *alten* Wert für einen Undo-Eintrag festzuhalten (siehe
 * `lib/undoStack.ts`, `DECISIONS.md` ADR-0027). Anders als
 * [`patchPhotoEverywhere`] (das *alle* Fundstellen patcht) genügt hier die
 * erste Fundstelle — der Wert ist an jeder Stelle identisch. */
/** Liest die für bedingte Presets relevanten Metadaten des aktuell
 * ausgewählten Fotos (`lib/presets.ts`s `evaluateCondition`) — `null` ohne
 * Auswahl, wodurch jede Bedingung dort konservativ als nicht erfüllt gilt.
 * Gibt bewusst die `PhotoDto`-Referenz selbst zurück (strukturell kompatibel
 * zu `PresetConditionPhotoMeta`, das nur eine Teilmenge ihrer Felder
 * verlangt) statt ein neues Objekt zu bauen — als React-Hook-Selektor
 * (`useAppStore(selectPresetConditionMeta)`) würde ein frisches Objekt bei
 * jedem Aufruf sonst Zustands eingebauten Referenzgleichheits-Check
 * durchgängig als „geändert" werten und eine Endlosschleife auslösen. */
export function selectPresetConditionMeta(state: AppStore): PresetConditionPhotoMeta | null {
  if (!state.selectedPhotoId) return null;
  return findPhotoAnywhere(state, state.selectedPhotoId) ?? null;
}

function findPhotoAnywhere(state: AppStore, photoId: string): PhotoDto | undefined {
  for (const list of Object.values(state.photosByFolder)) {
    const found = list.find((p) => p.id === photoId);
    if (found) return found;
  }
  for (const list of Object.values(state.collectionPhotos)) {
    const found = list.find((p) => p.id === photoId);
    if (found) return found;
  }
  return state.libraryResults?.find((p) => p.id === photoId);
}

/** Die aktuell anzuzeigende Fotoliste — Suchergebnisse (falls eine Suche/
 * ein Filter aktiv ist) haben Vorrang vor einer ausgewählten Sammlung, die
 * wiederum Vorrang vor einem ausgewählten Ordner hat. Raster und
 * Filmstreifen lesen beide über diese eine Funktion (siehe `DECISIONS.md`
 * ADR-0024: geteilter Fotoliste-Zustand statt eigener Parallel-Logik). */
function rawActivePhotos(state: AppStore): PhotoDto[] {
  if (state.libraryResults !== null) return state.libraryResults;
  if (state.selectedCollectionId) return state.collectionPhotos[state.selectedCollectionId] ?? [];
  if (state.selectedFolderId) return state.photosByFolder[state.selectedFolderId] ?? [];
  return [];
}

export function selectActivePhotos(state: AppStore): PhotoDto[] {
  // `??`-Fallbacks: Store-Tests bauen oft nur einen Teilzustand (siehe
  // `store/index.test.ts`s `makeState`) ohne die Sortierfelder — Default
  // entspricht dem bisherigen impliziten Verhalten (siehe `lib/sortPhotos.ts`).
  return sortPhotos(rawActivePhotos(state), state.librarySortField ?? "filename", state.librarySortDirection ?? "asc");
}

/**
 * Zustand-Store mit den in `PHASE1_PROMPT.md` Abschnitt 7 geforderten
 * Slices: `catalog`, `selection`, `viewer`, `jobs`. Alle vier leben in
 * einer Datei statt in getrennten Modulen — bei vier kleinen Slices
 * vermeidet das die sonst nötigen zirkulären Typ-Importe des
 * Zustand-"Slices-Patterns", ohne die Struktur zu verlieren (jeder
 * Abschnitt unten ist unabhängig lesbar).
 *
 * Undo/Redo-Middleware (laut SPEC.md Abschnitt 1 für State vorgesehen)
 * kommt erst mit den Entwickeln-Reglern in Phase 2 — Phase 1 hat noch
 * keine Bearbeitungs-Historie, die man rückgängig machen könnte.
 */

// ---- Catalog-Slice ---------------------------------------------------

interface CatalogSlice {
  folders: FolderDto[];
  photosByFolder: Record<string, PhotoDto[]>;
  catalogStatus: CatalogStatusDto | null;
  catalogError: string | null;
  refreshFolders: () => Promise<void>;
  refreshCatalogStatus: () => Promise<void>;
  loadPhotosForFolder: (folderId: string) => Promise<void>;
  /** Verknüpft einen als fehlend markierten Ordner mit `newPath` neu und
   * lädt Ordnerliste sowie (falls gerade geöffnet) dessen Fotos danach
   * neu — siehe `FolderDto.missing`. */
  relinkFolder: (folderId: string, newPath: string) => Promise<void>;
}

// ---- Selection-Slice ---------------------------------------------------

interface SelectionSlice {
  selectedFolderId: string | null;
  selectedPhotoId: string | null;
  selectFolder: (folderId: string | null) => void;
  selectPhoto: (photoId: string | null) => void;
  /** Wählt das nächste/vorherige Foto im aktuell selektierten Ordner. */
  stepSelection: (direction: 1 | -1) => void;
}

// ---- Viewer-Slice ---------------------------------------------------

export type FitMode = "fit" | "fill" | "manual";

interface ViewerSlice {
  zoom: number; // 1.0 = 100 %
  fitMode: FitMode;
  panX: number;
  panY: number;
  setZoom: (zoom: number, fitMode?: FitMode) => void;
  setPan: (x: number, y: number) => void;
  resetView: () => void;
}

// ---- Jobs-Slice (Import) ---------------------------------------------------

interface ImportProgressState {
  done: number;
  total: number;
  currentFile: string | null;
}

interface ImportResultState {
  imported: number;
  skipped: number;
  errorCount: number;
  cancelled: boolean;
  /** Anzahl Fotos, die laut exaktem Inhalts-Hash ein Duplikat eines
   * anderen Katalogeintrags sind (Schritt 8.2, `DECISIONS.md` ADR-0027) —
   * reine Anzeige, verhindert den Import nicht. */
  duplicateCount: number;
}

interface JobsSlice {
  importRunning: boolean;
  importProgress: ImportProgressState | null;
  importResult: ImportResultState | null;
  importErrors: string[];
  startImport: (path: string) => Promise<void>;
  /** Wie `startImport`, aber mit wählbarem Modus (Kopieren/Verschieben in
   * einen Zielordner) und optionalem Umbenennungsmuster (Phase 5 Schritt 9,
   * `DECISIONS.md` ADR-0031 Punkt 7 — Frontend-Anbindung des seit Phase 3
   * bestehenden, bis dahin ungenutzten `import_folder_with_mode`-Commands). */
  startImportWithMode: (path: string, mode: ImportModeDto, renamePattern: string | null) => Promise<void>;
  cancelImport: () => Promise<void>;
  setImportProgress: (progress: ImportProgressState) => void;
  addImportError: (line: string) => void;
  finishImport: (result: ImportResultState) => void;

  /** Gespeicherte Import-Presets (Modus + Zielordner + Umbenennungsmuster
   * unter einem Namen) — reine Datei-Konfiguration ohne Katalog-Bezug,
   * siehe `crate::import::presets`-Moduldoku. */
  importPresets: ImportPresetDto[];
  refreshImportPresets: () => Promise<void>;
  saveImportPresetEntry: (preset: ImportPresetDto) => Promise<void>;
  deleteImportPresetEntry: (name: string) => Promise<void>;
}

// ---- Develop-Slice (ab Phase 2) ---------------------------------------

interface DevelopSlice {
  developPanelOpen: boolean;
  /** Der aktuell im Panel gezeigte (u. U. noch nicht committete)
   * Bearbeitungszustand — das volle EDL (alle zehn Phase-4-Kategorien,
   * siehe `lib/edl.ts`s `EdlPayload`), laufend über `setBasicField`
   * (und ab den jeweiligen späteren Schritten weitere Setter je
   * Werkzeugkategorie) verändert, während gezogen wird; nur
   * `commitDevelopEdit()` schreibt ihn dauerhaft in den Katalog (siehe
   * `crates/apx-catalog`s `edit_history`, ADR-0014). */
  developEdl: EdlPayload;
  /** Zu welchem Foto `developEdl` gehört — verhindert, dass beim
   * schnellen Fotowechsel ein veralteter Zustand kurz sichtbar bleibt. */
  developPhotoId: string | null;
  toggleDevelopPanel: () => void;
  /** Lädt den zuletzt gespeicherten Bearbeitungszustand für `photoId`
   * (oder neutral, falls noch nie bearbeitet) — aufgerufen beim Öffnen
   * des Panels und bei jedem Fotowechsel, während es offen ist. */
  loadDevelopStateForPhoto: (photoId: string) => Promise<void>;

  // ---- Schritt 9: Einstellungen kopieren/einfügen, Vorherige, Sync --------
  /** Der zuletzt kopierte Ausschnitt (Schritt 9, `SPEC.md` §3.4) —
   * derselbe `PresetEdlSubset`-Mechanismus wie Presets aus Phase 5, nur
   * direkt aus `developEdl` gebaut statt aus einem gespeicherten Preset. */
  copiedEdlSubset: PresetEdlSubset | null;
  copyDevelopSettings: (sections: PresetSectionKey[]) => void;
  /** Fügt `copiedEdlSubset` in das aktuelle `developEdl` ein und committet
   * sofort — No-op, wenn noch nichts kopiert wurde. */
  pasteDevelopSettings: () => void;
  /** Das zuletzt im Entwickeln-Panel aktive Foto *vor* dem aktuellen —
   * gepflegt in `loadDevelopStateForPhoto`, Grundlage für
   * `applyPreviousSettings` ("Vorherige übernehmen", Lightroom-Analog). */
  lastDevelopPhotoId: string | null;
  /** Übernimmt den zuletzt gespeicherten Stand von `lastDevelopPhotoId`
   * vollständig (alle Sektionen) auf das aktuelle Foto und committet. */
  applyPreviousSettings: () => Promise<void>;
  /** Überträgt einen Ausschnitt des aktuellen `developEdl` auf die
   * übrige Mehrfachauswahl (`multiSelectedIds`, siehe `setPhotoRating`s
   * Stapel-Bearbeitungs-Muster) — jedes Zielfoto behält seine sonstigen
   * Einstellungen, nur die gewählten Sektionen werden überschrieben. */
  syncSettingsToSelection: (sections: PresetSectionKey[]) => Promise<void>;
  /** Ist Auto-Sync aktiv, überträgt jeder `commitDevelopEdit`-Aufruf das
   * *gesamte* `developEdl` (alle Sektionen — bewusste Vereinfachung ggü.
   * `syncSettingsToSelection`s granularer Auswahl, siehe Moduldoku dort)
   * sofort auf die übrige Mehrfachauswahl. */
  autoSyncActive: boolean;
  toggleAutoSync: () => void;

  /** Benannte EDL-Zwischenstände zusätzlich zum linearen Verlauf (Phase 6
   * Schritt 8, `SPEC.md` §3.4) — siehe `crates/apx-app/src/commands.rs`s
   * Moduldoku zur Abgrenzung gegenüber Undo/Redo. Für das gerade in
   * `developPhotoId` offene Foto. */
  snapshots: SnapshotDto[];
  refreshSnapshots: () => Promise<void>;
  /** Legt einen Schnappschuss des *aktuellen* `developEdl`-Stands an. */
  saveSnapshot: (name: string) => Promise<void>;
  renameSnapshotAction: (snapshotId: string, name: string) => Promise<void>;
  removeSnapshot: (snapshotId: string) => Promise<void>;
  /** Committet den gespeicherten Schnappschuss-EDL als neuen aktiven
   * Bearbeitungsschritt (reuse von `apply_develop_edit`, kein eigener
   * Backend-Restore-Weg) und übernimmt ihn sofort in die Anzeige. */
  restoreSnapshot: (snapshotId: string) => Promise<void>;

  /** Vorher/Nachher-Ansicht im Viewer (Phase 6 Schritt 8, `SPEC.md` §3.4:
   * „in vier Ansichten") — „Vorher" ist das neutrale EDL (wie
   * aufgenommen), „Nachher" der aktuelle `developEdl`-Stand. */
  beforeAfterMode: "none" | "sideBySide" | "stacked" | "splitVertical" | "splitHorizontal";
  setBeforeAfterMode: (mode: AppStore["beforeAfterMode"]) => void;

  /** Referenzansicht (Phase 6 Schritt 10, `SPEC.md` §3.4/§7:
   * "Referenzbild links, Arbeitsbild rechts") — ein beliebiges anderes
   * Foto wird links statisch (letzter committeter Stand) neben dem
   * rechts live bearbeiteten Arbeitsbild angezeigt, siehe
   * `ReferenceView.tsx`. Unabhängig vom globalen `zoom`/`panX`/`panY`:
   * jede Bildhälfte führt ihren eigenen Zoom/Pan-Zustand lokal in der
   * Komponente (siehe deren Moduldoku zur Begründung). */
  referenceViewActive: boolean;
  referencePhotoId: string | null;
  toggleReferenceView: () => void;
  setReferencePhotoId: (photoId: string | null) => void;

  /** Soft-Proof-Vorschau (Phase 6 Schritt 10, `SPEC.md` §3.4/§7) — siehe
   * `lib/softProof.ts`s Moduldoku für die Vereinfachung gegenüber echtem
   * ICC-Farbmanagement (`DECISIONS.md` ADR-0032 Punkt 6). Reine
   * Anzeige-Einstellung, nicht Teil des EDL/der Datenbank. */
  softProofActive: boolean;
  softProofProfile: SoftProofProfile;
  softProofIntent: SoftProofIntent;
  softProofGamutWarning: boolean;
  softProofPaperWhite: boolean;
  toggleSoftProof: () => void;
  setSoftProofProfile: (profile: SoftProofProfile) => void;
  setSoftProofIntent: (intent: SoftProofIntent) => void;
  toggleSoftProofGamutWarning: () => void;
  toggleSoftProofPaperWhite: () => void;

  /** Setzt ein einzelnes Grundeinstellungs-Feld (Regler-Zwischenwert beim
   * Ziehen) — siehe `lib/edl.ts`s `SliderSpec.key` für gültige Schlüssel. */
  setBasicField: (key: string, value: number) => void;
  /** Ob die Weißabgleich-Pipette gerade auf einen Klick in den Viewer
   * wartet (Phase 4 Schritt 3, siehe `lib/whiteBalancePicker.ts`). */
  wbPickerActive: boolean;
  toggleWbPicker: () => void;
  /** Wertet einen im Viewer angeklickten RGBA8-Bildpunkt aus, korrigiert
   * den Weißabgleich additiv zum bestehenden Wert und committet sofort
   * (kein Zwischenzustand wie bei den Reglern — ein Klick ist eine
   * abgeschlossene Aktion). Schaltet die Pipette danach automatisch aus. */
  pickWhiteBalanceAt: (r: number, g: number, b: number) => void;
  /** Setzt den Weißabgleich absolut auf eines der `WHITE_BALANCE_PRESETS`
   * und committet sofort. */
  applyWhiteBalancePreset: (key: string) => void;
  /** Ersetzt eine der fünf Kurven (Phase 4 Schritt 4, siehe
   * `components/CurveEditor.tsx`) — Zwischenstand beim Ziehen, committet
   * wird separat über `commitDevelopEdit()`. */
  setCurveChannel: (key: keyof CurvesAdjustment, channel: CurveChannel) => void;
  /** Setzt ein einzelnes Feld eines der acht festen HSL-Bänder (Phase 4
   * Schritt 5) — Zwischenstand beim Ziehen. */
  setHslBandField: (band: keyof HslAdjustment, field: keyof HslBand, value: number) => void;
  /** Ob der Farbmischer gerade auf einen Klick in den Viewer wartet, um
   * eine neue Region anzulegen (teilt den Sampling-Code im Viewer mit
   * der Weißabgleich-Pipette, siehe `lib/colorSampling.ts`). */
  colorMixerPickerActive: boolean;
  toggleColorMixerPicker: () => void;
  /** Legt aus einem im Viewer angeklickten RGBA8-Bildpunkt eine neue
   * Farbmischer-Region an (Zielfarbton = Farbton des Klickpunkts,
   * restliche Regler neutral) — no-op, wenn `MAX_COLOR_MIXER_REGIONS`
   * bereits erreicht ist. Schaltet den Picker danach automatisch aus und
   * committet sofort. */
  addColorMixerRegionAt: (r: number, g: number, b: number) => void;
  /** Entfernt eine Farbmischer-Region per Index und committet sofort. */
  removeColorMixerRegion: (index: number) => void;
  /** Ändert ein Feld einer bestehenden Farbmischer-Region — Zwischenstand
   * beim Ziehen. */
  updateColorMixerRegion: (index: number, patch: Partial<ColorMixerRegion>) => void;
  /** Ersetzt eines der vier Color-Grading-Farbräder (Phase 4 Schritt 6) —
   * Zwischenstand beim Ziehen. */
  setColorGradingWheel: (key: keyof Pick<ColorGradingAdjustment, "shadows" | "midtones" | "highlights" | "global">, wheel: ColorGradingWheel) => void;
  setColorGradingBalance: (value: number) => void;
  setColorGradingBlending: (value: number) => void;
  /** Ändert ein Feld (Farbton/Sättigung) einer der drei Kalibrierungs-
   * Primärfarben (Phase 4 Schritt 7) — Zwischenstand beim Ziehen. */
  setCalibrationPrimaryField: (
    primary: keyof Pick<CalibrationAdjustment, "red_primary" | "green_primary" | "blue_primary">,
    field: keyof PrimaryColorAdjustment,
    value: number,
  ) => void;
  setCalibrationShadowTint: (value: number) => void;
  /** Setzt das Kameraprofil absolut (`null` = Standardprofil) und
   * committet sofort — ein Dropdown-Wechsel ist wie ein WB-Preset-Klick
   * eine abgeschlossene Aktion, kein Zwischenstand beim Ziehen. */
  setCalibrationCameraProfile: (value: string | null) => void;
  /** Setzt eines der zehn numerischen Details-Felder (Phase 4 Schritt 8)
   * — Zwischenstand beim Ziehen. */
  setDetailsField: (key: keyof Omit<DetailsAdjustment, "use_deconvolution_sharpen">, value: number) => void;
  /** Schaltet den Deconvolution-Schärfung-Alternativmodus um und
   * committet sofort — eine Checkbox ist wie ein Dropdown-Wechsel eine
   * abgeschlossene Aktion. */
  setDetailsUseDeconvolutionSharpen: (value: boolean) => void;
  /** Setzt eines der vier numerischen Objektivkorrektur-Felder (Phase 4
   * Schritt 9, ohne `manual_transform`) — Zwischenstand beim Ziehen. */
  setLensCorrectionField: (
    key: keyof Pick<LensCorrectionAdjustment, "ca_red_cyan" | "ca_blue_yellow" | "vignette_amount" | "distortion_amount">,
    value: number,
  ) => void;
  /** Setzt eines der sieben `manual_transform`-Felder — Zwischenstand
   * beim Ziehen. */
  setLensCorrectionManualTransformField: (key: keyof ManualTransform, value: number) => void;
  /** Setzt das Objektivprofil absolut und committet sofort (wie ein
   * WB-/Kameraprofil-Dropdown-Wechsel). */
  setLensCorrectionProfile: (value: string | null) => void;
  /** Schaltet die automatische CA-Korrektur um und committet sofort. */
  setLensCorrectionAutoCa: (value: boolean) => void;
  /** Setzt den Perspektive/Upright-Modus absolut und committet sofort. */
  setLensCorrectionUprightMode: (value: UprightMode) => void;
  /** Setzt ein Feld einer der zwei Guided-Hilfslinien (Phase 4 Schritt 9
   * — siehe `DECISIONS.md` ADR-0030: Zahlenfelder statt einer
   * Klick-Interaktion im Viewer) — legt die Linie mit Nullkoordinaten an,
   * falls sie noch nicht existiert. Zwischenstand beim Ziehen/Tippen. */
  setLensCorrectionGuidedLineField: (lineIndex: 0 | 1, field: keyof GuidedLine, value: number) => void;
  /** Setzt eines der acht numerischen Effekte-Felder (Phase 4 Schritt
   * 10, Vignettierung + Körnung) — Zwischenstand beim Ziehen. */
  setEffectsField: (key: keyof EffectsAdjustment, value: number) => void;
  /** Ob das Freistellen-Werkzeug gerade aktiv ist (Phase 4 Schritt 11)
   * — blendet `CropOverlay` im Viewer ein. */
  geometryCropActive: boolean;
  toggleGeometryCropActive: () => void;
  /** Ersetzt das Freistellungsrechteck — Zwischenstand beim Ziehen. */
  setGeometryCrop: (crop: CropRect) => void;
  setGeometryAngle: (value: number) => void;
  /** Setzt das Seitenverhältnis-Preset absolut und committet sofort. */
  setGeometryAspectRatio: (value: number | null) => void;
  /** Setzt die Rasterüberlagerung absolut und committet sofort. */
  setGeometryOverlay: (value: GridOverlay) => void;
  setGeometryAutoHorizon: (value: boolean) => void;
  /** Ob das Reparatur-Werkzeug (Klonen/Reparieren, Phase 4 Schritt 12)
   * gerade aktiv ist — ein erster Klick im Viewer setzt danach den
   * Quellpunkt, ein Ziehvorgang malt den Zielpfad (siehe
   * `components/RepairOverlay.tsx`). */
  repairActive: boolean;
  toggleRepairActive: () => void;
  /** Pinsel-Einstellungen für den *nächsten* Strich — bereits gemalte
   * Striche behalten ihre zum Malzeitpunkt gültigen Werte unverändert. */
  repairDraftMode: RepairMode;
  repairDraftRadius: number;
  repairDraftFeather: number;
  repairDraftOpacity: number;
  setRepairDraftMode: (mode: RepairMode) => void;
  setRepairDraftField: (key: "radius" | "feather" | "opacity", value: number) => void;
  /** Der nach dem ersten Klick gesetzte Quellpunkt eines neuen Strichs,
   * bis der Zielpfad fertig gemalt ist (`null` = als Nächstes wird der
   * Quellpunkt gesetzt). */
  repairPendingSource: RepairPoint | null;
  setRepairSourcePoint: (point: RepairPoint) => void;
  cancelRepairSource: () => void;
  /** Schließt den aktuellen Strich ab (Quellpunkt + gemalter, bereits
   * ausgedünnter Zielpfad + aktuelle Pinsel-Einstellungen) und committet
   * sofort — wie ein abgeschlossener Pinselzug in Lightroom. No-op ohne
   * gesetzten Quellpunkt oder leeren Pfad. */
  addRepairStroke: (targetPath: RepairPoint[]) => void;
  /** Entfernt einen Reparatur-Strich per Index und committet sofort. */
  removeRepairStroke: (index: number) => void;
  /** Schreibt `developEdl` als neuen Verlaufs-Schritt (siehe `PLAN.md`
   * Phase 2 Schritt 5/6: ausgelöst beim Loslassen eines Reglers, nicht
   * bei jedem Zwischenwert). */
  /** `preservePresetStrengthContext`: siehe `presetStrengthContext` unten
   * — nur `applyPreset`/`setPresetStrength` setzen dieses Flag, jeder
   * andere Aufrufer (der weit überwiegende Regelfall) lässt es weg und
   * löscht damit automatisch einen laufenden Stärke-Anpassungskontext,
   * sobald „ein anderer Edit dazwischen liegt" (`SPEC.md` §3.5). */
  commitDevelopEdit: (label?: string, options?: { preservePresetStrengthContext?: boolean }) => Promise<void>;
  undoDevelop: () => Promise<void>;
  redoDevelop: () => Promise<void>;
  /** Zuletzt gemessene Ende-zu-Ende-Antwortzeit der `develop/...`-Route
   * in Millisekunden (siehe `hooks/useDevelopRender`) — nur zur
   * Beobachtung/ehrlichen Dokumentation des 16-ms-Ziels (`PLAN.md` Phase
   * 2 Schritt 7), keine Business-Logik hängt daran. */
  developLastLatencyMs: number | null;
  setDevelopLatencyMs: (ms: number) => void;
}

// ---- Library-Slice (ab Phase 3: Raster, Bewertung/Flagge/Farbe,
// Sammlungen, Suche/Filter, Metadaten-Panel) --------------------------------

interface LibrarySlice {
  /** Was in der Mitte statt des Viewers gezeigt wird — `"grid"` ist das
   * neue Raster (Schritt 6), `"viewer"` der bisherige Einzelbild-Viewer,
   * `"map"` die Kartenansicht (Phase 8 Schritt 7). */
  centerView: "viewer" | "grid" | "map";
  toggleCenterView: () => void;
  setCenterView: (view: "viewer" | "grid" | "map") => void;

  /** Mehrfachauswahl fürs Stapel-Bearbeiten (Bewertung/Flagge/Sammlung-
   * Hinzufügen) — geteilt zwischen Raster und Filmstreifen. Enthält
   * `selectedPhotoId`, sobald eines gesetzt ist. */
  multiSelectedIds: string[];
  togglePhotoSelection: (photoId: string, mode: SelectionMode) => void;

  metadataPanelOpen: boolean;
  toggleMetadataPanel: () => void;

  photoKeywords: Record<string, KeywordDto[]>;
  loadKeywordsForPhoto: (photoId: string) => Promise<void>;
  addKeywordToPhoto: (photoId: string, name: string) => Promise<void>;
  removeKeywordFromPhoto: (photoId: string, keywordId: string) => Promise<void>;

  /** Setzt Bewertung/Flagge/Farbe. Ist `photoId` Teil einer Mehrfach-
   * auswahl mit mehr als einem Eintrag, wirkt die Änderung auf die
   * gesamte Auswahl (Stapel-Bearbeitung) — sonst nur auf `photoId`. */
  setPhotoRating: (photoId: string, rating: number) => Promise<void>;
  setPhotoFlag: (photoId: string, flag: number) => Promise<void>;
  setPhotoColorLabel: (photoId: string, colorLabel: string | null) => Promise<void>;

  collections: CollectionDto[];
  selectedCollectionId: string | null;
  collectionPhotos: Record<string, PhotoDto[]>;
  refreshCollections: () => Promise<void>;
  createCollection: (name: string) => Promise<void>;
  selectCollection: (collectionId: string | null) => void;
  loadPhotosForCollection: (collectionId: string) => Promise<void>;
  /** Fügt die aktuelle Mehrfachauswahl (oder, falls leer, das fokussierte
   * Foto) zu `collectionId` hinzu. */
  addSelectionToCollection: (collectionId: string) => Promise<void>;

  /** Freitextsuche (FTS5 über Dateiname/Kamera/Objektiv) und Attributfilter
   * sind kombinierbar (per UND, Schritt 8.4, `DECISIONS.md` ADR-0027) —
   * beide wirken gemeinsam über [`runLibrarySearchAndFilter`]. */
  libraryQuery: string;
  libraryFilter: FilterCriteriaDto;
  /** `null` = keine Suche/kein Filter aktiv, Raster/Filmstreifen zeigen
   * den ausgewählten Ordner/die Sammlung normal (siehe
   * [`selectActivePhotos`]). */
  libraryResults: PhotoDto[] | null;
  setLibraryQuery: (query: string) => void;
  /** Führt Suche und Attributfilter kombiniert aus (siehe
   * `crates/apx-catalog/src/repository/search.rs::search_and_filter_photos`)
   * — leeres Suchfeld und leerer Filter zusammen setzen `libraryResults`
   * wieder auf `null`. */
  runLibrarySearchAndFilter: () => Promise<void>;
  /** Setzt oder entfernt (bei `undefined`) einen Filter-Chip und wendet
   * das Ergebnis (kombiniert mit einer evtl. aktiven Suche) sofort an. */
  setLibraryFilterChip: (patch: FilterCriteriaDto) => Promise<void>;
  clearLibraryFilters: () => void;

  /** Duplikaterkennung per exaktem Hash (Schritt 8.2, `DECISIONS.md`
   * ADR-0027) — lädt alle Duplikatgruppen vom Backend, flacht sie ab und
   * zeigt sie wie ein Suchergebnis über `libraryResults` an. */
  showDuplicatePhotos: () => Promise<void>;

  /** Sortierung nach beliebigem Feld (Schritt 8.3, `DECISIONS.md`
   * ADR-0027) — angewendet als letzter Schritt in [`selectActivePhotos`]. */
  librarySortField: SortField;
  librarySortDirection: SortDirection;
  setLibrarySort: (field: SortField, direction: SortDirection) => void;

  /** Undo/Redo für Bibliotheks-Metadaten (Schritt 8.1, `DECISIONS.md`
   * ADR-0027) — reiner Frontend-Zustand, siehe `lib/undoStack.ts`. Deckt
   * bewusst *nicht* Sammlung anlegen/umbenennen/löschen ab. */
  libraryUndoStack: UndoEntry[];
  libraryRedoStack: UndoEntry[];
  undoLibraryAction: () => Promise<void>;
  redoLibraryAction: () => Promise<void>;
}

// ---- Presets-Slice (ab Phase 5, siehe DECISIONS.md ADR-0031) --------------

interface PresetsSlice {
  presetFolders: PresetFolderDto[];
  /** Metadaten aller Presets — ihre EDL-Teilmenge wird separat je nach
   * Bedarf geladen (`api.latestPresetVersion`/`api.listPresetVersions`),
   * nicht hier vorgehalten (kann pro Preset mehrere Versionen haben). */
  presets: PresetDto[];
  /** `null` = Wurzel-Ansicht (Presets ohne Ordner + alle Unterordner). */
  selectedPresetFolderId: string | null;
  refreshPresetFolders: () => Promise<void>;
  refreshPresets: () => Promise<void>;
  createPresetFolder: (name: string, parentId: string | null) => Promise<void>;
  renamePresetFolder: (folderId: string, name: string) => Promise<void>;
  deletePresetFolder: (folderId: string) => Promise<void>;
  selectPresetFolder: (folderId: string | null) => void;
  setPresetFavorite: (presetId: string, isFavorite: boolean) => Promise<void>;
  /** Benennt ein Preset um, ohne seine übrigen Metadaten (Ordner/Tags/
   * Bedingungen) zu verändern. */
  renamePreset: (presetId: string, name: string) => Promise<void>;
  movePresetToFolder: (presetId: string, folderId: string | null) => Promise<void>;
  deletePreset: (presetId: string) => Promise<void>;
  /** Öffnet den nativen Speichern-Dialog und schreibt das eigene `.apx`-
   * Format (`SPEC.md` §3.5: „Import/Export .apx") — no-op (kein Fehler),
   * wenn der Dialog abgebrochen wird. */
  exportPresetAsApxFile: (presetId: string) => Promise<void>;
  /** Öffnet den nativen Öffnen-Dialog, liest eine `.apx`-Datei und legt
   * daraus ein neues Preset in `folderId` an. */
  importPresetFromApxFile: (folderId: string | null) => Promise<void>;
  /** Legt ein neues Preset aus dem aktuellen `developEdl` an — nur die
   * ausgewählten Sektionen wandern in die EDL-Teilmenge (siehe
   * `lib/presets.ts`s `buildPresetEdlSubset`). No-op ohne Namen oder ohne
   * mindestens eine ausgewählte Sektion. */
  savePresetFromCurrentEdl: (
    name: string,
    folderId: string | null,
    tags: string[],
    sections: PresetSectionKey[],
    conditions?: PresetCondition[],
  ) => Promise<void>;

  /** Zustand für den nachträglich änderbaren Preset-Stärke-Regler
   * (`SPEC.md` §3.5: „auch nachträglich änderbar, solange kein anderer
   * Edit dazwischen liegt") — `baseEdl` ist der `developEdl`-Stand
   * *vor* dem Anwenden, `subset` die ungeskalierte EDL-Teilmenge des
   * Presets. `setPresetStrength` leitet den aktuellen `developEdl`-Stand
   * bei jeder Änderung neu aus `baseEdl` + skaliertem `subset` ab, statt
   * auf dem zuletzt angewendeten Zustand aufzubauen — nur so bleibt eine
   * Stärke-Änderung wiederholbar (z. B. erst 150 %, dann zurück auf
   * 80 %), statt sich mit jeder Reglerbewegung zu verselbständigen.
   * Wird von jedem *anderen* `commitDevelopEdit`-Aufruf automatisch
   * gelöscht (siehe dort). */
  presetStrengthContext: { presetId: string; presetName: string; baseEdl: EdlPayload; subset: PresetEdlSubset; strength: number } | null;
  /** Wendet die aktuellste Version von `presetId` bei 100 % Stärke auf
   * `developEdl` an und committet sofort (wie ein Dropdown-Wechsel — ein
   * Klick auf ein Preset ist eine abgeschlossene Aktion). */
  applyPreset: (presetId: string) => Promise<void>;
  /** Skaliert das zuletzt angewendete Preset auf `strengthPercent`
   * (0–200) neu — no-op ohne aktiven `presetStrengthContext`. */
  setPresetStrength: (strengthPercent: number) => void;
  /** Committet den zuletzt per `setPresetStrength` gesetzten Zwischenwert
   * dauerhaft (Loslassen des Reglers) — Zwischenwerte während des Ziehens
   * werden nicht einzeln committet, analog zu jedem `DevelopSlider`. */
  commitPresetStrength: () => void;
  dismissPresetStrengthContext: () => void;

  /** Preset-Stapel (`SPEC.md` §3.5: „mehrere Presets nacheinander
   * anwenden, Reihenfolge editierbar") — eine vom Nutzer zusammengestellte
   * Liste von Preset-IDs, angewendet in dieser Reihenfolge. Jedes Preset
   * im Stapel wirkt bei 100 % (keine Einzel-Stärke je Stapel-Eintrag,
   * siehe `DECISIONS.md` ADR-0031-Folgenotiz in `PLAN.md` Schritt 5). */
  presetStack: string[];
  addPresetToStack: (presetId: string) => void;
  removePresetFromStack: (index: number) => void;
  movePresetInStack: (index: number, direction: -1 | 1) => void;
  clearPresetStack: () => void;
  /** Wendet alle Presets im Stapel sequenziell auf `developEdl` an
   * (spätere Einträge überschreiben gemeinsame Sektionen früherer) und
   * committet einmal am Ende. */
  applyPresetStack: () => Promise<void>;

  /** Live-Vorschau beim Überfahren eines Preset-Eintrags mit der Maus
   * (`SPEC.md` §3.5) — rein visuell im Viewer, ändert `developEdl` nicht
   * und committet nichts. `Viewer.tsx` rendert `hoverPresetSubset`
   * zusammengeführt mit `developEdl`, sobald gesetzt. */
  hoverPresetSubset: PresetEdlSubset | null;
  previewPresetHover: (presetId: string) => Promise<void>;
  clearPresetHoverPreview: () => void;
}

// ---- Masken-Slice (ab Phase 6, siehe DECISIONS.md ADR-0032) ----------------

export type MaskKind = "LinearGradient" | "RadialGradient" | "Brush" | "ColorRange" | "LuminanceRange";

const MASK_KIND_DEFAULT_GEOMETRY: Record<MaskKind, () => MaskGeometry> = {
  LinearGradient: defaultLinearGradientGeometry,
  RadialGradient: defaultRadialGradientGeometry,
  Brush: emptyBrushGeometry,
  ColorRange: defaultColorRangeGeometry,
  LuminanceRange: defaultLuminanceRangeGeometry,
};

/** `MaskGeometry["kind"]` verwendet dieselben String-Literale wie
 * `MaskKind` — exportiert, damit `MasksPanel.tsx` bestehende
 * Komponenten-Geometrien beschriften kann, ohne die Zuordnung zu
 * duplizieren. */
export const MASK_KIND_LABEL: Record<MaskKind, string> = {
  LinearGradient: "Linearer Verlauf",
  RadialGradient: "Radialer Verlauf",
  Brush: "Pinsel",
  ColorRange: "Farbbereich",
  LuminanceRange: "Luminanzbereich",
};

/** Die fünf KI-Maskenarten in derselben Reihenfolge wie die Knöpfe im
 * „KI-Maske hinzufügen"-Abschnitt von `MasksPanel.tsx`. */
export const AI_MASK_KINDS: readonly AiMaskKind[] = ["Subject", "Sky", "Background", "ClickRegion", "Person"];

/** Rusts `generate_ai_mask`-Command erwartet die Maskenart als
 * `snake_case`-String (siehe `apx-app/src/commands.rs::parse_ai_mask_kind`),
 * das Frontend-Enum `AiMaskKind` ist dagegen `PascalCase` (spiegelt
 * `apx_pipeline::edl::AiMaskKind`, siehe `lib/edl.ts`). */
const AI_MASK_KIND_TO_BACKEND: Record<AiMaskKind, string> = {
  Subject: "subject",
  Sky: "sky",
  Background: "background",
  ClickRegion: "click_region",
  Person: "person",
};

/** Masken leben als Teil von `developEdl.masks` (siehe `lib/edl.ts`s
 * `Mask`) — dieser Slice ergänzt nur Auswahl-/Interaktionszustand plus
 * die Aktionen, die `developEdl.masks` verändern. Wie bei Reparatur
 * (Phase 4) committen diskrete Aktionen (Anlegen/Löschen/Sichtbarkeit)
 * sofort; Geometrie-Änderungen während des Ziehens im Viewer mutieren nur
 * `developEdl` (Live-Vorschau über `useDevelopRender`), `commitMaskDrag`
 * committet erst beim Loslassen — dieselbe onChange/onCommit-Trennung wie
 * bei jedem `DevelopSlider`. */
interface MasksSlice {
  selectedMaskId: string | null;
  /** Wählt eine Maske aus und setzt die aktive Komponente (siehe
   * `selectedMaskComponentIndex`) zurück auf die erste. */
  selectMask: (maskId: string | null) => void;
  /** Legt eine neue Maske mit einer einzelnen Startkomponente des
   * gewählten Geometrietyps an, wählt sie samt ihrer (einzigen)
   * Komponente aus und committet sofort. */
  addMask: (kind: MaskKind) => void;
  removeMask: (maskId: string) => void;
  setMaskVisible: (maskId: string, visible: boolean) => void;
  renameMask: (maskId: string, name: string) => void;
  setMaskBlendMode: (maskId: string, mode: BlendMode) => void;

  /** Index in `mask.components`, dessen Geometrie gerade im Viewer
   * bearbeitet wird bzw. den Pinsel-/Farbbereich-Klick-Werkzeuge
   * betreffen (Phase 6 Schritt 6: mehrere Komponenten je Maske,
   * `SPEC.md` §5 „Maskenkombination"). */
  selectedMaskComponentIndex: number;
  selectMaskComponent: (index: number) => void;
  /** Hängt eine neue Komponente mit Standardgeometrie des gewählten Typs
   * an, `combine: "Add"`, wählt sie als aktive Komponente aus und
   * committet sofort. */
  addMaskComponent: (maskId: string, kind: MaskKind) => void;
  /** No-op, wenn die Maske nur noch eine Komponente hätte (mindestens
   * eine Komponente ist Pflicht). Committet sofort. */
  removeMaskComponent: (maskId: string, componentIndex: number) => void;
  setMaskComponentCombine: (maskId: string, componentIndex: number, combine: MaskCombine) => void;
  setMaskComponentInvert: (maskId: string, componentIndex: number, invert: boolean) => void;
  /** Aktualisiert die Geometrie der *aktiven* Komponente
   * (`selectedMaskComponentIndex`) — nur Live-Zustand, kein Commit. */
  updateMaskGeometry: (maskId: string, geometry: MaskGeometry) => void;
  /** Committet den zuletzt per `updateMaskGeometry` gesetzten Zwischenwert
   * (Loslassen eines Ziehgriffs im Viewer). */
  commitMaskDrag: () => void;
  setMaskOpacity: (maskId: string, opacity: number) => void;
  setMaskFeather: (maskId: string, feather: number) => void;
  setMaskBasicField: (maskId: string, key: string, value: number) => void;

  /** Radius/Weichzeichnung für den *nächsten* gemalten Pinselstrich
   * (Phase 6 Schritt 4) — analog zu `repairDraftRadius`/`repairDraftFeather`:
   * kein EDL-Feld, sondern reiner Interaktionszustand, der beim Malen in
   * den neuen `BrushStroke` übernommen wird. */
  maskBrushDraftRadius: number;
  maskBrushDraftFeather: number;
  setMaskBrushDraftField: (key: "radius" | "feather", value: number) => void;
  /** Hängt einen fertig gemalten Strich (bereits ausgedünnter Zielpfad,
   * siehe `MaskOverlay.tsx`) an die *aktive* Komponente der Maske
   * (`selectedMaskComponentIndex`) an — ein No-op, falls deren Geometrie
   * kein `Brush` ist. Committet sofort. */
  addMaskBrushStroke: (maskId: string, points: MaskPoint[]) => void;
  removeMaskBrushStroke: (maskId: string, strokeIndex: number) => void;

  /** Bild-Klick-Werkzeug zum Aufnehmen der Zielfarbe einer Farbbereich-
   * Maske (Phase 6 Schritt 5) — teilt sich `Viewer.tsx`s Sampling-Code mit
   * der Weißabgleich-Pipette/dem Farbmischer (`wbPickerActive`/
   * `colorMixerPickerActive`), siehe dort. */
  maskColorRangePickerActive: boolean;
  toggleMaskColorRangePicker: () => void;
  /** `r`/`g`/`b` als Byte-Werte (`0..=255`) aus dem gerenderten Vorschau-
   * Frame — dieselbe Vereinfachung wie bei der WB-Pipette/dem
   * Farbmischer: `masks.rs`s `ColorRange` vergleicht eigentlich im
   * linearen Arbeitsraum, aber der Vorschau-Frame ist bereits
   * display-referred/gamma-kodiert. Für ein interaktives Klick-Werkzeug
   * reicht die Näherung (siehe `MasksPanel.tsx`s Moduldoku). */
  setMaskColorRangeTargetAt: (maskId: string, r: number, g: number, b: number) => void;

  // ---- Schritt 7: volle Sechs-Sektionen-Reglerabdeckung je Maske -----------
  // Dieselben Setter-Muster wie die globalen Pendants oben (siehe
  // `setHslBandField`/`setColorGradingWheel`/`setDetailsField` etc.), nur
  // auf `mask.adjustments.<sektion>` statt `developEdl.<sektion>`
  // gerichtet. Kontinuierliche Regler committen wie überall über
  // `commitMaskDrag` (kein eigener Commit hier), diskrete Aktionen
  // (Farbmischer-Region anlegen/entfernen, Deconvolution-Umschalter)
  // committen sofort.
  setMaskCurveChannel: (maskId: string, channel: keyof CurvesAdjustment, next: CurveChannel) => void;
  setMaskHslBandField: (maskId: string, band: keyof HslAdjustment, field: keyof HslBand, value: number) => void;
  maskColorMixerPickerActive: boolean;
  toggleMaskColorMixerPicker: () => void;
  addMaskColorMixerRegionAt: (maskId: string, r: number, g: number, b: number) => void;
  removeMaskColorMixerRegion: (maskId: string, regionIndex: number) => void;
  updateMaskColorMixerRegion: (maskId: string, regionIndex: number, patch: Partial<ColorMixerRegion>) => void;
  setMaskColorGradingWheel: (maskId: string, key: keyof Pick<ColorGradingAdjustment, "shadows" | "midtones" | "highlights" | "global">, wheel: ColorGradingWheel) => void;
  setMaskColorGradingBalance: (maskId: string, value: number) => void;
  setMaskColorGradingBlending: (maskId: string, value: number) => void;
  setMaskDetailsField: (maskId: string, key: keyof Omit<DetailsAdjustment, "use_deconvolution_sharpen">, value: number) => void;
  setMaskDetailsUseDeconvolutionSharpen: (maskId: string, value: boolean) => void;

  // ---- Schritt 7: Maskengruppen (`SPEC.md` §3.3) ---------------------------
  addMaskGroup: (name: string) => void;
  renameMaskGroup: (groupId: string, name: string) => void;
  /** Löst die Gruppenzuordnung aller Mitgliedsmasken (setzt ihr `group_id`
   * auf `null`), statt sie mitzulöschen — eine Gruppe ist rein
   * organisatorisch (siehe `MaskGroup`s Moduldoku). */
  removeMaskGroup: (groupId: string) => void;
  setMaskGroupVisible: (groupId: string, visible: boolean) => void;
  /** `groupId: null` löst die Zuordnung. */
  setMaskGroup: (maskId: string, groupId: string | null) => void;

  // ---- Schritt 7: Verwaltung (Duplizieren/Sortieren/Übertragen/Bausteine) -
  /** Tiefe Kopie mit neuer ID und „(Kopie)"-Namenssuffix, direkt hinter dem
   * Original eingefügt und ausgewählt. Committet sofort. */
  duplicateMask: (maskId: string) => void;
  /** Verschiebt eine Maske an eine neue Position in `developEdl.masks`
   * (Drag-&-Drop-Umsortierung im Panel) — die Reihenfolge ist zugleich die
   * Anwendungsreihenfolge (siehe `EdlV3::masks`-Moduldoku), Umsortieren
   * kann das Ergebnis also tatsächlich verändern, nicht nur die Anzeige.
   * Committet sofort. */
  reorderMask: (fromIndex: number, toIndex: number) => void;
  /** Kopiert eine Maske auf ein anderes, nicht notwendigerweise gerade
   * geöffnetes Foto — lädt dessen aktuellen Bearbeitungsstand, hängt die
   * Maske an, speichert ihn zurück. **Bewusste Vereinfachung:** nutzt
   * denselben `current_develop_edit`/`apply_develop_edit`-Pfad wie
   * `loadDevelopStateForPhoto` und erbt dessen Alt-Schema-Einschränkung
   * (ein Zielfoto, dessen letzter Bearbeitungsstand noch nicht auf das
   * aktuelle EDL-Schema angehoben wurde, bekäme sonst nur die neue Maske
   * ohne seine sonstigen Anpassungen — in der Praxis nur relevant für
   * Fotos, die seit einem EDL-Schema-Sprung nie neu bearbeitet wurden). */
  transferMaskToPhoto: (maskId: string, targetPhotoId: string) => Promise<void>;
  /** Wiederverwendbare Bausteine (`PLAN.md` Phase 6 Schritt 7): eine
   * Momentaufnahme aus Geometrie+Anpassungen einer Maske, benannt, um
   * später als Ausgangspunkt für eine neue Maske zu dienen — nicht die
   * Maske selbst (die lebt weiter unverändert in `developEdl.masks`).
   * **Bewusste Vereinfachung ggü. der Presets-Infrastruktur aus Phase 5:**
   * rein clientseitig im Zustand dieser Sitzung gehalten (kein
   * Backend-Katalog-Eintrag, keine Ordner/Versionen) — Bausteine
   * überleben also keinen App-Neustart. Ein katalogseitiges Pendant wäre
   * dieselbe Größenordnung an Aufwand wie das gesamte Presets-System aus
   * Phase 5 und würde diesen ohnehin schon großen Schritt sprengen; bei
   * echtem Bedarf ist das ein eigener späterer Schritt/eigene Phase. */
  maskBuildingBlocks: Array<{ id: string; name: string; mask: Mask }>;
  saveMaskAsBuildingBlock: (maskId: string, name: string) => void;
  /** Legt eine neue Maske aus dem Baustein an (neue ID, Bausteinname als
   * Startname), wählt sie aus, committet sofort. */
  applyMaskBuildingBlock: (blockId: string) => void;
  removeMaskBuildingBlock: (blockId: string) => void;
}

// ---- KI-Slice (Phase 7, siehe DECISIONS.md ADR-0033) -----------------------

/** Ein bereits geparster Vorschlag des Preset-Generators, siehe
 * `generatePresetFromDescription`/`generatePresetFromReferenceImage`/
 * `generatePresetVariationsFromBase`/`learnPresetFromSelectedPhotos`
 * unten — alle vier füllen dieselbe `presetGeneratorPreview`-Liste
 * (Variationen: mehrere Einträge gleichzeitig, die anderen drei: genau
 * einer), damit die Vorschau-UI nur einen einzigen Zustand kennen muss. */
interface AiSlice {
  // -- Die fünf KI-Masken (Schritt 2) --
  /** `true`, während auf einen Bildklick für die „Objekte"-KI-Maske
   * gewartet wird — nur `ClickRegion` braucht einen Klickpunkt, die
   * anderen vier Arten erzeugen sofort. Analog zu
   * `maskColorRangePickerActive`. */
  aiMaskClickPickerActive: boolean;
  toggleAiMaskClickPicker: () => void;
  /** Welche KI-Maskenart gerade per Tauri-Aufruf erzeugt wird (Anzeige
   * eines Ladezustands am jeweiligen Knopf) — `null`, wenn keine läuft. */
  aiMaskLoading: AiMaskKind | null;
  /** Erzeugt eine neue Maske mit `MaskGeometry::AiGenerated`-Geometrie für
   * `kind` und committet sofort. `click` ist nur für `"ClickRegion"`
   * Pflicht (siehe `aiMaskClickPickerActive`) — bei fehlendem Klick für
   * diese Art ist es ein No-op. */
  addAiMask: (kind: AiMaskKind, click?: { x: number; y: number }) => Promise<void>;

  // -- Reparatur-Erweiterungen (Schritt 3) --
  /** Solange aktiv, löst der erste Klick im `RepairOverlay` (der sonst
   * direkt den Quellpunkt setzt) stattdessen `suggestRepairSourceForTarget`
   * an dieser Position aus — die Klickposition gilt dann als ungefährer
   * Zielbereich, nicht als Quelle. */
  autoSourceModeActive: boolean;
  toggleAutoSourceMode: () => void;
  repairSourceSuggestionLoading: boolean;
  /** Fragt für den aktuellen Reparatur-Entwurfsradius einen Quellpunkt
   * für `(targetX, targetY)` vor (`apx_ai::repair_analysis::
   * suggest_source_point`) und setzt ihn als `repairPendingSource` — der
   * Nutzer malt danach wie gewohnt den Zielpfad. */
  suggestRepairSourceForTarget: (targetX: number, targetY: number) => Promise<void>;
  sensorSpotCandidates: SpotCandidateDto[];
  sensorSpotsLoading: boolean;
  /** Sucht Sensorflecken im aktuell ausgewählten Foto
   * (`apx_ai::repair_analysis::detect_spots`). */
  detectSensorSpotsForCurrentPhoto: (sensitivity: number) => Promise<void>;
  clearSensorSpots: () => void;
  /** Übernimmt einen erkannten Sensorfleck als neuen
   * `ContentAwareFill`-Reparaturstrich (ein einzelner Zielpunkt, Radius
   * aus dem erkannten Fleck übernommen) und committet sofort. */
  applySensorSpotAsRepairStroke: (spot: SpotCandidateDto) => void;

  // -- Preset-Generator (Schritt 4) --
  aiSettings: AiSettingsDto | null;
  loadAiSettings: () => Promise<void>;
  saveAnthropicApiKey: (apiKey: string) => Promise<void>;
  presetGeneratorLoading: boolean;
  presetGeneratorPreview: PresetEdlSubset[];
  /** Index innerhalb `presetGeneratorPreview`, der gerade in der
   * Live-Vorschau markiert ist (Variationen: der Nutzer wählt eine von
   * mehreren aus). */
  presetGeneratorSelectedIndex: number;
  generatePresetFromDescription: (description: string) => Promise<void>;
  /** **Manueller LLM-Modus ohne API-Schlüssel:** kopiert einen fertigen
   * Prompt-Text (System-Prompt + `description`) in die Zwischenablage,
   * zum Einfügen in die Claude-App (claude.ai). Kein Netzwerk-Aufruf. */
  copyPresetPromptForClaudeApp: (description: string) => Promise<void>;
  /** Validiert ein von Hand aus der Claude-App zurückkopiertes
   * JSON-Ergebnis serverseitig und übernimmt es als Vorschlag — dieselbe
   * Prüfung wie `generatePresetFromDescription`s Antwort, nur ohne den
   * API-Aufruf selbst. */
  importPresetFromPastedJson: (json: string) => Promise<void>;
  /** Öffnet einen Datei-Auswahldialog (Referenzbild) — kein LLM, kein
   * API-Schlüssel nötig. No-op (kein Fehler), wenn der Dialog abgebrochen
   * wird. */
  generatePresetFromReferenceImage: () => Promise<void>;
  generatePresetVariationsFromBase: (base: PresetEdlSubset, count: number, seed: number) => Promise<void>;
  /** Mittelt `sections` über den aktuell committeten Bearbeitungsstand
   * der genannten Fotos (`apx_ai::preset_generator::average_subsets`). */
  learnPresetFromSelectedPhotos: (photoIds: string[], sections: PresetSectionKey[]) => Promise<void>;
  selectPresetGeneratorPreview: (index: number) => void;
  /** Mischt `presetGeneratorPreview[presetGeneratorSelectedIndex]` in
   * `developEdl` (wie das Anwenden eines Presets, `mergeEdlSubset`) und
   * committet — der Nutzer kann das Ergebnis danach über den
   * bestehenden „Preset speichern"-Dialog (Phase 5) als echtes Preset
   * sichern, ohne dass der Generator eine eigene Speicher-Logik
   * bräuchte. */
  applyPresetGeneratorPreview: () => void;
  clearPresetGeneratorPreview: () => void;

  // -- Auto-Tagging (Schritt 5) --
  tagSuggestions: string[];
  tagSuggestionsLoading: boolean;
  fetchTagSuggestions: (photoId: string) => Promise<void>;
  /** Übernimmt einen Vorschlag als echtes Schlagwort
   * (`add_photo_keyword`, Phase 3) und entfernt ihn aus
   * `tagSuggestions`. */
  acceptTagSuggestion: (photoId: string, tag: string) => Promise<void>;
  clearTagSuggestions: () => void;
}

/** Export-Engine-Grundgerüst (Phase 8 Schritt 1, siehe `DECISIONS.md`
 * ADR-0034 und `apx_export::engine`s Moduldoku). Exportiert eine oder
 * mehrere Fotos mit ihrem jeweils *aktuellen* committeten Bearbeitungsstand
 * — dieselbe Quelle, die `Entwickeln` anzeigt. Reiht alle Fotos in die
 * Backend-Warteschlange ein (`apx_export::queue`, Schritt 2 — Fortschritt/
 * Pausieren/Priorisieren, siehe ADR-0034 Punkt 1) und pollt danach den
 * Fortschritt, statt selbst sequenziell zu warten — **vereinfacht**:
 * Abfragen alle 250ms statt eines Event-Push, kein Persistieren der
 * Warteschlange über App-Neustarts hinweg. */
interface ExportSlice {
  exportDialogOpen: boolean;
  openExportDialog: () => void;
  closeExportDialog: () => void;
  exportRunning: boolean;
  exportProgress: { done: number; total: number; failed: number } | null;
  exportError: string | null;
  exportQueuePaused: boolean;
  /** Reiht `photoIds` in die Export-Warteschlange ein und pollt den
   * Fortschritt bis alle abgeschlossen sind (egal ob erfolgreich oder
   * fehlgeschlagen). */
  exportPhotos: (photoIds: string[], destFolder: string, options: ExportPhotoOptions) => Promise<void>;
  toggleExportQueuePause: () => Promise<void>;
}

/** Drucken (Phase 8 Schritt 3) — wiederverwendet die Export-Engine
 * komplett (`apx_export::print`), rendert also serverseitig; das
 * Frontend wählt nur Layout/Seitengröße und den Zieldateipfad. */
interface PrintSlice {
  printDialogOpen: boolean;
  openPrintDialog: () => void;
  closePrintDialog: () => void;
  printRunning: boolean;
  printError: string | null;
  printLastOutcome: ExportOutcomeDto | null;
  printPhotos: (photoIds: string[], destPath: string, options: PrintLayoutOptions) => Promise<void>;
}

/** Diashow (Phase 8 Schritt 4) — Übergänge/Ken-Burns-Effekt/Intro-Outro-
 * Screens/Musik-Synchronisation laufen für die Live-Wiedergabe komplett im
 * Frontend (`lib/slideshow.ts`, `SlideshowPlayer.tsx`), diese Slice deckt
 * nur den Dialog-Zustand und den optionalen Video-Export ab (siehe
 * `apx_export::video`). */
interface SlideshowSlice {
  slideshowDialogOpen: boolean;
  openSlideshowDialog: () => void;
  closeSlideshowDialog: () => void;
  /** `null` = noch nicht geprüft. */
  ffmpegAvailable: boolean | null;
  checkFfmpegAvailability: () => Promise<void>;
  videoExportRunning: boolean;
  videoExportError: string | null;
  videoExportOutcome: SlideshowVideoOutcomeDto | null;
  exportSlideshowVideo: (photoIds: string[], destPath: string, options: SlideshowVideoOptions) => Promise<void>;
}

/** Buch (Phase 8 Schritt 5) — wiederverwendet die Export-Engine +
 * `apx_export::print` komplett (siehe `apx_export::book`s Moduldoku);
 * das Frontend wählt nur Seitenvorlage/-größe und den Zieldateipfad. */
interface BookSlice {
  bookDialogOpen: boolean;
  openBookDialog: () => void;
  closeBookDialog: () => void;
  bookExportRunning: boolean;
  bookExportError: string | null;
  bookExportOutcome: BookOutcomeDto | null;
  exportBookPdf: (photoIds: string[], destPath: string, options: BookOptions) => Promise<void>;
}

/** Web-Galerie (Phase 8 Schritt 6) — rendert eine statische HTML-Galerie
 * (`apx_export::web`) und lädt sie optional per FTP/SFTP hoch. */
interface WebSlice {
  webDialogOpen: boolean;
  openWebDialog: () => void;
  closeWebDialog: () => void;
  webExportRunning: boolean;
  webExportError: string | null;
  webExportOutcome: WebGalleryOutcomeDto | null;
  exportWebGallery: (photoIds: string[], destDir: string, options: WebGalleryOptions) => Promise<void>;
}

/** Karte (Phase 8 Schritt 7) — GPS-Koordinaten selbst kommen aus den
 * normalen Foto-Listen (`PhotoDto.gps_lat`/`gps_lon`, EXIF beim Import
 * gelesen); dieser Slice hält nur, was die Kartenansicht selbst zusätzlich
 * braucht: die geotaggten Fotos, einen optional geladenen GPX-Track und
 * den "Standort setzen"-Modus fürs Foto-ohne-GPS-Platzieren per Klick. */
interface MapSlice {
  geotaggedPhotos: PhotoDto[];
  refreshGeotaggedPhotos: () => Promise<void>;
  gpxTrack: GpxTrackPointDto[] | null;
  loadGpxTrack: (path: string) => Promise<void>;
  clearGpxTrack: () => void;
  /** Fotos-ID, für die der nächste Karten-Klick den GPS-Standort setzt —
   * `null`, wenn der Platzieren-Modus aus ist. */
  placingGpsForPhotoId: string | null;
  startPlacingGps: (photoId: string) => void;
  cancelPlacingGps: () => void;
  setPhotoGpsFromMapClick: (lat: number, lon: number) => Promise<void>;
}

/** Vorlagen (Phase 8 Schritt 8) — eine generische Backend-Tabelle deckt
 * Export-/Layout-Vorlagen für alle Ausgabemodule ab (siehe
 * `apx_catalog::Template`s Moduldoku); Workflow-Vorlagen laufen bewusst
 * hier im Frontend (`runWorkflowTemplate`), weil das EDL-Vorlagen-Mischen
 * (`mergeEdlSubset`) bislang nur hier existiert — kein zweiter,
 * serverseitiger Merge-Codepfad. */
interface TemplatesSlice {
  templatesByKind: Partial<Record<TemplateKind, TemplateDto[]>>;
  refreshTemplates: (kind: TemplateKind) => Promise<void>;
  saveTemplateAction: (kind: TemplateKind, name: string, payload: unknown) => Promise<void>;
  deleteTemplateAction: (kind: TemplateKind, templateId: string) => Promise<void>;
  importTemplateFile: () => Promise<void>;
  /** Läuft die ausgewählten Fotos einmal durch: Preset-EDL-Teilmenge der
   * Vorlage auf den aktuellen Bearbeitungsstand mischen, committen, dann
   * mit den Vorlagen-Exportoptionen nach `destFolder` exportieren — das
   * „Import → Filter → Preset → Export als ein Klick" aus `PLAN.md`
   * Schritt 8, **bewusst ohne den Filter-Schritt** (läuft auf der
   * jeweils schon getroffenen Fotoauswahl, wie alle übrigen Phase-8-
   * Exportdialoge) und ohne Import (setzt bereits importierte Fotos
   * voraus). */
  workflowRunning: boolean;
  workflowProgress: { done: number; total: number; failed: number } | null;
  runWorkflowTemplate: (photoIds: string[], template: WorkflowTemplatePayload, destFolder: string) => Promise<void>;
}

/** Bibliotheks-Backlog (Phase 9 Schritt 1, siehe DECISIONS.md ADR-0032/
 * ADR-0035): Sammlungssätze, Stapel, virtuelle Kopien, erweiterbare
 * Farbmarkierungen, Perceptual-Hash-Duplikat-Assistent. */
interface LibraryBacklogSlice {
  collectionFolders: CollectionFolderDto[];
  refreshCollectionFolders: () => Promise<void>;
  createCollectionFolder: (name: string, parentId?: string) => Promise<void>;
  renameCollectionFolder: (folderId: string, name: string) => Promise<void>;
  deleteCollectionFolder: (folderId: string) => Promise<void>;
  createSmartCollection: (name: string, folderId: string | undefined, criteria: FilterCriteriaDto) => Promise<void>;
  moveCollectionToFolder: (collectionId: string, folderId: string | null) => Promise<void>;

  stacks: StackDto[];
  refreshStacks: () => Promise<void>;
  createStackFromSelection: (name?: string) => Promise<void>;
  deleteStack: (stackId: string) => Promise<void>;
  setStackCover: (stackId: string, coverPhotoId: string) => Promise<void>;
  autoStackSelectionByTime: (windowSeconds: number) => Promise<void>;

  virtualCopiesByPhotoId: Record<string, PhotoDto[]>;
  createVirtualCopyForSelected: () => Promise<void>;
  refreshVirtualCopies: (photoId: string) => Promise<void>;

  colorLabelDefinitions: ColorLabelDefinitionDto[];
  refreshColorLabelDefinitions: () => Promise<void>;
  createColorLabelDefinition: (name: string, displayName: string, hex: string) => Promise<void>;
  deleteColorLabelDefinition: (name: string) => Promise<void>;

  perceptualDuplicateGroups: PhotoDto[][];
  perceptualDuplicatesRunning: boolean;
  runPerceptualDuplicateDetection: (maxDistance: number) => Promise<void>;
}

export type AppStore = CatalogSlice &
  SelectionSlice &
  ViewerSlice &
  JobsSlice &
  DevelopSlice &
  LibrarySlice &
  PresetsSlice &
  MasksSlice &
  AiSlice &
  ExportSlice &
  PrintSlice &
  SlideshowSlice &
  BookSlice &
  WebSlice &
  MapSlice &
  TemplatesSlice &
  LibraryBacklogSlice;

export const useAppStore = create<AppStore>()(
  immer((set, get) => {
    /** Pusht einen Undo-Eintrag auf den Bibliotheks-Undo-Stack und leert
     * den Redo-Stack (siehe `lib/undoStack.ts`) — gemeinsam genutzt von
     * allen Bibliotheks-Metadaten-Aktionen (Bewertung/Flagge/Farbe/
     * Schlagworte/Sammlungsmitgliedschaft, Schritt 8.1, `DECISIONS.md`
     * ADR-0027). */
    function pushLibraryUndo(entry: UndoEntry) {
      set((state) => {
        const stacks = undoStackLib.pushUndo({ undoStack: state.libraryUndoStack, redoStack: state.libraryRedoStack }, entry);
        state.libraryUndoStack = stacks.undoStack;
        state.libraryRedoStack = stacks.redoStack;
      });
    }

    return {
    // Catalog
    folders: [],
    photosByFolder: {},
    catalogStatus: null,
    catalogError: null,

    refreshFolders: async () => {
      try {
        const folders = await api.listFolders();
        set((state) => {
          state.folders = folders;
          state.catalogError = null;
        });
      } catch (err) {
        set((state) => {
          state.catalogError = String(err);
        });
      }
    },

    refreshCatalogStatus: async () => {
      try {
        const status = await api.getCatalogStatus();
        set((state) => {
          state.catalogStatus = status;
          state.catalogError = null;
        });
      } catch (err) {
        set((state) => {
          state.catalogError = String(err);
        });
      }
    },

    loadPhotosForFolder: async (folderId) => {
      try {
        const photos = await api.listPhotosInFolder(folderId);
        set((state) => {
          state.photosByFolder[folderId] = photos;
        });
      } catch (err) {
        set((state) => {
          state.catalogError = String(err);
        });
      }
    },

    relinkFolder: async (folderId, newPath) => {
      try {
        await api.relinkFolder(folderId, newPath);
        await get().refreshFolders();
        if (get().photosByFolder[folderId]) {
          await get().loadPhotosForFolder(folderId);
        }
      } catch (err) {
        set((state) => {
          state.catalogError = String(err);
        });
      }
    },

    // Selection
    selectedFolderId: null,
    selectedPhotoId: null,

    selectFolder: (folderId) => {
      set((state) => {
        state.selectedFolderId = folderId;
        state.selectedCollectionId = null;
        state.selectedPhotoId = null;
        state.multiSelectedIds = [];
        state.libraryQuery = "";
        state.libraryResults = null;
      });
      if (folderId) {
        void get().loadPhotosForFolder(folderId);
      }
    },

    selectPhoto: (photoId) => {
      set((state) => {
        state.selectedPhotoId = photoId;
        state.multiSelectedIds = photoId ? [photoId] : [];
      });
      get().resetView();
      // Läuft das Entwickeln-Panel bereits, muss es beim Fotowechsel den
      // Bearbeitungszustand des *neuen* Fotos laden statt den alten kurz
      // weiter anzuzeigen.
      if (get().developPanelOpen) {
        if (photoId) {
          void get().loadDevelopStateForPhoto(photoId);
        } else {
          set((state) => {
            state.developEdl = neutralEdlPayload();
            state.developPhotoId = null;
          });
        }
      }
      if (get().metadataPanelOpen && photoId) {
        void get().loadKeywordsForPhoto(photoId);
      }
    },

    stepSelection: (direction) => {
      const state = get();
      const photos = selectActivePhotos(state);
      if (photos.length === 0) return;

      const currentIndex = photos.findIndex((p) => p.id === state.selectedPhotoId);
      const nextIndex = currentIndex === -1 ? 0 : (currentIndex + direction + photos.length) % photos.length;
      const next = photos[nextIndex];
      if (next) {
        get().selectPhoto(next.id);
      }
    },

    // Viewer
    zoom: 1,
    fitMode: "fit",
    panX: 0,
    panY: 0,

    setZoom: (zoom, fitMode = "manual") => {
      set((state) => {
        state.zoom = zoom;
        state.fitMode = fitMode;
      });
    },

    setPan: (x, y) => {
      set((state) => {
        state.panX = x;
        state.panY = y;
      });
    },

    resetView: () => {
      set((state) => {
        state.zoom = 1;
        state.fitMode = "fit";
        state.panX = 0;
        state.panY = 0;
      });
    },

    // Jobs (Import)
    importRunning: false,
    importProgress: null,
    importResult: null,
    importErrors: [],

    startImport: async (path) => {
      set((state) => {
        state.importRunning = true;
        state.importProgress = { done: 0, total: 0, currentFile: null };
        state.importResult = null;
        state.importErrors = [];
      });
      try {
        await api.importFolder(path);
      } catch (err) {
        set((state) => {
          state.importRunning = false;
          state.importErrors.push(String(err));
        });
      }
    },

    startImportWithMode: async (path, mode, renamePattern) => {
      set((state) => {
        state.importRunning = true;
        state.importProgress = { done: 0, total: 0, currentFile: null };
        state.importResult = null;
        state.importErrors = [];
      });
      try {
        await api.importFolderWithMode(path, mode, renamePattern);
      } catch (err) {
        set((state) => {
          state.importRunning = false;
          state.importErrors.push(String(err));
        });
      }
    },

    cancelImport: async () => {
      await api.cancelImport();
    },

    importPresets: [],

    refreshImportPresets: async () => {
      try {
        const presets = await api.listImportPresets();
        set((state) => {
          state.importPresets = presets;
        });
      } catch (err) {
        set((state) => {
          state.catalogError = String(err);
        });
      }
    },

    saveImportPresetEntry: async (preset) => {
      try {
        const presets = await api.saveImportPreset(preset);
        set((state) => {
          state.importPresets = presets;
        });
      } catch (err) {
        set((state) => {
          state.catalogError = String(err);
        });
      }
    },

    deleteImportPresetEntry: async (name) => {
      try {
        const presets = await api.deleteImportPreset(name);
        set((state) => {
          state.importPresets = presets;
        });
      } catch (err) {
        set((state) => {
          state.catalogError = String(err);
        });
      }
    },

    setImportProgress: (progress) => {
      set((state) => {
        state.importProgress = progress;
      });
    },

    addImportError: (line) => {
      set((state) => {
        state.importErrors.push(line);
      });
    },

    finishImport: (result) => {
      set((state) => {
        state.importRunning = false;
        state.importResult = result;
        state.importProgress = null;
      });
      void get().refreshFolders();
      void get().refreshCatalogStatus();
      const { selectedFolderId } = get();
      if (selectedFolderId) {
        void get().loadPhotosForFolder(selectedFolderId);
      }
    },

    // Develop
    developPanelOpen: false,
    developEdl: neutralEdlPayload(),
    developPhotoId: null,

    toggleDevelopPanel: () => {
      const willOpen = !get().developPanelOpen;
      set((state) => {
        state.developPanelOpen = willOpen;
      });
      const { selectedPhotoId } = get();
      if (willOpen && selectedPhotoId) {
        void get().loadDevelopStateForPhoto(selectedPhotoId);
      }
    },

    loadDevelopStateForPhoto: async (photoId) => {
      const previousPhotoId = get().developPhotoId;
      try {
        const position = await api.currentDevelopEdit(photoId);
        set((state) => {
          state.developEdl = edlFromHistoryPosition(position);
          state.developPhotoId = photoId;
          if (previousPhotoId && previousPhotoId !== photoId) state.lastDevelopPhotoId = previousPhotoId;
        });
      } catch (err) {
        console.error("Bearbeitungszustand konnte nicht geladen werden:", err);
        set((state) => {
          state.developEdl = neutralEdlPayload();
          state.developPhotoId = photoId;
          if (previousPhotoId && previousPhotoId !== photoId) state.lastDevelopPhotoId = previousPhotoId;
        });
      }
      void get().refreshSnapshots();
    },

    copiedEdlSubset: null,

    copyDevelopSettings: (sections) => {
      set((state) => {
        state.copiedEdlSubset = buildPresetEdlSubset(state.developEdl, sections);
      });
    },

    pasteDevelopSettings: () => {
      const subset = get().copiedEdlSubset;
      if (!subset) return;
      set((state) => {
        state.developEdl = mergeEdlSubset(state.developEdl, subset);
      });
      void get().commitDevelopEdit("Einstellungen eingefügt");
    },

    lastDevelopPhotoId: null,

    applyPreviousSettings: async () => {
      const { developPhotoId, lastDevelopPhotoId } = get();
      if (!developPhotoId || !lastDevelopPhotoId) return;
      try {
        const position = await api.currentDevelopEdit(lastDevelopPhotoId);
        const payload = edlFromHistoryPosition(position);
        await api.applyDevelopEdit(developPhotoId, buildEdlEnvelopeJson(payload), "Vorherige Einstellungen übernommen");
        set((state) => {
          state.developEdl = payload;
        });
      } catch (err) {
        console.error("Vorherige Einstellungen konnten nicht übernommen werden:", err);
      }
    },

    syncSettingsToSelection: async (sections) => {
      const { developPhotoId, multiSelectedIds, developEdl } = get();
      if (!developPhotoId || sections.length === 0) return;
      const targets = multiSelectedIds.includes(developPhotoId) ? multiSelectedIds.filter((id) => id !== developPhotoId) : [];
      if (targets.length === 0) return;
      const subset = buildPresetEdlSubset(developEdl, sections);
      for (const targetId of targets) {
        try {
          const position = await api.currentDevelopEdit(targetId);
          const targetPayload = mergeEdlSubset(edlFromHistoryPosition(position), subset);
          await api.applyDevelopEdit(targetId, buildEdlEnvelopeJson(targetPayload), "Synchronisiert");
          if (get().developPhotoId === targetId) {
            set((state) => {
              state.developEdl = targetPayload;
            });
          }
        } catch (err) {
          console.error(`Synchronisieren mit Foto ${targetId} fehlgeschlagen:`, err);
        }
      }
    },

    autoSyncActive: false,

    toggleAutoSync: () => {
      set((state) => {
        state.autoSyncActive = !state.autoSyncActive;
      });
    },

    snapshots: [],

    refreshSnapshots: async () => {
      const photoId = get().developPhotoId;
      if (!photoId) {
        set((state) => {
          state.snapshots = [];
        });
        return;
      }
      try {
        const snapshots = await api.listSnapshots(photoId);
        set((state) => {
          state.snapshots = snapshots;
        });
      } catch (err) {
        console.error("Schnappschüsse konnten nicht geladen werden:", err);
      }
    },

    saveSnapshot: async (name) => {
      const trimmed = name.trim();
      const photoId = get().developPhotoId;
      if (!trimmed || !photoId) return;
      try {
        await api.createSnapshot(photoId, trimmed, buildEdlEnvelopeJson(get().developEdl));
        await get().refreshSnapshots();
      } catch (err) {
        console.error("Schnappschuss konnte nicht angelegt werden:", err);
      }
    },

    renameSnapshotAction: async (snapshotId, name) => {
      const trimmed = name.trim();
      if (!trimmed) return;
      try {
        await api.renameSnapshot(snapshotId, trimmed);
        await get().refreshSnapshots();
      } catch (err) {
        console.error("Schnappschuss konnte nicht umbenannt werden:", err);
      }
    },

    removeSnapshot: async (snapshotId) => {
      try {
        await api.deleteSnapshot(snapshotId);
        await get().refreshSnapshots();
      } catch (err) {
        console.error("Schnappschuss konnte nicht gelöscht werden:", err);
      }
    },

    restoreSnapshot: async (snapshotId) => {
      const photoId = get().developPhotoId;
      const snapshot = get().snapshots.find((s) => s.id === snapshotId);
      if (!photoId || !snapshot) return;
      const payload = parseEdlEnvelopeJson(snapshot.edl_json);
      if (!payload) {
        console.error("Schnappschuss enthält ein unlesbares EDL:", snapshot.id);
        return;
      }
      try {
        await api.applyDevelopEdit(photoId, snapshot.edl_json, `Schnappschuss „${snapshot.name}" wiederhergestellt`);
        set((state) => {
          state.developEdl = payload;
        });
      } catch (err) {
        console.error("Schnappschuss konnte nicht wiederhergestellt werden:", err);
      }
    },

    beforeAfterMode: "none",

    setBeforeAfterMode: (mode) => {
      set((state) => {
        state.beforeAfterMode = mode;
      });
    },

    referenceViewActive: false,
    referencePhotoId: null,

    toggleReferenceView: () => {
      set((state) => {
        state.referenceViewActive = !state.referenceViewActive;
      });
    },

    setReferencePhotoId: (photoId) => {
      set((state) => {
        state.referencePhotoId = photoId;
      });
    },

    softProofActive: false,
    softProofProfile: "srgb",
    softProofIntent: "perceptual",
    softProofGamutWarning: false,
    softProofPaperWhite: false,

    toggleSoftProof: () => {
      set((state) => {
        state.softProofActive = !state.softProofActive;
      });
    },

    setSoftProofProfile: (profile) => {
      set((state) => {
        state.softProofProfile = profile;
      });
    },

    setSoftProofIntent: (intent) => {
      set((state) => {
        state.softProofIntent = intent;
      });
    },

    toggleSoftProofGamutWarning: () => {
      set((state) => {
        state.softProofGamutWarning = !state.softProofGamutWarning;
      });
    },

    toggleSoftProofPaperWhite: () => {
      set((state) => {
        state.softProofPaperWhite = !state.softProofPaperWhite;
      });
    },

    setBasicField: (key, value) => {
      set((state) => {
        writeBasicField(state.developEdl.basic, key, value);
      });
    },

    wbPickerActive: false,

    toggleWbPicker: () => {
      set((state) => {
        state.wbPickerActive = !state.wbPickerActive;
      });
    },

    pickWhiteBalanceAt: (r, g, b) => {
      set((state) => {
        state.developEdl.basic.white_balance = computeWhiteBalanceShiftFromSample(r, g, b, state.developEdl.basic.white_balance);
        state.wbPickerActive = false;
      });
      void get().commitDevelopEdit();
    },

    applyWhiteBalancePreset: (key) => {
      const preset = WHITE_BALANCE_PRESETS.find((p) => p.key === key);
      if (!preset) return;
      set((state) => {
        state.developEdl.basic.white_balance = { temp_shift_kelvin: preset.temp_shift_kelvin, tint_shift: preset.tint_shift };
      });
      void get().commitDevelopEdit();
    },

    setCurveChannel: (key, channel) => {
      set((state) => {
        state.developEdl.curves[key] = channel;
      });
    },

    setHslBandField: (band, field, value) => {
      set((state) => {
        state.developEdl.hsl[band][field] = value;
      });
    },

    setColorGradingWheel: (key, wheel) => {
      set((state) => {
        state.developEdl.color_grading[key] = wheel;
      });
    },

    setColorGradingBalance: (value) => {
      set((state) => {
        state.developEdl.color_grading.balance = value;
      });
    },

    setColorGradingBlending: (value) => {
      set((state) => {
        state.developEdl.color_grading.blending = value;
      });
    },

    setCalibrationPrimaryField: (primary, field, value) => {
      set((state) => {
        state.developEdl.calibration[primary][field] = value;
      });
    },

    setCalibrationShadowTint: (value) => {
      set((state) => {
        state.developEdl.calibration.shadow_tint = value;
      });
    },

    setCalibrationCameraProfile: (value) => {
      set((state) => {
        state.developEdl.calibration.camera_profile = value;
      });
      void get().commitDevelopEdit();
    },

    setDetailsField: (key, value) => {
      set((state) => {
        state.developEdl.details[key] = value;
      });
    },

    setDetailsUseDeconvolutionSharpen: (value) => {
      set((state) => {
        state.developEdl.details.use_deconvolution_sharpen = value;
      });
      void get().commitDevelopEdit();
    },

    setLensCorrectionField: (key, value) => {
      set((state) => {
        state.developEdl.lens_corrections[key] = value;
      });
    },

    setLensCorrectionManualTransformField: (key, value) => {
      set((state) => {
        state.developEdl.lens_corrections.manual_transform[key] = value;
      });
    },

    setLensCorrectionProfile: (value) => {
      set((state) => {
        state.developEdl.lens_corrections.profile_id = value;
      });
      void get().commitDevelopEdit();
    },

    setLensCorrectionAutoCa: (value) => {
      set((state) => {
        state.developEdl.lens_corrections.auto_ca = value;
      });
      void get().commitDevelopEdit();
    },

    setLensCorrectionUprightMode: (value) => {
      set((state) => {
        state.developEdl.lens_corrections.upright_mode = value;
      });
      void get().commitDevelopEdit();
    },

    setLensCorrectionGuidedLineField: (lineIndex, field, value) => {
      set((state) => {
        const lines = state.developEdl.lens_corrections.guided_lines;
        while (lines.length <= lineIndex) {
          lines.push({ x1: 0, y1: 0, x2: 0, y2: 0 });
        }
        const line = lines[lineIndex];
        if (line) line[field] = value;
      });
    },

    setEffectsField: (key, value) => {
      set((state) => {
        state.developEdl.effects[key] = value;
      });
    },

    geometryCropActive: false,

    toggleGeometryCropActive: () => {
      set((state) => {
        state.geometryCropActive = !state.geometryCropActive;
      });
    },

    setGeometryCrop: (crop) => {
      set((state) => {
        state.developEdl.geometry.crop = crop;
      });
    },

    setGeometryAngle: (value) => {
      set((state) => {
        state.developEdl.geometry.angle_degrees = value;
      });
    },

    setGeometryAspectRatio: (value) => {
      set((state) => {
        state.developEdl.geometry.aspect_ratio = value;
      });
      void get().commitDevelopEdit();
    },

    setGeometryOverlay: (value) => {
      set((state) => {
        state.developEdl.geometry.overlay = value;
      });
      void get().commitDevelopEdit();
    },

    setGeometryAutoHorizon: (value) => {
      set((state) => {
        state.developEdl.geometry.auto_horizon = value;
      });
      void get().commitDevelopEdit();
    },

    repairActive: false,

    toggleRepairActive: () => {
      set((state) => {
        state.repairActive = !state.repairActive;
        state.repairPendingSource = null;
      });
    },

    repairDraftMode: "Clone",
    repairDraftRadius: 0.05,
    repairDraftFeather: 0.02,
    repairDraftOpacity: 1,

    setRepairDraftMode: (mode) => {
      set((state) => {
        state.repairDraftMode = mode;
      });
    },

    setRepairDraftField: (key, value) => {
      set((state) => {
        if (key === "radius") state.repairDraftRadius = value;
        else if (key === "feather") state.repairDraftFeather = value;
        else state.repairDraftOpacity = value;
      });
    },

    repairPendingSource: null,

    setRepairSourcePoint: (point) => {
      set((state) => {
        state.repairPendingSource = point;
      });
    },

    cancelRepairSource: () => {
      set((state) => {
        state.repairPendingSource = null;
      });
    },

    addRepairStroke: (targetPath) => {
      const { repairPendingSource, repairDraftMode, repairDraftRadius, repairDraftFeather, repairDraftOpacity } = get();
      // Inhaltsbasiertes Füllen (Phase 7) sucht seinen Füllinhalt selbst
      // aus der Bildumgebung — anders als Klonen/Reparieren braucht es
      // keinen vom Nutzer gesetzten Quellpunkt (`source` wird von
      // `apx-pipeline` für diesen Modus ignoriert, siehe ADR-0033 Punkt 4).
      const isContentAwareFill = repairDraftMode === "ContentAwareFill";
      if ((!repairPendingSource && !isContentAwareFill) || targetPath.length === 0) return;
      set((state) => {
        state.developEdl.repair.push({
          mode: repairDraftMode,
          source: repairPendingSource ?? { x: 0, y: 0 },
          target_path: targetPath,
          radius: repairDraftRadius,
          feather: repairDraftFeather,
          opacity: repairDraftOpacity,
        });
        state.repairPendingSource = null;
      });
      void get().commitDevelopEdit();
    },

    removeRepairStroke: (index) => {
      set((state) => {
        state.developEdl.repair.splice(index, 1);
      });
      void get().commitDevelopEdit();
    },

    colorMixerPickerActive: false,

    toggleColorMixerPicker: () => {
      set((state) => {
        state.colorMixerPickerActive = !state.colorMixerPickerActive;
      });
    },

    addColorMixerRegionAt: (r, g, b) => {
      if (get().developEdl.color_mixer.regions.length >= MAX_COLOR_MIXER_REGIONS) {
        set((state) => {
          state.colorMixerPickerActive = false;
        });
        return;
      }
      const hue = hueDegreesFromRgbByte(r, g, b);
      set((state) => {
        state.developEdl.color_mixer.regions.push(newColorMixerRegion(hue));
        state.colorMixerPickerActive = false;
      });
      void get().commitDevelopEdit();
    },

    removeColorMixerRegion: (index) => {
      set((state) => {
        state.developEdl.color_mixer.regions.splice(index, 1);
      });
      void get().commitDevelopEdit();
    },

    updateColorMixerRegion: (index, patch) => {
      set((state) => {
        const region = state.developEdl.color_mixer.regions[index];
        if (region) Object.assign(region, patch);
      });
    },

    commitDevelopEdit: async (label, options) => {
      const { developPhotoId, developEdl } = get();
      if (!developPhotoId) return;
      if (!options?.preservePresetStrengthContext) {
        set((state) => {
          state.presetStrengthContext = null;
        });
      }
      try {
        await api.applyDevelopEdit(developPhotoId, buildEdlEnvelopeJson(developEdl), label);
      } catch (err) {
        console.error("Bearbeitung konnte nicht gespeichert werden:", err);
      }
      if (get().autoSyncActive) {
        void get().syncSettingsToSelection([...PRESET_SECTION_KEYS]);
      }
    },

    undoDevelop: async () => {
      const { developPhotoId } = get();
      if (!developPhotoId) return;
      const position = await api.undoDevelopEdit(developPhotoId).catch((err: unknown) => {
        console.error("Rückgängig fehlgeschlagen:", err);
        return null;
      });
      if (!position) return;
      set((state) => {
        state.developEdl = edlFromHistoryPosition(position);
      });
    },

    redoDevelop: async () => {
      const { developPhotoId } = get();
      if (!developPhotoId) return;
      const position = await api.redoDevelopEdit(developPhotoId).catch((err: unknown) => {
        console.error("Wiederholen fehlgeschlagen:", err);
        return null;
      });
      if (!position) return;
      set((state) => {
        state.developEdl = edlFromHistoryPosition(position);
      });
    },

    developLastLatencyMs: null,

    setDevelopLatencyMs: (ms) => {
      set((state) => {
        state.developLastLatencyMs = ms;
      });
    },

    // Library (ab Phase 3)
    centerView: "viewer",

    toggleCenterView: () => {
      set((state) => {
        state.centerView = state.centerView === "viewer" ? "grid" : "viewer";
      });
    },

    setCenterView: (view) => {
      set((state) => {
        state.centerView = view;
      });
    },

    multiSelectedIds: [],

    togglePhotoSelection: (photoId, mode) => {
      if (mode === "toggle") {
        const wasSelected = get().multiSelectedIds.includes(photoId);
        set((state) => {
          state.multiSelectedIds = wasSelected
            ? state.multiSelectedIds.filter((id) => id !== photoId)
            : [...state.multiSelectedIds, photoId];
          state.selectedPhotoId = photoId;
        });
        get().resetView();
        if (get().developPanelOpen) void get().loadDevelopStateForPhoto(photoId);
        if (get().metadataPanelOpen) void get().loadKeywordsForPhoto(photoId);
        return;
      }
      if (mode === "range") {
        const photos = selectActivePhotos(get());
        const anchorId = get().selectedPhotoId;
        const anchorIndex = photos.findIndex((p) => p.id === anchorId);
        const targetIndex = photos.findIndex((p) => p.id === photoId);
        if (anchorIndex === -1 || targetIndex === -1) {
          get().selectPhoto(photoId);
          return;
        }
        const [start, end] = anchorIndex < targetIndex ? [anchorIndex, targetIndex] : [targetIndex, anchorIndex];
        const range = photos.slice(start, end + 1).map((p) => p.id);
        set((state) => {
          state.multiSelectedIds = range;
        });
        return;
      }
      get().selectPhoto(photoId);
    },

    metadataPanelOpen: false,

    toggleMetadataPanel: () => {
      const willOpen = !get().metadataPanelOpen;
      set((state) => {
        state.metadataPanelOpen = willOpen;
      });
      const { selectedPhotoId } = get();
      if (willOpen && selectedPhotoId) {
        void get().loadKeywordsForPhoto(selectedPhotoId);
      }
    },

    photoKeywords: {},

    loadKeywordsForPhoto: async (photoId) => {
      try {
        const keywords = await api.listPhotoKeywords(photoId);
        set((state) => {
          state.photoKeywords[photoId] = keywords;
        });
      } catch (err) {
        set((state) => {
          state.catalogError = String(err);
        });
      }
    },

    addKeywordToPhoto: async (photoId, name) => {
      const trimmed = name.trim();
      if (!trimmed) return;
      try {
        const keywordId = await api.addPhotoKeyword(photoId, trimmed);
        await get().loadKeywordsForPhoto(photoId);
        pushLibraryUndo({
          label: "Schlagwort hinzufügen",
          undo: async () => {
            await api.removePhotoKeyword(photoId, keywordId);
            await get().loadKeywordsForPhoto(photoId);
          },
          redo: async () => {
            // `add_photo_keyword` legt das Schlagwort bei Bedarf per Name
            // an (`find_or_create`) — dieselbe ID wie beim ersten Mal, da
            // das Schlagwort selbst beim Entfernen nie gelöscht wird (nur
            // die Verknüpfung), siehe `repository::keywords::add`.
            await api.addPhotoKeyword(photoId, trimmed);
            await get().loadKeywordsForPhoto(photoId);
          },
        });
      } catch (err) {
        set((state) => {
          state.catalogError = String(err);
        });
      }
    },

    removeKeywordFromPhoto: async (photoId, keywordId) => {
      const keyword = get().photoKeywords[photoId]?.find((k) => k.id === keywordId);
      try {
        await api.removePhotoKeyword(photoId, keywordId);
        await get().loadKeywordsForPhoto(photoId);
        if (keyword) {
          pushLibraryUndo({
            label: "Schlagwort entfernen",
            undo: async () => {
              await api.addPhotoKeyword(photoId, keyword.name);
              await get().loadKeywordsForPhoto(photoId);
            },
            redo: async () => {
              await api.removePhotoKeyword(photoId, keywordId);
              await get().loadKeywordsForPhoto(photoId);
            },
          });
        }
      } catch (err) {
        set((state) => {
          state.catalogError = String(err);
        });
      }
    },

    setPhotoRating: async (photoId, rating) => {
      const { multiSelectedIds } = get();
      const targets = multiSelectedIds.includes(photoId) && multiSelectedIds.length > 1 ? multiSelectedIds : [photoId];
      const previous = new Map(targets.map((id) => [id, findPhotoAnywhere(get(), id)?.rating ?? 0]));
      try {
        await Promise.all(targets.map((id) => api.setPhotoRating(id, rating)));
        set((state) => {
          for (const id of targets) patchPhotoEverywhere(state, id, { rating });
        });
        pushLibraryUndo({
          label: "Bewertung",
          undo: async () => {
            await Promise.all(targets.map((id) => api.setPhotoRating(id, previous.get(id) ?? 0)));
            set((state) => {
              for (const id of targets) patchPhotoEverywhere(state, id, { rating: previous.get(id) ?? 0 });
            });
          },
          redo: async () => {
            await Promise.all(targets.map((id) => api.setPhotoRating(id, rating)));
            set((state) => {
              for (const id of targets) patchPhotoEverywhere(state, id, { rating });
            });
          },
        });
      } catch (err) {
        set((state) => {
          state.catalogError = String(err);
        });
      }
    },

    setPhotoFlag: async (photoId, flag) => {
      const { multiSelectedIds } = get();
      const targets = multiSelectedIds.includes(photoId) && multiSelectedIds.length > 1 ? multiSelectedIds : [photoId];
      const previous = new Map(targets.map((id) => [id, findPhotoAnywhere(get(), id)?.flag ?? 0]));
      try {
        await Promise.all(targets.map((id) => api.setPhotoFlag(id, flag)));
        set((state) => {
          for (const id of targets) patchPhotoEverywhere(state, id, { flag });
        });
        pushLibraryUndo({
          label: "Flagge",
          undo: async () => {
            await Promise.all(targets.map((id) => api.setPhotoFlag(id, previous.get(id) ?? 0)));
            set((state) => {
              for (const id of targets) patchPhotoEverywhere(state, id, { flag: previous.get(id) ?? 0 });
            });
          },
          redo: async () => {
            await Promise.all(targets.map((id) => api.setPhotoFlag(id, flag)));
            set((state) => {
              for (const id of targets) patchPhotoEverywhere(state, id, { flag });
            });
          },
        });
      } catch (err) {
        set((state) => {
          state.catalogError = String(err);
        });
      }
    },

    setPhotoColorLabel: async (photoId, colorLabel) => {
      const { multiSelectedIds } = get();
      const targets = multiSelectedIds.includes(photoId) && multiSelectedIds.length > 1 ? multiSelectedIds : [photoId];
      const previous = new Map(targets.map((id) => [id, findPhotoAnywhere(get(), id)?.color_label ?? null]));
      try {
        await Promise.all(targets.map((id) => api.setPhotoColorLabel(id, colorLabel)));
        set((state) => {
          for (const id of targets) patchPhotoEverywhere(state, id, { color_label: colorLabel });
        });
        pushLibraryUndo({
          label: "Farbmarkierung",
          undo: async () => {
            await Promise.all(targets.map((id) => api.setPhotoColorLabel(id, previous.get(id) ?? null)));
            set((state) => {
              for (const id of targets) patchPhotoEverywhere(state, id, { color_label: previous.get(id) ?? null });
            });
          },
          redo: async () => {
            await Promise.all(targets.map((id) => api.setPhotoColorLabel(id, colorLabel)));
            set((state) => {
              for (const id of targets) patchPhotoEverywhere(state, id, { color_label: colorLabel });
            });
          },
        });
      } catch (err) {
        set((state) => {
          state.catalogError = String(err);
        });
      }
    },

    collections: [],
    selectedCollectionId: null,
    collectionPhotos: {},

    refreshCollections: async () => {
      try {
        const collections = await api.listCollections();
        set((state) => {
          state.collections = collections;
        });
      } catch (err) {
        set((state) => {
          state.catalogError = String(err);
        });
      }
    },

    createCollection: async (name) => {
      const trimmed = name.trim();
      if (!trimmed) return;
      try {
        await api.createCollection(trimmed);
        await get().refreshCollections();
      } catch (err) {
        set((state) => {
          state.catalogError = String(err);
        });
      }
    },

    selectCollection: (collectionId) => {
      set((state) => {
        state.selectedCollectionId = collectionId;
        if (collectionId) state.selectedFolderId = null;
        state.selectedPhotoId = null;
        state.multiSelectedIds = [];
        state.libraryQuery = "";
        state.libraryResults = null;
      });
      if (collectionId) {
        void get().loadPhotosForCollection(collectionId);
      }
    },

    loadPhotosForCollection: async (collectionId) => {
      try {
        const photos = await api.listPhotosInCollection(collectionId);
        set((state) => {
          state.collectionPhotos[collectionId] = photos;
        });
      } catch (err) {
        set((state) => {
          state.catalogError = String(err);
        });
      }
    },

    addSelectionToCollection: async (collectionId) => {
      const { multiSelectedIds, selectedPhotoId } = get();
      const targets = multiSelectedIds.length > 0 ? multiSelectedIds : selectedPhotoId ? [selectedPhotoId] : [];
      if (targets.length === 0) return;
      try {
        await Promise.all(targets.map((id) => api.addToCollection(collectionId, id)));
        if (get().collectionPhotos[collectionId]) {
          await get().loadPhotosForCollection(collectionId);
        }
        pushLibraryUndo({
          label: "Zu Sammlung hinzufügen",
          // Entfernt beim Rückgängig-Machen genau die Fotos, die diese
          // Aktion hinzugefügt hat — war eines davon schon vorher Mitglied
          // (Grenzfall, siehe `DECISIONS.md` ADR-0027), entfernt "Rückgängig"
          // es trotzdem; bewusst in Kauf genommene Vereinfachung, keine
          // Undo-Historie pro Mitgliedschaft.
          undo: async () => {
            await Promise.all(targets.map((id) => api.removeFromCollection(collectionId, id)));
            if (get().collectionPhotos[collectionId]) {
              await get().loadPhotosForCollection(collectionId);
            }
          },
          redo: async () => {
            await Promise.all(targets.map((id) => api.addToCollection(collectionId, id)));
            if (get().collectionPhotos[collectionId]) {
              await get().loadPhotosForCollection(collectionId);
            }
          },
        });
      } catch (err) {
        set((state) => {
          state.catalogError = String(err);
        });
      }
    },

    libraryQuery: "",
    libraryFilter: {},
    libraryResults: null,

    setLibraryQuery: (query) => {
      set((state) => {
        state.libraryQuery = query;
      });
    },

    runLibrarySearchAndFilter: async () => {
      const { libraryQuery, libraryFilter } = get();
      const trimmed = libraryQuery.trim();
      const hasFilter = Object.keys(libraryFilter).length > 0;
      if (!trimmed && !hasFilter) {
        set((state) => {
          state.libraryResults = null;
        });
        return;
      }
      try {
        const results = await api.searchAndFilterPhotos(trimmed || null, libraryFilter);
        set((state) => {
          state.libraryResults = results;
        });
      } catch (err) {
        set((state) => {
          state.catalogError = String(err);
        });
      }
    },

    setLibraryFilterChip: async (patch) => {
      const nextFilter: FilterCriteriaDto = { ...get().libraryFilter, ...patch };
      // Ein `undefined`-Wert im Patch soll das Attribut wieder entfernen
      // (Chip abwählen), nicht das Feld auf `undefined` überschreiben.
      for (const key of Object.keys(patch) as (keyof FilterCriteriaDto)[]) {
        if (patch[key] === undefined) delete nextFilter[key];
      }
      set((state) => {
        state.libraryFilter = nextFilter;
      });
      await get().runLibrarySearchAndFilter();
    },

    clearLibraryFilters: () => {
      set((state) => {
        state.libraryQuery = "";
        state.libraryFilter = {};
        state.libraryResults = null;
      });
    },

    showDuplicatePhotos: async () => {
      try {
        const groups = await api.listDuplicatePhotoGroups();
        set((state) => {
          state.libraryResults = groups.flat();
        });
      } catch (err) {
        set((state) => {
          state.catalogError = String(err);
        });
      }
    },

    librarySortField: "filename",
    librarySortDirection: "asc",

    setLibrarySort: (field, direction) => {
      set((state) => {
        state.librarySortField = field;
        state.librarySortDirection = direction;
      });
    },

    libraryUndoStack: [],
    libraryRedoStack: [],

    undoLibraryAction: async () => {
      const { libraryUndoStack, libraryRedoStack } = get();
      const next = await undoStackLib.undo({ undoStack: libraryUndoStack, redoStack: libraryRedoStack });
      set((state) => {
        state.libraryUndoStack = next.undoStack;
        state.libraryRedoStack = next.redoStack;
      });
    },

    redoLibraryAction: async () => {
      const { libraryUndoStack, libraryRedoStack } = get();
      const next = await undoStackLib.redo({ undoStack: libraryUndoStack, redoStack: libraryRedoStack });
      set((state) => {
        state.libraryUndoStack = next.undoStack;
        state.libraryRedoStack = next.redoStack;
      });
    },

    presetFolders: [],
    presets: [],
    selectedPresetFolderId: null,

    refreshPresetFolders: async () => {
      try {
        const folders = await api.listPresetFolders();
        set((state) => {
          state.presetFolders = folders;
        });
      } catch (err) {
        set((state) => {
          state.catalogError = String(err);
        });
      }
    },

    refreshPresets: async () => {
      try {
        const presets = await api.listPresets();
        set((state) => {
          state.presets = presets;
        });
      } catch (err) {
        set((state) => {
          state.catalogError = String(err);
        });
      }
    },

    createPresetFolder: async (name, parentId) => {
      const trimmed = name.trim();
      if (!trimmed) return;
      try {
        await api.createPresetFolder(trimmed, parentId);
        await get().refreshPresetFolders();
      } catch (err) {
        set((state) => {
          state.catalogError = String(err);
        });
      }
    },

    renamePresetFolder: async (folderId, name) => {
      const trimmed = name.trim();
      if (!trimmed) return;
      try {
        await api.renamePresetFolder(folderId, trimmed);
        await get().refreshPresetFolders();
      } catch (err) {
        set((state) => {
          state.catalogError = String(err);
        });
      }
    },

    deletePresetFolder: async (folderId) => {
      try {
        await api.deletePresetFolder(folderId);
        await Promise.all([get().refreshPresetFolders(), get().refreshPresets()]);
        if (get().selectedPresetFolderId === folderId) {
          set((state) => {
            state.selectedPresetFolderId = null;
          });
        }
      } catch (err) {
        set((state) => {
          state.catalogError = String(err);
        });
      }
    },

    selectPresetFolder: (folderId) => {
      set((state) => {
        state.selectedPresetFolderId = folderId;
      });
    },

    setPresetFavorite: async (presetId, isFavorite) => {
      try {
        await api.setPresetFavorite(presetId, isFavorite);
        await get().refreshPresets();
      } catch (err) {
        set((state) => {
          state.catalogError = String(err);
        });
      }
    },

    renamePreset: async (presetId, name) => {
      const trimmed = name.trim();
      if (!trimmed) return;
      const preset = get().presets.find((p) => p.id === presetId);
      if (!preset) return;
      try {
        await api.updatePresetMetadata(presetId, preset.folder_id, trimmed, preset.tags, preset.conditions_json);
        await get().refreshPresets();
      } catch (err) {
        set((state) => {
          state.catalogError = String(err);
        });
      }
    },

    movePresetToFolder: async (presetId, folderId) => {
      const preset = get().presets.find((p) => p.id === presetId);
      if (!preset) return;
      try {
        await api.updatePresetMetadata(presetId, folderId, preset.name, preset.tags, preset.conditions_json);
        await get().refreshPresets();
      } catch (err) {
        set((state) => {
          state.catalogError = String(err);
        });
      }
    },

    deletePreset: async (presetId) => {
      try {
        await api.deletePreset(presetId);
        await get().refreshPresets();
      } catch (err) {
        set((state) => {
          state.catalogError = String(err);
        });
      }
    },

    exportPresetAsApxFile: async (presetId) => {
      try {
        await api.exportPresetToApxFile(presetId);
      } catch (err) {
        set((state) => {
          state.catalogError = String(err);
        });
      }
    },

    importPresetFromApxFile: async (folderId) => {
      try {
        const imported = await api.importPresetFromApxFile(folderId);
        if (imported) await get().refreshPresets();
      } catch (err) {
        set((state) => {
          state.catalogError = String(err);
        });
      }
    },

    savePresetFromCurrentEdl: async (name, folderId, tags, sections, conditions = []) => {
      const trimmed = name.trim();
      if (!trimmed || sections.length === 0) return;
      try {
        const subset = buildPresetEdlSubset(get().developEdl, sections);
        await api.createPreset(folderId, trimmed, tags, serializeConditions(conditions), serializeEdlSubset(subset));
        await get().refreshPresets();
      } catch (err) {
        set((state) => {
          state.catalogError = String(err);
        });
      }
    },

    presetStrengthContext: null,

    applyPreset: async (presetId) => {
      try {
        const version = await api.latestPresetVersion(presetId);
        const rawSubset = parseEdlSubset(version.edl_subset_json);
        const preset = get().presets.find((p) => p.id === presetId);
        const conditions = parseConditions(preset?.conditions_json ?? "[]");
        const subset = applyConditionsToSubset(rawSubset, conditions, selectPresetConditionMeta(get()));
        if (subset === null) {
          // Bedingung fürs ganze Preset nicht erfüllt (`section: null`) —
          // Preset wird gar nicht angewendet (`lib/presets.ts`s
          // `applyConditionsToSubset`).
          set((state) => {
            state.catalogError = `Preset „${preset?.name ?? presetId}" erfüllt die Bedingungen für dieses Foto nicht.`;
          });
          return;
        }
        const baseEdl = get().developEdl;
        const merged = mergeEdlSubset(baseEdl, subset);
        set((state) => {
          state.developEdl = merged;
          state.presetStrengthContext = { presetId, presetName: preset?.name ?? "Preset", baseEdl, subset, strength: 100 };
        });
        await get().commitDevelopEdit(`Preset „${preset?.name ?? presetId}" angewendet`, { preservePresetStrengthContext: true });
      } catch (err) {
        set((state) => {
          state.catalogError = String(err);
        });
      }
    },

    setPresetStrength: (strengthPercent) => {
      const context = get().presetStrengthContext;
      if (!context) return;
      const clamped = Math.min(200, Math.max(0, strengthPercent));
      const scaled = scalePresetEdlSubset(context.subset, clamped);
      const merged = mergeEdlSubset(context.baseEdl, scaled);
      // Nur der Live-Zustand wird aktualisiert (löst über `useDevelopRender`
      // sofort eine neue Vorschau aus) — das eigentliche Committen passiert
      // erst bei `commitPresetStrength` (Loslassen des Reglers), genau wie
      // bei jedem anderen `DevelopSlider` (`onChange` vs. `onCommit`).
      set((state) => {
        state.developEdl = merged;
        if (state.presetStrengthContext) state.presetStrengthContext.strength = clamped;
      });
    },

    commitPresetStrength: () => {
      void get().commitDevelopEdit(undefined, { preservePresetStrengthContext: true });
    },

    dismissPresetStrengthContext: () => {
      set((state) => {
        state.presetStrengthContext = null;
      });
    },

    presetStack: [],

    addPresetToStack: (presetId) => {
      set((state) => {
        state.presetStack.push(presetId);
      });
    },

    removePresetFromStack: (index) => {
      set((state) => {
        state.presetStack.splice(index, 1);
      });
    },

    movePresetInStack: (index, direction) => {
      set((state) => {
        const target = index + direction;
        if (target < 0 || target >= state.presetStack.length) return;
        const [entry] = state.presetStack.splice(index, 1);
        if (entry === undefined) return;
        state.presetStack.splice(target, 0, entry);
      });
    },

    clearPresetStack: () => {
      set((state) => {
        state.presetStack = [];
      });
    },

    applyPresetStack: async () => {
      const { presetStack, presets } = get();
      if (presetStack.length === 0) return;
      try {
        let merged = get().developEdl;
        const meta = selectPresetConditionMeta(get());
        for (const presetId of presetStack) {
          const version = await api.latestPresetVersion(presetId);
          const rawSubset: PresetEdlSubset = parseEdlSubset(version.edl_subset_json);
          const preset = presets.find((p) => p.id === presetId);
          const conditions = parseConditions(preset?.conditions_json ?? "[]");
          const subset = applyConditionsToSubset(rawSubset, conditions, meta);
          if (subset === null) continue;
          merged = mergeEdlSubset(merged, subset);
        }
        const names = presetStack.map((id) => presets.find((p) => p.id === id)?.name ?? id).join(" + ");
        set((state) => {
          state.developEdl = merged;
        });
        await get().commitDevelopEdit(`Preset-Stapel angewendet (${names})`);
      } catch (err) {
        set((state) => {
          state.catalogError = String(err);
        });
      }
    },

    hoverPresetSubset: null,

    previewPresetHover: async (presetId) => {
      try {
        const version = await api.latestPresetVersion(presetId);
        const rawSubset = parseEdlSubset(version.edl_subset_json);
        const preset = get().presets.find((p) => p.id === presetId);
        const conditions = parseConditions(preset?.conditions_json ?? "[]");
        const subset = applyConditionsToSubset(rawSubset, conditions, selectPresetConditionMeta(get()));
        set((state) => {
          // `null` (Bedingung fürs ganze Preset nicht erfüllt) zeigt eine
          // leere Vorschau — entspricht dem tatsächlichen `applyPreset`-
          // Verhalten (Preset würde gar nicht angewendet).
          state.hoverPresetSubset = subset ?? {};
        });
      } catch {
        // Keine Vorschau statt eines Absturzes — z. B. bei einem
        // zwischenzeitlich gelöschten Preset.
      }
    },

    clearPresetHoverPreview: () => {
      set((state) => {
        state.hoverPresetSubset = null;
      });
    },

    selectedMaskId: null,

    selectMask: (maskId) => {
      set((state) => {
        state.selectedMaskId = maskId;
        state.selectedMaskComponentIndex = 0;
      });
    },

    addMask: (kind) => {
      const id = `mask-${crypto.randomUUID()}`;
      const geometry: MaskGeometry = MASK_KIND_DEFAULT_GEOMETRY[kind]();
      const name = MASK_KIND_LABEL[kind];
      set((state) => {
        state.developEdl.masks.push(newMask(id, name, geometry));
        state.selectedMaskId = id;
        state.selectedMaskComponentIndex = 0;
      });
      void get().commitDevelopEdit(`Maske „${name}" hinzugefügt`);
    },

    removeMask: (maskId) => {
      set((state) => {
        const index = state.developEdl.masks.findIndex((m) => m.id === maskId);
        if (index >= 0) state.developEdl.masks.splice(index, 1);
        if (state.selectedMaskId === maskId) state.selectedMaskId = null;
      });
      void get().commitDevelopEdit();
    },

    setMaskVisible: (maskId, visible) => {
      set((state) => {
        const mask = state.developEdl.masks.find((m) => m.id === maskId);
        if (mask) mask.visible = visible;
      });
      void get().commitDevelopEdit();
    },

    renameMask: (maskId, name) => {
      const trimmed = name.trim();
      if (!trimmed) return;
      set((state) => {
        const mask = state.developEdl.masks.find((m) => m.id === maskId);
        if (mask) mask.name = trimmed;
      });
      void get().commitDevelopEdit();
    },

    setMaskBlendMode: (maskId, mode) => {
      set((state) => {
        const mask = state.developEdl.masks.find((m) => m.id === maskId);
        if (mask) mask.blend_mode = mode;
      });
      void get().commitDevelopEdit();
    },

    selectedMaskComponentIndex: 0,

    selectMaskComponent: (index) => {
      set((state) => {
        state.selectedMaskComponentIndex = index;
      });
    },

    addMaskComponent: (maskId, kind) => {
      const geometry: MaskGeometry = MASK_KIND_DEFAULT_GEOMETRY[kind]();
      set((state) => {
        const mask = state.developEdl.masks.find((m) => m.id === maskId);
        if (!mask) return;
        mask.components.push({ geometry, combine: "Add", invert: false });
        state.selectedMaskComponentIndex = mask.components.length - 1;
      });
      void get().commitDevelopEdit(`Maskenkomponente „${MASK_KIND_LABEL[kind]}" hinzugefügt`);
    },

    removeMaskComponent: (maskId, componentIndex) => {
      set((state) => {
        const mask = state.developEdl.masks.find((m) => m.id === maskId);
        if (!mask || mask.components.length <= 1) return;
        mask.components.splice(componentIndex, 1);
        state.selectedMaskComponentIndex = Math.min(state.selectedMaskComponentIndex, mask.components.length - 1);
      });
      void get().commitDevelopEdit();
    },

    setMaskComponentCombine: (maskId, componentIndex, combine) => {
      set((state) => {
        const component = state.developEdl.masks.find((m) => m.id === maskId)?.components[componentIndex];
        if (component) component.combine = combine;
      });
      void get().commitDevelopEdit();
    },

    setMaskComponentInvert: (maskId, componentIndex, invert) => {
      set((state) => {
        const component = state.developEdl.masks.find((m) => m.id === maskId)?.components[componentIndex];
        if (component) component.invert = invert;
      });
      void get().commitDevelopEdit();
    },

    updateMaskGeometry: (maskId, geometry) => {
      set((state) => {
        const mask = state.developEdl.masks.find((m) => m.id === maskId);
        const component = mask?.components[state.selectedMaskComponentIndex];
        if (component) component.geometry = geometry;
      });
    },

    commitMaskDrag: () => {
      void get().commitDevelopEdit();
    },

    setMaskOpacity: (maskId, opacity) => {
      set((state) => {
        const mask = state.developEdl.masks.find((m) => m.id === maskId);
        if (mask) mask.opacity = opacity;
      });
    },

    setMaskFeather: (maskId, feather) => {
      set((state) => {
        const mask = state.developEdl.masks.find((m) => m.id === maskId);
        if (mask) mask.feather = feather;
      });
    },

    setMaskBasicField: (maskId, key, value) => {
      set((state) => {
        const mask = state.developEdl.masks.find((m) => m.id === maskId);
        if (mask) writeBasicField(mask.adjustments.basic, key, value);
      });
    },

    maskBrushDraftRadius: 0.05,
    maskBrushDraftFeather: 0.02,

    setMaskBrushDraftField: (key, value) => {
      set((state) => {
        if (key === "radius") state.maskBrushDraftRadius = value;
        else state.maskBrushDraftFeather = value;
      });
    },

    addMaskBrushStroke: (maskId, points) => {
      if (points.length === 0) return;
      const { maskBrushDraftRadius, maskBrushDraftFeather } = get();
      set((state) => {
        const geometry = state.developEdl.masks.find((m) => m.id === maskId)?.components[state.selectedMaskComponentIndex]?.geometry;
        if (geometry?.kind !== "Brush") return;
        geometry.strokes.push({ points, radius: maskBrushDraftRadius, feather: maskBrushDraftFeather });
      });
      void get().commitDevelopEdit();
    },

    removeMaskBrushStroke: (maskId, strokeIndex) => {
      set((state) => {
        const geometry = state.developEdl.masks.find((m) => m.id === maskId)?.components[state.selectedMaskComponentIndex]?.geometry;
        if (geometry?.kind !== "Brush") return;
        geometry.strokes.splice(strokeIndex, 1);
      });
      void get().commitDevelopEdit();
    },

    maskColorRangePickerActive: false,

    toggleMaskColorRangePicker: () => {
      set((state) => {
        state.maskColorRangePickerActive = !state.maskColorRangePickerActive;
      });
    },

    setMaskColorRangeTargetAt: (maskId, r, g, b) => {
      set((state) => {
        const geometry = state.developEdl.masks.find((m) => m.id === maskId)?.components[state.selectedMaskComponentIndex]?.geometry;
        if (geometry?.kind !== "ColorRange") return;
        geometry.target_r = r / 255;
        geometry.target_g = g / 255;
        geometry.target_b = b / 255;
        state.maskColorRangePickerActive = false;
      });
      void get().commitDevelopEdit();
    },

    setMaskCurveChannel: (maskId, channel, next) => {
      set((state) => {
        const mask = state.developEdl.masks.find((m) => m.id === maskId);
        if (mask) mask.adjustments.curves[channel] = next;
      });
    },

    setMaskHslBandField: (maskId, band, field, value) => {
      set((state) => {
        const mask = state.developEdl.masks.find((m) => m.id === maskId);
        if (mask) mask.adjustments.hsl[band][field] = value;
      });
    },

    maskColorMixerPickerActive: false,

    toggleMaskColorMixerPicker: () => {
      set((state) => {
        state.maskColorMixerPickerActive = !state.maskColorMixerPickerActive;
      });
    },

    addMaskColorMixerRegionAt: (maskId, r, g, b) => {
      set((state) => {
        const mask = state.developEdl.masks.find((m) => m.id === maskId);
        if (!mask || mask.adjustments.color_mixer.regions.length >= MAX_COLOR_MIXER_REGIONS) return;
        mask.adjustments.color_mixer.regions.push(newColorMixerRegion(hueDegreesFromRgbByte(r, g, b)));
        state.maskColorMixerPickerActive = false;
      });
      void get().commitDevelopEdit();
    },

    removeMaskColorMixerRegion: (maskId, regionIndex) => {
      set((state) => {
        const mask = state.developEdl.masks.find((m) => m.id === maskId);
        mask?.adjustments.color_mixer.regions.splice(regionIndex, 1);
      });
      void get().commitDevelopEdit();
    },

    updateMaskColorMixerRegion: (maskId, regionIndex, patch) => {
      set((state) => {
        const region = state.developEdl.masks.find((m) => m.id === maskId)?.adjustments.color_mixer.regions[regionIndex];
        if (region) Object.assign(region, patch);
      });
    },

    setMaskColorGradingWheel: (maskId, key, wheel) => {
      set((state) => {
        const mask = state.developEdl.masks.find((m) => m.id === maskId);
        if (mask) mask.adjustments.color_grading[key] = wheel;
      });
    },

    setMaskColorGradingBalance: (maskId, value) => {
      set((state) => {
        const mask = state.developEdl.masks.find((m) => m.id === maskId);
        if (mask) mask.adjustments.color_grading.balance = value;
      });
    },

    setMaskColorGradingBlending: (maskId, value) => {
      set((state) => {
        const mask = state.developEdl.masks.find((m) => m.id === maskId);
        if (mask) mask.adjustments.color_grading.blending = value;
      });
    },

    setMaskDetailsField: (maskId, key, value) => {
      set((state) => {
        const mask = state.developEdl.masks.find((m) => m.id === maskId);
        if (mask) mask.adjustments.details[key] = value;
      });
    },

    setMaskDetailsUseDeconvolutionSharpen: (maskId, value) => {
      set((state) => {
        const mask = state.developEdl.masks.find((m) => m.id === maskId);
        if (mask) mask.adjustments.details.use_deconvolution_sharpen = value;
      });
      void get().commitDevelopEdit();
    },

    addMaskGroup: (name) => {
      const trimmed = name.trim();
      if (!trimmed) return;
      const id = `mask-group-${crypto.randomUUID()}`;
      set((state) => {
        state.developEdl.mask_groups.push({ id, name: trimmed, visible: true });
      });
      void get().commitDevelopEdit(`Maskengruppe „${trimmed}" angelegt`);
    },

    renameMaskGroup: (groupId, name) => {
      const trimmed = name.trim();
      if (!trimmed) return;
      set((state) => {
        const group = state.developEdl.mask_groups.find((g) => g.id === groupId);
        if (group) group.name = trimmed;
      });
      void get().commitDevelopEdit();
    },

    removeMaskGroup: (groupId) => {
      set((state) => {
        const index = state.developEdl.mask_groups.findIndex((g) => g.id === groupId);
        if (index >= 0) state.developEdl.mask_groups.splice(index, 1);
        for (const mask of state.developEdl.masks) {
          if (mask.group_id === groupId) mask.group_id = null;
        }
      });
      void get().commitDevelopEdit();
    },

    setMaskGroupVisible: (groupId, visible) => {
      set((state) => {
        const group = state.developEdl.mask_groups.find((g) => g.id === groupId);
        if (group) group.visible = visible;
      });
      void get().commitDevelopEdit();
    },

    setMaskGroup: (maskId, groupId) => {
      set((state) => {
        const mask = state.developEdl.masks.find((m) => m.id === maskId);
        if (mask) mask.group_id = groupId;
      });
      void get().commitDevelopEdit();
    },

    duplicateMask: (maskId) => {
      set((state) => {
        const index = state.developEdl.masks.findIndex((m) => m.id === maskId);
        if (index < 0) return;
        const original = state.developEdl.masks[index];
        if (!original) return;
        // Immer-Draft (`original`) lässt sich nicht direkt strukturell
        // klonen (Proxy) — über JSON in einen Klartext-Wert wandeln, dann
        // erst kopieren (`Mask` ist rein JSON-serialisierbar).
        const clone: Mask = JSON.parse(JSON.stringify(original));
        clone.id = `mask-${crypto.randomUUID()}`;
        clone.name = `${original.name} (Kopie)`;
        state.developEdl.masks.splice(index + 1, 0, clone);
        state.selectedMaskId = clone.id;
        state.selectedMaskComponentIndex = 0;
      });
      void get().commitDevelopEdit("Maske dupliziert");
    },

    reorderMask: (fromIndex, toIndex) => {
      set((state) => {
        const masks = state.developEdl.masks;
        if (fromIndex < 0 || fromIndex >= masks.length || toIndex < 0 || toIndex >= masks.length || fromIndex === toIndex) return;
        const [moved] = masks.splice(fromIndex, 1);
        if (moved) masks.splice(toIndex, 0, moved);
      });
      void get().commitDevelopEdit("Maskenreihenfolge geändert");
    },

    transferMaskToPhoto: async (maskId, targetPhotoId) => {
      const mask = get().developEdl.masks.find((m) => m.id === maskId);
      if (!mask) return;
      const clone: Mask = JSON.parse(JSON.stringify(mask));
      clone.id = `mask-${crypto.randomUUID()}`;
      try {
        const position = await api.currentDevelopEdit(targetPhotoId);
        const targetPayload = edlFromHistoryPosition(position);
        targetPayload.masks.push(clone);
        await api.applyDevelopEdit(targetPhotoId, buildEdlEnvelopeJson(targetPayload), `Maske „${clone.name}" übertragen`);
        // Ist das Zielfoto gerade selbst im Entwickeln-Modul geöffnet,
        // dessen Anzeige nachziehen (sonst zeigt es den alten Stand, bis
        // es erneut ausgewählt wird).
        if (get().developPhotoId === targetPhotoId) {
          set((state) => {
            state.developEdl = targetPayload;
          });
        }
      } catch (err) {
        console.error("Maske konnte nicht auf das Zielfoto übertragen werden:", err);
        set((state) => {
          state.catalogError = "Maske konnte nicht auf das Zielfoto übertragen werden.";
        });
      }
    },

    maskBuildingBlocks: [],

    saveMaskAsBuildingBlock: (maskId, name) => {
      const trimmed = name.trim();
      if (!trimmed) return;
      const mask = get().developEdl.masks.find((m) => m.id === maskId);
      if (!mask) return;
      const snapshot: Mask = JSON.parse(JSON.stringify(mask));
      set((state) => {
        state.maskBuildingBlocks.push({ id: `mask-block-${crypto.randomUUID()}`, name: trimmed, mask: snapshot });
      });
    },

    applyMaskBuildingBlock: (blockId) => {
      const block = get().maskBuildingBlocks.find((b) => b.id === blockId);
      if (!block) return;
      const clone: Mask = JSON.parse(JSON.stringify(block.mask));
      clone.id = `mask-${crypto.randomUUID()}`;
      clone.name = block.name;
      set((state) => {
        state.developEdl.masks.push(clone);
        state.selectedMaskId = clone.id;
        state.selectedMaskComponentIndex = 0;
      });
      void get().commitDevelopEdit(`Baustein „${block.name}" angewendet`);
    },

    removeMaskBuildingBlock: (blockId) => {
      set((state) => {
        const index = state.maskBuildingBlocks.findIndex((b) => b.id === blockId);
        if (index >= 0) state.maskBuildingBlocks.splice(index, 1);
      });
    },

    // ---- KI-Slice (Phase 7) ----

    aiMaskClickPickerActive: false,

    toggleAiMaskClickPicker: () => {
      set((state) => {
        state.aiMaskClickPickerActive = !state.aiMaskClickPickerActive;
      });
    },

    aiMaskLoading: null,

    addAiMask: async (kind, click) => {
      const { selectedPhotoId } = get();
      if (!selectedPhotoId) return;
      if (kind === "ClickRegion" && !click) return;
      set((state) => {
        state.aiMaskLoading = kind;
        state.aiMaskClickPickerActive = false;
      });
      try {
        const dto = await api.generateAiMask(selectedPhotoId, AI_MASK_KIND_TO_BACKEND[kind], click?.x, click?.y);
        const geometry: MaskGeometry = {
          kind: "AiGenerated",
          ai_kind: kind,
          width: dto.width,
          height: dto.height,
          alpha: base64ToByteArray(dto.alpha_base64),
        };
        const id = `mask-${crypto.randomUUID()}`;
        const name = AI_MASK_KIND_LABELS[kind];
        set((state) => {
          state.developEdl.masks.push(newMask(id, name, geometry));
          state.selectedMaskId = id;
          state.selectedMaskComponentIndex = 0;
        });
        void get().commitDevelopEdit(`KI-Maske „${name}" hinzugefügt`);
      } catch (err) {
        set((state) => {
          state.catalogError = String(err);
        });
      } finally {
        set((state) => {
          state.aiMaskLoading = null;
        });
      }
    },

    autoSourceModeActive: false,

    toggleAutoSourceMode: () => {
      set((state) => {
        state.autoSourceModeActive = !state.autoSourceModeActive;
      });
    },

    repairSourceSuggestionLoading: false,

    suggestRepairSourceForTarget: async (targetX, targetY) => {
      const { selectedPhotoId, repairDraftRadius } = get();
      if (!selectedPhotoId) return;
      set((state) => {
        state.repairSourceSuggestionLoading = true;
      });
      try {
        const dto = await api.suggestRepairSource(selectedPhotoId, targetX, targetY, repairDraftRadius);
        set((state) => {
          state.repairPendingSource = { x: dto.x, y: dto.y };
        });
      } catch (err) {
        set((state) => {
          state.catalogError = String(err);
        });
      } finally {
        set((state) => {
          state.repairSourceSuggestionLoading = false;
        });
      }
    },

    sensorSpotCandidates: [],
    sensorSpotsLoading: false,

    detectSensorSpotsForCurrentPhoto: async (sensitivity) => {
      const { selectedPhotoId } = get();
      if (!selectedPhotoId) return;
      set((state) => {
        state.sensorSpotsLoading = true;
      });
      try {
        const spots = await api.detectSensorSpots(selectedPhotoId, sensitivity, 20);
        set((state) => {
          state.sensorSpotCandidates = spots;
        });
      } catch (err) {
        set((state) => {
          state.catalogError = String(err);
        });
      } finally {
        set((state) => {
          state.sensorSpotsLoading = false;
        });
      }
    },

    clearSensorSpots: () => {
      set((state) => {
        state.sensorSpotCandidates = [];
      });
    },

    applySensorSpotAsRepairStroke: (spot) => {
      set((state) => {
        state.developEdl.repair.push({
          mode: "ContentAwareFill",
          source: { x: 0, y: 0 },
          target_path: [{ x: spot.x, y: spot.y }],
          radius: spot.radius,
          feather: Math.min(spot.radius * 0.3, 0.05),
          opacity: 1,
        });
        state.sensorSpotCandidates = state.sensorSpotCandidates.filter((candidate) => candidate !== spot);
      });
      void get().commitDevelopEdit("Sensorfleck automatisch repariert");
    },

    aiSettings: null,

    loadAiSettings: async () => {
      try {
        const settings = await api.getAiSettings();
        set((state) => {
          state.aiSettings = settings;
        });
      } catch (err) {
        set((state) => {
          state.catalogError = String(err);
        });
      }
    },

    saveAnthropicApiKey: async (apiKey) => {
      try {
        await api.setAnthropicApiKey(apiKey.trim() || null);
        await get().loadAiSettings();
      } catch (err) {
        set((state) => {
          state.catalogError = String(err);
        });
      }
    },

    presetGeneratorLoading: false,
    presetGeneratorPreview: [],
    presetGeneratorSelectedIndex: 0,

    generatePresetFromDescription: async (description) => {
      const trimmed = description.trim();
      if (!trimmed) return;
      set((state) => {
        state.presetGeneratorLoading = true;
      });
      try {
        const json = await api.generatePresetFromLlm(trimmed);
        set((state) => {
          state.presetGeneratorPreview = [parseEdlSubset(json)];
          state.presetGeneratorSelectedIndex = 0;
        });
      } catch (err) {
        set((state) => {
          state.catalogError = String(err);
        });
      } finally {
        set((state) => {
          state.presetGeneratorLoading = false;
        });
      }
    },

    copyPresetPromptForClaudeApp: async (description) => {
      const trimmed = description.trim();
      if (!trimmed) return;
      try {
        const prompt = await api.buildPresetPromptText(trimmed);
        await navigator.clipboard.writeText(prompt);
      } catch (err) {
        set((state) => {
          state.catalogError = String(err);
        });
      }
    },

    importPresetFromPastedJson: async (json) => {
      const trimmed = json.trim();
      if (!trimmed) return;
      set((state) => {
        state.presetGeneratorLoading = true;
      });
      try {
        const validated = await api.importPresetJson(trimmed);
        set((state) => {
          state.presetGeneratorPreview = [parseEdlSubset(validated)];
          state.presetGeneratorSelectedIndex = 0;
        });
      } catch (err) {
        set((state) => {
          state.catalogError = String(err);
        });
      } finally {
        set((state) => {
          state.presetGeneratorLoading = false;
        });
      }
    },

    generatePresetFromReferenceImage: async () => {
      const { selectedPhotoId } = get();
      if (!selectedPhotoId) return;
      set((state) => {
        state.presetGeneratorLoading = true;
      });
      try {
        const json = await api.generatePresetFromReference(selectedPhotoId);
        if (json) {
          set((state) => {
            state.presetGeneratorPreview = [parseEdlSubset(json)];
            state.presetGeneratorSelectedIndex = 0;
          });
        }
      } catch (err) {
        set((state) => {
          state.catalogError = String(err);
        });
      } finally {
        set((state) => {
          state.presetGeneratorLoading = false;
        });
      }
    },

    generatePresetVariationsFromBase: async (base, count, seed) => {
      set((state) => {
        state.presetGeneratorLoading = true;
      });
      try {
        const jsonList = await api.generatePresetVariations(serializeEdlSubset(base), count, seed);
        set((state) => {
          state.presetGeneratorPreview = jsonList.map(parseEdlSubset);
          state.presetGeneratorSelectedIndex = 0;
        });
      } catch (err) {
        set((state) => {
          state.catalogError = String(err);
        });
      } finally {
        set((state) => {
          state.presetGeneratorLoading = false;
        });
      }
    },

    learnPresetFromSelectedPhotos: async (photoIds, sections) => {
      if (photoIds.length === 0) return;
      set((state) => {
        state.presetGeneratorLoading = true;
      });
      try {
        const json = await api.learnPresetFromPhotos(photoIds, sections);
        set((state) => {
          state.presetGeneratorPreview = [parseEdlSubset(json)];
          state.presetGeneratorSelectedIndex = 0;
        });
      } catch (err) {
        set((state) => {
          state.catalogError = String(err);
        });
      } finally {
        set((state) => {
          state.presetGeneratorLoading = false;
        });
      }
    },

    selectPresetGeneratorPreview: (index) => {
      set((state) => {
        state.presetGeneratorSelectedIndex = index;
      });
    },

    applyPresetGeneratorPreview: () => {
      const { presetGeneratorPreview, presetGeneratorSelectedIndex } = get();
      const subset = presetGeneratorPreview[presetGeneratorSelectedIndex];
      if (!subset) return;
      set((state) => {
        state.developEdl = mergeEdlSubset(state.developEdl, subset);
      });
      void get().commitDevelopEdit("KI-Preset angewendet");
    },

    clearPresetGeneratorPreview: () => {
      set((state) => {
        state.presetGeneratorPreview = [];
        state.presetGeneratorSelectedIndex = 0;
      });
    },

    tagSuggestions: [],
    tagSuggestionsLoading: false,

    fetchTagSuggestions: async (photoId) => {
      set((state) => {
        state.tagSuggestionsLoading = true;
      });
      try {
        const tags = await api.suggestTags(photoId);
        set((state) => {
          state.tagSuggestions = tags;
        });
      } catch (err) {
        set((state) => {
          state.catalogError = String(err);
        });
      } finally {
        set((state) => {
          state.tagSuggestionsLoading = false;
        });
      }
    },

    acceptTagSuggestion: async (photoId, tag) => {
      await get().addKeywordToPhoto(photoId, tag);
      set((state) => {
        state.tagSuggestions = state.tagSuggestions.filter((candidate) => candidate !== tag);
      });
    },

    clearTagSuggestions: () => {
      set((state) => {
        state.tagSuggestions = [];
      });
    },

    // ---- Export (Phase 8 Schritt 1+2) --------------------------------

    exportDialogOpen: false,
    exportRunning: false,
    exportProgress: null,
    exportError: null,
    exportQueuePaused: false,

    openExportDialog: () => {
      set((state) => {
        state.exportDialogOpen = true;
      });
    },

    closeExportDialog: () => {
      set((state) => {
        state.exportDialogOpen = false;
      });
    },

    exportPhotos: async (photoIds, destFolder, options) => {
      set((state) => {
        state.exportRunning = true;
        state.exportError = null;
        state.exportProgress = { done: 0, total: 0, failed: 0 };
        state.exportQueuePaused = false;
      });

      let firstError: string | null = null;
      try {
        for (const photoId of photoIds) {
          await api.enqueueExportPhoto(photoId, destFolder, options);
        }
      } catch (err) {
        firstError = err instanceof Error ? err.message : String(err);
      }

      // Fortschritt abfragen (siehe `ExportSlice`s Moduldoku), bis alle
      // eingereihten Aufträge dieser Sitzung abgeschlossen sind — die
      // Warteschlange kann bereits ältere Aufträge enthalten, daher zählt
      // hier `total` immer die komplette Backend-Warteschlange, nicht nur
      // `photoIds.length`.
      let progress = await api.getExportQueueProgress();
      while (progress.done < progress.total) {
        set((state) => {
          state.exportProgress = { done: progress.done, total: progress.total, failed: progress.failed };
          state.exportQueuePaused = progress.paused;
        });
        await new Promise((resolve) => setTimeout(resolve, 250));
        progress = await api.getExportQueueProgress();
      }
      set((state) => {
        state.exportProgress = { done: progress.done, total: progress.total, failed: progress.failed };
        state.exportQueuePaused = progress.paused;
      });

      set((state) => {
        state.exportRunning = false;
        state.exportError = firstError;
      });
      await api.clearFinishedExportJobs();
    },

    toggleExportQueuePause: async () => {
      const paused = get().exportQueuePaused;
      if (paused) {
        await api.resumeExportQueue();
      } else {
        await api.pauseExportQueue();
      }
      set((state) => {
        state.exportQueuePaused = !paused;
      });
    },

    // ---- Drucken (Phase 8 Schritt 3) ---------------------------------

    printDialogOpen: false,
    printRunning: false,
    printError: null,
    printLastOutcome: null,

    openPrintDialog: () => {
      set((state) => {
        state.printDialogOpen = true;
      });
    },

    closePrintDialog: () => {
      set((state) => {
        state.printDialogOpen = false;
      });
    },

    printPhotos: async (photoIds, destPath, options) => {
      set((state) => {
        state.printRunning = true;
        state.printError = null;
      });
      try {
        const outcome = await api.printPhotos(photoIds, destPath, options);
        set((state) => {
          state.printLastOutcome = outcome;
        });
      } catch (err) {
        set((state) => {
          state.printError = err instanceof Error ? err.message : String(err);
        });
      } finally {
        set((state) => {
          state.printRunning = false;
        });
      }
    },

    // ---- Diashow (Phase 8 Schritt 4) ---------------------------------

    slideshowDialogOpen: false,
    ffmpegAvailable: null,
    videoExportRunning: false,
    videoExportError: null,
    videoExportOutcome: null,

    openSlideshowDialog: () => {
      set((state) => {
        state.slideshowDialogOpen = true;
      });
    },

    closeSlideshowDialog: () => {
      set((state) => {
        state.slideshowDialogOpen = false;
      });
    },

    checkFfmpegAvailability: async () => {
      const available = await api.checkFfmpegAvailable();
      set((state) => {
        state.ffmpegAvailable = available;
      });
    },

    exportSlideshowVideo: async (photoIds, destPath, options) => {
      set((state) => {
        state.videoExportRunning = true;
        state.videoExportError = null;
      });
      try {
        const outcome = await api.exportSlideshowVideo(photoIds, destPath, options);
        set((state) => {
          state.videoExportOutcome = outcome;
        });
      } catch (err) {
        set((state) => {
          state.videoExportError = err instanceof Error ? err.message : String(err);
        });
      } finally {
        set((state) => {
          state.videoExportRunning = false;
        });
      }
    },

    // ---- Buch (Phase 8 Schritt 5) -------------------------------------

    bookDialogOpen: false,
    bookExportRunning: false,
    bookExportError: null,
    bookExportOutcome: null,

    openBookDialog: () => {
      set((state) => {
        state.bookDialogOpen = true;
      });
    },

    closeBookDialog: () => {
      set((state) => {
        state.bookDialogOpen = false;
      });
    },

    exportBookPdf: async (photoIds, destPath, options) => {
      set((state) => {
        state.bookExportRunning = true;
        state.bookExportError = null;
      });
      try {
        const outcome = await api.exportBookPdf(photoIds, destPath, options);
        set((state) => {
          state.bookExportOutcome = outcome;
        });
      } catch (err) {
        set((state) => {
          state.bookExportError = err instanceof Error ? err.message : String(err);
        });
      } finally {
        set((state) => {
          state.bookExportRunning = false;
        });
      }
    },

    // ---- Web (Phase 8 Schritt 6) --------------------------------------

    webDialogOpen: false,
    webExportRunning: false,
    webExportError: null,
    webExportOutcome: null,

    openWebDialog: () => {
      set((state) => {
        state.webDialogOpen = true;
      });
    },

    closeWebDialog: () => {
      set((state) => {
        state.webDialogOpen = false;
      });
    },

    exportWebGallery: async (photoIds, destDir, options) => {
      set((state) => {
        state.webExportRunning = true;
        state.webExportError = null;
      });
      try {
        const outcome = await api.exportWebGallery(photoIds, destDir, options);
        set((state) => {
          state.webExportOutcome = outcome;
        });
      } catch (err) {
        set((state) => {
          state.webExportError = err instanceof Error ? err.message : String(err);
        });
      } finally {
        set((state) => {
          state.webExportRunning = false;
        });
      }
    },

    // ---- Karte (Phase 8 Schritt 7) -------------------------------------

    geotaggedPhotos: [],
    gpxTrack: null,
    placingGpsForPhotoId: null,

    refreshGeotaggedPhotos: async () => {
      const photos = await api.listGeotaggedPhotos();
      set((state) => {
        state.geotaggedPhotos = photos;
      });
    },

    loadGpxTrack: async (path) => {
      const points = await api.importGpxTrack(path);
      set((state) => {
        state.gpxTrack = points;
      });
    },

    clearGpxTrack: () => {
      set((state) => {
        state.gpxTrack = null;
      });
    },

    startPlacingGps: (photoId) => {
      set((state) => {
        state.placingGpsForPhotoId = photoId;
      });
    },

    cancelPlacingGps: () => {
      set((state) => {
        state.placingGpsForPhotoId = null;
      });
    },

    setPhotoGpsFromMapClick: async (lat, lon) => {
      const photoId = get().placingGpsForPhotoId;
      if (!photoId) return;
      await api.setPhotoGps(photoId, lat, lon);
      set((state) => {
        state.placingGpsForPhotoId = null;
      });
      await get().refreshGeotaggedPhotos();
    },

    // ---- Vorlagen (Phase 8 Schritt 8) -----------------------------------

    templatesByKind: {},
    workflowRunning: false,
    workflowProgress: null,

    refreshTemplates: async (kind) => {
      const templates = await api.listTemplates(kind);
      set((state) => {
        state.templatesByKind[kind] = templates;
      });
    },

    saveTemplateAction: async (kind, name, payload) => {
      await api.saveTemplate(kind, name, JSON.stringify(payload));
      await get().refreshTemplates(kind);
    },

    deleteTemplateAction: async (kind, templateId) => {
      await api.deleteTemplate(templateId);
      await get().refreshTemplates(kind);
    },

    importTemplateFile: async () => {
      const imported = await api.importTemplateFromFile();
      if (imported) await get().refreshTemplates(imported.kind as TemplateKind);
    },

    runWorkflowTemplate: async (photoIds, template, destFolder) => {
      set((state) => {
        state.workflowRunning = true;
        state.workflowProgress = { done: 0, total: photoIds.length, failed: 0 };
      });
      try {
        const version = await api.latestPresetVersion(template.presetId);
        const subset = parseEdlSubset(version.edl_subset_json);
        for (const photoId of photoIds) {
          try {
            const position = await api.currentDevelopEdit(photoId);
            const merged = mergeEdlSubset(edlFromHistoryPosition(position), subset);
            await api.applyDevelopEdit(photoId, buildEdlEnvelopeJson(merged), "Workflow-Vorlage angewendet");
            await api.exportPhoto(photoId, destFolder, template.exportOptions);
            set((state) => {
              if (state.workflowProgress) state.workflowProgress.done += 1;
            });
          } catch (err) {
            console.error(`Workflow für Foto ${photoId} fehlgeschlagen:`, err);
            set((state) => {
              if (state.workflowProgress) state.workflowProgress.failed += 1;
            });
          }
        }
      } finally {
        set((state) => {
          state.workflowRunning = false;
        });
      }
    },

    // ---- Bibliotheks-Backlog (Phase 9 Schritt 1) ------------------------

    collectionFolders: [],
    stacks: [],
    virtualCopiesByPhotoId: {},
    colorLabelDefinitions: [],
    perceptualDuplicateGroups: [],
    perceptualDuplicatesRunning: false,

    refreshCollectionFolders: async () => {
      const folders = await api.listCollectionFolders();
      set((state) => {
        state.collectionFolders = folders;
      });
    },

    createCollectionFolder: async (name, parentId) => {
      const trimmed = name.trim();
      if (!trimmed) return;
      await api.createCollectionFolder(trimmed, parentId);
      await get().refreshCollectionFolders();
    },

    renameCollectionFolder: async (folderId, name) => {
      await api.renameCollectionFolder(folderId, name);
      await get().refreshCollectionFolders();
    },

    deleteCollectionFolder: async (folderId) => {
      await api.deleteCollectionFolder(folderId);
      await get().refreshCollectionFolders();
      await get().refreshCollections();
    },

    createSmartCollection: async (name, folderId, criteria) => {
      const trimmed = name.trim();
      if (!trimmed) return;
      await api.createSmartCollection(trimmed, folderId, criteria);
      await get().refreshCollections();
    },

    moveCollectionToFolder: async (collectionId, folderId) => {
      await api.moveCollectionToFolder(collectionId, folderId);
      await get().refreshCollections();
    },

    refreshStacks: async () => {
      const stacks = await api.listStacks();
      set((state) => {
        state.stacks = stacks;
      });
    },

    createStackFromSelection: async (name) => {
      const { multiSelectedIds, selectedPhotoId } = get();
      const targets = multiSelectedIds.length > 0 ? multiSelectedIds : selectedPhotoId ? [selectedPhotoId] : [];
      if (targets.length < 2) return;
      await api.createStack(name, targets);
      await get().refreshStacks();
    },

    deleteStack: async (stackId) => {
      await api.deleteStack(stackId);
      await get().refreshStacks();
    },

    setStackCover: async (stackId, coverPhotoId) => {
      await api.setStackCover(stackId, coverPhotoId);
      await get().refreshStacks();
    },

    autoStackSelectionByTime: async (windowSeconds) => {
      const { multiSelectedIds, selectedPhotoId } = get();
      const targets = multiSelectedIds.length > 0 ? multiSelectedIds : selectedPhotoId ? [selectedPhotoId] : [];
      if (targets.length < 2) return;
      await api.autoStackByTime(targets, windowSeconds);
      await get().refreshStacks();
    },

    createVirtualCopyForSelected: async () => {
      const { selectedPhotoId } = get();
      if (!selectedPhotoId) return;
      await api.createVirtualCopy(selectedPhotoId);
      await get().refreshVirtualCopies(selectedPhotoId);
      await get().refreshFolders();
    },

    refreshVirtualCopies: async (photoId) => {
      const copies = await api.listVirtualCopies(photoId);
      set((state) => {
        state.virtualCopiesByPhotoId[photoId] = copies;
      });
    },

    refreshColorLabelDefinitions: async () => {
      const defs = await api.listColorLabelDefinitions();
      set((state) => {
        state.colorLabelDefinitions = defs;
      });
    },

    createColorLabelDefinition: async (name, displayName, hex) => {
      await api.createColorLabelDefinition(name, displayName, hex);
      await get().refreshColorLabelDefinitions();
    },

    deleteColorLabelDefinition: async (name) => {
      await api.deleteColorLabelDefinition(name);
      await get().refreshColorLabelDefinitions();
    },

    runPerceptualDuplicateDetection: async (maxDistance) => {
      set((state) => {
        state.perceptualDuplicatesRunning = true;
      });
      try {
        const groups = await api.listPerceptualDuplicateGroups(maxDistance);
        set((state) => {
          state.perceptualDuplicateGroups = groups;
        });
      } finally {
        set((state) => {
          state.perceptualDuplicatesRunning = false;
        });
      }
    },
    };
  }),
);

// Store im Debug-Build am `window` verfügbar machen — üblich für
// Zustand-Projekte, praktisch zum manuellen Nachstellen von Zuständen in
// der Browser-Konsole (z. B. große Foto-Listen zum Testen der
// Filmstreifen-Virtualisierung). `import.meta.env.DEV` wird von Vite zur
// Build-Zeit ausgewertet — im Produktions-Build entfällt der Codepfad
// komplett (Dead-Code-Elimination), landet also nicht im ausgelieferten
// Bundle.
if (import.meta.env.DEV) {
  (window as unknown as { __appStore: typeof useAppStore }).__appStore = useAppStore;
}
