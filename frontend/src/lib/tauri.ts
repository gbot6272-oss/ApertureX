import { invoke } from "@tauri-apps/api/core";

/**
 * Dünne, typisierte Hülle um die Tauri-Commands aus
 * `crates/apx-app/src/commands.rs`. Zentral an einer Stelle, statt
 * `invoke("...")`-Strings über die Komponenten verstreut.
 */

export interface FolderDto {
  id: string;
  path: string;
  photo_count: number;
  /** `null` bei einem Wurzelordner. */
  parent_id: string | null;
  /** `true`, wenn der Ordnerpfad im Dateisystem nicht mehr existiert. */
  missing: boolean;
}

export interface CatalogStatusDto {
  catalog_path: string;
  folder_count: number;
  photo_count: number;
}

export interface PhotoDto {
  id: string;
  filename: string;
  /** Dateigröße in Byte — Grundlage für die Sortierung nach Dateigröße,
   * siehe `lib/sortPhotos.ts`. */
  file_size: number;
  width: number | null;
  height: number | null;
  camera_make: string | null;
  camera_model: string | null;
  lens: string | null;
  iso: number | null;
  aperture: number | null;
  shutter: number | null;
  focal_length: number | null;
  captured_at: string | null;
  missing: boolean;
  /** Sternebewertung 0–5. */
  rating: number;
  /** Pick/Reject-Flagge: 1 = Pick, -1 = Reject, 0 = keine. */
  flag: number;
  color_label: string | null;
}

export interface KeywordDto {
  id: string;
  name: string;
}

export interface CollectionDto {
  id: string;
  name: string;
}

/** Alle Felder optional — ein leeres Objekt liefert alle Fotos (siehe
 * `apx_catalog::FilterCriteria`). */
export interface FilterCriteriaDto {
  rating_at_least?: number;
  flag?: number;
  color_label?: string;
  camera_model?: string;
}

// ---- Presets (ab Phase 5, siehe DECISIONS.md ADR-0031) --------------------

export interface PresetFolderDto {
  id: string;
  name: string;
  parent_id: string | null;
  position: number;
}

/** Metadaten eines Presets, ohne seine EDL-Teilmenge — die kommt separat
 * über {@link listPresetVersions}/{@link latestPresetVersion}. */
export interface PresetDto {
  id: string;
  folder_id: string | null;
  name: string;
  is_favorite: boolean;
  tags: string[];
  /** JSON-String, siehe `lib/presets.ts`s `PresetCondition[]`. */
  conditions_json: string;
  created_at: string;
}

export interface PresetVersionDto {
  id: string;
  preset_id: string;
  sequence: number;
  /** JSON-String, siehe `lib/presets.ts`s `PresetEdlSubset`. */
  edl_subset_json: string;
  created_at: string;
}

export interface CreatePresetResultDto {
  preset_id: string;
  version_id: string;
}

export function selectFolderDialog(): Promise<string | null> {
  return invoke<string | null>("select_folder");
}

export function getCatalogStatus(): Promise<CatalogStatusDto> {
  return invoke<CatalogStatusDto>("catalog_status");
}

export function listFolders(): Promise<FolderDto[]> {
  return invoke<FolderDto[]>("list_folders");
}

export function listPhotosInFolder(folderId: string): Promise<PhotoDto[]> {
  return invoke<PhotoDto[]>("list_photos_in_folder", { folderId });
}

export function importFolder(path: string): Promise<void> {
  return invoke<void>("import_folder", { path });
}

export function cancelImport(): Promise<void> {
  return invoke<void>("cancel_import");
}

// ---- Import mit Modus + Umbenennungsmuster + Presets (ab Phase 3, Frontend
// erst Phase 5 Schritt 9 — siehe DECISIONS.md ADR-0031 Punkt 7) -------------

/** Spiegelt `crate::commands::ImportModeDto` (`#[serde(tag = "kind")]`) —
 * Eingabe für {@link importFolderWithMode}. */
