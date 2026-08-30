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
