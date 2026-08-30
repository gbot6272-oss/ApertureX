import { create } from "zustand";
import { immer } from "zustand/middleware/immer";

import { buildEdlEnvelopeJson, MAX_COLOR_MIXER_REGIONS, neutralEdlPayload, newColorMixerRegion, parseEdlEnvelopeJson, WHITE_BALANCE_PRESETS, writeBasicField } from "../lib/edl";
import type { CalibrationAdjustment, ColorGradingAdjustment, ColorGradingWheel, ColorMixerRegion, CropRect, CurveChannel, CurvesAdjustment, DetailsAdjustment, EdlPayload, EffectsAdjustment, GridOverlay, GuidedLine, HslAdjustment, HslBand, LensCorrectionAdjustment, ManualTransform, PrimaryColorAdjustment, RepairMode, RepairPoint, UprightMode } from "../lib/edl";
import { hueDegreesFromRgbByte } from "../lib/colorSampling";
import {
  applyConditionsToSubset,
  buildPresetEdlSubset,
  mergeEdlSubset,
  parseConditions,
  parseEdlSubset,
  scalePresetEdlSubset,
  serializeConditions,
  serializeEdlSubset,
} from "../lib/presets";
import type { PresetCondition, PresetConditionPhotoMeta, PresetEdlSubset, PresetSectionKey } from "../lib/presets";
import { sortPhotos } from "../lib/sortPhotos";
import type { SortDirection, SortField } from "../lib/sortPhotos";
import * as api from "../lib/tauri";
import type {
  CatalogStatusDto,
  CollectionDto,
  FilterCriteriaDto,
  FolderDto,
  HistoryPositionDto,
  ImportModeDto,
  ImportPresetDto,
  KeywordDto,
  PhotoDto,
  PresetDto,
  PresetFolderDto,
} from "../lib/tauri";
import * as undoStackLib from "../lib/undoStack";
import type { UndoEntry } from "../lib/undoStack";
import { computeWhiteBalanceShiftFromSample } from "../lib/whiteBalancePicker";

/** Wandelt eine `HistoryPositionDto` (siehe `lib/tauri.ts`) in ein volles
 * `EdlPayload` um — `Neutral` bedeutet "wie aufgenommen", ein unlesbares
 * `edl_json` fällt (mit einer Konsolen-Warnung) ebenfalls auf neutral
 * zurück statt abzustürzen. */
function edlFromHistoryPosition(position: HistoryPositionDto): EdlPayload {
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
   * neue Raster (Schritt 6), `"viewer"` der bisherige Einzelbild-Viewer. */
  centerView: "viewer" | "grid";
  toggleCenterView: () => void;

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

export type AppStore = CatalogSlice & SelectionSlice & ViewerSlice & JobsSlice & DevelopSlice & LibrarySlice & PresetsSlice;

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
      try {
        const position = await api.currentDevelopEdit(photoId);
        set((state) => {
          state.developEdl = edlFromHistoryPosition(position);
          state.developPhotoId = photoId;
        });
      } catch (err) {
        console.error("Bearbeitungszustand konnte nicht geladen werden:", err);
        set((state) => {
          state.developEdl = neutralEdlPayload();
          state.developPhotoId = photoId;
        });
      }
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
      if (!repairPendingSource || targetPath.length === 0) return;
      set((state) => {
        state.developEdl.repair.push({
          mode: repairDraftMode,
          source: repairPendingSource,
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