export type ImportModeDto = { kind: "AddInPlace" } | { kind: "Copy"; target_dir: string } | { kind: "Move"; target_dir: string };

/** Spiegelt `crate::import::presets::PresetMode` (`#[serde(tag = "mode")]`
 * — eigenes Tag-Feld, ebenfalls `"mode"` genannt, siehe dessen Moduldoku).
 * Bewusst ein eigener Typ statt `ImportModeDto` wiederzuverwenden — beide
 * Rust-Typen sind strukturell identisch, tragen ihre Variante aber unter
 * unterschiedlichen Feldnamen (`kind` vs. `mode`); siehe
 * {@link importPresetModeToImportModeDto} für die Umwandlung. */
export type ImportPresetModeDto = { mode: "AddInPlace" } | { mode: "Copy"; target_dir: string } | { mode: "Move"; target_dir: string };

/** Spiegelt `crate::import::presets::ImportPreset`. */
export interface ImportPresetDto {
  name: string;
  mode: ImportPresetModeDto;
  rename_pattern: string | null;
}

export function importPresetModeToImportModeDto(mode: ImportPresetModeDto): ImportModeDto {
  switch (mode.mode) {
    case "AddInPlace":
      return { kind: "AddInPlace" };
    case "Copy":
      return { kind: "Copy", target_dir: mode.target_dir };
    case "Move":
      return { kind: "Move", target_dir: mode.target_dir };
  }
}

/** Wie {@link importFolder}, aber mit wählbarem Modus (Hinzufügen an Ort
 * und Stelle/Kopieren/Verschieben in einen Zielordner) und optionalem
 * Umbenennungsmuster — `crates/apx-app/src/commands.rs::import_folder_with_mode`. */
export function importFolderWithMode(path: string, mode: ImportModeDto, renamePattern: string | null): Promise<void> {
  return invoke<void>("import_folder_with_mode", { path, mode, renamePattern });
}

export function listImportPresets(): Promise<ImportPresetDto[]> {
  return invoke<ImportPresetDto[]>("list_import_presets");
}

export function saveImportPreset(preset: ImportPresetDto): Promise<ImportPresetDto[]> {
  return invoke<ImportPresetDto[]>("save_import_preset", { preset });
}

export function deleteImportPreset(name: string): Promise<ImportPresetDto[]> {
  return invoke<ImportPresetDto[]>("delete_import_preset", { name });
}

/** Verknüpft einen fehlenden Ordner mit einem neuen Speicherort — siehe
 * `FolderDto.missing` und `crates/apx-app/src/commands.rs::relink_folder`. */
export function relinkFolder(folderId: string, newPath: string): Promise<void> {
  return invoke<void>("relink_folder", { folderId, newPath });
}

// ---- Entwickeln-Verlauf (ab Phase 2, siehe crates/apx-app/src/commands.rs) ----

export type HistoryPositionDto = { kind: "Neutral" } | { kind: "At"; edl_json: string };

export function applyDevelopEdit(photoId: string, edlJson: string, label?: string): Promise<void> {
  return invoke<void>("apply_develop_edit", { photoId, edlJson, label: label ?? null });
}

export function currentDevelopEdit(photoId: string): Promise<HistoryPositionDto> {
  return invoke<HistoryPositionDto>("current_develop_edit", { photoId });
}

export function undoDevelopEdit(photoId: string): Promise<HistoryPositionDto | null> {
  return invoke<HistoryPositionDto | null>("undo_develop_edit", { photoId });
}

export function redoDevelopEdit(photoId: string): Promise<HistoryPositionDto | null> {
  return invoke<HistoryPositionDto | null>("redo_develop_edit", { photoId });
}

// ---- Schnappschüsse (Phase 6 Schritt 8) -------------------------------------
// Anders als der lineare Verlauf oben: siehe `crates/apx-app/src/commands.rs`s
// Moduldoku für die Abgrenzung. Kein eigener "restore"-Aufruf — die
// gespeicherte `edl_json` wird wie jeder andere EDL-Stand über
// `applyDevelopEdit` committet (siehe `store/index.ts`s `restoreSnapshot`).

