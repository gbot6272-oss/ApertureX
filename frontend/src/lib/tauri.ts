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
  /** GPS-Koordinaten aus EXIF oder von Hand über die Kartenansicht gesetzt
   * (Phase 8 Schritt 7) — `null`, wenn kein Standort bekannt. */
  gps_lat: number | null;
  gps_lon: number | null;
  /** `null` = echtes Foto. Gesetzt = virtuelle Kopie (Phase 9 Schritt 1)
   * — teilt sich die Datei mit dem referenzierten Foto. */
  source_photo_id: string | null;
  /** IPTC-artige Metadaten-Überschreibungen (Phase 9 Schritt 2). */
  title: string | null;
  caption: string | null;
  copyright: string | null;
  creator: string | null;
}

export interface KeywordDto {
  id: string;
  name: string;
  /** `null` = Wurzel-Schlagwort (Phase 9 Schritt 2). */
  parent_id: string | null;
  synonyms: string[];
}

/** Bedingte Auto-Schlagwort-Regel (Phase 9 Schritt 2, siehe
 * `DECISIONS.md` ADR-0035). `conditions_json` ist derselbe
 * `PresetCondition[]`-Vertrag wie bei Import-Presets (`lib/presets.ts`). */
export interface TagRuleDto {
  id: string;
  name: string;
  keyword_id: string;
  conditions_json: string;
  enabled: boolean;
}

export interface CollectionDto {
  id: string;
  name: string;
  /** `null` = Sammlung liegt an der Wurzel (Phase 9 Schritt 1). */
  folder_id: string | null;
  is_smart: boolean;
  /** JSON-String (`FilterCriteriaDto`-Form), nur gesetzt bei `is_smart`. */
  smart_criteria_json: string | null;
}

// ---- Bibliotheks-Backlog (Phase 9 Schritt 1, siehe DECISIONS.md ADR-0032/ADR-0035) ----

export interface CollectionFolderDto {
  id: string;
  name: string;
  parent_id: string | null;
  position: number;
}

export interface StackDto {
  id: string;
  name: string | null;
  cover_photo_id: string | null;
  photo_ids: string[];
}

export interface ColorLabelDefinitionDto {
  name: string;
  display_name: string;
  hex: string;
  position: number;
}

export interface GeocodedLocationDto {
  name: string;
  admin1: string;
  country_code: string;
  distance_km: number;
}

