//! Tauri-Commands: die einzige Schnittstelle, über die das Frontend mit
//! dem Backend spricht (siehe `ARCHITECTURE.md` Abschnitt 4 — Frontend
//! kennt kein SQL, keine Dateisystempfade, keine Bildpuffer). Fehler
//! werden für Phase 1 als `String` an das Frontend gereicht; eine
//! strukturierte Fehler-DTO kommt bei Bedarf in einer späteren Phase.

use std::path::PathBuf;

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