export interface SnapshotDto {
  id: string;
  name: string;
  edl_json: string;
  created_at: string;
}

export function createSnapshot(photoId: string, name: string, edlJson: string): Promise<void> {
  return invoke<void>("create_snapshot", { photoId, name, edlJson });
}

export function listSnapshots(photoId: string): Promise<SnapshotDto[]> {
  return invoke<SnapshotDto[]>("list_snapshots", { photoId });
}

export function renameSnapshot(snapshotId: string, name: string): Promise<void> {
  return invoke<void>("rename_snapshot", { snapshotId, name });
}

export function deleteSnapshot(snapshotId: string): Promise<void> {
  return invoke<void>("delete_snapshot", { snapshotId });
}

// ---- Bibliothek: Bewertung/Flagge/Farbe (ab Phase 3) -----------------------

export function setPhotoRating(photoId: string, rating: number): Promise<void> {
  return invoke<void>("set_photo_rating", { photoId, rating });
}

export function setPhotoFlag(photoId: string, flag: number): Promise<void> {
  return invoke<void>("set_photo_flag", { photoId, flag });
}

export function setPhotoColorLabel(photoId: string, colorLabel: string | null): Promise<void> {
  return invoke<void>("set_photo_color_label", { photoId, colorLabel });
}

// ---- Bibliothek: Schlagworte (ab Phase 3) ----------------------------------

export function addPhotoKeyword(photoId: string, name: string): Promise<string> {
  return invoke<string>("add_photo_keyword", { photoId, name });
}

export function removePhotoKeyword(photoId: string, keywordId: string): Promise<void> {
  return invoke<void>("remove_photo_keyword", { photoId, keywordId });
}

export function listPhotoKeywords(photoId: string): Promise<KeywordDto[]> {
  return invoke<KeywordDto[]>("list_photo_keywords", { photoId });
}

// ---- Bibliothek: Sammlungen (ab Phase 3) -----------------------------------

export function createCollection(name: string): Promise<string> {
  return invoke<string>("create_collection", { name });
}

export function listCollections(): Promise<CollectionDto[]> {
  return invoke<CollectionDto[]>("list_collections");
}

export function addToCollection(collectionId: string, photoId: string): Promise<void> {
  return invoke<void>("add_to_collection", { collectionId, photoId });
}

export function removeFromCollection(collectionId: string, photoId: string): Promise<void> {
  return invoke<void>("remove_from_collection", { collectionId, photoId });
}

export function listPhotosInCollection(collectionId: string): Promise<PhotoDto[]> {
  return invoke<PhotoDto[]>("list_photos_in_collection", { collectionId });
}

// ---- Presets (ab Phase 5, siehe DECISIONS.md ADR-0031) --------------------

export function createPresetFolder(name: string, parentId: string | null): Promise<string> {
  return invoke<string>("create_preset_folder", { name, parentId });
}

export function renamePresetFolder(folderId: string, name: string): Promise<void> {
  return invoke<void>("rename_preset_folder", { folderId, name });
}

export function deletePresetFolder(folderId: string): Promise<void> {
  return invoke<void>("delete_preset_folder", { folderId });
}

export function listPresetFolders(): Promise<PresetFolderDto[]> {
  return invoke<PresetFolderDto[]>("list_preset_folders");
}

export function createPreset(
  folderId: string | null,
  name: string,
  tags: string[],
  conditionsJson: string,
  edlSubsetJson: string,
): Promise<CreatePresetResultDto> {
  return invoke<CreatePresetResultDto>("create_preset", { folderId, name, tags, conditionsJson, edlSubsetJson });
}

export function updatePresetMetadata(
  presetId: string,
  folderId: string | null,
  name: string,
  tags: string[],
  conditionsJson: string,
): Promise<void> {
  return invoke<void>("update_preset_metadata", { presetId, folderId, name, tags, conditionsJson });
}