export interface GpxTrackPointDto {
  lat: number;
  lon: number;
  elevation: number | null;
  time: string | null;
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

/** Ein Eintrag im vollständigen Bearbeitungsverlauf (Phase 9 Schritt 7,
 * „Zeitleisten-Ansicht"/„Verlaufs-Vergleich") — spiegelt
 * `apx_app::commands::EditHistoryEntryDto`. */
export interface EditHistoryEntryDto {
  sequence: number;
  label: string | null;
  edl_json: string;
  created_at: string;
}

export function listDevelopHistory(photoId: string): Promise<EditHistoryEntryDto[]> {
  return invoke<EditHistoryEntryDto[]>("list_develop_history", { photoId });
}

export function gotoDevelopEdit(photoId: string, sequence: number): Promise<HistoryPositionDto | null> {
  return invoke<HistoryPositionDto | null>("goto_develop_edit", { photoId, sequence });
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

export function listAllKeywords(): Promise<KeywordDto[]> {
  return invoke<KeywordDto[]>("list_all_keywords");
}

// ---- Bibliothek: Schlagworthierarchie, Tag-Regeln, Metadaten (ab Phase 9
// Schritt 2, siehe DECISIONS.md ADR-0035) ------------------------------------

export function setKeywordParent(keywordId: string, parentId: string | null): Promise<void> {
  return invoke<void>("set_keyword_parent", { keywordId, parentId });
}

export function setKeywordSynonyms(keywordId: string, synonyms: string[]): Promise<void> {
  return invoke<void>("set_keyword_synonyms", { keywordId, synonyms });
}

export function deleteKeyword(keywordId: string): Promise<void> {
  return invoke<void>("delete_keyword", { keywordId });
}

export function createTagRule(name: string, keywordId: string, conditionsJson: string): Promise<string> {
  return invoke<string>("create_tag_rule", { name, keywordId, conditionsJson });
}

export function setTagRuleEnabled(tagRuleId: string, enabled: boolean): Promise<void> {
  return invoke<void>("set_tag_rule_enabled", { tagRuleId, enabled });
}

export function deleteTagRule(tagRuleId: string): Promise<void> {
  return invoke<void>("delete_tag_rule", { tagRuleId });
}

export function listTagRules(): Promise<TagRuleDto[]> {
  return invoke<TagRuleDto[]>("list_tag_rules");
}

/** Stapel-Metadatenbearbeitung: der Aufrufer ruft dies für jedes Foto in
 * der Auswahl einzeln auf. */
export function setPhotoMetadata(
  photoId: string,
  title: string | null,
  caption: string | null,
  copyright: string | null,
  creator: string | null,
): Promise<void> {
  return invoke<void>("set_photo_metadata", { photoId, title, caption, copyright, creator });
}

/** Schreibt eine `.xmp`-Sidecar-Datei neben dem Original, gibt deren Pfad
 * zurück. Siehe `apx_export::xmp`s Moduldoku für den genauen Umfang
 * (Basic+HSL, kein Weißabgleich/Kurven/Masken). */
export function exportXmpSidecar(photoId: string, withDevelopSettings: boolean): Promise<string> {
  return invoke<string>("export_xmp_sidecar", { photoId, withDevelopSettings });
}

/** Liest die `crs:`-Entwickeln-Einstellungen aus `xmpContent` und
 * committet sie als neuen Bearbeitungsschritt für `photoId`. */
export function importXmpDevelopSettings(photoId: string, xmpContent: string): Promise<void> {
  return invoke<void>("import_xmp_develop_settings", { photoId, xmpContent });
}

/** Wie {@link importXmpDevelopSettings}, öffnet aber einen nativen
 * Datei-Dialog statt den Inhalt entgegenzunehmen — `false` = Dialog
 * abgebrochen. */
export function importXmpSidecarFromFile(photoId: string): Promise<boolean> {
  return invoke<boolean>("import_xmp_sidecar_from_file", { photoId });
}

// ---- Bibliothek: Sammlungen (ab Phase 3, Sammlungssätze/intelligente ------
// Sammlungen ab Phase 9 Schritt 1) -------------------------------------------

export function createCollection(name: string, folderId?: string): Promise<string> {
  return invoke<string>("create_collection", { name, folderId: folderId ?? null });
}

/** Siehe `apx_catalog::Catalog::create_smart_collection`s Moduldoku für
 * die Vereinfachung (flache UND-Verknüpfung statt verschachtelter Regeln). */
export function createSmartCollection(name: string, folderId: string | undefined, criteria: FilterCriteriaDto): Promise<string> {
  return invoke<string>("create_smart_collection", { name, folderId: folderId ?? null, criteria });
}

export function moveCollectionToFolder(collectionId: string, folderId: string | null): Promise<void> {
  return invoke<void>("move_collection_to_folder", { collectionId, folderId });
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

// ---- Bibliothek: Sammlungssätze (Phase 9 Schritt 1) ------------------------

export function createCollectionFolder(name: string, parentId?: string): Promise<string> {
  return invoke<string>("create_collection_folder", { name, parentId: parentId ?? null });
}

export function renameCollectionFolder(folderId: string, name: string): Promise<void> {
  return invoke<void>("rename_collection_folder", { folderId, name });
}

export function deleteCollectionFolder(folderId: string): Promise<void> {
  return invoke<void>("delete_collection_folder", { folderId });
}

export function listCollectionFolders(): Promise<CollectionFolderDto[]> {
  return invoke<CollectionFolderDto[]>("list_collection_folders");
}

// ---- Bibliothek: virtuelle Kopien (Phase 9 Schritt 1) ----------------------

export function createVirtualCopy(photoId: string): Promise<PhotoDto> {
  return invoke<PhotoDto>("create_virtual_copy", { photoId });
}

export function listVirtualCopies(photoId: string): Promise<PhotoDto[]> {
  return invoke<PhotoDto[]>("list_virtual_copies", { photoId });
}

// ---- Bibliothek: Stapel (Phase 9 Schritt 1) --------------------------------

export function createStack(name: string | undefined, photoIds: string[]): Promise<string> {
  return invoke<string>("create_stack", { name: name ?? null, photoIds });
}

export function deleteStack(stackId: string): Promise<void> {
  return invoke<void>("delete_stack", { stackId });
}

export function setStackCover(stackId: string, coverPhotoId: string): Promise<void> {
  return invoke<void>("set_stack_cover", { stackId, coverPhotoId });
}

export function listStacks(): Promise<StackDto[]> {
  return invoke<StackDto[]>("list_stacks");
}

/** Siehe `apx_catalog::Catalog::auto_stack_by_time`s Moduldoku. */
export function autoStackByTime(photoIds: string[], windowSeconds: number): Promise<string[]> {
  return invoke<string[]>("auto_stack_by_time", { photoIds, windowSeconds });
}

// ---- Bibliothek: erweiterbare Farbmarkierungen (Phase 9 Schritt 1) --------

export function listColorLabelDefinitions(): Promise<ColorLabelDefinitionDto[]> {
  return invoke<ColorLabelDefinitionDto[]>("list_color_label_definitions");
}

export function createColorLabelDefinition(name: string, displayName: string, hex: string): Promise<void> {
  return invoke<void>("create_color_label_definition", { name, displayName, hex });
}

export function deleteColorLabelDefinition(name: string): Promise<void> {
  return invoke<void>("delete_color_label_definition", { name });
}

// ---- Bibliothek: Perceptual-Hash-Duplikaterkennung (Phase 9 Schritt 1) -----

/** Siehe `apx-app`s `list_perceptual_duplicate_groups`-Command-Moduldoku
 * für die Vereinfachung (nur bereits generierte Miniaturansichten). */
export function listPerceptualDuplicateGroups(maxDistance: number): Promise<PhotoDto[][]> {
  return invoke<PhotoDto[][]>("list_perceptual_duplicate_groups", { maxDistance });
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

// ---- UI-Einstellungen (Phase 10 Schritt 1) --------------------------------

export type Theme = "dark" | "light";

export interface UiSettingsDto {
  theme: Theme;
  accent_color: string | null;
  locale: string;
  ui_scale_percent: number;
  high_contrast: boolean;
  reduced_motion: boolean;
  onboarding_seen: boolean;
}

export function getUiSettings(): Promise<UiSettingsDto> {
  return invoke<UiSettingsDto>("get_ui_settings");
}

export function setUiSettings(settings: UiSettingsDto): Promise<void> {
  return invoke<void>("set_ui_settings", { settings });
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

export type ExportFormat = "jpeg" | "png" | "tiff" | "webp" | "avif" | "psd" | "jxl";

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

// ---- Buch (Phase 8 Schritt 5) -----------------------------------------

export type BookPageTemplate = "full_bleed" | "two_side_by_side" | "grid_2x2" | "photo_with_caption";

export interface BookOptions {
  template: BookPageTemplate;
  pageWidthIn: number;
  pageHeightIn: number;
  dpi: number;
  marginIn?: number;
  fit?: "contain" | "cover";
  backgroundRgb?: [number, number, number];
  /** Name aus {@link PRINT_SHOP_PRESET_NAMES} — überschreibt dpi/backgroundRgb. */
  printShopPreset?: string;
  /** Titelseite voranstellen — braucht `fontPath`. */
  title?: string;
  /** Für Titelseite und `photo_with_caption`-Bildunterschriften (= Dateiname). */
  fontPath?: string;
}

export interface BookOutcomeDto {
  path: string;
  page_count: number;
  byte_size: number;
}

/** Namen der eingebauten Druckerei-Presets (`apx_export::book::PRINT_SHOP_PRESETS`). */
export const PRINT_SHOP_PRESET_NAMES = [
  "Digitaldruck (Standard, 300 dpi)",
  "Fotobuch (Premium, 400 dpi)",
  "Softcover (kein Beschnitt, 250 dpi)",
] as const;

/** Rendert `photoIds` (mit ihrem aktuellen Bearbeitungsstand, wie
 * {@link exportPhoto}) zu einem Fotobuch — automatische Befüllung gemäß
 * `options.template` — und schreibt es als mehrseitige PDF-Datei nach
 * `destPath`. */
export function exportBookPdf(photoIds: string[], destPath: string, options: BookOptions): Promise<BookOutcomeDto> {
  return invoke<BookOutcomeDto>("export_book_pdf", { photoIds, destPath, options });
}

// ---- Web (Phase 8 Schritt 6) -----------------------------------------

export type GalleryTheme = "light" | "dark" | "minimal";

export interface WebUploadOptions {
  /** `"ftp"`/`"sftp"`. */
  protocol: "ftp" | "sftp";
  host: string;
  port: number;
  username: string;
  password: string;
  remoteDir?: string;
}

export interface WebGalleryOptions {
  title: string;
  theme: GalleryTheme;
  maxEdge?: number;
  upload?: WebUploadOptions;
}

export interface WebGalleryOutcomeDto {
  dest_dir: string;
  photo_count: number;
  uploaded_count: number | null;
}

/** Rendert `photoIds` (mit ihrem aktuellen Bearbeitungsstand, wie
 * {@link exportPhoto}) zu einer statischen HTML-Galerie unter `destDir`
 * und lädt sie optional per FTP/SFTP hoch (`apx_export::web`). */
export function exportWebGallery(photoIds: string[], destDir: string, options: WebGalleryOptions): Promise<WebGalleryOutcomeDto> {
  return invoke<WebGalleryOutcomeDto>("export_web_gallery", { photoIds, destDir, options });
}

// ---- Karte (Phase 8 Schritt 7) -----------------------------------------
//
// GPS-Koordinaten selbst kommen über die normalen Foto-Listen-Aufrufe
// (`PhotoDto.gps_lat`/`gps_lon`, aus EXIF beim Import gelesen) — die
// folgenden Commands decken nur die Kartenansicht selbst ab.

/** Alle Fotos mit bekannten GPS-Koordinaten, ordnerübergreifend. */
export function listGeotaggedPhotos(): Promise<PhotoDto[]> {
  return invoke<PhotoDto[]>("list_geotagged_photos");
}

/** Vollständig offline Reverse-Geocoding (kein Netzwerkaufruf), siehe
 * `apx_export::map`s Moduldoku. */
export function reverseGeocodeLocation(lat: number, lon: number): Promise<GeocodedLocationDto> {
  return invoke<GeocodedLocationDto>("reverse_geocode_location", { lat, lon });
}

/** Liest und parst eine GPX-Datei (Pfad über {@link pickFilePath}) —
 * gibt alle Trackpunkte für die Reiserouten-Anzeige zurück. */
export function importGpxTrack(path: string): Promise<GpxTrackPointDto[]> {
  return invoke<GpxTrackPointDto[]>("import_gpx_track", { path });
}

/** Setzt oder löscht (beide `null`) die GPS-Koordinaten eines Fotos von
 * Hand — z. B. per Klick auf die Kartenansicht platziert. */
export function setPhotoGps(photoId: string, lat: number | null, lon: number | null): Promise<void> {
  return invoke<void>("set_photo_gps", { photoId, lat, lon });
}

// ---- Vorlagen (Phase 8 Schritt 8) --------------------------------------
//
// Eine generische Vorlage — `kind` ist eine der Zeichenketten "export"/
// "print"/"book"/"slideshow"/"web"/"workflow", `payload_json` das
// jeweilige `*Options`-DTO als JSON (für Export-/Layout-Vorlagen)
// beziehungsweise `{ presetId, exportOptions }` (für Workflow-Vorlagen,
// siehe {@link WorkflowTemplatePayload}).
export type TemplateKind = "export" | "print" | "book" | "slideshow" | "web" | "workflow" | "filter";

export interface TemplateDto {
  id: string;
  kind: string;
  name: string;
  payload_json: string;
  created_at: string;
}

export interface WorkflowTemplatePayload {
  presetId: string;
  exportOptions: ExportPhotoOptions;
}

export function saveTemplate(kind: TemplateKind, name: string, payloadJson: string): Promise<string> {
  return invoke<string>("save_template", { kind, name, payloadJson });
}

export function listTemplates(kind: TemplateKind): Promise<TemplateDto[]> {
  return invoke<TemplateDto[]>("list_templates", { kind });
}

export function deleteTemplate(templateId: string): Promise<void> {
  return invoke<void>("delete_template", { templateId });
}

/** Öffnet einen Speichern-Dialog und schreibt die Vorlage als `.apxt`-Datei
 * — das lokale Dateiformat-„Marktplatz"-Format aus `PLAN.md` Schritt 8
 * (kein Online-Hosting). `null`, wenn der Dialog abgebrochen wurde. */
export function exportTemplateToFile(templateId: string): Promise<string | null> {
  return invoke<string | null>("export_template_to_file", { templateId });
}

/** Öffnet einen Öffnen-Dialog und legt die gewählte `.apxt`-Datei als neue
 * Vorlage an ("Installation" einer lokal geteilten Vorlage). `null`, wenn
 * der Dialog abgebrochen wurde. */
export function importTemplateFromFile(): Promise<TemplateDto | null> {
  return invoke<TemplateDto | null>("import_template_from_file");
}

/** Einzelnes Foto per ID — u. a. für das sekundäre Display, das als
 * eigenes Fenster keinen Zugriff auf den Store des Hauptfensters hat. */
export function getPhoto(photoId: string): Promise<PhotoDto> {
  return invoke<PhotoDto>("get_photo", { photoId });
}

// ---- Bibliothek: Statistik, Vorschau-Cache (ab Phase 9 Schritt 3, siehe
// DECISIONS.md ADR-0035) ------------------------------------------------

export interface CatalogStatisticsDto {
  total_photos: number;
  total_file_size: number;
  earliest_captured_at: string | null;
  latest_captured_at: string | null;
  rating_distribution: [number, number][];
  top_camera_models: [string, number][];
  top_lenses: [string, number][];
}

export function catalogStatistics(): Promise<CatalogStatisticsDto> {
  return invoke<CatalogStatisticsDto>("catalog_statistics");
}

export interface PreviewCacheStatsDto {
  file_count: number;
  total_bytes: number;
}

export function previewCacheStats(): Promise<PreviewCacheStatsDto> {
  return invoke<PreviewCacheStatsDto>("preview_cache_stats");
}

/** Leert den Vorschau-Cache — Vorschauen werden bei Bedarf aus dem
 * Original neu generiert, kein Datenverlust. */
export function clearPreviewCache(): Promise<void> {
  return invoke<void>("clear_preview_cache");
}

/** Smart Previews (Phase 11 Schritt 4, siehe `DECISIONS.md` ADR-0038):
 * erzeugt je Foto eine feste, verkleinerte JPEG-Zwischendatei, die als
 * Fallback dient, wenn die Originaldatei später nicht erreichbar ist
 * (z. B. eine getrennte externe Festplatte) — ermöglicht eingeschränktes
 * Weiterarbeiten offline. Gibt die Zahl tatsächlich erzeugter Previews
 * zurück (überspringt Fotos, deren Original selbst schon nicht
 * erreichbar ist). */
export function generateSmartPreviews(photoIds: string[]): Promise<number> {
  return invoke<number>("generate_smart_previews", { photoIds });
}

// ---- Entwickeln: Entrauschung, Hochskalierung (ab Phase 9 Schritt 6, siehe
// DECISIONS.md ADR-0035) — klassische Algorithmen, keine echte
// Modellinferenz (dieselbe Ehrlichkeitslinie wie ADR-0033). -----------------

/** Entrauscht (kantenerhaltender Bilateral-Filter) und schreibt eine neue
 * PNG-Datei neben dem Original, gibt deren Pfad zurück. */
export function denoisePhoto(photoId: string, rangeSigma?: number): Promise<string> {
  return invoke<string>("denoise_photo", { photoId, rangeSigma: rangeSigma ?? null });
}

/** Skaliert auf das Doppelte hoch (kantengerichtete Interpolation) und
 * schreibt eine neue PNG-Datei neben dem Original, gibt deren Pfad
 * zurück. */
export function upscalePhoto(photoId: string): Promise<string> {
  return invoke<string>("upscale_photo", { photoId });
}

/** DNG-Konvertierung (Phase 11 Schritt 1) — schreibt eine „Linear DNG" aus
 * den unveränderten, kamera-nativen RAW-Daten neben das Original, gibt
 * deren Pfad zurück. */
export function convertPhotoToDng(photoId: string): Promise<string> {
  return invoke<string>("convert_photo_to_dng", { photoId });
}

// ---- Fortgeschrittenes: Fokus-/HDR-/Panorama-/Astro-Stacking (Phase 9
// Schritt 8, siehe DECISIONS.md ADR-0035 Punkt 2) — reine, deterministische
// Algorithmen in apx-stacking, keine externe Registrierungs-Bibliothek. ----

/** Ergebnis eines Stacking-Commands — spiegelt
 * `apx_app::commands::StackResultDto`. */
export interface StackResultDto {
  photo_id: string;
  stack_id: string;
  width: number;
  height: number;
}

/** Fokus-Stacking über bereits ausgerichtete Aufnahmen (Laplacian-
 * Schärfemaß, schärfste Quelle je Pixel). */
export function stackFocus(photoIds: string[]): Promise<StackResultDto> {
  return invoke<StackResultDto>("stack_focus", { photoIds });
}

/** HDR-Zusammenführung über eine Belichtungsreihe (jedes Foto braucht
 * eine EXIF-Belichtungszeit). */
export function stackHdr(photoIds: string[]): Promise<StackResultDto> {
  return invoke<StackResultDto>("stack_hdr", { photoIds });
}

/** Panorama-Zusammenführung — v1 nur Verschiebungs-Registrierung per
 * Phasenkorrelation (siehe `apx_stacking::panorama`s Moduldoku). */
export function stackPanorama(photoIds: string[]): Promise<StackResultDto> {
  return invoke<StackResultDto>("stack_panorama", { photoIds });
}

/** Astro-Stacking — Sigma-geclipptes Mittel über viele Kurzbelichtungen,
 * registriert per Phasenkorrelation. */
export function stackAstro(photoIds: string[], sigma?: number): Promise<StackResultDto> {
  return invoke<StackResultDto>("stack_astro", { photoIds, sigma: sigma ?? null });
}

// ---- Fortgeschrittenes: Skript-API (Rhai) + Plugin-System (Phase 9
// Schritt 9, siehe DECISIONS.md ADR-0035 Punkt 3) ---------------------------

/** Führt ein Rhai-Skript gegen den aktuellen Bearbeitungsstand aus
 * (schmale, primitiv-typisierte API — siehe `apx_script`s Moduldoku:
 * `edl.set_exposure(1.5)` statt ganze EDL-Structs) und committet das
 * Ergebnis. */
export function runDevelopScript(photoId: string, script: string): Promise<void> {
  return invoke<void>("run_develop_script", { photoId, script });
}

/** Lädt ein Plugin (`.so`/`.dylib`/`.dll`, ABI-Version wird hart geprüft)
 * und wendet dessen Custom-Effekt auf `photoId` an — schreibt eine neue
 * PNG-Datei neben dem Original, gibt deren Pfad zurück. */
export function runPluginCustomEffect(photoId: string, pluginPath: string, param: number): Promise<string> {
  return invoke<string>("run_plugin_custom_effect", { photoId, pluginPath, param });
}

// ---- Fortgeschrittenes: Kollaborationsmodus (Phase 9 Schritt 10, siehe
// DECISIONS.md ADR-0035 Punkt 4) ---------------------------------------------

export interface UnmatchedSharePhotoDto {
  filename: string;
  content_hash: string;
}

export interface ShareConflictDto {
  photo_id: string;
  filename: string;
  incoming_edl_json: string;
  prefer_incoming: boolean;
  local_edited_at: string;
  incoming_edited_at: string;
}

export interface ImportShareResultDto {
  name: string;
  unmatched: UnmatchedSharePhotoDto[];
  unchanged: string[];
  conflicts: ShareConflictDto[];
}

/** Öffnet einen Speichern-Dialog und schreibt die aktuellen Bearbeitungs-
 * stände von `photoIds` als `.apxs`-Datei (keine Pixel-Bytes, Matching über
 * `content_hash`). `null`, wenn der Dialog abgebrochen wurde. */
export function exportCatalogShare(photoIds: string[], name: string): Promise<string | null> {
  return invoke<string | null>("export_catalog_share", { photoIds, name });
}

/** Öffnet einen Öffnen-Dialog, liest eine `.apxs`-Datei und berechnet den
 * Abgleich gegen den lokalen Katalog — schreibt dabei nichts. `null`, wenn
 * der Dialog abgebrochen wurde. */
export function importCatalogShare(): Promise<ImportShareResultDto | null> {
  return invoke<ImportShareResultDto | null>("import_catalog_share", {});
}

/** Löst einen einzelnen Konflikt aus [`ImportShareResultDto.conflicts`]
 * auf: `"mine"` (nichts tun), `"theirs"` (importierten Stand committen)
 * oder `"virtual_copy"` (als neue virtuelle Kopie behalten). */
export function resolveShareConflict(
  photoId: string,
  incomingEdlJson: string,
  resolution: "mine" | "theirs" | "virtual_copy",
): Promise<void> {
  return invoke<void>("resolve_share_conflict", { photoId, incomingEdlJson, resolution });
}

// ---- Fortgeschrittenes: Tethered Shooting (Phase 9 Schritt 11, siehe
// DECISIONS.md ADR-0035 Punkt 5) ---------------------------------------------

export interface CameraInfoDto {
  model: string;
  port: string;
  /** true, wenn dieser Build ohne das `tethering`-Feature läuft (oder keine
   * echte Kamera gefunden wurde) — zeigt eine Simulation statt echter
   * Hardware. */
  simulated: boolean;
}

/** (Neu-)Verbindet zu einer Kamera und erkennt sie. `null`, wenn keine
 * gefunden wurde. */
export function tetherConnect(): Promise<CameraInfoDto | null> {
  return invoke<CameraInfoDto | null>("tether_connect", {});
}

/** Löst über die verbundene Kamera aus, lädt herunter und importiert über
 * den bestehenden Import-Pfad — optional mit einem benannten Import-Preset
 * (Phase 3/5). `null`, wenn die heruntergeladene Datei nicht neu war. */
export function tetherCapture(presetName?: string): Promise<PhotoDto | null> {
  return invoke<PhotoDto | null>("tether_capture", { presetName: presetName ?? null });
}
