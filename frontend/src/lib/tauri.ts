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
}

export interface CatalogStatusDto {
  catalog_path: string;
  folder_count: number;
  photo_count: number;
}

export interface PhotoDto {
  id: string;
  filename: string;
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