export function setPresetFavorite(presetId: string, isFavorite: boolean): Promise<void> {
  return invoke<void>("set_preset_favorite", { presetId, isFavorite });
}

export function deletePreset(presetId: string): Promise<void> {
  return invoke<void>("delete_preset", { presetId });
}

export function listPresets(): Promise<PresetDto[]> {
  return invoke<PresetDto[]>("list_presets");
}

export function addPresetVersion(presetId: string, edlSubsetJson: string): Promise<string> {
  return invoke<string>("add_preset_version", { presetId, edlSubsetJson });
}

export function listPresetVersions(presetId: string): Promise<PresetVersionDto[]> {
  return invoke<PresetVersionDto[]>("list_preset_versions", { presetId });
}

export function latestPresetVersion(presetId: string): Promise<PresetVersionDto> {
  return invoke<PresetVersionDto>("latest_preset_version", { presetId });
}

/** Öffnet einen Speichern-Dialog und schreibt das Preset als `.apx`-Datei.
 * `null`, wenn der Dialog abgebrochen wurde. */
export function exportPresetToApxFile(presetId: string): Promise<string | null> {
  return invoke<string | null>("export_preset_to_apx_file", { presetId });
}

/** Öffnet einen Öffnen-Dialog und legt die gewählte `.apx`-Datei als neues
 * Preset an. `null`, wenn der Dialog abgebrochen wurde. */
export function importPresetFromApxFile(folderId: string | null): Promise<PresetDto | null> {
  return invoke<PresetDto | null>("import_preset_from_apx_file", { folderId });
}

// ---- Bibliothek: Suche/Filter (ab Phase 3) ---------------------------------

export function searchPhotos(query: string): Promise<PhotoDto[]> {
  return invoke<PhotoDto[]>("search_photos", { query });
}

export function filterPhotos(criteria: FilterCriteriaDto): Promise<PhotoDto[]> {
  return invoke<PhotoDto[]>("filter_photos", { criteria });
}

/** Kombiniert Volltextsuche (optional) und Attributfilter per UND — additiv
 * zu {@link searchPhotos}/{@link filterPhotos}, siehe `DECISIONS.md` ADR-0027. */
export function searchAndFilterPhotos(query: string | null, criteria: FilterCriteriaDto): Promise<PhotoDto[]> {
  return invoke<PhotoDto[]>("search_and_filter_photos", { query, criteria });
}

// ---- Bibliothek: Duplikaterkennung (ab Phase 3, Schritt 8.2) ---------------

/** Gruppen von Fotos mit identischem Inhalt (exakter Hash-Vergleich), siehe
 * `DECISIONS.md` ADR-0027 — reine Anzeige, verhindert den Import selbst nicht. */
export function listDuplicatePhotoGroups(): Promise<PhotoDto[][]> {
  return invoke<PhotoDto[][]>("list_duplicate_photo_groups");
}

// ---- KI-Funktionen (Phase 7, siehe DECISIONS.md ADR-0033) ------------------

export interface AiMaskAlphaDto {
  kind: string;
  width: number;
  height: number;
  /** Base64-kodierte Ein-Kanal-`u8`-Alpha-Bitmap. */
  alpha_base64: string;
}

/** Die fünf KI-Masken (`apx_ai::segmentation`) — `clickX`/`clickY` (normierte
 * Bildkoordinaten) sind nur für `kind === "click_region"` Pflicht,
 * `tolerance` nur dafür relevant (Vorgabe serverseitig `0.15`). */
export function generateAiMask(
  photoId: string,
  kind: string,
  clickX?: number,
  clickY?: number,
  tolerance?: number,
): Promise<AiMaskAlphaDto> {
  return invoke<AiMaskAlphaDto>("generate_ai_mask", {
    photoId,
    kind,
    clickX: clickX ?? null,
    clickY: clickY ?? null,
    tolerance: tolerance ?? null,
  });
}

