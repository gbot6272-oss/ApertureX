//! Tauri-Commands: die einzige Schnittstelle, über die das Frontend mit
//! dem Backend spricht (siehe `ARCHITECTURE.md` Abschnitt 4 — Frontend
//! kennt kein SQL, keine Dateisystempfade, keine Bildpuffer). Fehler
//! werden für Phase 1 als `String` an das Frontend gereicht; eine
//! strukturierte Fehler-DTO kommt bei Bedarf in einer späteren Phase.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, State};
use tauri_plugin_dialog::DialogExt;

use crate::state::AppState;

#[derive(Debug, Clone, Serialize)]
pub struct FolderDto {
    pub id: String,
    pub path: String,
    pub photo_count: u64,
    /// `None` bei einem Wurzelordner — sonst die `FolderId` des
    /// übergeordneten Ordners, Grundlage für die Baumdarstellung im
    /// Sidebar (siehe `PLAN.md` Phase 3, Schritt 5).
    pub parent_id: Option<String>,
    /// `true`, wenn `path` im Dateisystem nicht mehr existiert (Ordner
    /// wurde außerhalb der App verschoben/gelöscht) — analog zu
    /// `PhotoDto::missing`, aber pro Aufruf live geprüft statt in der
    /// Datenbank persistiert (kein extra Reconcile-Schritt beim
    /// App-Start nötig).
    pub missing: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct CatalogStatusDto {
    pub catalog_path: String,
    pub folder_count: usize,
    pub photo_count: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct PhotoDto {
    pub id: String,
    pub filename: String,
    /// Dateigröße in Byte — Grundlage für die Sortierung nach Dateigröße
    /// im Frontend (Schritt 8.3, `DECISIONS.md` ADR-0027).
    pub file_size: u64,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub camera_make: Option<String>,
    pub camera_model: Option<String>,
    pub lens: Option<String>,
    pub iso: Option<u32>,
    pub aperture: Option<f32>,
    pub shutter: Option<f32>,
    pub focal_length: Option<f32>,
    /// RFC-3339-Zeitstempel, falls bekannt — das Frontend parst ihn mit
    /// `new Date(...)`. Siehe `apx_raw::RawMetadata::captured_at` für die
    /// Zeitzonen-Annahme, wenn EXIF keinen Offset trug.
    pub captured_at: Option<String>,
    pub missing: bool,
    /// Sternebewertung 0–5, siehe `apx_catalog::Photo::rating`.
    pub rating: u8,
    /// Pick/Reject-Flagge: 1 = Pick, -1 = Reject, 0 = keine.
    pub flag: i8,
    pub color_label: Option<String>,
}

impl From<apx_catalog::Photo> for PhotoDto {
    fn from(photo: apx_catalog::Photo) -> Self {
        Self {
            id: photo.id.to_string(),
            filename: photo.filename,
            file_size: photo.file_size,
            width: photo.width,
            height: photo.height,
            camera_make: photo.camera_make,
            camera_model: photo.camera_model,
            lens: photo.lens,
            iso: photo.iso,
            aperture: photo.aperture,
            shutter: photo.shutter,
            focal_length: photo.focal_length,
            captured_at: photo.captured_at.and_then(|dt| {
                dt.format(&time::format_description::well_known::Rfc3339)
                    .ok()
            }),
            missing: photo.missing,
            rating: photo.rating,
            flag: photo.flag,
            color_label: photo.color_label,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct KeywordDto {
    pub id: String,
    pub name: String,
}

impl From<apx_catalog::Keyword> for KeywordDto {
    fn from(keyword: apx_catalog::Keyword) -> Self {
        Self {
            id: keyword.id.to_string(),
            name: keyword.name,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct CollectionDto {
    pub id: String,
    pub name: String,
}

impl From<apx_catalog::Collection> for CollectionDto {
    fn from(collection: apx_catalog::Collection) -> Self {
        Self {
            id: collection.id.to_string(),
            name: collection.name,
        }
    }
}

// ---- Presets (ab Phase 5, siehe DECISIONS.md ADR-0031) ---------------------

#[derive(Debug, Clone, Serialize)]
pub struct PresetFolderDto {
    pub id: String,
    pub name: String,
    pub parent_id: Option<String>,
    pub position: i64,
}

impl From<apx_catalog::PresetFolder> for PresetFolderDto {
    fn from(folder: apx_catalog::PresetFolder) -> Self {
        Self {
            id: folder.id.to_string(),
            name: folder.name,
            parent_id: folder.parent_id.map(|id| id.to_string()),
            position: folder.position,
        }
    }
}

/// Metadaten eines Presets, ohne seine EDL-Teilmenge — die lebt in
/// [`PresetVersionDto`] (siehe `apx_catalog::repository::presets`s
/// Moduldoku: ein Preset kann mehrere Versionen haben, nur die aktuellste
/// zählt normalerweise).
#[derive(Debug, Clone, Serialize)]
pub struct PresetDto {
    pub id: String,
    pub folder_id: Option<String>,
    pub name: String,
    pub is_favorite: bool,
    pub tags: Vec<String>,
    /// Bedingungsregeln (Feld/Operator/Wert, UND-verknüpft) als JSON-
    /// String — für `apx-app`/`apx-catalog` opak, nur das Frontend kennt
    /// die Struktur (siehe `DECISIONS.md` ADR-0031 Punkt 4).
    pub conditions_json: String,
    pub created_at: String,
}

impl From<apx_catalog::Preset> for PresetDto {
    fn from(preset: apx_catalog::Preset) -> Self {
        Self {
            id: preset.id.to_string(),
            folder_id: preset.folder_id.map(|id| id.to_string()),
            name: preset.name,
            is_favorite: preset.is_favorite,
            tags: preset.tags,
            conditions_json: preset.conditions_json,
            created_at: preset
                .created_at
                .format(&time::format_description::well_known::Rfc3339)
                .unwrap_or_default(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct PresetVersionDto {
    pub id: String,
    pub preset_id: String,
    pub sequence: i64,
    /// Die EDL-Teilmenge — für `apx-app`/`apx-catalog` ein opaker JSON-
    /// String, nur das Frontend (`lib/presets.ts`) kennt seine Struktur
    /// (ein `Partial<EdlPayload>`-artiges Objekt).
    pub edl_subset_json: String,
    pub created_at: String,
}

impl From<apx_catalog::PresetVersion> for PresetVersionDto {
    fn from(version: apx_catalog::PresetVersion) -> Self {
        Self {
            id: version.id.to_string(),
            preset_id: version.preset_id.to_string(),
            sequence: version.sequence,
            edl_subset_json: version.edl_subset_json,
            created_at: version
                .created_at
                .format(&time::format_description::well_known::Rfc3339)
                .unwrap_or_default(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct CreatePresetResultDto {
    pub preset_id: String,
    pub version_id: String,
}

/// Eigenes Dateiformat für Preset-Im-/Export (`DECISIONS.md` ADR-0031
/// Punkt 3 — kein Adobe-`.xmp`/`.lrtemplate`). `edl_subset`/`conditions`
/// sind hier bewusst eingebettetes JSON (`serde_json::Value`) statt
/// eines noch einmal String-kodierten Strings — eine `.apx`-Datei soll
/// bei Bedarf lesbar sein, kein doppelt escapetes JSON-in-JSON.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ApxPresetFile {
    schema_version: u32,
    name: String,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default = "default_json_array")]
    conditions: serde_json::Value,
    edl_subset: serde_json::Value,
}

fn default_json_array() -> serde_json::Value {
    serde_json::Value::Array(Vec::new())
}

const APX_PRESET_SCHEMA_VERSION: u32 = 1;

/// Eingabe für [`filter_photos`] — spiegelt `apx_catalog::FilterCriteria`,
/// aber mit `#[serde(default)]`-Feldern, damit das Frontend nur die
/// tatsächlich gesetzten Filter mitschicken muss.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct FilterCriteriaDto {
    #[serde(default)]
    pub rating_at_least: Option<u8>,
    #[serde(default)]
    pub flag: Option<i8>,
    #[serde(default)]
    pub color_label: Option<String>,
    #[serde(default)]
    pub camera_model: Option<String>,
}

impl From<FilterCriteriaDto> for apx_catalog::FilterCriteria {
    fn from(dto: FilterCriteriaDto) -> Self {
        Self {
            rating_at_least: dto.rating_at_least,
            flag: dto.flag,
            color_label: dto.color_label,
            camera_model: dto.camera_model,
        }
    }
}

/// Öffnet den nativen Ordner-Auswahldialog. Gibt `None` zurück, wenn der
/// Nutzer den Dialog ohne Auswahl schließt (kein Fehler).
#[tauri::command]
pub async fn select_folder(app: tauri::AppHandle) -> Result<Option<String>, String> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    app.dialog().file().pick_folder(move |folder| {
        // Empfangsseite kann bereits verworfen sein, falls das Fenster
        // inzwischen geschlossen wurde — send() ignoriert das dann still.
        let _ = tx.send(folder);
    });
    let picked = rx
        .await
        .map_err(|err| format!("Ordnerauswahl fehlgeschlagen: {err}"))?;
    Ok(picked.map(|path| path.to_string()))
}

/// Katalogpfad sowie Ordner-/Fotoanzahl — dient in Phase 1 als
/// Smoke-Test, dass Frontend, Tauri-Commands und `apx-catalog` korrekt
/// verdrahtet sind. Der Import-Befehl selbst kommt in Schritt 6.
#[tauri::command]
pub fn catalog_status(state: State<'_, AppState>) -> Result<CatalogStatusDto, String> {
    let folders = state
        .catalog
        .list_folders()
        .map_err(|err| err.to_string())?;
    let mut photo_count = 0u64;
    for folder in &folders {
        photo_count += state
            .catalog
            .count_photos_in_folder(folder.id)
            .map_err(|err| err.to_string())?;
    }
    Ok(CatalogStatusDto {
        catalog_path: state
            .paths
            .default_catalog_file()
            .to_string_lossy()
            .to_string(),
        folder_count: folders.len(),
        photo_count,
    })
}

/// Listet alle bekannten Ordner mit Fotoanzahl — Grundlage für den
/// Ordnerbaum im Frontend (voller Ausbau folgt in Phase 3).
#[tauri::command]
pub fn list_folders(state: State<'_, AppState>) -> Result<Vec<FolderDto>, String> {
    let folders = state
        .catalog
        .list_folders()
        .map_err(|err| err.to_string())?;
    folders
        .into_iter()
        .map(|folder| {
            let photo_count = state
                .catalog
                .count_photos_in_folder(folder.id)
                .map_err(|err| err.to_string())?;
            Ok(FolderDto {
                id: folder.id.to_string(),
                missing: !folder.path.exists(),
                path: folder.path.to_string_lossy().to_string(),
                photo_count,
                parent_id: folder.parent_id.map(|id| id.to_string()),
            })
        })
        .collect()
}

/// Verknüpft einen als fehlend erkannten Ordner mit seinem neuen
/// Speicherort (z. B. nach Verschieben/Umbenennen im Dateisystem) und
/// gleicht danach die zugehörigen Fotos gegen den neuen Pfad ab (siehe
/// `crate::reconcile`) — derselbe Mechanismus, der auch beim regulären
/// Öffnen eines Ordners läuft (`list_photos_in_folder`).
#[tauri::command]
pub fn relink_folder(
    state: State<'_, AppState>,
    folder_id: String,
    new_path: String,
) -> Result<(), String> {
    let folder_id: apx_core::FolderId = folder_id
        .parse()
        .map_err(|err: apx_core::AppError| err.to_string())?;
    let new_path = PathBuf::from(new_path);
    if !new_path.is_dir() {
        return Err(format!("'{}' ist kein Verzeichnis", new_path.display()));
    }

    state
        .catalog
        .relink_folder(folder_id, &new_path)
        .map_err(|err| err.to_string())?;

    let photos = state
        .catalog
        .list_photos_by_folder(folder_id)
        .map_err(|err| err.to_string())?;
    crate::reconcile::reconcile_missing(&state.catalog, &new_path, photos)
        .map_err(|err| err.to_string())?;
    Ok(())
}

/// Startet den Import-Job für `path` im Hintergrund und kehrt sofort
/// zurück — Fortschritt läuft über die Events `import:progress`,
/// `import:error`, `import:finished` (siehe `import`-Modul). Es kann
/// jeweils nur ein Import gleichzeitig laufen. Immer im
/// Add-in-Place-Modus — siehe [`import_folder_with_mode`] für Copy/Move.
#[tauri::command]
pub async fn import_folder(
    app: AppHandle,
    state: State<'_, AppState>,
    path: String,
) -> Result<(), String> {
    start_import(
        app,
        state,
        path,
        crate::import::ImportMode::AddInPlace,
        None,
    )
    .await
}

/// Eingabe für [`import_folder_with_mode`] — spiegelt
/// `crate::import::ImportMode`, aber mit `String`-Pfaden (Tauri-IPC kennt
/// kein `PathBuf` direkt) und getaggt nach `kind`, analog zu
/// `HistoryPositionDto`.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "kind")]
pub enum ImportModeDto {
    AddInPlace,
    Copy { target_dir: String },
    Move { target_dir: String },
}

impl From<ImportModeDto> for crate::import::ImportMode {
    fn from(dto: ImportModeDto) -> Self {
        match dto {
            ImportModeDto::AddInPlace => crate::import::ImportMode::AddInPlace,
            ImportModeDto::Copy { target_dir } => {
                crate::import::ImportMode::Copy(PathBuf::from(target_dir))
            }
            ImportModeDto::Move { target_dir } => {
                crate::import::ImportMode::Move(PathBuf::from(target_dir))
            }
        }
    }
}

/// Wie [`import_folder`], aber mit wählbarem Import-Modus (Kopieren/
/// Verschieben in einen Zielordner, optional mit Umbenennungsmuster) —
/// siehe `DECISIONS.md` ADR-0025. Additiv zu `import_folder`, das
/// unverändert den bisherigen Add-in-Place-Ablauf ohne die neuen
/// Parameter anbietet (Rückwärtskompatibilität zum bestehenden
/// Frontend-Aufruf, siehe `PLAN.md` Phase 3, Schritt 4).
#[tauri::command]
pub async fn import_folder_with_mode(
    app: AppHandle,
    state: State<'_, AppState>,
    path: String,
    mode: ImportModeDto,
    rename_pattern: Option<String>,
) -> Result<(), String> {
    start_import(app, state, path, mode.into(), rename_pattern).await
}

async fn start_import(
    app: AppHandle,
    state: State<'_, AppState>,
    path: String,
    mode: crate::import::ImportMode,
    rename_pattern: Option<String>,
) -> Result<(), String> {
    let folder = PathBuf::from(&path);
    if !folder.is_dir() {
        return Err(format!("'{path}' ist kein Verzeichnis"));
    }

    let cancel_token = {
        let mut guard = state
            .active_import
            .lock()
            .map_err(|_| "Import-Status ist blockiert (vergiftete Sperre)".to_string())?;
        if guard.is_some() {
            return Err("Es läuft bereits ein Import".to_string());
        }
        let token = tokio_util::sync::CancellationToken::new();
        *guard = Some(token.clone());
        token
    };

    let catalog = state.catalog.clone();
    let cache_root = state.paths.preview_cache_dir();
    let active_import = state.active_import.clone();
    let app_for_blocking = app.clone();
    let cancel_for_blocking = cancel_token.clone();

    tauri::async_runtime::spawn(async move {
        let join_result = tokio::task::spawn_blocking(move || {
            let events = crate::import::TauriEvents(&app_for_blocking);
            crate::import::run_with_mode(
                &events,
                &catalog,
                &cache_root,
                &folder,
                &cancel_for_blocking,
                &mode,
                rename_pattern.as_deref(),
            );
        })
        .await;

        if let Ok(mut guard) = active_import.lock() {
            *guard = None;
        }

        if let Err(join_err) = join_result {
            tracing::error!(error = %join_err, "Import-Task ist abgestürzt");
        }
    });

    Ok(())
}

/// Bricht einen laufenden Import ab. Kein Fehler, wenn gerade keiner
/// läuft — das ist ein harmloser Wettlauf (Import ist zwischen
/// Frontend-Klick und Ankunft dieses Commands bereits fertig geworden).
#[tauri::command]
pub fn cancel_import(state: State<'_, AppState>) -> Result<(), String> {
    let guard = state
        .active_import
        .lock()
        .map_err(|_| "Import-Status ist blockiert (vergiftete Sperre)".to_string())?;
    if let Some(token) = guard.as_ref() {
        token.cancel();
    }
    Ok(())
}

/// Listet alle Fotos eines Ordners — Grundlage für Filmstreifen und
/// Viewer.
///
/// Gleicht dabei den `missing`-Status jedes Fotos mit der tatsächlichen
/// Dateisystem-Existenz ab (siehe `crate::reconcile` und
/// `PHASE1_PROMPT.md` Abschnitt 9, Akzeptanzkriterium 8): Wurde eine
/// Datei außerhalb der App gelöscht, wird sie beim nächsten Öffnen dieses
/// Ordners als `missing` markiert, ohne dass die App abstürzt.
#[tauri::command]
pub fn list_photos_in_folder(
    state: State<'_, AppState>,
    folder_id: String,
) -> Result<Vec<PhotoDto>, String> {
    let folder_id: apx_core::FolderId = folder_id
        .parse()
        .map_err(|err: apx_core::AppError| err.to_string())?;
    let folder = state
        .catalog
        .get_folder(folder_id)
        .map_err(|err| err.to_string())?;
    let photos = state
        .catalog
        .list_photos_by_folder(folder_id)
        .map_err(|err| err.to_string())?;
    let photos = crate::reconcile::reconcile_missing(&state.catalog, &folder.path, photos)
        .map_err(|err| err.to_string())?;
    Ok(photos.into_iter().map(PhotoDto::from).collect())
}

// ---- Entwickeln-Verlauf (ab Phase 2) --------------------------------

/// Der aktuelle Bearbeitungsstand eines Fotos fürs Frontend — entweder
/// noch nie bearbeitet (`Neutral`) oder mit dem zuletzt aktiven EDL als
/// JSON (dasselbe Format, das auch die `develop/...`-Protokoll-Route
/// erwartet, siehe `crate::protocol`).
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind")]
pub enum HistoryPositionDto {
    Neutral,
    At { edl_json: String },
}

fn history_position_to_dto(
    position: apx_catalog::HistoryPosition,
) -> Result<HistoryPositionDto, String> {
    match position {
        apx_catalog::HistoryPosition::Neutral => Ok(HistoryPositionDto::Neutral),
        apx_catalog::HistoryPosition::At(entry) => Ok(HistoryPositionDto::At {
            edl_json: entry.edl.to_json_string().map_err(|err| err.to_string())?,
        }),
    }
}

fn parse_photo_id(photo_id: String) -> Result<apx_core::PhotoId, String> {
    photo_id
        .parse()
        .map_err(|err: apx_core::AppError| err.to_string())
}

fn parse_keyword_id(keyword_id: String) -> Result<apx_core::KeywordId, String> {
    keyword_id
        .parse()
        .map_err(|err: apx_core::AppError| err.to_string())
}

fn parse_collection_id(collection_id: String) -> Result<apx_core::CollectionId, String> {
    collection_id
        .parse()
        .map_err(|err: apx_core::AppError| err.to_string())
}

fn parse_preset_folder_id(id: String) -> Result<apx_core::PresetFolderId, String> {
    id.parse()
        .map_err(|err: apx_core::AppError| err.to_string())
}

fn parse_preset_id(id: String) -> Result<apx_core::PresetId, String> {
    id.parse()
        .map_err(|err: apx_core::AppError| err.to_string())
}

/// Committet `edl_json` als neuen, aktiven Bearbeitungsschritt für
/// `photo_id` — ausgelöst beim Loslassen eines Reglers, nicht bei jedem
/// Zwischenwert (siehe `PLAN.md` Phase 2 Schritt 5). Validiert die
/// Nutzlast vor dem Schreiben (lehnt kaputtes JSON und unbekannte
/// EDL-Schema-Versionen ab), damit der Katalog nie einen unlesbaren
/// Verlaufs-Eintrag bekommt.
#[tauri::command]
pub fn apply_develop_edit(
    state: State<'_, AppState>,
    photo_id: String,
    edl_json: String,
    label: Option<String>,
) -> Result<(), String> {
    let photo_id = parse_photo_id(photo_id)?;
    let envelope =
        apx_core::EdlEnvelope::from_json_str(&edl_json).map_err(|err| err.to_string())?;
    apx_pipeline::edl::from_envelope(&envelope).map_err(|err| err.to_string())?;

    state
        .catalog
        .commit_edit(photo_id, &envelope, label.as_deref())
        .map_err(|err| err.to_string())?;
    Ok(())
}

/// Der aktuell aktive Bearbeitungsstand — wird beim Öffnen eines Fotos im
/// Entwickeln-Modul geladen, damit die Regler den zuletzt gespeicherten
/// Zustand zeigen (siehe `SPEC.md` §7: „EDL nach Neustart identisch
/// reproduzierbar").
#[tauri::command]
pub fn current_develop_edit(
    state: State<'_, AppState>,
    photo_id: String,
) -> Result<HistoryPositionDto, String> {
    let photo_id = parse_photo_id(photo_id)?;
    let position = state
        .catalog
        .current_edit(photo_id)
        .map_err(|err| err.to_string())?;
    history_position_to_dto(position)
}

/// Geht einen Bearbeitungsschritt zurück. `None`, wenn schon am
/// Ausgangszustand (kein Rückgängig möglich) — kein Fehler, siehe
/// `apx_catalog::Catalog::undo_edit`.
#[tauri::command]
pub fn undo_develop_edit(
    state: State<'_, AppState>,
    photo_id: String,
) -> Result<Option<HistoryPositionDto>, String> {
    let photo_id = parse_photo_id(photo_id)?;
    let result = state
        .catalog
        .undo_edit(photo_id)
        .map_err(|err| err.to_string())?;
    result.map(history_position_to_dto).transpose()
}

/// Geht einen Bearbeitungsschritt vor. `None`, wenn nichts zu wiederholen
/// ist, siehe `apx_catalog::Catalog::redo_edit`.
#[tauri::command]
pub fn redo_develop_edit(
    state: State<'_, AppState>,
    photo_id: String,
) -> Result<Option<HistoryPositionDto>, String> {
    let photo_id = parse_photo_id(photo_id)?;
    let result = state
        .catalog
        .redo_edit(photo_id)
        .map_err(|err| err.to_string())?;
    result.map(history_position_to_dto).transpose()
}

// ---- Schnappschüsse (Phase 6 Schritt 8) ------------------------------------
//
// Anders als der lineare Verlauf oben (`apply_develop_edit`/
// `current_develop_edit`/…): ein Schnappschuss trägt seine eigene Kopie
// des EDL (siehe `apx_catalog::repository::snapshots`s Moduldoku) und
// bleibt bestehen, auch wenn spätere Bearbeitungen den linearen Verlauf
// umschreiben. Ihn "anzuwenden" heißt einfach: sein `edl_json` wie jeden
// anderen EDL-Stand über das bestehende `apply_develop_edit` committen
// — kein eigener Restore-Befehl nötig (reuse statt Duplikat).

fn parse_snapshot_id(id: String) -> Result<apx_core::SnapshotId, String> {
    id.parse()
        .map_err(|err: apx_core::AppError| err.to_string())
}

#[derive(Debug, Clone, Serialize)]
pub struct SnapshotDto {
    pub id: String,
    pub name: String,
    pub edl_json: String,
    pub created_at: String,
}

impl SnapshotDto {
    fn try_from_model(snapshot: apx_catalog::Snapshot) -> Result<Self, String> {
        Ok(Self {
            id: snapshot.id.to_string(),
            name: snapshot.name,
            edl_json: snapshot
                .edl
                .to_json_string()
                .map_err(|err| err.to_string())?,
            created_at: snapshot
                .created_at
                .format(&time::format_description::well_known::Rfc3339)
                .unwrap_or_default(),
        })
    }
}

/// Legt einen neuen Schnappschuss des aktuell übergebenen EDL an —
/// validiert `edl_json` genau wie `apply_develop_edit`, damit der
/// Katalog nie einen unlesbaren Schnappschuss bekommt.
#[tauri::command]
pub fn create_snapshot(
    state: State<'_, AppState>,
    photo_id: String,
    name: String,
    edl_json: String,
) -> Result<(), String> {
    let photo_id = parse_photo_id(photo_id)?;
    let envelope =
        apx_core::EdlEnvelope::from_json_str(&edl_json).map_err(|err| err.to_string())?;
    apx_pipeline::edl::from_envelope(&envelope).map_err(|err| err.to_string())?;
    state
        .catalog
        .create_snapshot(photo_id, &name, &envelope)
        .map_err(|err| err.to_string())?;
    Ok(())
}

/// Alle Schnappschüsse eines Fotos, älteste zuerst.
#[tauri::command]
pub fn list_snapshots(
    state: State<'_, AppState>,
    photo_id: String,
) -> Result<Vec<SnapshotDto>, String> {
    let photo_id = parse_photo_id(photo_id)?;
    state
        .catalog
        .list_snapshots(photo_id)
        .map_err(|err| err.to_string())?
        .into_iter()
        .map(SnapshotDto::try_from_model)
        .collect()
}

#[tauri::command]
pub fn rename_snapshot(
    state: State<'_, AppState>,
    snapshot_id: String,
    name: String,
) -> Result<(), String> {
    let snapshot_id = parse_snapshot_id(snapshot_id)?;
    state
        .catalog
        .rename_snapshot(snapshot_id, &name)
        .map_err(|err| err.to_string())
}

#[tauri::command]
pub fn delete_snapshot(state: State<'_, AppState>, snapshot_id: String) -> Result<(), String> {
    let snapshot_id = parse_snapshot_id(snapshot_id)?;
    state
        .catalog
        .delete_snapshot(snapshot_id)
        .map_err(|err| err.to_string())
}

// ---- Bibliothek: Import-Presets (ab Phase 3) -------------------------------
//
// Presets werden bewusst direkt als `import::presets::ImportPreset`
// durchgereicht statt über eine eigene Dto zu laufen — anders als
// `Photo`/`Folder` ist das kein Katalog-Datenmodell mit eigener Historie,
// sondern eine reine Import-Werkzeug-Konfiguration, die 1:1 dem Frontend
// entspricht (siehe `PLAN.md` Phase 3, Schritt 4).

#[tauri::command]
pub fn list_import_presets(
    state: State<'_, AppState>,
) -> Result<Vec<crate::import::presets::ImportPreset>, String> {
    crate::import::presets::load_presets(&state.paths.import_presets_file())
}

#[tauri::command]
pub fn save_import_preset(
    state: State<'_, AppState>,
    preset: crate::import::presets::ImportPreset,
) -> Result<Vec<crate::import::presets::ImportPreset>, String> {
    crate::import::presets::upsert_preset(&state.paths.import_presets_file(), preset)
}

#[tauri::command]
pub fn delete_import_preset(
    state: State<'_, AppState>,
    name: String,
) -> Result<Vec<crate::import::presets::ImportPreset>, String> {
    crate::import::presets::delete_preset(&state.paths.import_presets_file(), &name)
}

// ---- Bibliothek: Bewertung/Flagge/Farbe (ab Phase 3) -----------------------

#[tauri::command]
pub fn set_photo_rating(
    state: State<'_, AppState>,
    photo_id: String,
    rating: u8,
) -> Result<(), String> {
    let photo_id = parse_photo_id(photo_id)?;
    state
        .catalog
        .set_photo_rating(photo_id, rating)
        .map_err(|err| err.to_string())
}

#[tauri::command]
pub fn set_photo_flag(
    state: State<'_, AppState>,
    photo_id: String,
    flag: i8,
) -> Result<(), String> {
    let photo_id = parse_photo_id(photo_id)?;
    state
        .catalog
        .set_photo_flag(photo_id, flag)
        .map_err(|err| err.to_string())
}

#[tauri::command]
pub fn set_photo_color_label(
    state: State<'_, AppState>,
    photo_id: String,
    color_label: Option<String>,
) -> Result<(), String> {
    let photo_id = parse_photo_id(photo_id)?;
    state
        .catalog
        .set_photo_color_label(photo_id, color_label.as_deref())
        .map_err(|err| err.to_string())
}

// ---- Bibliothek: Schlagworte (ab Phase 3) ----------------------------------

#[tauri::command]
pub fn add_photo_keyword(
    state: State<'_, AppState>,
    photo_id: String,
    name: String,
) -> Result<String, String> {
    let photo_id = parse_photo_id(photo_id)?;
    let keyword_id = state
        .catalog
        .add_keyword(photo_id, &name)
        .map_err(|err| err.to_string())?;
    Ok(keyword_id.to_string())
}

#[tauri::command]
pub fn remove_photo_keyword(
    state: State<'_, AppState>,
    photo_id: String,
    keyword_id: String,
) -> Result<(), String> {
    let photo_id = parse_photo_id(photo_id)?;
    let keyword_id = parse_keyword_id(keyword_id)?;
    state
        .catalog
        .remove_keyword(photo_id, keyword_id)
        .map_err(|err| err.to_string())
}

#[tauri::command]
pub fn list_photo_keywords(
    state: State<'_, AppState>,
    photo_id: String,
) -> Result<Vec<KeywordDto>, String> {
    let photo_id = parse_photo_id(photo_id)?;
    let keywords = state
        .catalog
        .list_keywords_for_photo(photo_id)
        .map_err(|err| err.to_string())?;
    Ok(keywords.into_iter().map(KeywordDto::from).collect())
}

#[tauri::command]
pub fn list_all_keywords(state: State<'_, AppState>) -> Result<Vec<KeywordDto>, String> {
    let keywords = state
        .catalog
        .list_all_keywords()
        .map_err(|err| err.to_string())?;
    Ok(keywords.into_iter().map(KeywordDto::from).collect())
}

// ---- Bibliothek: Sammlungen (ab Phase 3) -----------------------------------

#[tauri::command]
pub fn create_collection(state: State<'_, AppState>, name: String) -> Result<String, String> {
    let id = state
        .catalog
        .create_collection(&name)
        .map_err(|err| err.to_string())?;
    Ok(id.to_string())
}

#[tauri::command]
pub fn rename_collection(
    state: State<'_, AppState>,
    collection_id: String,
    name: String,
) -> Result<(), String> {
    let collection_id = parse_collection_id(collection_id)?;
    state
        .catalog
        .rename_collection(collection_id, &name)
        .map_err(|err| err.to_string())
}

#[tauri::command]
pub fn delete_collection(state: State<'_, AppState>, collection_id: String) -> Result<(), String> {
    let collection_id = parse_collection_id(collection_id)?;
    state
        .catalog
        .delete_collection(collection_id)
        .map_err(|err| err.to_string())
}

#[tauri::command]
pub fn list_collections(state: State<'_, AppState>) -> Result<Vec<CollectionDto>, String> {
    let collections = state
        .catalog
        .list_collections()
        .map_err(|err| err.to_string())?;
    Ok(collections.into_iter().map(CollectionDto::from).collect())
}

#[tauri::command]
pub fn add_to_collection(
    state: State<'_, AppState>,
    collection_id: String,
    photo_id: String,
) -> Result<(), String> {
    let collection_id = parse_collection_id(collection_id)?;
    let photo_id = parse_photo_id(photo_id)?;
    state
        .catalog
        .add_photo_to_collection(collection_id, photo_id)
        .map_err(|err| err.to_string())
}

#[tauri::command]
pub fn remove_from_collection(
    state: State<'_, AppState>,
    collection_id: String,
    photo_id: String,
) -> Result<(), String> {
    let collection_id = parse_collection_id(collection_id)?;
    let photo_id = parse_photo_id(photo_id)?;
    state
        .catalog
        .remove_photo_from_collection(collection_id, photo_id)
        .map_err(|err| err.to_string())
}

#[tauri::command]
pub fn list_photos_in_collection(
    state: State<'_, AppState>,
    collection_id: String,
) -> Result<Vec<PhotoDto>, String> {
    let collection_id = parse_collection_id(collection_id)?;
    let photos = state
        .catalog
        .list_photos_in_collection(collection_id)
        .map_err(|err| err.to_string())?;
    Ok(photos.into_iter().map(PhotoDto::from).collect())
}

// ---- Presets (ab Phase 5, siehe DECISIONS.md ADR-0031) ---------------------

#[tauri::command]
pub fn create_preset_folder(
    state: State<'_, AppState>,
    name: String,
    parent_id: Option<String>,
) -> Result<String, String> {
    let parent_id = parent_id.map(parse_preset_folder_id).transpose()?;
    let id = state
        .catalog
        .create_preset_folder(&name, parent_id)
        .map_err(|err| err.to_string())?;
    Ok(id.to_string())
}

#[tauri::command]
pub fn rename_preset_folder(
    state: State<'_, AppState>,
    folder_id: String,
    name: String,
) -> Result<(), String> {
    let folder_id = parse_preset_folder_id(folder_id)?;
    state
        .catalog
        .rename_preset_folder(folder_id, &name)
        .map_err(|err| err.to_string())
}

#[tauri::command]
pub fn delete_preset_folder(state: State<'_, AppState>, folder_id: String) -> Result<(), String> {
    let folder_id = parse_preset_folder_id(folder_id)?;
    state
        .catalog
        .delete_preset_folder(folder_id)
        .map_err(|err| err.to_string())
}

#[tauri::command]
pub fn list_preset_folders(state: State<'_, AppState>) -> Result<Vec<PresetFolderDto>, String> {
    let folders = state
        .catalog
        .list_preset_folders()
        .map_err(|err| err.to_string())?;
    Ok(folders.into_iter().map(PresetFolderDto::from).collect())
}

#[tauri::command]
pub fn create_preset(
    state: State<'_, AppState>,
    folder_id: Option<String>,
    name: String,
    tags: Vec<String>,
    conditions_json: String,
    edl_subset_json: String,
) -> Result<CreatePresetResultDto, String> {
    let folder_id = folder_id.map(parse_preset_folder_id).transpose()?;
    let (preset_id, version_id) = state
        .catalog
        .create_preset(folder_id, &name, &tags, &conditions_json, &edl_subset_json)
        .map_err(|err| err.to_string())?;
    Ok(CreatePresetResultDto {
        preset_id: preset_id.to_string(),
        version_id: version_id.to_string(),
    })
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub fn update_preset_metadata(
    state: State<'_, AppState>,
    preset_id: String,
    folder_id: Option<String>,
    name: String,
    tags: Vec<String>,
    conditions_json: String,
) -> Result<(), String> {
    let preset_id = parse_preset_id(preset_id)?;
    let folder_id = folder_id.map(parse_preset_folder_id).transpose()?;
    state
        .catalog
        .update_preset_metadata(preset_id, folder_id, &name, &tags, &conditions_json)
        .map_err(|err| err.to_string())
}

#[tauri::command]
pub fn set_preset_favorite(
    state: State<'_, AppState>,
    preset_id: String,
    is_favorite: bool,
) -> Result<(), String> {
    let preset_id = parse_preset_id(preset_id)?;
    state
        .catalog
        .set_preset_favorite(preset_id, is_favorite)
        .map_err(|err| err.to_string())
}

#[tauri::command]
pub fn delete_preset(state: State<'_, AppState>, preset_id: String) -> Result<(), String> {
    let preset_id = parse_preset_id(preset_id)?;
    state
        .catalog
        .delete_preset(preset_id)
        .map_err(|err| err.to_string())
}

#[tauri::command]
pub fn list_presets(state: State<'_, AppState>) -> Result<Vec<PresetDto>, String> {
    let presets = state
        .catalog
        .list_presets()
        .map_err(|err| err.to_string())?;
    Ok(presets.into_iter().map(PresetDto::from).collect())
}

/// Legt eine neue Version an (siehe `repository::presets`s Moduldoku:
/// ältere Versionen bleiben erhalten, nur die höchste `sequence` zählt
/// als aktuell).
#[tauri::command]
pub fn add_preset_version(
    state: State<'_, AppState>,
    preset_id: String,
    edl_subset_json: String,
) -> Result<String, String> {
    let preset_id = parse_preset_id(preset_id)?;
    let version_id = state
        .catalog
        .add_preset_version(preset_id, &edl_subset_json)
        .map_err(|err| err.to_string())?;
    Ok(version_id.to_string())
}

#[tauri::command]
pub fn list_preset_versions(
    state: State<'_, AppState>,
    preset_id: String,
) -> Result<Vec<PresetVersionDto>, String> {
    let preset_id = parse_preset_id(preset_id)?;
    let versions = state
        .catalog
        .list_preset_versions(preset_id)
        .map_err(|err| err.to_string())?;
    Ok(versions.into_iter().map(PresetVersionDto::from).collect())
}

#[tauri::command]
pub fn latest_preset_version(
    state: State<'_, AppState>,
    preset_id: String,
) -> Result<PresetVersionDto, String> {
    let preset_id = parse_preset_id(preset_id)?;
    let version = state
        .catalog
        .latest_preset_version(preset_id)
        .map_err(|err| err.to_string())?;
    Ok(PresetVersionDto::from(version))
}

/// Schreibt die aktuellste Version von `preset_id` als eigenes
/// `.apx`-JSON-Format in eine vom Nutzer gewählte Datei (siehe
/// `DECISIONS.md` ADR-0031 Punkt 3 — kein Adobe-Format). `Ok(None)`, wenn
/// der Dateidialog abgebrochen wurde.
#[tauri::command]
pub async fn export_preset_to_apx_file(
    app: AppHandle,
    state: State<'_, AppState>,
    preset_id: String,
) -> Result<Option<String>, String> {
    let preset_id = parse_preset_id(preset_id)?;
    let preset = state
        .catalog
        .list_presets()
        .map_err(|err| err.to_string())?
        .into_iter()
        .find(|p| p.id == preset_id)
        .ok_or_else(|| "Preset nicht gefunden".to_string())?;
    let version = state
        .catalog
        .latest_preset_version(preset_id)
        .map_err(|err| err.to_string())?;

    let file = ApxPresetFile {
        schema_version: APX_PRESET_SCHEMA_VERSION,
        name: preset.name.clone(),
        tags: preset.tags,
        conditions: serde_json::from_str(&preset.conditions_json)
            .unwrap_or(serde_json::Value::Array(Vec::new())),
        edl_subset: serde_json::from_str(&version.edl_subset_json)
            .map_err(|err| format!("Preset-EDL-Teilmenge ist kein gültiges JSON: {err}"))?,
    };
    let json = serde_json::to_string_pretty(&file)
        .map_err(|err| format!("Preset nicht serialisierbar: {err}"))?;

    let (tx, rx) = tokio::sync::oneshot::channel();
    app.dialog()
        .file()
        .add_filter("Aperture X Preset", &["apx"])
        .set_file_name(format!("{}.apx", preset.name))
        .save_file(move |path| {
            let _ = tx.send(path);
        });
    let picked = rx
        .await
        .map_err(|err| format!("Speichern-Dialog fehlgeschlagen: {err}"))?;
    let Some(picked) = picked else {
        return Ok(None);
    };
    let path = picked
        .into_path()
        .map_err(|err| format!("Ungültiger Pfad: {err}"))?;
    std::fs::write(&path, json)
        .map_err(|err| format!("Datei '{}' nicht schreibbar: {err}", path.display()))?;
    Ok(Some(path.display().to_string()))
}

/// Liest eine `.apx`-Datei und legt sie als neues Preset in `folder_id`
/// an. `Ok(None)`, wenn der Dateidialog abgebrochen wurde.
#[tauri::command]
pub async fn import_preset_from_apx_file(
    app: AppHandle,
    state: State<'_, AppState>,
    folder_id: Option<String>,
) -> Result<Option<PresetDto>, String> {
    let folder_id = folder_id.map(parse_preset_folder_id).transpose()?;

    let (tx, rx) = tokio::sync::oneshot::channel();
    app.dialog()
        .file()
        .add_filter("Aperture X Preset", &["apx"])
        .pick_file(move |path| {
            let _ = tx.send(path);
        });
    let picked = rx
        .await
        .map_err(|err| format!("Öffnen-Dialog fehlgeschlagen: {err}"))?;
    let Some(picked) = picked else {
        return Ok(None);
    };
    let path = picked
        .into_path()
        .map_err(|err| format!("Ungültiger Pfad: {err}"))?;
    let text = std::fs::read_to_string(&path)
        .map_err(|err| format!("Datei '{}' nicht lesbar: {err}", path.display()))?;
    let file: ApxPresetFile = serde_json::from_str(&text).map_err(|err| {
        format!(
            "Datei '{}' ist keine gültige .apx-Datei: {err}",
            path.display()
        )
    })?;
    if file.schema_version > APX_PRESET_SCHEMA_VERSION {
        return Err(format!(
            "Datei '{}' hat Schema-Version {}, diese Aperture-X-Version kennt nur {}",
            path.display(),
            file.schema_version,
            APX_PRESET_SCHEMA_VERSION
        ));
    }

    let conditions_json =
        serde_json::to_string(&file.conditions).unwrap_or_else(|_| "[]".to_string());
    let edl_subset_json = serde_json::to_string(&file.edl_subset)
        .map_err(|err| format!("EDL-Teilmenge nicht serialisierbar: {err}"))?;
    let (preset_id, _) = state
        .catalog
        .create_preset(
            folder_id,
            &file.name,
            &file.tags,
            &conditions_json,
            &edl_subset_json,
        )
        .map_err(|err| err.to_string())?;
    let preset = state
        .catalog
        .list_presets()
        .map_err(|err| err.to_string())?
        .into_iter()
        .find(|p| p.id == preset_id)
        .ok_or_else(|| "Preset nach dem Anlegen nicht gefunden".to_string())?;
    Ok(Some(PresetDto::from(preset)))
}

// ---- Bibliothek: Suche/Filter (ab Phase 3) ---------------------------------

#[tauri::command]
pub fn search_photos(state: State<'_, AppState>, query: String) -> Result<Vec<PhotoDto>, String> {
    let photos = state
        .catalog
        .search_photos(&query)
        .map_err(|err| err.to_string())?;
    Ok(photos.into_iter().map(PhotoDto::from).collect())
}

#[tauri::command]
pub fn filter_photos(
    state: State<'_, AppState>,
    criteria: FilterCriteriaDto,
) -> Result<Vec<PhotoDto>, String> {
    let photos = state
        .catalog
        .filter_photos(&criteria.into())
        .map_err(|err| err.to_string())?;
    Ok(photos.into_iter().map(PhotoDto::from).collect())
}

/// Kombiniert Volltextsuche (optional) und Attributfilter per UND — additiv
/// zu [`search_photos`]/[`filter_photos`], die unverändert bestehen bleiben
/// (siehe `DECISIONS.md` ADR-0027). `query` als leerer String oder `None`
/// wirkt wie kein Suchtext.
#[tauri::command]
pub fn search_and_filter_photos(
    state: State<'_, AppState>,
    query: Option<String>,
    criteria: FilterCriteriaDto,
) -> Result<Vec<PhotoDto>, String> {
    let photos = state
        .catalog
        .search_and_filter_photos(query.as_deref(), &criteria.into())
        .map_err(|err| err.to_string())?;
    Ok(photos.into_iter().map(PhotoDto::from).collect())
}

// ---- Bibliothek: Duplikaterkennung (ab Phase 3, Schritt 8.2) --------------

/// Gruppen von Fotos mit identischem Inhalt (exakter Hash-Vergleich), siehe
/// `DECISIONS.md` ADR-0027 — reine Anzeige, verhindert den Import selbst
/// nicht.
#[tauri::command]
pub fn list_duplicate_photo_groups(
    state: State<'_, AppState>,
) -> Result<Vec<Vec<PhotoDto>>, String> {
    let groups = state
        .catalog
        .list_duplicate_photo_groups()
        .map_err(|err| err.to_string())?;
    Ok(groups
        .into_iter()
        .map(|group| group.into_iter().map(PhotoDto::from).collect())
        .collect())
}

// ---- KI-Funktionen (Phase 7, siehe `DECISIONS.md` ADR-0033) ---------------
//
// Alle Analyse-Algorithmen selbst leben in `apx-ai` (klassische
// Bildverarbeitungsheuristiken statt echter ONNX-Modellinferenz, siehe
// dessen Moduldoku) — diese Commands sind reine Verdrahtung: Foto-Pfad
// auflösen, über den bestehenden `TileCache` dekodieren (derselbe Weg wie
// `protocol::compute_develop`), Ergebnis als DTO zurückreichen.

fn resolve_source_path_for_ai(
    catalog: &apx_catalog::Catalog,
    photo_id: apx_core::PhotoId,
) -> Result<PathBuf, String> {
    let photo = catalog.get_photo(photo_id).map_err(|err| err.to_string())?;
    let folder = catalog
        .get_folder(photo.folder_id)
        .map_err(|err| err.to_string())?;
    Ok(folder.path.join(photo.filename))
}

fn parse_ai_mask_kind(kind: &str) -> Result<apx_pipeline::edl::AiMaskKind, String> {
    match kind {
        "subject" => Ok(apx_pipeline::edl::AiMaskKind::Subject),
        "sky" => Ok(apx_pipeline::edl::AiMaskKind::Sky),
        "background" => Ok(apx_pipeline::edl::AiMaskKind::Background),
        "click_region" => Ok(apx_pipeline::edl::AiMaskKind::ClickRegion),
        "person" => Ok(apx_pipeline::edl::AiMaskKind::Person),
        other => Err(format!("unbekannte KI-Maskenart '{other}'")),
    }
}

fn ai_mask_kind_to_string(kind: apx_pipeline::edl::AiMaskKind) -> String {
    match kind {
        apx_pipeline::edl::AiMaskKind::Subject => "subject",
        apx_pipeline::edl::AiMaskKind::Sky => "sky",
        apx_pipeline::edl::AiMaskKind::Background => "background",
        apx_pipeline::edl::AiMaskKind::ClickRegion => "click_region",
        apx_pipeline::edl::AiMaskKind::Person => "person",
    }
    .to_string()
}

#[derive(Debug, Clone, Serialize)]
pub struct AiMaskAlphaDto {
    pub kind: String,
    pub width: u32,
    pub height: u32,
    /// Base64-kodierte Ein-Kanal-`u8`-Alpha-Bitmap, `width * height` Bytes
    /// nach dem Dekodieren — direkt als `MaskGeometry::AiGenerated.alpha`
    /// im Frontend übernehmbar.
    pub alpha_base64: String,
}

/// Erzeugt eine KI-Masken-Alpha-Bitmap für `photo_id` (siehe
/// `apx_ai::segmentation`). `click_x`/`click_y` (normierte
/// Bildkoordinaten) sind nur für `kind == "click_region"` Pflicht,
/// `tolerance` nur dafür relevant (Vorgabe `0.15`, wenn nicht gesetzt).
#[tauri::command]
pub fn generate_ai_mask(
    state: State<'_, AppState>,
    photo_id: String,
    kind: String,
    click_x: Option<f32>,
    click_y: Option<f32>,
    tolerance: Option<f32>,
) -> Result<AiMaskAlphaDto, String> {
    let photo_id = parse_photo_id(photo_id)?;
    let kind = parse_ai_mask_kind(&kind)?;
    let source_path = resolve_source_path_for_ai(&state.catalog, photo_id)?;

    let max_edge = Some(apx_ai::segmentation::ANALYSIS_MAX_EDGE);
    let linear = state
        .tile_cache
        .get_or_decode(photo_id, max_edge, || {
            apx_raw::decode_linear(&source_path, max_edge)
        })
        .map_err(|err| err.to_string())?;

    let click = match (click_x, click_y) {
        (Some(x), Some(y)) => Some((x, y)),
        _ => None,
    };
    let alpha = apx_ai::segmentation::generate(
        kind,
        &linear.pixels,
        linear.width,
        linear.height,
        click,
        tolerance.unwrap_or(0.15),
    )
    .map_err(|err| err.to_string())?;

    Ok(AiMaskAlphaDto {
        kind: ai_mask_kind_to_string(kind),
        width: linear.width,
        height: linear.height,
        alpha_base64: base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &alpha),
    })
}

#[derive(Debug, Clone, Serialize)]
pub struct RepairSourceSuggestionDto {
    pub x: f32,
    pub y: f32,
}

/// Auto-Quellenfindung (`apx_ai::repair_analysis::suggest_source_point`) —
/// schlägt für einen geplanten Klon-/Reparatur-Strich einen Quellpunkt
/// vor, ohne selbst etwas zu committen (der Nutzer kann den Vorschlag im
/// Frontend noch verwerfen/verschieben).
#[tauri::command]
pub fn suggest_repair_source(
    state: State<'_, AppState>,
    photo_id: String,
    target_x: f32,
    target_y: f32,
    brush_radius: f32,
) -> Result<RepairSourceSuggestionDto, String> {
    let photo_id = parse_photo_id(photo_id)?;
    let source_path = resolve_source_path_for_ai(&state.catalog, photo_id)?;

    let max_edge = Some(apx_ai::segmentation::ANALYSIS_MAX_EDGE);
    let linear = state
        .tile_cache
        .get_or_decode(photo_id, max_edge, || {
            apx_raw::decode_linear(&source_path, max_edge)
        })
        .map_err(|err| err.to_string())?;

    let (x, y) = apx_ai::repair_analysis::suggest_source_point(
        &linear.pixels,
        linear.width,
        linear.height,
        target_x,
        target_y,
        brush_radius,
    )
    .map_err(|err| err.to_string())?;
    Ok(RepairSourceSuggestionDto { x, y })
}

#[derive(Debug, Clone, Serialize)]
pub struct SpotCandidateDto {
    pub x: f32,
    pub y: f32,
    pub radius: f32,
    pub strength: f32,
}

impl From<apx_ai::repair_analysis::SpotCandidate> for SpotCandidateDto {
    fn from(candidate: apx_ai::repair_analysis::SpotCandidate) -> Self {
        Self {
            x: candidate.x,
            y: candidate.y,
            radius: candidate.radius,
            strength: candidate.strength,
        }
    }
}

/// Sensorflecken-Visualisierung (`apx_ai::repair_analysis::detect_spots`)
/// — reine Analyse, legt selbst keine Reparatur-Striche an. `max_spots`
/// deckelt die Ergebnisliste, `sensitivity` (`0.0..=1.0`) die
/// Erkennungsschwelle.
#[tauri::command]
pub fn detect_sensor_spots(
    state: State<'_, AppState>,
    photo_id: String,
    sensitivity: f32,
    max_spots: u32,
) -> Result<Vec<SpotCandidateDto>, String> {
    let photo_id = parse_photo_id(photo_id)?;
    let source_path = resolve_source_path_for_ai(&state.catalog, photo_id)?;

    let max_edge = Some(apx_ai::segmentation::ANALYSIS_MAX_EDGE);
    let linear = state
        .tile_cache
        .get_or_decode(photo_id, max_edge, || {
            apx_raw::decode_linear(&source_path, max_edge)
        })
        .map_err(|err| err.to_string())?;

    let spots = apx_ai::repair_analysis::detect_spots(
        &linear.pixels,
        linear.width,
        linear.height,
        sensitivity,
        max_spots as usize,
    )
    .map_err(|err| err.to_string())?;
    Ok(spots.into_iter().map(SpotCandidateDto::from).collect())
}

// ---- KI: Einstellungen (Phase 7 Schritt 4) ---------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiSettingsDto {
    /// Liegt im Klartext in der Einstellungsdatei — dieselbe
    /// Vertrauensgrenze wie z. B. `last_opened_catalog`, siehe
    /// `apx_core::settings::AiSettings`s Moduldoku. Wird dem Frontend
    /// unverändert zurückgegeben, damit das Eingabefeld den hinterlegten
    /// Schlüssel zur Kontrolle/Bearbeitung zeigen kann (maskiert per
    /// `type="password"`, nicht serverseitig verborgen — ein lokaler,
    /// nicht synchronisierter Einzelnutzer-Schlüssel).
    pub anthropic_api_key: Option<String>,
}

#[tauri::command]
pub fn get_ai_settings(state: State<'_, AppState>) -> Result<AiSettingsDto, String> {
    let settings = apx_core::Settings::load_or_default(&state.paths.settings_file())
        .map_err(|err| err.to_string())?;
    Ok(AiSettingsDto {
        anthropic_api_key: settings.ai.anthropic_api_key,
    })
}

/// `None`/leerer String löscht den hinterlegten Schlüssel.
#[tauri::command]
pub fn set_anthropic_api_key(
    state: State<'_, AppState>,
    api_key: Option<String>,
) -> Result<(), String> {
    let path = state.paths.settings_file();
    let mut settings = apx_core::Settings::load_or_default(&path).map_err(|err| err.to_string())?;
    settings.ai.anthropic_api_key = api_key.filter(|key| !key.trim().is_empty());
    settings.save(&path).map_err(|err| err.to_string())
}

// ---- KI: Preset-Generator (Phase 7 Schritt 4) ------------------------------
//
// Alle vier Erzeugungsarten liefern eine EDL-Teilmenge als JSON-String
// zurück — dasselbe Format wie `PresetVersionDto::edl_subset_json` — statt
// direkt ein Preset anzulegen: das Frontend zeigt das Ergebnis erst in
// einer Vorschau, der Nutzer entscheidet per `create_preset`/
// `add_preset_version` (bestehende Commands aus Phase 5), ob er es
// tatsächlich übernimmt.

/// LLM-Modus: `description` ist die Freitextbeschreibung des gewünschten
/// Looks. Braucht einen hinterlegten Anthropic-API-Schlüssel (siehe
/// [`get_ai_settings`]).
#[tauri::command]
pub async fn generate_preset_from_llm(
    state: State<'_, AppState>,
    description: String,
) -> Result<String, String> {
    let settings = apx_core::Settings::load_or_default(&state.paths.settings_file())
        .map_err(|err| err.to_string())?;
    let api_key = settings
        .ai
        .anthropic_api_key
        .filter(|key| !key.trim().is_empty())
        .ok_or_else(|| apx_ai::AiError::MissingApiKey.to_string())?;
    let subset = apx_ai::preset_generator::generate_from_llm(&api_key, &description)
        .await
        .map_err(|err| err.to_string())?;
    serde_json::to_string(&subset)
        .map_err(|err| format!("Preset-Teilmenge nicht serialisierbar: {err}"))
}

/// **Manueller LLM-Modus ohne API-Schlüssel:** liefert einen fertigen
/// Prompt-Text (System-Prompt + `description` zu einer Nachricht
/// zusammengefügt) zum Einfügen in die **Claude-App** (claude.ai) — die
/// Antwort dort kommt über [`import_preset_json`] zurück. Kein Netzwerk-
/// Aufruf, keine Einstellungen nötig.
#[tauri::command]
pub fn build_preset_prompt_text(description: String) -> String {
    apx_ai::preset_generator::standalone_prompt_text(&description)
}

/// Validiert ein von Hand eingefügtes JSON-Ergebnis (aus der Claude-App
/// kopiert, siehe [`build_preset_prompt_text`]) serverseitig — dieselbe
/// Prüfung wie [`generate_preset_from_llm`]s Antwort, nur ohne den
/// API-Aufruf selbst.
#[tauri::command]
pub fn import_preset_json(json: String) -> Result<String, String> {
    let subset = apx_ai::preset_generator::parse_and_validate_pasted_json(&json)
        .map_err(|err| err.to_string())?;
    serde_json::to_string(&subset)
        .map_err(|err| format!("Preset-Teilmenge nicht serialisierbar: {err}"))
}

/// Referenzbild-Modus: öffnet einen Datei-Auswahldialog für ein beliebiges
/// Bild (RAW oder JPEG/PNG/TIFF, siehe `apx_raw::decode_linear`s
/// Fallback-Pfad) und gleicht die sieben Tonwertregler von `photo_id`
/// daran an. `None`, wenn der Dialog abgebrochen wurde — kein LLM, kein
/// API-Schlüssel nötig.
#[tauri::command]
pub async fn generate_preset_from_reference(
    app: AppHandle,
    state: State<'_, AppState>,
    photo_id: String,
) -> Result<Option<String>, String> {
    let photo_id = parse_photo_id(photo_id)?;

    let (tx, rx) = tokio::sync::oneshot::channel();
    app.dialog()
        .file()
        .add_filter("Bilder", &["jpg", "jpeg", "png", "tif", "tiff"])
        .pick_file(move |path| {
            let _ = tx.send(path);
        });
    let picked = rx
        .await
        .map_err(|err| format!("Öffnen-Dialog fehlgeschlagen: {err}"))?;
    let Some(picked) = picked else {
        return Ok(None);
    };
    let reference_path = picked
        .into_path()
        .map_err(|err| format!("Ungültiger Pfad: {err}"))?;

    let source_path = resolve_source_path_for_ai(&state.catalog, photo_id)?;
    let max_edge = Some(apx_ai::segmentation::ANALYSIS_MAX_EDGE);
    let source = state
        .tile_cache
        .get_or_decode(photo_id, max_edge, || {
            apx_raw::decode_linear(&source_path, max_edge)
        })
        .map_err(|err| err.to_string())?;
    // Kein Tile-Cache für das Referenzbild — eine beliebige externe Datei,
    // deren Wiederverwendung sich nicht lohnt (anders als das Foto selbst,
    // das über mehrere Regler-Ticks hinweg im Cache bleibt).
    let reference =
        apx_raw::decode_linear(&reference_path, max_edge).map_err(|err| err.to_string())?;

    let subset = apx_ai::preset_generator::generate_from_reference(
        &source.pixels,
        source.width,
        source.height,
        &reference.pixels,
        reference.width,
        reference.height,
    )
    .map_err(|err| err.to_string())?;
    let json = serde_json::to_string(&subset)
        .map_err(|err| format!("Preset-Teilmenge nicht serialisierbar: {err}"))?;
    Ok(Some(json))
}

/// Variationen-Generator: erzeugt `count` deterministisch geseedete
/// kleine Störungen von `edl_subset_json` — derselbe `seed` liefert immer
/// dieselben Varianten (Kontaktbogen-Vorschau im Frontend).
#[tauri::command]
pub fn generate_preset_variations(
    edl_subset_json: String,
    count: u32,
    seed: u64,
) -> Result<Vec<String>, String> {
    let base: serde_json::Value = serde_json::from_str(&edl_subset_json)
        .map_err(|err| format!("EDL-Teilmenge ist kein gültiges JSON: {err}"))?;
    let variations = apx_ai::preset_generator::generate_variations(&base, count as usize, seed)
        .map_err(|err| err.to_string())?;
    variations
        .iter()
        .map(|variation| {
            serde_json::to_string(variation)
                .map_err(|err| format!("Variante nicht serialisierbar: {err}"))
        })
        .collect()
}

/// Preset aus Bearbeitung lernen: mittelt die genannten `sections` über
/// den *aktuell committeten* Bearbeitungsstand (`Catalog::current_edit`,
/// nicht die gerade im Frontend offene Live-Vorschau) mehrerer
/// ausgewählter Fotos.
#[tauri::command]
pub fn learn_preset_from_photos(
    state: State<'_, AppState>,
    photo_ids: Vec<String>,
    sections: Vec<String>,
) -> Result<String, String> {
    let photo_ids: Vec<apx_core::PhotoId> = photo_ids
        .into_iter()
        .map(parse_photo_id)
        .collect::<Result<Vec<_>, String>>()?;

    let mut subsets = Vec::with_capacity(photo_ids.len());
    for photo_id in photo_ids {
        let edl = match state
            .catalog
            .current_edit(photo_id)
            .map_err(|err| err.to_string())?
        {
            apx_catalog::HistoryPosition::Neutral => apx_pipeline::edl::EdlV3::neutral(),
            apx_catalog::HistoryPosition::At(entry) => {
                apx_pipeline::edl::from_envelope(&entry.edl).map_err(|err| err.to_string())?
            }
        };
        let full =
            serde_json::to_value(&edl).map_err(|err| format!("EDL nicht serialisierbar: {err}"))?;
        let mut subset = serde_json::Map::new();
        if let serde_json::Value::Object(map) = &full {
            for key in &sections {
                if let Some(value) = map.get(key) {
                    subset.insert(key.clone(), value.clone());
                }
            }
        }
        subsets.push(serde_json::Value::Object(subset));
    }

    let averaged =
        apx_ai::preset_generator::average_subsets(&subsets).map_err(|err| err.to_string())?;
    serde_json::to_string(&averaged)
        .map_err(|err| format!("Preset-Teilmenge nicht serialisierbar: {err}"))
}

// ---- KI: Auto-Tagging (Phase 7 Schritt 5) ----------------------------------

/// Schlagwort-Vorschläge für `photo_id` (`apx_ai::tagging`) — schreibt
/// nichts in den Katalog, das Frontend übernimmt ausgewählte Vorschläge
/// über das bestehende `add_photo_keyword` (Phase 3).
#[tauri::command]
pub fn suggest_tags(state: State<'_, AppState>, photo_id: String) -> Result<Vec<String>, String> {
    let photo_id = parse_photo_id(photo_id)?;
    let photo = state
        .catalog
        .get_photo(photo_id)
        .map_err(|err| err.to_string())?;
    let source_path = resolve_source_path_for_ai(&state.catalog, photo_id)?;

    let max_edge = Some(apx_ai::segmentation::ANALYSIS_MAX_EDGE);
    let linear = state
        .tile_cache
        .get_or_decode(photo_id, max_edge, || {
            apx_raw::decode_linear(&source_path, max_edge)
        })
        .map_err(|err| err.to_string())?;

    let exif = apx_ai::tagging::TagExifInput {
        iso: photo.iso,
        aperture: photo.aperture,
        focal_length: photo.focal_length,
    };
    apx_ai::tagging::suggest_tags(&linear.pixels, linear.width, linear.height, &exif)
        .map_err(|err| err.to_string())
}

// ---- Export (Phase 8, siehe `DECISIONS.md` ADR-0034) ----------------------
//
// Reine Verdrahtung: aktuellen EDL-Stand auflösen (dieselbe Logik wie
// `current_develop_edit`), Foto-Pfad auflösen (wie `resolve_source_path_for_ai`),
// `apx_export::engine` aufrufen, Ergebnis als DTO zurückreichen. Die
// eigentliche Render-/Kodier-/Größenlogik lebt komplett in `apx-export`.

/// Generischer Datei-Auswahldialog, der nur den gewählten Pfad zurückgibt
/// (nichts liest/parst) — fürs Exportdialog-Grundgerüst (ICC-Profil,
/// Wasserzeichen-Schriftdatei/-Bild). `None`, wenn abgebrochen.
#[tauri::command]
pub async fn pick_file_path(
    app: AppHandle,
    filter_name: String,
    extensions: Vec<String>,
) -> Result<Option<String>, String> {
    let extension_refs: Vec<&str> = extensions.iter().map(String::as_str).collect();
    let (tx, rx) = tokio::sync::oneshot::channel();
    app.dialog()
        .file()
        .add_filter(&filter_name, &extension_refs)
        .pick_file(move |path| {
            let _ = tx.send(path);
        });
    let picked = rx
        .await
        .map_err(|err| format!("Öffnen-Dialog fehlgeschlagen: {err}"))?;
    Ok(picked.map(|p| p.to_string()))
}

/// Generischer Speichern-unter-Dialog (Drucken/Buch — eine fertige Datei
/// statt eines Zielordners). `None`, wenn abgebrochen.
#[tauri::command]
pub async fn pick_save_file_path(
    app: AppHandle,
    filter_name: String,
    extensions: Vec<String>,
    default_file_name: String,
) -> Result<Option<String>, String> {
    let extension_refs: Vec<&str> = extensions.iter().map(String::as_str).collect();
    let (tx, rx) = tokio::sync::oneshot::channel();
    app.dialog()
        .file()
        .add_filter(&filter_name, &extension_refs)
        .set_file_name(default_file_name)
        .save_file(move |path| {
            let _ = tx.send(path);
        });
    let picked = rx
        .await
        .map_err(|err| format!("Speichern-Dialog fehlgeschlagen: {err}"))?;
    Ok(picked.map(|p| p.to_string()))
}

/// Der aktuell aktive EDL-Stand eines Fotos, aufgelöst zu `EdlV3` — dieselbe
/// Quelle wie `current_develop_edit`, nur direkt als Rust-Wert statt als
/// JSON-DTO (der Export braucht kein IPC-JSON, er rendert serverseitig).
fn resolve_current_edl(
    catalog: &apx_catalog::Catalog,
    photo_id: apx_core::PhotoId,
) -> Result<apx_pipeline::edl::EdlV3, String> {
    match catalog
        .current_edit(photo_id)
        .map_err(|err| err.to_string())?
    {
        apx_catalog::HistoryPosition::Neutral => Ok(apx_pipeline::edl::EdlV3::default()),
        apx_catalog::HistoryPosition::At(entry) => {
            apx_pipeline::edl::from_envelope(&entry.edl).map_err(|err| err.to_string())
        }
    }
}

fn parse_export_format(format: &str) -> Result<apx_export::format::ExportFormat, String> {
    match format {
        "jpeg" => Ok(apx_export::format::ExportFormat::Jpeg),
        "png" => Ok(apx_export::format::ExportFormat::Png),
        "tiff" => Ok(apx_export::format::ExportFormat::Tiff),
        "webp" => Ok(apx_export::format::ExportFormat::WebP),
        "avif" => Ok(apx_export::format::ExportFormat::Avif),
        other => Err(format!("unbekanntes Exportformat '{other}'")),
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ExportOutcomeDto {
    pub path: String,
    pub width: u32,
    pub height: u32,
    pub byte_size: usize,
}

fn parse_icc_target(
    profile: &str,
    custom_path: Option<&str>,
) -> Result<apx_export::icc::IccTarget, String> {
    use apx_export::icc::{IccTarget, StandardIccProfile};
    match profile {
        "srgb" => Ok(IccTarget::Standard(StandardIccProfile::Srgb)),
        "adobe_rgb" => Ok(IccTarget::Standard(StandardIccProfile::AdobeRgb)),
        "pro_photo_rgb" => Ok(IccTarget::Standard(StandardIccProfile::ProPhotoRgb)),
        "display_p3" => Ok(IccTarget::Standard(StandardIccProfile::DisplayP3)),
        "custom" => {
            let path = custom_path
                .ok_or_else(|| "iccProfilePath fehlt für iccProfile='custom'".to_string())?;
            Ok(IccTarget::CustomFile(PathBuf::from(path)))
        }
        other => Err(format!("unbekanntes ICC-Zielprofil '{other}'")),
    }
}

fn parse_watermark_position(
    position: &str,
) -> Result<apx_export::watermark::WatermarkPosition, String> {
    use apx_export::watermark::WatermarkPosition;
    match position {
        "top_left" => Ok(WatermarkPosition::TopLeft),
        "top_right" => Ok(WatermarkPosition::TopRight),
        "bottom_left" => Ok(WatermarkPosition::BottomLeft),
        "bottom_right" => Ok(WatermarkPosition::BottomRight),
        "center" => Ok(WatermarkPosition::Center),
        other => Err(format!("unbekannte Wasserzeichen-Position '{other}'")),
    }
}

/// Export-Parameter fürs Frontend-Exportdialog. Größenbegrenzung/
/// Zieldateigröße/Schärfung (Schritt 1) sowie ICC-Zielprofil/Wasserzeichen/
/// Metadaten (Schritt 2) sind optional — nicht gesetzt heißt "unverändert
/// lassen"/"weglassen" (siehe `apx_export::resize`/`sharpen`/`icc`/
/// `watermark`/`metadata`).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportPhotoOptions {
    pub format: String,
    pub quality: Option<u8>,
    pub bit_depth_16: Option<bool>,
    pub max_edge: Option<u32>,
    pub max_megapixels: Option<f32>,
    pub max_file_size_bytes: Option<u64>,
    pub sharpen_amount: Option<f32>,
    pub sharpen_radius: Option<f32>,
    /// Dateiname ohne Endung — wird um `format`s Endung ergänzt. `None`
    /// übernimmt den Ausgangsdateinamen (ohne dessen ursprüngliche Endung).
    pub filename: Option<String>,
    /// `"srgb"`/`"adobe_rgb"`/`"pro_photo_rgb"`/`"display_p3"`/`"custom"`.
    pub icc_profile: Option<String>,
    /// Nur bei `icc_profile == "custom"` gelesen.
    pub icc_profile_path: Option<String>,
    /// Text-Wasserzeichen — braucht zusätzlich `watermark_font_path`.
    pub watermark_text: Option<String>,
    pub watermark_font_path: Option<String>,
    pub watermark_font_size: Option<f32>,
    /// `[R, G, B]`, `0..=255`.
    pub watermark_color: Option<[u8; 3]>,
    /// Bild-Wasserzeichen — Alternative zu `watermark_text`, wird
    /// dekodiert (beliebiges von `image` unterstütztes Format).
    pub watermark_image_path: Option<String>,
    /// `"top_left"`/`"top_right"`/`"bottom_left"`/`"bottom_right"`/`"center"`.
    pub watermark_position: Option<String>,
    pub watermark_opacity: Option<f32>,
    pub watermark_margin: Option<u32>,
    pub metadata_make: Option<String>,
    pub metadata_model: Option<String>,
    pub metadata_date_time: Option<String>,
    pub metadata_copyright: Option<String>,
    pub metadata_artist: Option<String>,
}

/// Baut aus `options` einen `apx_export::engine::ExportRequest` samt
/// Zielpfad, ohne selbst zu rendern — gemeinsam genutzt von
/// [`export_photo`] (sofort, synchron) und [`enqueue_export_photo`]
/// (Schritt 2: über die Warteschlange).
fn build_export_request(
    state: &AppState,
    photo_id: apx_core::PhotoId,
    dest_folder: &str,
    options: ExportPhotoOptions,
) -> Result<(apx_export::engine::ExportRequest, PathBuf), String> {
    let photo = state
        .catalog
        .get_photo(photo_id)
        .map_err(|err| err.to_string())?;
    let folder = state
        .catalog
        .get_folder(photo.folder_id)
        .map_err(|err| err.to_string())?;
    let source_path = folder.path.join(&photo.filename);

    let edl = resolve_current_edl(&state.catalog, photo_id)?;
    let format = parse_export_format(&options.format)?;

    let mut request = apx_export::engine::ExportRequest::new(source_path, edl, format);
    if let Some(quality) = options.quality {
        request.quality = quality;
    }
    if options.bit_depth_16.unwrap_or(false) {
        request.bit_depth = apx_export::format::BitDepth::Sixteen;
    }
    request.size_constraint = match (options.max_edge, options.max_megapixels) {
        (Some(edge), _) => apx_export::resize::SizeConstraint::MaxEdge(edge),
        (None, Some(megapixels)) => apx_export::resize::SizeConstraint::MaxMegapixels(megapixels),
        (None, None) => apx_export::resize::SizeConstraint::Original,
    };
    request.max_file_size_bytes = options.max_file_size_bytes;
    if let Some(amount) = options.sharpen_amount {
        if amount > 0.0 {
            request.sharpen = Some((amount, options.sharpen_radius.unwrap_or(1.0)));
        }
    }

    if let Some(profile) = &options.icc_profile {
        request.icc_target = Some(parse_icc_target(
            profile,
            options.icc_profile_path.as_deref(),
        )?);
    }

    if let Some(image_path) = &options.watermark_image_path {
        let decoded = image::open(image_path)
            .map_err(|err| {
                format!("Wasserzeichen-Bild '{image_path}' konnte nicht geladen werden: {err}")
            })?
            .to_rgba8();
        request.watermark = Some(apx_export::engine::WatermarkSpec::Image {
            width: decoded.width(),
            height: decoded.height(),
            rgba: decoded.into_raw(),
            position: parse_watermark_position(
                options
                    .watermark_position
                    .as_deref()
                    .unwrap_or("bottom_right"),
            )?,
            opacity: options.watermark_opacity.unwrap_or(1.0),
            margin: options.watermark_margin.unwrap_or(16),
        });
    } else if let Some(text) = &options.watermark_text {
        let font_path = options
            .watermark_font_path
            .as_deref()
            .ok_or_else(|| "watermarkFontPath fehlt für ein Text-Wasserzeichen".to_string())?;
        let font_bytes = std::fs::read(font_path).map_err(|err| {
            format!("Schriftdatei '{font_path}' konnte nicht gelesen werden: {err}")
        })?;
        request.watermark = Some(apx_export::engine::WatermarkSpec::Text {
            font_bytes,
            text: text.clone(),
            font_size_px: options.watermark_font_size.unwrap_or(24.0),
            color: options.watermark_color.unwrap_or([255, 255, 255]),
            position: parse_watermark_position(
                options
                    .watermark_position
                    .as_deref()
                    .unwrap_or("bottom_right"),
            )?,
            opacity: options.watermark_opacity.unwrap_or(1.0),
            margin: options.watermark_margin.unwrap_or(16),
        });
    }

    request.metadata = apx_export::metadata::MetadataFilter {
        make: options.metadata_make.clone(),
        model: options.metadata_model.clone(),
        date_time: options.metadata_date_time.clone(),
        copyright: options.metadata_copyright.clone(),
        artist: options.metadata_artist.clone(),
    };

    let stem = options.filename.clone().unwrap_or_else(|| {
        PathBuf::from(&photo.filename)
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| photo.filename.clone())
    });
    let dest_path = PathBuf::from(dest_folder).join(format!("{stem}.{}", format.extension()));

    Ok((request, dest_path))
}

/// Exportiert ein einzelnes Foto mit seinem aktuellen Bearbeitungsstand
/// nach `dest_folder`. Rendert über denselben Pfad wie die Entwickeln-
/// Vorschau (`apx_pipeline::develop::render_rgba8`, siehe
/// `apx_export::engine`s Moduldoku) — keine zweite Rendering-Logik.
/// Läuft synchron/sofort im aufrufenden Tauri-Command — für einen
/// Stapelexport mehrerer Fotos mit Fortschritt/Pausieren siehe
/// [`enqueue_export_photo`] (Schritt 2).
#[tauri::command]
pub fn export_photo(
    state: State<'_, AppState>,
    photo_id: String,
    dest_folder: String,
    options: ExportPhotoOptions,
) -> Result<ExportOutcomeDto, String> {
    let photo_id = parse_photo_id(photo_id)?;
    let (request, dest_path) = build_export_request(&state, photo_id, &dest_folder, options)?;

    let outcome = apx_export::engine::export_to_file(Some(&state.pipeline), &request, &dest_path)
        .map_err(|err| err.to_string())?;

    Ok(ExportOutcomeDto {
        path: dest_path.to_string_lossy().to_string(),
        width: outcome.width,
        height: outcome.height,
        byte_size: outcome.bytes.len(),
    })
}

// ---- Export-Warteschlange (Phase 8 Schritt 2) ------------------------------
//
// `apx_export::queue::ExportQueue` ist reine, threading-freie Logik — die
// eigentliche Hintergrund-Arbeit (rendern+kodieren+schreiben) läuft in
// einer einzigen, beim App-Start gestarteten Tokio-Task (siehe `main.rs`),
// die die Warteschlange in einer kurzen Schlaufe abfragt (dieselbe
// Größenordnung wie der Import-Job, siehe `PLAN.md` Phase 8 Schritt 2 —
// **vereinfacht**: Abfragen statt einer Weck-Benachrichtigung, für eine
// Warteschlange, die nicht auf Sub-Sekunden-Reaktionszeit angewiesen ist).

#[derive(Debug, Clone, Serialize)]
pub struct ExportQueueProgressDto {
    pub done: usize,
    pub total: usize,
    pub failed: usize,
    pub paused: bool,
}

/// Reiht einen Foto-Export in die Warteschlange ein, statt ihn sofort
/// auszuführen — gibt die Auftrags-ID zurück (für [`cancel_export_job`]).
#[tauri::command]
pub fn enqueue_export_photo(
    state: State<'_, AppState>,
    photo_id: String,
    dest_folder: String,
    options: ExportPhotoOptions,
) -> Result<u64, String> {
    let photo_id = parse_photo_id(photo_id)?;
    let (request, dest_path) = build_export_request(&state, photo_id, &dest_folder, options)?;
    let mut queue = state
        .export_queue
        .lock()
        .map_err(|_| "Export-Warteschlange nicht erreichbar".to_string())?;
    Ok(queue.push(crate::state::QueuedExport { request, dest_path }, 0))
}

#[tauri::command]
pub fn export_queue_progress(state: State<'_, AppState>) -> Result<ExportQueueProgressDto, String> {
    let queue = state
        .export_queue
        .lock()
        .map_err(|_| "Export-Warteschlange nicht erreichbar".to_string())?;
    let progress = queue.progress();
    Ok(ExportQueueProgressDto {
        done: progress.done,
        total: progress.total,
        failed: progress.failed,
        paused: queue.is_paused(),
    })
}

#[tauri::command]
pub fn pause_export_queue(state: State<'_, AppState>) -> Result<(), String> {
    state
        .export_queue
        .lock()
        .map_err(|_| "Export-Warteschlange nicht erreichbar".to_string())?
        .pause();
    Ok(())
}

#[tauri::command]
pub fn resume_export_queue(state: State<'_, AppState>) -> Result<(), String> {
    state
        .export_queue
        .lock()
        .map_err(|_| "Export-Warteschlange nicht erreichbar".to_string())?
        .resume();
    Ok(())
}

#[tauri::command]
pub fn cancel_export_job(state: State<'_, AppState>, job_id: u64) -> Result<bool, String> {
    Ok(state
        .export_queue
        .lock()
        .map_err(|_| "Export-Warteschlange nicht erreichbar".to_string())?
        .cancel(job_id))
}

#[tauri::command]
pub fn clear_finished_export_jobs(state: State<'_, AppState>) -> Result<(), String> {
    state
        .export_queue
        .lock()
        .map_err(|_| "Export-Warteschlange nicht erreichbar".to_string())?
        .clear_finished();
    Ok(())
}

// ---- Drucken (Phase 8 Schritt 3) -------------------------------------------
//
// Wiederverwendet die Export-Engine komplett: pro Foto rendert
// `apx_export::engine::render_to_pixels` (Größenbegrenzung/Wasserzeichen/
// Metadaten-Filter sind hier bedeutungslos — eine Druckseite exportiert
// keine Einzeldatei), `apx_export::print` setzt die gerenderten Zellbilder
// zu einer Seite zusammen, `apx_export::format` kodiert sie als JPEG
// ("Speichern als JPEG" statt echtem Druckdialog, siehe ADR-0034).

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrintLayoutOptions {
    /// `"single"`/`"contact_sheet"`/`"custom_grid"`/`"picture_package"`.
    pub layout: String,
    pub cols: Option<u32>,
    pub rows: Option<u32>,
    /// Nur bei `layout == "picture_package"`: `"one_large_two_small"`/
    /// `"four_equal"`/`"eight_wallet"`.
    pub picture_package_template: Option<String>,
    pub page_width_in: f32,
    pub page_height_in: f32,
    pub dpi: u32,
    pub margin_in: Option<f32>,
    pub gap_in: Option<f32>,
    /// `"contain"` (Standard) oder `"cover"`.
    pub fit: Option<String>,
    pub background_rgb: Option<[u8; 3]>,
    pub sharpen_amount: Option<f32>,
    pub sharpen_radius: Option<f32>,
    pub icc_profile: Option<String>,
    pub icc_profile_path: Option<String>,
}

fn parse_fit_mode(fit: Option<&str>) -> Result<apx_export::print::FitMode, String> {
    match fit.unwrap_or("contain") {
        "contain" => Ok(apx_export::print::FitMode::Contain),
        "cover" => Ok(apx_export::print::FitMode::Cover),
        other => Err(format!("unbekannter Anpassungsmodus '{other}'")),
    }
}

fn resolve_print_slots(
    options: &PrintLayoutOptions,
) -> Result<Vec<apx_export::print::PrintSlot>, String> {
    use apx_export::print::{grid_slots, picture_package_slots, PicturePackageTemplate};
    match options.layout.as_str() {
        "single" => Ok(grid_slots(
            options.page_width_in,
            options.page_height_in,
            options.margin_in.unwrap_or(0.25),
            options.gap_in.unwrap_or(0.1),
            1,
            1,
        )),
        "contact_sheet" | "custom_grid" => Ok(grid_slots(
            options.page_width_in,
            options.page_height_in,
            options.margin_in.unwrap_or(0.25),
            options.gap_in.unwrap_or(0.1),
            options.cols.unwrap_or(1),
            options.rows.unwrap_or(1),
        )),
        "picture_package" => {
            let template = match options.picture_package_template.as_deref() {
                Some("one_large_two_small") => PicturePackageTemplate::OneLargeTwoSmall,
                Some("four_equal") => PicturePackageTemplate::FourEqual,
                Some("eight_wallet") => PicturePackageTemplate::EightWallet,
                other => return Err(format!("unbekannte Bilderpaket-Vorlage '{other:?}'")),
            };
            Ok(picture_package_slots(template))
        }
        other => Err(format!("unbekanntes Drucklayout '{other}'")),
    }
}

/// Rendert `photo_ids` (eines je Zelle, überzählige Fotos werden ignoriert
/// — der Nutzer wählt im Frontend genau so viele Fotos wie das Layout
/// Zellen hat) auf eine gemeinsame Druckseite und schreibt sie als JPEG
/// nach `dest_path`.
#[tauri::command]
pub fn print_photos(
    state: State<'_, AppState>,
    photo_ids: Vec<String>,
    dest_path: String,
    options: PrintLayoutOptions,
) -> Result<ExportOutcomeDto, String> {
    let slots = resolve_print_slots(&options)?;
    let fit = parse_fit_mode(options.fit.as_deref())?;

    let mut rendered_cells: Vec<(u32, u32, Vec<u8>)> = Vec::new();
    for photo_id in &photo_ids {
        if rendered_cells.len() >= slots.len() {
            break;
        }
        let photo_id = parse_photo_id(photo_id.clone())?;
        let photo = state
            .catalog
            .get_photo(photo_id)
            .map_err(|err| err.to_string())?;
        let folder = state
            .catalog
            .get_folder(photo.folder_id)
            .map_err(|err| err.to_string())?;
        let edl = resolve_current_edl(&state.catalog, photo_id)?;

        let mut request = apx_export::engine::ExportRequest::new(
            folder.path.join(&photo.filename),
            edl,
            apx_export::format::ExportFormat::Jpeg,
        );
        if let Some(amount) = options.sharpen_amount {
            if amount > 0.0 {
                request.sharpen = Some((amount, options.sharpen_radius.unwrap_or(1.0)));
            }
        }
        if let Some(profile) = &options.icc_profile {
            request.icc_target = Some(parse_icc_target(
                profile,
                options.icc_profile_path.as_deref(),
            )?);
        }

        let (width, height, rgba) =
            apx_export::engine::render_to_pixels(Some(&state.pipeline), &request)
                .map_err(|err| err.to_string())?;
        rendered_cells.push((width, height, rgba));
    }

    let cells: Vec<apx_export::print::PrintCell> = slots
        .iter()
        .zip(rendered_cells.iter())
        .map(
            |(slot, (width, height, rgba))| apx_export::print::PrintCell {
                slot: *slot,
                width: *width,
                height: *height,
                rgba,
                fit,
            },
        )
        .collect();

    let (page_w, page_h, page_pixels) = apx_export::print::compose_page(
        options.page_width_in,
        options.page_height_in,
        options.dpi,
        options.background_rgb.unwrap_or([255, 255, 255]),
        &cells,
    )
    .map_err(|err| err.to_string())?;

    let bytes = apx_export::format::encode_rgba8(
        page_w,
        page_h,
        &page_pixels,
        apx_export::format::ExportFormat::Jpeg,
        &apx_export::format::EncodeOptions::default(),
    )
    .map_err(|err| err.to_string())?;

    std::fs::write(&dest_path, &bytes)
        .map_err(|err| format!("Datei '{dest_path}' konnte nicht geschrieben werden: {err}"))?;

    Ok(ExportOutcomeDto {
        path: dest_path,
        width: page_w,
        height: page_h,
        byte_size: bytes.len(),
    })
}

// ---- Diashow (Phase 8 Schritt 4) -------------------------------------------
//
// Übergänge/Ken-Burns-Effekt/Intro-Outro-Screens/Musik-Synchronisation
// laufen für die *Live-Wiedergabe* komplett im Frontend (`<canvas>` +
// `<audio>`, siehe `SlideshowPlayer.tsx`) — hier nur der Video-Export:
// jedes ausgewählte Foto wird wie beim normalen Export gerendert
// (`engine::render_to_pixels`), `apx_export::video` bildet daraus + den
// optionalen Titelkarten dieselbe Zeitachse nach und pipet sie an ein
// System-`ffmpeg` (siehe `video.rs`s Moduldoku, `DECISIONS.md` ADR-0034).

/// Ob ein aufrufbares `ffmpeg` gefunden wurde — das Frontend blendet den
/// Video-Export-Knopf danach ein/aus, statt ihn erst beim Fehlschlagen zu
/// deaktivieren.
#[tauri::command]
pub fn check_ffmpeg_available() -> bool {
    apx_export::video::ffmpeg_available()
}

fn parse_transition_kind(transition: &str) -> Result<apx_export::video::TransitionKind, String> {
    match transition {
        "cut" => Ok(apx_export::video::TransitionKind::Cut),
        "cross_fade" => Ok(apx_export::video::TransitionKind::CrossFade),
        other => Err(format!("unbekannter Übergang '{other}'")),
    }
}

/// Eine Intro-/Outro-Titelkarte, wie sie das Frontend auch für die
/// Live-Vorschau verwendet (siehe `slideshow.ts`) — `fontPath` fehlt nur,
/// wenn `text` leer ist (reine Farbfläche, siehe
/// `apx_export::video::render_title_card`s Moduldoku).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SlideshowTitleCardOptions {
    pub text: String,
    pub seconds: f32,
    pub background_rgb: [u8; 3],
    pub text_color: [u8; 3],
    pub font_path: Option<String>,
    pub font_size: Option<f32>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SlideshowVideoOptions {
    pub slide_seconds: f32,
    pub ken_burns: bool,
    /// `"cut"`/`"cross_fade"`.
    pub transition: String,
    pub transition_seconds: Option<f32>,
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub intro: Option<SlideshowTitleCardOptions>,
    pub outro: Option<SlideshowTitleCardOptions>,
    /// Beliebiges von `ffmpeg` unterstütztes Audioformat — `None`
    /// exportiert stumm.
    pub music_path: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SlideshowVideoOutcomeDto {
    pub path: String,
    pub frame_count: usize,
    pub duration_seconds: f32,
}

fn build_title_slide(
    card: &SlideshowTitleCardOptions,
    width: u32,
    height: u32,
) -> Result<apx_export::video::TimelineSlide, String> {
    let font_bytes =
        match &card.font_path {
            Some(path) => Some(std::fs::read(path).map_err(|err| {
                format!("Schriftdatei '{path}' konnte nicht gelesen werden: {err}")
            })?),
            None => None,
        };
    let rgba = apx_export::video::render_title_card(
        width,
        height,
        card.background_rgb,
        &card.text,
        font_bytes.as_deref(),
        card.font_size.unwrap_or(48.0),
        card.text_color,
    )
    .map_err(|err| err.to_string())?;
    Ok(apx_export::video::TimelineSlide::Title {
        width,
        height,
        rgba,
        hold_seconds: card.seconds.max(0.1),
    })
}

/// Rendert `photo_ids` (mit ihrem aktuellen Bearbeitungsstand, wie
/// [`export_photo`]) zu einer Diashow und kodiert sie über ein System-
/// `ffmpeg` als MP4 nach `dest_path` — siehe `apx_export::video`s
/// Moduldoku für die Zeitachse (Ken-Burns/Übergänge/Titelkarten) und die
/// Musik-Synchronisationsregel.
#[tauri::command]
pub fn export_slideshow_video(
    state: State<'_, AppState>,
    photo_ids: Vec<String>,
    dest_path: String,
    options: SlideshowVideoOptions,
) -> Result<SlideshowVideoOutcomeDto, String> {
    if photo_ids.is_empty() {
        return Err("Keine Fotos für die Diashow ausgewählt".to_string());
    }
    let transition = parse_transition_kind(&options.transition)?;

    let mut slides = Vec::new();
    if let Some(intro) = &options.intro {
        slides.push(build_title_slide(intro, options.width, options.height)?);
    }

    for (index, photo_id) in photo_ids.iter().enumerate() {
        let photo_id = parse_photo_id(photo_id.clone())?;
        let photo = state
            .catalog
            .get_photo(photo_id)
            .map_err(|err| err.to_string())?;
        let folder = state
            .catalog
            .get_folder(photo.folder_id)
            .map_err(|err| err.to_string())?;
        let edl = resolve_current_edl(&state.catalog, photo_id)?;

        let request = apx_export::engine::ExportRequest::new(
            folder.path.join(&photo.filename),
            edl,
            apx_export::format::ExportFormat::Jpeg,
        );
        let (width, height, rgba) =
            apx_export::engine::render_to_pixels(Some(&state.pipeline), &request)
                .map_err(|err| err.to_string())?;
        slides.push(apx_export::video::TimelineSlide::Photo {
            width,
            height,
            rgba,
            ken_burns: apx_export::video::default_ken_burns(index, options.ken_burns),
            hold_seconds: options.slide_seconds.max(0.1),
        });
    }

    if let Some(outro) = &options.outro {
        slides.push(build_title_slide(outro, options.width, options.height)?);
    }

    let video_options = apx_export::video::VideoExportOptions {
        output_width: options.width,
        output_height: options.height,
        fps: options.fps,
        audio_path: options.music_path.as_ref().map(PathBuf::from),
    };

    let outcome = apx_export::video::export_slideshow_video(
        &slides,
        transition,
        options.transition_seconds.unwrap_or(1.0),
        &video_options,
        Path::new(&dest_path),
    )
    .map_err(|err| err.to_string())?;

    Ok(SlideshowVideoOutcomeDto {
        path: dest_path,
        frame_count: outcome.frame_count,
        duration_seconds: outcome.duration_seconds,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // `#[tauri::command]`-Funktionen selbst werden hier nicht unit-getestet
    // (wie auch die übrigen Commands in dieser Datei nicht — sie sind
    // reine Verdrahtung, siehe Modul-Doku oben; `tauri::State` lässt sich
    // in einem reinen Unit-Test ohne echte Tauri-Laufzeit auch nicht
    // sinnvoll konstruieren). Die eigentliche Logik — `commit_edit`/
    // `current_edit`/`undo_edit`/`redo_edit` — ist bereits vollständig in
    // `apx-catalog`s `repository::edits`-Tests abgedeckt; hier wird nur
    // die private Umwandlungsfunktion getestet, die diese Datei selbst
    // beisteuert.

    fn sample_envelope(marker: f32) -> apx_core::EdlEnvelope {
        let edl = apx_pipeline::edl::EdlV3 {
            basic: apx_pipeline::edl::BasicAdjustments {
                exposure_ev: marker,
                ..apx_pipeline::edl::BasicAdjustments::NEUTRAL
            },
            ..apx_pipeline::edl::EdlV3::neutral()
        };
        apx_core::EdlEnvelope::new(
            apx_pipeline::EDL_SCHEMA_VERSION,
            serde_json::to_value(edl).expect("EDL sollte serialisierbar sein"),
        )
    }

    #[test]
    fn neutral_position_maps_to_neutral_dto() {
        let dto = history_position_to_dto(apx_catalog::HistoryPosition::Neutral)
            .expect("sollte gelingen");
        assert!(matches!(dto, HistoryPositionDto::Neutral));
    }

    #[test]
    fn at_position_maps_to_edl_json_roundtripping_through_envelope() {
        let entry = apx_catalog::EditHistoryEntry {
            id: apx_core::EditHistoryId::new(),
            photo_id: apx_core::PhotoId::new(),
            sequence: 0,
            label: None,
            edl: sample_envelope(0.7),
            created_at: time::OffsetDateTime::now_utc(),
        };
        let dto = history_position_to_dto(apx_catalog::HistoryPosition::At(entry))
            .expect("sollte gelingen");
        match dto {
            HistoryPositionDto::At { edl_json } => {
                let roundtripped =
                    apx_core::EdlEnvelope::from_json_str(&edl_json).expect("sollte wieder parsen");
                let parsed = apx_pipeline::edl::from_envelope(&roundtripped)
                    .expect("sollte gültiges EdlV3 ergeben");
                assert_eq!(parsed.basic.exposure_ev, 0.7);
            }
            HistoryPositionDto::Neutral => panic!("sollte nicht neutral sein"),
        }
    }

    #[test]
    fn parse_photo_id_rejects_invalid_string() {
        assert!(parse_photo_id("nicht-valide".to_string()).is_err());
    }

    #[test]
    fn parse_preset_id_rejects_invalid_string() {
        assert!(parse_preset_id("nicht-valide".to_string()).is_err());
    }

    #[test]
    fn parse_preset_folder_id_rejects_invalid_string() {
        assert!(parse_preset_folder_id("nicht-valide".to_string()).is_err());
    }

    #[test]
    fn apx_preset_file_roundtrips_through_json() {
        let file = ApxPresetFile {
            schema_version: APX_PRESET_SCHEMA_VERSION,
            name: "Warmer Filmlook".to_string(),
            tags: vec!["warm".to_string(), "film".to_string()],
            conditions: serde_json::json!([{"field": "iso", "op": ">", "value": "3200"}]),
            edl_subset: serde_json::json!({"basic": {"exposure_ev": 0.3}}),
        };
        let json = serde_json::to_string(&file).expect("sollte serialisierbar sein");
        let parsed: ApxPresetFile = serde_json::from_str(&json).expect("sollte wieder parsen");
        assert_eq!(parsed.name, "Warmer Filmlook");
        assert_eq!(parsed.tags, vec!["warm".to_string(), "film".to_string()]);
        assert_eq!(parsed.edl_subset, file.edl_subset);
    }

    #[test]
    fn apx_preset_file_defaults_missing_tags_and_conditions() {
        let json = r#"{"schema_version":1,"name":"Minimal","edl_subset":{}}"#;
        let parsed: ApxPresetFile = serde_json::from_str(json).expect("sollte parsen");
        assert!(parsed.tags.is_empty());
        assert_eq!(parsed.conditions, serde_json::Value::Array(Vec::new()));
    }
}