export interface RepairSourceSuggestionDto {
  x: number;
  y: number;
}

/** Auto-Quellenfindung (`apx_ai::repair_analysis::suggest_source_point`). */
export function suggestRepairSource(
  photoId: string,
  targetX: number,
  targetY: number,
  brushRadius: number,
): Promise<RepairSourceSuggestionDto> {
  return invoke<RepairSourceSuggestionDto>("suggest_repair_source", { photoId, targetX, targetY, brushRadius });
}

export interface SpotCandidateDto {
  x: number;
  y: number;
  radius: number;
  strength: number;
}

/** Sensorflecken-Visualisierung (`apx_ai::repair_analysis::detect_spots`). */
export function detectSensorSpots(photoId: string, sensitivity: number, maxSpots: number): Promise<SpotCandidateDto[]> {
  return invoke<SpotCandidateDto[]>("detect_sensor_spots", { photoId, sensitivity, maxSpots });
}

export interface AiSettingsDto {
  anthropic_api_key: string | null;
}

export function getAiSettings(): Promise<AiSettingsDto> {
  return invoke<AiSettingsDto>("get_ai_settings");
}

/** `null`/leerer String löscht den hinterlegten Schlüssel. */
export function setAnthropicApiKey(apiKey: string | null): Promise<void> {
  return invoke<void>("set_anthropic_api_key", { apiKey: apiKey || null });
}

/** LLM-Modus des Preset-Generators — liefert die EDL-Teilmenge als
 * JSON-String (`lib/presets.ts::parseEdlSubset`). Braucht einen
 * hinterlegten Anthropic-API-Schlüssel. */
export function generatePresetFromLlm(description: string): Promise<string> {
  return invoke<string>("generate_preset_from_llm", { description });
}

/** **Manueller LLM-Modus ohne API-Schlüssel:** liefert einen fertigen
 * Prompt-Text (System-Prompt + `description`) zum Einfügen in die
 * Claude-App (claude.ai) — kein Netzwerk-Aufruf, keine Einstellungen
 * nötig. Die Antwort von dort kommt über {@link importPresetJson}
 * zurück. */
export function buildPresetPromptText(description: string): Promise<string> {
  return invoke<string>("build_preset_prompt_text", { description });
}

/** Validiert ein von Hand eingefügtes JSON-Ergebnis (aus der Claude-App
 * kopiert, siehe {@link buildPresetPromptText}) serverseitig, ohne
 * selbst einen API-Aufruf zu machen — liefert es normalisiert als
 * JSON-String zurück (dieselbe Form wie {@link generatePresetFromLlm}). */
export function importPresetJson(json: string): Promise<string> {
  return invoke<string>("import_preset_json", { json });
}

/** Referenzbild-Modus — öffnet einen Datei-Auswahldialog, `null` wenn
 * abgebrochen. Kein LLM, kein API-Schlüssel nötig. */
export function generatePresetFromReference(photoId: string): Promise<string | null> {
  return invoke<string | null>("generate_preset_from_reference", { photoId });
}

/** Variationen-Generator — `seed` reproduzierbar, liefert `count`
 * EDL-Teilmengen als JSON-Strings. */
export function generatePresetVariations(edlSubsetJson: string, count: number, seed: number): Promise<string[]> {
  return invoke<string[]>("generate_preset_variations", { edlSubsetJson, count, seed });
}

/** Preset aus Bearbeitung lernen — mittelt `sections` über den aktuell
 * committeten Bearbeitungsstand der genannten Fotos. */
export function learnPresetFromPhotos(photoIds: string[], sections: string[]): Promise<string> {
  return invoke<string>("learn_preset_from_photos", { photoIds, sections });
}

/** Auto-Tagging-Vorschläge (`apx_ai::tagging`) — schreibt nichts in den
 * Katalog, das Frontend übernimmt ausgewählte Vorschläge über
 * {@link addPhotoKeyword}. */
export function suggestTags(photoId: string): Promise<string[]> {
  return invoke<string[]>("suggest_tags", { photoId });
}

// ---- Export (Phase 8, siehe DECISIONS.md ADR-0034) -------------------------

export type ExportFormat = "jpeg" | "png" | "tiff" | "webp" | "avif";

export type WatermarkPosition = "top_left" | "top_right" | "bottom_left" | "bottom_right" | "center";
export type IccProfileChoice = "srgb" | "adobe_rgb" | "pro_photo_rgb" | "display_p3" | "custom";

export interface ExportPhotoOptions {
  format: ExportFormat;
  quality?: number;
  bitDepth16?: boolean;
  maxEdge?: number;
  maxMegapixels?: number;
  maxFileSizeBytes?: number;
  sharpenAmount?: number;
  sharpenRadius?: number;
  filename?: string;
  /** Schritt 2: ICC-Farbmanagement, Wasserzeichen, Metadaten-Filter. */
  iccProfile?: IccProfileChoice;
  iccProfilePath?: string;
  watermarkText?: string;
  watermarkFontPath?: string;
  watermarkFontSize?: number;
  watermarkColor?: [number, number, number];
  watermarkImagePath?: string;
  watermarkPosition?: WatermarkPosition;
  watermarkOpacity?: number;
  watermarkMargin?: number;
  metadataMake?: string;
  metadataModel?: string;
  metadataDateTime?: string;
  metadataCopyright?: string;
  metadataArtist?: string;
}

export interface ExportOutcomeDto {
  path: string;
  width: number;
  height: number;
  byte_size: number;
}

/** Exportiert ein Foto mit seinem aktuellen Bearbeitungsstand nach
 * `destFolder` (siehe `apx_export::engine`) — rendert serverseitig über
 * denselben Pfad wie die Entwickeln-Vorschau. Läuft synchron/sofort; für
 * einen Stapelexport mehrerer Fotos mit Fortschritt/Pausieren siehe
 * {@link enqueueExportPhoto} (Schritt 2). */
export function exportPhoto(
  photoId: string,
  destFolder: string,
  options: ExportPhotoOptions,
): Promise<ExportOutcomeDto> {
  return invoke<ExportOutcomeDto>("export_photo", { photoId, destFolder, options });
}

// ---- Export-Warteschlange (Phase 8 Schritt 2) ------------------------------

export interface ExportQueueProgressDto {
  done: number;
  total: number;
  failed: number;
  paused: boolean;
}

/** Reiht einen Foto-Export in die Backend-Warteschlange ein, statt ihn
 * sofort auszuführen — gibt die Auftrags-ID zurück. */
export function enqueueExportPhoto(photoId: string, destFolder: string, options: ExportPhotoOptions): Promise<number> {
  return invoke<number>("enqueue_export_photo", { photoId, destFolder, options });
}

export function getExportQueueProgress(): Promise<ExportQueueProgressDto> {
  return invoke<ExportQueueProgressDto>("export_queue_progress");
}

export function pauseExportQueue(): Promise<void> {
  return invoke<void>("pause_export_queue");
}

export function resumeExportQueue(): Promise<void> {
  return invoke<void>("resume_export_queue");
}

export function cancelExportJob(jobId: number): Promise<boolean> {
  return invoke<boolean>("cancel_export_job", { jobId });
}

export function clearFinishedExportJobs(): Promise<void> {
  return invoke<void>("clear_finished_export_jobs");
}

/** Generischer Datei-Auswahldialog (ICC-Profil, Wasserzeichen-Schriftdatei/
 * -Bild) — gibt nur den gewählten Pfad zurück, `null` wenn abgebrochen. */
export function pickFilePath(filterName: string, extensions: string[]): Promise<string | null> {
  return invoke<string | null>("pick_file_path", { filterName, extensions });
}

/** Speichern-unter-Dialog (Drucken/Buch) — gibt den gewählten Zielpfad
 * zurück, `null` wenn abgebrochen. */
export function pickSaveFilePath(filterName: string, extensions: string[], defaultFileName: string): Promise<string | null> {
  return invoke<string | null>("pick_save_file_path", { filterName, extensions, defaultFileName });
}

// ---- Drucken (Phase 8 Schritt 3) -------------------------------------------

export type PrintLayoutKind = "single" | "contact_sheet" | "custom_grid" | "picture_package";
export type PrintFit = "contain" | "cover";
export type PicturePackageTemplate = "one_large_two_small" | "four_equal" | "eight_wallet";

export interface PrintLayoutOptions {
  layout: PrintLayoutKind;
  cols?: number;
  rows?: number;
  picturePackageTemplate?: PicturePackageTemplate;
  pageWidthIn: number;
  pageHeightIn: number;
  dpi: number;
  marginIn?: number;
  gapIn?: number;
  fit?: PrintFit;
  backgroundRgb?: [number, number, number];
  sharpenAmount?: number;
  sharpenRadius?: number;
  iccProfile?: IccProfileChoice;
  iccProfilePath?: string;
}

/** Rendert `photoIds` (eines je Layout-Zelle) auf eine gemeinsame
 * Druckseite und schreibt sie als JPEG nach `destPath` — wiederverwendet
 * dieselbe Export-Engine wie {@link exportPhoto} (siehe
 * `apx_export::print`s Moduldoku). Kein System-Druckdialog in dieser
 * Phase, siehe ADR-0034. */
export function printPhotos(photoIds: string[], destPath: string, options: PrintLayoutOptions): Promise<ExportOutcomeDto> {
  return invoke<ExportOutcomeDto>("print_photos", { photoIds, destPath, options });
}

// ---- Diashow (Phase 8 Schritt 4) -------------------------------------------
//
// Übergänge/Ken-Burns-Effekt/Intro-Outro-Screens/Musik-Synchronisation
// laufen für die Live-Wiedergabe komplett im Frontend (siehe
// `lib/slideshow.ts`, `SlideshowPlayer.tsx`) — diese Commands decken nur
// den optionalen Video-Export ab (`apx_export::video`, `DECISIONS.md`
// ADR-0034).

export type SlideshowTransition = "cut" | "cross_fade";

export interface SlideshowTitleCardOptions {
  text: string;
  seconds: number;
  backgroundRgb: [number, number, number];
  textColor: [number, number, number];
  /** Fehlt nur, wenn `text` leer ist (reine Farbfläche). */
  fontPath?: string;
  fontSize?: number;
}

export interface SlideshowVideoOptions {
  slideSeconds: number;
  kenBurns: boolean;
  transition: SlideshowTransition;
  transitionSeconds?: number;
  width: number;
  height: number;
  fps: number;
  intro?: SlideshowTitleCardOptions;
  outro?: SlideshowTitleCardOptions;
  /** Beliebiges von `ffmpeg` unterstütztes Audioformat. */
  musicPath?: string;
}

export interface SlideshowVideoOutcomeDto {
  path: string;
  frame_count: number;
  duration_seconds: number;
}

/** Ob ein aufrufbares System-`ffmpeg` gefunden wurde — steuert, ob der
 * Video-Export-Knopf im Diashow-Dialog aktiv ist. */
export function checkFfmpegAvailable(): Promise<boolean> {
  return invoke<boolean>("check_ffmpeg_available");
}

/** Rendert `photoIds` (mit ihrem aktuellen Bearbeitungsstand, wie
 * {@link exportPhoto}) zu einer Diashow und kodiert sie über ein System-
 * `ffmpeg` als MP4 nach `destPath`. */
export function exportSlideshowVideo(
  photoIds: string[],
  destPath: string,
  options: SlideshowVideoOptions,
): Promise<SlideshowVideoOutcomeDto> {
  return invoke<SlideshowVideoOutcomeDto>("export_slideshow_video", { photoIds, destPath, options });
}
