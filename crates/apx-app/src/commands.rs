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
    /// GPS-Koordinaten aus EXIF oder von Hand über die Kartenansicht
    /// gesetzt (Phase 8 Schritt 7) — `None`, wenn kein Foto-Standort bekannt.
    pub gps_lat: Option<f64>,
    pub gps_lon: Option<f64>,
    /// `None` = echtes Foto. `Some(quelle)` = virtuelle Kopie (Phase 9
    /// Schritt 1) — teilt sich die Datei mit dem referenzierten Foto.
    pub source_photo_id: Option<String>,
    /// IPTC-artige Metadaten-Überschreibungen (Phase 9 Schritt 2).
    pub title: Option<String>,
    pub caption: Option<String>,
    pub copyright: Option<String>,
    pub creator: Option<String>,
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
            gps_lat: photo.gps_lat,
            gps_lon: photo.gps_lon,
            source_photo_id: photo.source_photo_id.map(|id| id.to_string()),
            title: photo.title,
            caption: photo.caption,
            copyright: photo.copyright,
            creator: photo.creator,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct KeywordDto {
    pub id: String,
    pub name: String,
    pub parent_id: Option<String>,
    pub synonyms: Vec<String>,
}

impl From<apx_catalog::Keyword> for KeywordDto {
    fn from(keyword: apx_catalog::Keyword) -> Self {
        Self {
            id: keyword.id.to_string(),
            name: keyword.name,
            parent_id: keyword.parent_id.map(|id| id.to_string()),
            synonyms: keyword.synonyms,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct TagRuleDto {
    pub id: String,
    pub name: String,
    pub keyword_id: String,
    pub conditions_json: String,
    pub enabled: bool,
}

impl From<apx_catalog::TagRule> for TagRuleDto {
    fn from(rule: apx_catalog::TagRule) -> Self {
        Self {
            id: rule.id.to_string(),
            name: rule.name,
            keyword_id: rule.keyword_id.to_string(),
            conditions_json: rule.conditions_json,
            enabled: rule.enabled,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct CollectionDto {
    pub id: String,
    pub name: String,
    /// `None` = Sammlung liegt an der Wurzel (Phase 9 Schritt 1).
    pub folder_id: Option<String>,
    pub is_smart: bool,
    /// JSON-String (`FilterCriteriaDto`-Form), nur gesetzt bei `is_smart`.
    pub smart_criteria_json: Option<String>,
}

impl From<apx_catalog::Collection> for CollectionDto {
    fn from(collection: apx_catalog::Collection) -> Self {
        Self {
            id: collection.id.to_string(),
            name: collection.name,
            folder_id: collection.folder_id.map(|id| id.to_string()),
            is_smart: collection.is_smart,
            smart_criteria_json: collection.smart_criteria_json,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct CollectionFolderDto {
    pub id: String,
    pub name: String,
    pub parent_id: Option<String>,
    pub position: i64,
}

impl From<apx_catalog::CollectionFolder> for CollectionFolderDto {
    fn from(folder: apx_catalog::CollectionFolder) -> Self {
        Self {
            id: folder.id.to_string(),
            name: folder.name,
            parent_id: folder.parent_id.map(|id| id.to_string()),
            position: folder.position,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct StackDto {
    pub id: String,
    pub name: Option<String>,
    pub cover_photo_id: Option<String>,
    pub photo_ids: Vec<String>,
}

impl From<apx_catalog::Stack> for StackDto {
    fn from(stack: apx_catalog::Stack) -> Self {
        Self {
            id: stack.id.to_string(),
            name: stack.name,
            cover_photo_id: stack.cover_photo_id.map(|id| id.to_string()),
            photo_ids: stack
                .photo_ids
                .into_iter()
                .map(|id| id.to_string())
                .collect(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ColorLabelDefinitionDto {
    pub name: String,
    pub display_name: String,
    pub hex: String,
    pub position: i64,
}

impl From<apx_catalog::ColorLabelDefinition> for ColorLabelDefinitionDto {
    fn from(def: apx_catalog::ColorLabelDefinition) -> Self {
        Self {
            name: def.name,
            display_name: def.display_name,
            hex: def.hex,
            position: def.position,
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

/// Eine gespeicherte Vorlage (Phase 8 Schritt 8, siehe
/// `apx_catalog::Template`s Moduldoku) — `kind` ist eine der Zeichenketten
/// "export"/"print"/"book"/"slideshow"/"web"/"workflow", `payload_json`
/// das jeweilige `*Options`-DTO als JSON (für `apx-app`/`apx-catalog`
/// opak, wie bei `PresetDto.conditions_json`).
#[derive(Debug, Clone, Serialize)]
pub struct TemplateDto {
    pub id: String,
    pub kind: String,
    pub name: String,
    pub payload_json: String,
    pub created_at: String,
}

impl From<apx_catalog::Template> for TemplateDto {
    fn from(template: apx_catalog::Template) -> Self {
        Self {
            id: template.id.to_string(),
            kind: template.kind,
            name: template.name,
            payload_json: template.payload_json,
            created_at: template
                .created_at
                .format(&time::format_description::well_known::Rfc3339)
                .unwrap_or_default(),
        }
    }
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

/// Eigenes Dateiformat für Vorlagen-Im-/Export (Phase 8 Schritt 8) — das
/// "lokale Repo-Format mit Manifest" aus `PLAN.md`s Beschreibung: kein
/// Online-Marktplatz-Server, sondern eine einzelne lesbare `.apxt`-Datei
/// mit einem kleinen Manifest (`schema_version`/`kind`/`name`) plus dem
/// eingebetteten Parametersatz (`payload`, dasselbe JSON wie in
/// `TemplateDto.payload_json`) — spiegelt `ApxPresetFile` oben.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ApxTemplateFile {
    schema_version: u32,
    kind: String,
    name: String,
    payload: serde_json::Value,
}

const APX_TEMPLATE_SCHEMA_VERSION: u32 = 1;

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

fn parse_collection_folder_id(id: String) -> Result<apx_core::CollectionFolderId, String> {
    id.parse()
        .map_err(|err: apx_core::AppError| err.to_string())
}

fn parse_stack_id(id: String) -> Result<apx_core::StackId, String> {
    id.parse()
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

fn parse_template_id(id: String) -> Result<apx_core::TemplateId, String> {
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

/// Ein Eintrag im vollständigen Bearbeitungsverlauf eines Fotos (Phase 9
/// Schritt 7, „Zeitleisten-Ansicht"/„Verlaufs-Vergleich" — siehe
/// `PLAN.md`, `DECISIONS.md` ADR-0035 Punkt 1). Anders als
/// [`HistoryPositionDto`] (nur der *aktuelle* Stand) trägt dieser DTO
/// `sequence`/`created_at` mit, die die Zeitleiste zum Anordnen und
/// Beschriften braucht.
#[derive(Debug, Clone, Serialize)]
pub struct EditHistoryEntryDto {
    pub sequence: i64,
    pub label: Option<String>,
    pub edl_json: String,
    pub created_at: String,
}

impl TryFrom<apx_catalog::EditHistoryEntry> for EditHistoryEntryDto {
    type Error = String;

    fn try_from(entry: apx_catalog::EditHistoryEntry) -> Result<Self, String> {
        Ok(Self {
            sequence: entry.sequence,
            label: entry.label,
            edl_json: entry.edl.to_json_string().map_err(|err| err.to_string())?,
            created_at: entry
                .created_at
                .format(&time::format_description::well_known::Rfc3339)
                .unwrap_or_default(),
        })
    }
}

/// Der vollständige Bearbeitungsverlauf eines Fotos, älteste Sequenz
/// zuerst — Grundlage der Zeitleisten-Ansicht (Phase 9 Schritt 7).
#[tauri::command]
pub fn list_develop_history(
    state: State<'_, AppState>,
    photo_id: String,
) -> Result<Vec<EditHistoryEntryDto>, String> {
    let photo_id = parse_photo_id(photo_id)?;
    state
        .catalog
        .list_edit_history(photo_id)
        .map_err(|err| err.to_string())?
        .into_iter()
        .map(EditHistoryEntryDto::try_from)
        .collect()
}

/// Springt direkt zu einer Sequenznummer aus [`list_develop_history`]
/// (Phase 9 Schritt 7) — `None`, wenn die Sequenz nicht (mehr) existiert.
#[tauri::command]
pub fn goto_develop_edit(
    state: State<'_, AppState>,
    photo_id: String,
    sequence: i64,
) -> Result<Option<HistoryPositionDto>, String> {
    let photo_id = parse_photo_id(photo_id)?;
    let result = state
        .catalog
        .goto_edit(photo_id, sequence)
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

// ---- Bibliothek: Schlagworthierarchie, Tag-Regeln, Metadaten (ab Phase 9
// Schritt 2, siehe DECISIONS.md ADR-0035) -----------------------------------

#[tauri::command]
pub fn set_keyword_parent(
    state: State<'_, AppState>,
    keyword_id: String,
    parent_id: Option<String>,
) -> Result<(), String> {
    let keyword_id = parse_keyword_id(keyword_id)?;
    let parent_id = parent_id.map(parse_keyword_id).transpose()?;
    state
        .catalog
        .set_keyword_parent(keyword_id, parent_id)
        .map_err(|err| err.to_string())
}

#[tauri::command]
pub fn set_keyword_synonyms(
    state: State<'_, AppState>,
    keyword_id: String,
    synonyms: Vec<String>,
) -> Result<(), String> {
    let keyword_id = parse_keyword_id(keyword_id)?;
    state
        .catalog
        .set_keyword_synonyms(keyword_id, &synonyms)
        .map_err(|err| err.to_string())
}

#[tauri::command]
pub fn delete_keyword(state: State<'_, AppState>, keyword_id: String) -> Result<(), String> {
    let keyword_id = parse_keyword_id(keyword_id)?;
    state
        .catalog
        .delete_keyword(keyword_id)
        .map_err(|err| err.to_string())
}

fn parse_tag_rule_id(id: String) -> Result<apx_core::TagRuleId, String> {
    id.parse()
        .map_err(|err: apx_core::AppError| err.to_string())
}

#[tauri::command]
pub fn create_tag_rule(
    state: State<'_, AppState>,
    name: String,
    keyword_id: String,
    conditions_json: String,
) -> Result<String, String> {
    let keyword_id = parse_keyword_id(keyword_id)?;
    let id = state
        .catalog
        .create_tag_rule(&name, keyword_id, &conditions_json)
        .map_err(|err| err.to_string())?;
    Ok(id.to_string())
}

#[tauri::command]
pub fn set_tag_rule_enabled(
    state: State<'_, AppState>,
    tag_rule_id: String,
    enabled: bool,
) -> Result<(), String> {
    let id = parse_tag_rule_id(tag_rule_id)?;
    state
        .catalog
        .set_tag_rule_enabled(id, enabled)
        .map_err(|err| err.to_string())
}

#[tauri::command]
pub fn delete_tag_rule(state: State<'_, AppState>, tag_rule_id: String) -> Result<(), String> {
    let id = parse_tag_rule_id(tag_rule_id)?;
    state
        .catalog
        .delete_tag_rule(id)
        .map_err(|err| err.to_string())
}

#[tauri::command]
pub fn list_tag_rules(state: State<'_, AppState>) -> Result<Vec<TagRuleDto>, String> {
    let rules = state
        .catalog
        .list_tag_rules()
        .map_err(|err| err.to_string())?;
    Ok(rules.into_iter().map(TagRuleDto::from).collect())
}

/// Aktualisiert die vier IPTC-artigen Metadaten-Felder für eine oder
/// mehrere Fotos (Stapel-Metadatenbearbeitung: das Frontend ruft dies
/// einfach für jede `photo_id` in der Auswahl einzeln auf).
#[tauri::command]
pub fn set_photo_metadata(
    state: State<'_, AppState>,
    photo_id: String,
    title: Option<String>,
    caption: Option<String>,
    copyright: Option<String>,
    creator: Option<String>,
) -> Result<(), String> {
    let photo_id = parse_photo_id(photo_id)?;
    state
        .catalog
        .set_photo_metadata(
            photo_id,
            title.as_deref(),
            caption.as_deref(),
            copyright.as_deref(),
            creator.as_deref(),
        )
        .map_err(|err| err.to_string())
}

/// Exportiert eine `.xmp`-Sidecar-Datei neben dem Original — Metadaten
/// (Titel/Bildunterschrift/Copyright/Urheber/Schlagworte) plus optional
/// die Adobe-`crs:`-Entwickeln-Einstellungen (Basic+HSL, siehe
/// `apx_export::xmp`s Moduldoku). `with_develop_settings=false` schreibt
/// nur Metadaten.
#[tauri::command]
pub fn export_xmp_sidecar(
    state: State<'_, AppState>,
    photo_id: String,
    with_develop_settings: bool,
) -> Result<String, String> {
    let photo_id = parse_photo_id(photo_id)?;
    let photo = state
        .catalog
        .get_photo(photo_id)
        .map_err(|err| err.to_string())?;
    let folder = state
        .catalog
        .get_folder(photo.folder_id)
        .map_err(|err| err.to_string())?;
    let photo_path = folder.path.join(&photo.filename);

    let keywords = state
        .catalog
        .list_keywords_for_photo(photo_id)
        .map_err(|err| err.to_string())?;
    let metadata = apx_export::xmp::XmpSidecarMetadata {
        title: photo.title.clone(),
        caption: photo.caption.clone(),
        copyright: photo.copyright.clone(),
        creator: photo.creator.clone(),
        keywords: keywords.into_iter().map(|k| k.name).collect(),
    };

    let develop = if with_develop_settings {
        let position = state
            .catalog
            .current_edit(photo_id)
            .map_err(|err| err.to_string())?;
        match position {
            apx_catalog::HistoryPosition::At(entry) => {
                let edl =
                    apx_pipeline::edl::from_envelope(&entry.edl).map_err(|err| err.to_string())?;
                Some((edl.basic, edl.hsl))
            }
            apx_catalog::HistoryPosition::Neutral => None,
        }
    } else {
        None
    };
    let develop_ref = develop.as_ref().map(|(basic, hsl)| (basic, hsl));

    let sidecar_path = apx_export::xmp::write_sidecar(&photo_path, &metadata, develop_ref)
        .map_err(|err| err.to_string())?;
    Ok(sidecar_path.display().to_string())
}

/// Liest die geparsten `crs:`-Entwickeln-Einstellungen auf den aktuellen
/// Bearbeitungsstand von `photo_id` und committet sie als neuen
/// Bearbeitungsschritt — dieselbe Merge-Semantik wie ein Preset-Teilsatz
/// (`lib/presets.ts::mergeEdlSubset`): nur Basic/HSL werden ersetzt, alle
/// anderen EDL-Felder (Kurven/Masken/Weißabgleich/...) bleiben unverändert.
/// Gemeinsame Kernlogik für [`import_xmp_develop_settings`] (Inhalt vom
/// Frontend übergeben) und [`import_xmp_sidecar_from_file`] (nativer
/// Datei-Dialog, wie `import_template_from_file`s Muster).
fn apply_parsed_xmp_develop_settings(
    state: &State<'_, AppState>,
    photo_id: apx_core::PhotoId,
    parsed: apx_export::xmp::ParsedDevelopSettings,
) -> Result<(), String> {
    if parsed.basic.is_none() && parsed.hsl.is_none() {
        return Err(
            "Die XMP-Datei enthält keine unterstützten crs:-Entwickeln-Einstellungen".to_string(),
        );
    }

    let position = state
        .catalog
        .current_edit(photo_id)
        .map_err(|err| err.to_string())?;
    let mut edl = match position {
        apx_catalog::HistoryPosition::At(entry) => {
            apx_pipeline::edl::from_envelope(&entry.edl).map_err(|err| err.to_string())?
        }
        apx_catalog::HistoryPosition::Neutral => apx_pipeline::edl::EdlV4::neutral(),
    };
    if let Some(basic) = parsed.basic {
        edl.basic = basic;
    }
    if let Some(hsl) = parsed.hsl {
        edl.hsl = hsl;
    }

    let envelope = apx_pipeline::edl::to_envelope(&edl).map_err(|err| err.to_string())?;
    state
        .catalog
        .commit_edit(photo_id, &envelope, Some("XMP-Import"))
        .map_err(|err| err.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn import_xmp_develop_settings(
    state: State<'_, AppState>,
    photo_id: String,
    xmp_content: String,
) -> Result<(), String> {
    let photo_id = parse_photo_id(photo_id)?;
    let parsed =
        apx_export::xmp::parse_xmp_develop_settings(&xmp_content).map_err(|err| err.to_string())?;
    apply_parsed_xmp_develop_settings(&state, photo_id, parsed)
}

/// Wie [`import_xmp_develop_settings`], liest die Datei aber über einen
/// nativen Öffnen-Dialog statt vom Frontend übergebenen Inhalt (dasselbe
/// Muster wie `import_template_from_file`). `Ok(false)` = Dialog wurde
/// abgebrochen.
#[tauri::command]
pub async fn import_xmp_sidecar_from_file(
    app: AppHandle,
    state: State<'_, AppState>,
    photo_id: String,
) -> Result<bool, String> {
    let photo_id = parse_photo_id(photo_id)?;
    let (tx, rx) = tokio::sync::oneshot::channel();
    app.dialog()
        .file()
        .add_filter("XMP-Sidecar", &["xmp"])
        .pick_file(move |path| {
            let _ = tx.send(path);
        });
    let picked = rx
        .await
        .map_err(|err| format!("Öffnen-Dialog fehlgeschlagen: {err}"))?;
    let Some(picked) = picked else {
        return Ok(false);
    };
    let path = picked
        .into_path()
        .map_err(|err| format!("Ungültiger Pfad: {err}"))?;
    let text = std::fs::read_to_string(&path)
        .map_err(|err| format!("Datei '{}' nicht lesbar: {err}", path.display()))?;
    let parsed =
        apx_export::xmp::parse_xmp_develop_settings(&text).map_err(|err| err.to_string())?;
    apply_parsed_xmp_develop_settings(&state, photo_id, parsed)?;
    Ok(true)
}

// ---- Bibliothek: Sammlungen (ab Phase 3) -----------------------------------

#[tauri::command]
pub fn create_collection(
    state: State<'_, AppState>,
    name: String,
    folder_id: Option<String>,
) -> Result<String, String> {
    let folder_id = folder_id.map(parse_collection_folder_id).transpose()?;
    let id = state
        .catalog
        .create_collection(&name, folder_id)
        .map_err(|err| err.to_string())?;
    Ok(id.to_string())
}

/// Legt eine intelligente Sammlung an — siehe `Catalog::create_smart_collection`s
/// Moduldoku.
#[tauri::command]
pub fn create_smart_collection(
    state: State<'_, AppState>,
    name: String,
    folder_id: Option<String>,
    criteria: FilterCriteriaDto,
) -> Result<String, String> {
    let folder_id = folder_id.map(parse_collection_folder_id).transpose()?;
    let id = state
        .catalog
        .create_smart_collection(&name, folder_id, &criteria.into())
        .map_err(|err| err.to_string())?;
    Ok(id.to_string())
}

#[tauri::command]
pub fn move_collection_to_folder(
    state: State<'_, AppState>,
    collection_id: String,
    folder_id: Option<String>,
) -> Result<(), String> {
    let collection_id = parse_collection_id(collection_id)?;
    let folder_id = folder_id.map(parse_collection_folder_id).transpose()?;
    state
        .catalog
        .move_collection_to_folder(collection_id, folder_id)
        .map_err(|err| err.to_string())
}

// ---- Sammlungssätze (Phase 9 Schritt 1) ------------------------------------

#[tauri::command]
pub fn create_collection_folder(
    state: State<'_, AppState>,
    name: String,
    parent_id: Option<String>,
) -> Result<String, String> {
    let parent_id = parent_id.map(parse_collection_folder_id).transpose()?;
    let id = state
        .catalog
        .create_collection_folder(&name, parent_id)
        .map_err(|err| err.to_string())?;
    Ok(id.to_string())
}

#[tauri::command]
pub fn rename_collection_folder(
    state: State<'_, AppState>,
    folder_id: String,
    name: String,
) -> Result<(), String> {
    let folder_id = parse_collection_folder_id(folder_id)?;
    state
        .catalog
        .rename_collection_folder(folder_id, &name)
        .map_err(|err| err.to_string())
}

#[tauri::command]
pub fn delete_collection_folder(
    state: State<'_, AppState>,
    folder_id: String,
) -> Result<(), String> {
    let folder_id = parse_collection_folder_id(folder_id)?;
    state
        .catalog
        .delete_collection_folder(folder_id)
        .map_err(|err| err.to_string())
}

#[tauri::command]
pub fn list_collection_folders(
    state: State<'_, AppState>,
) -> Result<Vec<CollectionFolderDto>, String> {
    let folders = state
        .catalog
        .list_collection_folders()
        .map_err(|err| err.to_string())?;
    Ok(folders.into_iter().map(CollectionFolderDto::from).collect())
}

// ---- Virtuelle Kopien (Phase 9 Schritt 1) ----------------------------------

#[tauri::command]
pub fn create_virtual_copy(
    state: State<'_, AppState>,
    photo_id: String,
) -> Result<PhotoDto, String> {
    let photo_id = parse_photo_id(photo_id)?;
    let copy_id = state
        .catalog
        .create_virtual_copy(photo_id)
        .map_err(|err| err.to_string())?;
    let photo = state
        .catalog
        .get_photo(copy_id)
        .map_err(|err| err.to_string())?;
    Ok(PhotoDto::from(photo))
}

#[tauri::command]
pub fn list_virtual_copies(
    state: State<'_, AppState>,
    photo_id: String,
) -> Result<Vec<PhotoDto>, String> {
    let photo_id = parse_photo_id(photo_id)?;
    let copies = state
        .catalog
        .list_virtual_copies(photo_id)
        .map_err(|err| err.to_string())?;
    Ok(copies.into_iter().map(PhotoDto::from).collect())
}

// ---- Stapel (Phase 9 Schritt 1) --------------------------------------------

#[tauri::command]
pub fn create_stack(
    state: State<'_, AppState>,
    name: Option<String>,
    photo_ids: Vec<String>,
) -> Result<String, String> {
    let photo_ids = photo_ids
        .into_iter()
        .map(parse_photo_id)
        .collect::<Result<Vec<_>, _>>()?;
    let id = state
        .catalog
        .create_stack(name.as_deref(), &photo_ids)
        .map_err(|err| err.to_string())?;
    Ok(id.to_string())
}

#[tauri::command]
pub fn delete_stack(state: State<'_, AppState>, stack_id: String) -> Result<(), String> {
    let stack_id = parse_stack_id(stack_id)?;
    state
        .catalog
        .delete_stack(stack_id)
        .map_err(|err| err.to_string())
}

#[tauri::command]
pub fn set_stack_cover(
    state: State<'_, AppState>,
    stack_id: String,
    cover_photo_id: String,
) -> Result<(), String> {
    let stack_id = parse_stack_id(stack_id)?;
    let cover_photo_id = parse_photo_id(cover_photo_id)?;
    state
        .catalog
        .set_stack_cover(stack_id, cover_photo_id)
        .map_err(|err| err.to_string())
}

#[tauri::command]
pub fn list_stacks(state: State<'_, AppState>) -> Result<Vec<StackDto>, String> {
    let stacks = state.catalog.list_stacks().map_err(|err| err.to_string())?;
    Ok(stacks.into_iter().map(StackDto::from).collect())
}

/// Gruppiert `photo_ids` automatisch nach Aufnahmezeit — siehe
/// `Catalog::auto_stack_by_time`s Moduldoku.
#[tauri::command]
pub fn auto_stack_by_time(
    state: State<'_, AppState>,
    photo_ids: Vec<String>,
    window_seconds: i64,
) -> Result<Vec<String>, String> {
    let photo_ids = photo_ids
        .into_iter()
        .map(parse_photo_id)
        .collect::<Result<Vec<_>, _>>()?;
    let stack_ids = state
        .catalog
        .auto_stack_by_time(&photo_ids, window_seconds)
        .map_err(|err| err.to_string())?;
    Ok(stack_ids.into_iter().map(|id| id.to_string()).collect())
}

// ---- Erweiterbare Farbmarkierungen (Phase 9 Schritt 1) ---------------------

#[tauri::command]
pub fn list_color_label_definitions(
    state: State<'_, AppState>,
) -> Result<Vec<ColorLabelDefinitionDto>, String> {
    let defs = state
        .catalog
        .list_color_label_definitions()
        .map_err(|err| err.to_string())?;
    Ok(defs
        .into_iter()
        .map(ColorLabelDefinitionDto::from)
        .collect())
}

#[tauri::command]
pub fn create_color_label_definition(
    state: State<'_, AppState>,
    name: String,
    display_name: String,
    hex: String,
) -> Result<(), String> {
    state
        .catalog
        .create_color_label_definition(&name, &display_name, &hex)
        .map_err(|err| err.to_string())
}

#[tauri::command]
pub fn delete_color_label_definition(
    state: State<'_, AppState>,
    name: String,
) -> Result<(), String> {
    state
        .catalog
        .delete_color_label_definition(&name)
        .map_err(|err| err.to_string())
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

/// Adobe `.lrtemplate`-Export für ein Preset (Phase 11 Schritt 8, siehe
/// `DECISIONS.md` ADR-0038) — deckt dieselbe Teilmenge wie der bereits
/// vorhandene `.xmp`-`crs:`-Export ab (Basic ohne Weißabgleich + HSL,
/// siehe `apx_export::lrtemplate`s Moduldoku). Fehlt eine dieser beiden
/// Sektionen im Preset (`edl_subset_json` enthält sie nicht, weil das
/// Preset z. B. nur Kurven anpasst), wird sie als neutral exportiert —
/// Lightroom kennt für diese Felder kein „nicht gesetzt", jede
/// `.lrtemplate`-Datei trägt immer einen vollständigen Absolutwert je
/// Feld. Nur Export, siehe Moduldoku zum Grund. `Ok(None)`, wenn der
/// Dateidialog abgebrochen wurde.
#[tauri::command]
pub async fn export_preset_to_lrtemplate_file(
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

    let subset: serde_json::Value = serde_json::from_str(&version.edl_subset_json)
        .map_err(|err| format!("Preset-EDL-Teilmenge ist kein gültiges JSON: {err}"))?;
    let basic: apx_pipeline::edl::BasicAdjustments = subset
        .get("basic")
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or(apx_pipeline::edl::BasicAdjustments::NEUTRAL);
    let hsl: apx_pipeline::edl::HslAdjustment = subset
        .get("hsl")
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or(apx_pipeline::edl::HslAdjustment::NEUTRAL);

    let content = apx_export::lrtemplate::generate_lrtemplate(
        &preset.name,
        &preset_id.to_string(),
        &basic,
        &hsl,
    );

    let (tx, rx) = tokio::sync::oneshot::channel();
    app.dialog()
        .file()
        .add_filter("Lightroom-Vorlage", &["lrtemplate"])
        .set_file_name(format!("{}.lrtemplate", preset.name))
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
    std::fs::write(&path, content)
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

// ---- Bibliothek: Stapelverarbeitungs-Konsole (Phase 11 Schritt 9, siehe
// DECISIONS.md ADR-0038) -----------------------------------------------

/// Eingabe für [`apply_batch_rule`] — spiegelt `apx_catalog::BatchAction`.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "kind")]
pub enum BatchActionDto {
    SetRating { rating: u8 },
    SetColorLabel { color_label: Option<String> },
    AddKeyword { name: String },
}

impl From<BatchActionDto> for apx_catalog::BatchAction {
    fn from(dto: BatchActionDto) -> Self {
        match dto {
            BatchActionDto::SetRating { rating } => Self::SetRating(rating),
            BatchActionDto::SetColorLabel { color_label } => Self::SetColorLabel(color_label),
            BatchActionDto::AddKeyword { name } => Self::AddKeyword(name),
        }
    }
}

/// Fotos, die `criteria` treffen würden — schreibt nichts (Trockenlauf-
/// Vorschau vor dem eigentlichen Anwenden, siehe `BatchConsoleDialog.tsx`).
#[tauri::command]
pub fn preview_batch_rule(
    state: State<'_, AppState>,
    criteria: FilterCriteriaDto,
) -> Result<Vec<PhotoDto>, String> {
    let photos = state
        .catalog
        .preview_batch_rule(&criteria.into())
        .map_err(|err| err.to_string())?;
    Ok(photos.into_iter().map(PhotoDto::from).collect())
}

/// Wendet `action` auf alle `criteria`-treffenden Fotos an und
/// journalisiert jede tatsächliche Änderung — gibt die neue Stapel-ID
/// (für [`undo_batch_operation`]) als String zurück.
#[tauri::command]
pub fn apply_batch_rule(
    state: State<'_, AppState>,
    criteria: FilterCriteriaDto,
    action: BatchActionDto,
) -> Result<String, String> {
    let batch_id = state
        .catalog
        .apply_batch_rule(&criteria.into(), &action.into())
        .map_err(|err| err.to_string())?;
    Ok(batch_id.to_string())
}

/// Macht jede in `batch_id` journalisierte Änderung einzeln rückgängig.
/// Gibt die Zahl tatsächlich rückgängig gemachter Änderungen zurück.
#[tauri::command]
pub fn undo_batch_operation(state: State<'_, AppState>, batch_id: String) -> Result<usize, String> {
    let batch_id: apx_core::BatchOperationId = batch_id
        .parse()
        .map_err(|err: apx_core::AppError| err.to_string())?;
    state
        .catalog
        .undo_batch_operation(batch_id)
        .map_err(|err| err.to_string())
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

/// Gruppen ähnlicher (nicht notwendig byte-identischer) Fotos per
/// Perceptual Hash (Phase 9 Schritt 1, siehe `DECISIONS.md` ADR-0032) —
/// ergänzt [`list_duplicate_photo_groups`]s exaktem Hash-Vergleich um
/// nahe Duplikate (z. B. leicht unterschiedlich exportierte/skalierte
/// Versionen desselben Motivs). **Bewusste Vereinfachung:** hasht die
/// bereits vorhandene 256px-Miniaturansicht (`PreviewLevel::Thumbnail`)
/// statt jedes Mal neu von der RAW-Datei zu dekodieren — Fotos ohne
/// bereits generierte Miniaturansicht werden übersprungen, nicht
/// erzwungen dekodiert (hält diesen Command schnell, statt bei jedem
/// Aufruf über den gesamten Katalog neu zu rendern). Gruppierung ist
/// O(n²) (jedes Foto gegen den ersten Vertreter jeder bereits
/// gefundenen Gruppe) — für einen auf Abruf gestarteten Assistenten
/// über einen realistischen Katalogumfang tragbar, kein Hintergrund-Job.
#[tauri::command]
pub fn list_perceptual_duplicate_groups(
    state: State<'_, AppState>,
    max_distance: u32,
) -> Result<Vec<Vec<PhotoDto>>, String> {
    let photos = state
        .catalog
        .search_and_filter_photos(None, &apx_catalog::FilterCriteria::default())
        .map_err(|err| err.to_string())?;

    let hasher = image_hasher::HasherConfig::new().to_hasher();
    let mut hashed: Vec<(apx_catalog::Photo, image_hasher::ImageHash)> = Vec::new();
    for photo in photos {
        let Ok(Some(preview)) = state
            .catalog
            .get_preview(photo.id, apx_catalog::PreviewLevel::Thumbnail)
        else {
            continue;
        };
        let Ok(img) = image::open(&preview.path) else {
            continue;
        };
        let hash = hasher.hash_image(&img);
        hashed.push((photo, hash));
    }

    let mut groups: Vec<Vec<usize>> = Vec::new();
    for (index, (_, hash)) in hashed.iter().enumerate() {
        let mut placed = false;
        for group in groups.iter_mut() {
            if hashed[group[0]].1.dist(hash) <= max_distance {
                group.push(index);
                placed = true;
                break;
            }
        }
        if !placed {
            groups.push(vec![index]);
        }
    }

    Ok(groups
        .into_iter()
        .filter(|group| group.len() >= 2)
        .map(|group| {
            group
                .into_iter()
                .map(|i| PhotoDto::from(hashed[i].0.clone()))
                .collect()
        })
        .collect())
}

/// Personenansicht (Phase 11 Schritt 5, siehe `DECISIONS.md` ADR-0038):
/// grobe Vorsortierung von Fotos mit erkannten Gesichtsregionen nach
/// Ähnlichkeit (Blob-Anzahl/-Fläche als grobe „Signatur") — **keine
/// echte Personen-Identifizierung**, siehe `apx_ai::faces`s Moduldoku.
/// Arbeitet wie [`list_perceptual_duplicate_groups`] auf dem bereits
/// vorhandenen Thumbnail-Vorschau-Cache statt jedes Foto neu zu
/// dekodieren (dieselbe Begründung: schnell genug für die ganze
/// Bibliothek, kein zweiter teurer RAW-Dekodier-Durchlauf).
#[tauri::command]
pub fn list_people_groups(state: State<'_, AppState>) -> Result<Vec<Vec<PhotoDto>>, String> {
    let photos = state
        .catalog
        .search_and_filter_photos(None, &apx_catalog::FilterCriteria::default())
        .map_err(|err| err.to_string())?;

    // Signatur-Schlüssel: (Blob-Anzahl, bei 4 gekappt) × (grob gebuckete
    // durchschnittliche Blob-Fläche) — bewusst grob, siehe Moduldoku.
    let mut buckets: std::collections::BTreeMap<(u32, u32), Vec<apx_catalog::Photo>> =
        std::collections::BTreeMap::new();

    for photo in photos {
        let Ok(Some(preview)) = state
            .catalog
            .get_preview(photo.id, apx_catalog::PreviewLevel::Thumbnail)
        else {
            continue;
        };
        let Ok(img) = image::open(&preview.path) else {
            continue;
        };
        let rgb = img.to_rgb8();
        let (width, height) = rgb.dimensions();
        let pixels: Vec<f32> = rgb
            .into_raw()
            .iter()
            .map(|&v| f32::from(v) / 255.0)
            .collect();
        let Ok(regions) = apx_ai::faces::detect_face_regions(&pixels, width, height) else {
            continue;
        };
        if regions.is_empty() {
            continue;
        }
        let avg_area: f32 =
            regions.iter().map(|r| r.width * r.height).sum::<f32>() / regions.len() as f32;
        let key = (
            regions.len().min(4) as u32,
            (avg_area * 20.0).round() as u32,
        );
        buckets.entry(key).or_default().push(photo);
    }

    Ok(buckets
        .into_values()
        .filter(|group| group.len() >= 2)
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

// ---- UI: Einstellungen (Phase 10 Schritt 1) --------------------------------
//
// Dieselbe Vertrauensgrenze/dasselbe Lade-Muster wie `get_ai_settings`/
// `set_anthropic_api_key` oben: Einstellungen werden bei jedem Aufruf frisch
// von der Platte gelesen (kein In-Memory-Cache in `AppState`), es gibt genau
// eine gemeinsame TOML-Datei für alle Einstellungskategorien.

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiSettingsDto {
    pub theme: apx_core::Theme,
    pub accent_color: Option<String>,
    pub locale: String,
    pub ui_scale_percent: u16,
    pub high_contrast: bool,
    pub reduced_motion: bool,
    pub onboarding_seen: bool,
}

impl From<apx_core::UiSettings> for UiSettingsDto {
    fn from(ui: apx_core::UiSettings) -> Self {
        Self {
            theme: ui.theme,
            accent_color: ui.accent_color,
            locale: ui.locale,
            ui_scale_percent: ui.ui_scale_percent,
            high_contrast: ui.high_contrast,
            reduced_motion: ui.reduced_motion,
            onboarding_seen: ui.onboarding_seen,
        }
    }
}

#[tauri::command]
pub fn get_ui_settings(state: State<'_, AppState>) -> Result<UiSettingsDto, String> {
    let settings = apx_core::Settings::load_or_default(&state.paths.settings_file())
        .map_err(|err| err.to_string())?;
    Ok(settings.ui.into())
}

/// Speichert die komplette `UiSettingsDto` auf einmal (das Frontend hält
/// bereits den vollständigen Stand im Store, siehe `store/index.ts`s
/// `uiSettings` — kein granulares Patch-DTO nötig, derselbe Ansatz wie bei
/// den übrigen Mehrfeld-Einstellungsobjekten dieses Projekts).
#[tauri::command]
pub fn set_ui_settings(state: State<'_, AppState>, settings: UiSettingsDto) -> Result<(), String> {
    let path = state.paths.settings_file();
    let mut all = apx_core::Settings::load_or_default(&path).map_err(|err| err.to_string())?;
    all.ui = apx_core::UiSettings {
        theme: settings.theme,
        accent_color: settings.accent_color.filter(|c| !c.trim().is_empty()),
        locale: settings.locale,
        ui_scale_percent: settings.ui_scale_percent.clamp(75, 200),
        high_contrast: settings.high_contrast,
        reduced_motion: settings.reduced_motion,
        onboarding_seen: settings.onboarding_seen,
    };
    all.save(&path).map_err(|err| err.to_string())
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
            apx_catalog::HistoryPosition::Neutral => apx_pipeline::edl::EdlV4::neutral(),
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

/// Der aktuell aktive EDL-Stand eines Fotos, aufgelöst zu `EdlV4` — dieselbe
/// Quelle wie `current_develop_edit`, nur direkt als Rust-Wert statt als
/// JSON-DTO (der Export braucht kein IPC-JSON, er rendert serverseitig).
fn resolve_current_edl(
    catalog: &apx_catalog::Catalog,
    photo_id: apx_core::PhotoId,
) -> Result<apx_pipeline::edl::EdlV4, String> {
    match catalog
        .current_edit(photo_id)
        .map_err(|err| err.to_string())?
    {
        apx_catalog::HistoryPosition::Neutral => Ok(apx_pipeline::edl::EdlV4::default()),
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
        // Phase 11 Schritt 2 (siehe DECISIONS.md ADR-0038): PSD (ag-psd,
        // reines Rust) und JPEG-XL (gamut-jxl) — HEIF bleibt zurückgestellt
        // (`heif` 0.1.0 ist eine Fassade, `heif-rs` zu riskant für das
        // Plattenkontingent dieser Sandbox, siehe ADR-0038).
        "psd" => Ok(apx_export::format::ExportFormat::Psd),
        "jxl" => Ok(apx_export::format::ExportFormat::Jxl),
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

// ---- Buch (Phase 8 Schritt 5) -----------------------------------------
//
// Wiederverwendet die Export-Engine + `apx_export::print` komplett: pro
// Foto rendert `engine::render_to_pixels` wie beim normalen Export,
// `apx_export::book` setzt die Fotos gemäß Seitenvorlage zu Buchseiten
// zusammen (Bildunterschrift = Dateiname, keine manuelle Eingabe nötig —
// „automatische Befüllung") und bettet alle Seiten als eine PDF-Datei
// ein (`printpdf`, siehe `book.rs`s Moduldoku).

fn parse_book_template(template: &str) -> Result<apx_export::book::PageTemplate, String> {
    use apx_export::book::PageTemplate;
    match template {
        "full_bleed" => Ok(PageTemplate::FullBleed),
        "two_side_by_side" => Ok(PageTemplate::TwoSideBySide),
        "grid_2x2" => Ok(PageTemplate::Grid2x2),
        "photo_with_caption" => Ok(PageTemplate::PhotoWithCaption),
        other => Err(format!("unbekannte Buch-Seitenvorlage '{other}'")),
    }
}

fn parse_print_shop_preset(name: &str) -> Result<apx_export::book::PrintShopPreset, String> {
    apx_export::book::PRINT_SHOP_PRESETS
        .iter()
        .find(|preset| preset.name == name)
        .copied()
        .ok_or_else(|| format!("unbekanntes Druckerei-Preset '{name}'"))
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BookOptions {
    /// `"full_bleed"`/`"two_side_by_side"`/`"grid_2x2"`/`"photo_with_caption"`.
    pub template: String,
    pub page_width_in: f32,
    pub page_height_in: f32,
    /// Wird durch `print_shop_preset` überschrieben, falls gesetzt.
    pub dpi: u32,
    pub margin_in: Option<f32>,
    /// `"contain"` (Standard) oder `"cover"`.
    pub fit: Option<String>,
    pub background_rgb: Option<[u8; 3]>,
    /// Name aus `apx_export::book::PRINT_SHOP_PRESETS` — überschreibt `dpi`/`backgroundRgb`.
    pub print_shop_preset: Option<String>,
    /// Titelseite voranstellen, falls gesetzt — braucht `fontPath`.
    pub title: Option<String>,
    /// Für Titelseite und `photo_with_caption`-Bildunterschriften (=
    /// Dateiname des Fotos).
    pub font_path: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct BookOutcomeDto {
    pub path: String,
    pub page_count: usize,
    pub byte_size: usize,
}

/// Rendert `photo_ids` (mit ihrem aktuellen Bearbeitungsstand, wie
/// [`export_photo`]) zu einem Fotobuch — automatische Befüllung gemäß
/// `options.template` — und schreibt es als mehrseitige PDF-Datei nach
/// `dest_path`.
#[tauri::command]
pub fn export_book_pdf(
    state: State<'_, AppState>,
    photo_ids: Vec<String>,
    dest_path: String,
    options: BookOptions,
) -> Result<BookOutcomeDto, String> {
    if photo_ids.is_empty() {
        return Err("Keine Fotos für das Buch ausgewählt".to_string());
    }
    let template = parse_book_template(&options.template)?;
    let fit = match options.fit.as_deref().unwrap_or("contain") {
        "contain" => apx_export::print::FitMode::Contain,
        "cover" => apx_export::print::FitMode::Cover,
        other => return Err(format!("unbekannter Anpassungsmodus '{other}'")),
    };

    let (dpi, background_rgb, margin_in) = match &options.print_shop_preset {
        Some(name) => {
            let preset = parse_print_shop_preset(name)?;
            (preset.dpi, preset.background_rgb, preset.bleed_in)
        }
        None => (
            options.dpi,
            options.background_rgb.unwrap_or([255, 255, 255]),
            options.margin_in.unwrap_or(0.25),
        ),
    };

    let font_bytes =
        match &options.font_path {
            Some(path) => Some(std::fs::read(path).map_err(|err| {
                format!("Schriftdatei '{path}' konnte nicht gelesen werden: {err}")
            })?),
            None => None,
        };

    // Jedes Foto genau einmal rendern (dieselben Pixel für die Seite,
    // auf der es landet — `render_to_pixels` cacht selbst nicht, die
    // Warteschlange bleibt hier bewusst einfach synchron).
    let mut rendered: std::collections::HashMap<String, (u32, u32, Vec<u8>, String)> =
        std::collections::HashMap::new();
    for photo_id_str in &photo_ids {
        let photo_id = parse_photo_id(photo_id_str.clone())?;
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
        rendered.insert(
            photo_id_str.clone(),
            (width, height, rgba, photo.filename.clone()),
        );
    }

    let mut pages: Vec<(u32, u32, Vec<u8>)> = Vec::new();

    if let Some(title) = &options.title {
        let (w, h, pixels) = apx_export::book::render_book_page(
            apx_export::book::PageTemplate::TitlePage,
            options.page_width_in,
            options.page_height_in,
            dpi,
            margin_in,
            background_rgb,
            &[],
            fit,
            Some(title.as_str()),
            font_bytes.as_deref(),
            [0, 0, 0],
        )
        .map_err(|err| err.to_string())?;
        pages.push((w, h, pixels));
    }

    for group in apx_export::book::auto_fill_pages(&photo_ids, template) {
        let photos: Vec<apx_export::book::BookPagePhoto> = group
            .iter()
            .filter_map(|id| rendered.get(id))
            .map(|(width, height, rgba, _)| apx_export::book::BookPagePhoto {
                width: *width,
                height: *height,
                rgba,
            })
            .collect();
        let caption = if template == apx_export::book::PageTemplate::PhotoWithCaption {
            group
                .first()
                .and_then(|id| rendered.get(id))
                .map(|(_, _, _, filename)| filename.as_str())
        } else {
            None
        };
        let (w, h, pixels) = apx_export::book::render_book_page(
            template,
            options.page_width_in,
            options.page_height_in,
            dpi,
            margin_in,
            background_rgb,
            &photos,
            fit,
            caption,
            font_bytes.as_deref(),
            [0, 0, 0],
        )
        .map_err(|err| err.to_string())?;
        pages.push((w, h, pixels));
    }

    let page_count = pages.len();
    let bytes = apx_export::book::build_pdf(&pages, dpi, Path::new(&dest_path))
        .map_err(|err| err.to_string())?;

    Ok(BookOutcomeDto {
        path: dest_path,
        page_count,
        byte_size: bytes.len(),
    })
}

// ---- Web (Phase 8 Schritt 6) -------------------------------------------
//
// Wiederverwendet die Export-Engine komplett: pro Foto rendert
// `engine::render_to_pixels` wie beim normalen Export, `apx_export::web`
// baut daraus eine statische HTML-Galerie (Miniaturbilder + Themes) und
// lädt sie optional per FTP/SFTP hoch (siehe `web.rs`s Moduldoku für den
// Vertrauensrahmen — kein Host-Key-Pinning, Nutzername/Passwort statt
// Schlüsseldatei).

fn parse_gallery_theme(theme: &str) -> Result<apx_export::web::GalleryTheme, String> {
    match theme {
        "light" => Ok(apx_export::web::GalleryTheme::Light),
        "dark" => Ok(apx_export::web::GalleryTheme::Dark),
        "minimal" => Ok(apx_export::web::GalleryTheme::Minimal),
        other => Err(format!("unbekanntes Galerie-Theme '{other}'")),
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WebUploadOptions {
    /// `"ftp"`/`"sftp"`.
    pub protocol: String,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    pub remote_dir: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WebGalleryOptions {
    pub title: String,
    /// `"light"`/`"dark"`/`"minimal"`.
    pub theme: String,
    pub max_edge: Option<u32>,
    pub upload: Option<WebUploadOptions>,
}

#[derive(Debug, Clone, Serialize)]
pub struct WebGalleryOutcomeDto {
    pub dest_dir: String,
    pub photo_count: usize,
    pub uploaded_count: Option<usize>,
}

/// Rendert `photo_ids` (mit ihrem aktuellen Bearbeitungsstand) zu einer
/// statischen HTML-Galerie unter `dest_dir` und lädt sie optional per
/// FTP/SFTP hoch.
#[tauri::command]
pub async fn export_web_gallery(
    state: State<'_, AppState>,
    photo_ids: Vec<String>,
    dest_dir: String,
    options: WebGalleryOptions,
) -> Result<WebGalleryOutcomeDto, String> {
    if photo_ids.is_empty() {
        return Err("Keine Fotos für die Galerie ausgewählt".to_string());
    }
    let theme = parse_gallery_theme(&options.theme)?;

    let mut photos = Vec::with_capacity(photo_ids.len());
    for photo_id_str in &photo_ids {
        let photo_id = parse_photo_id(photo_id_str.clone())?;
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
        let caption = PathBuf::from(&photo.filename)
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| photo.filename.clone());
        photos.push((width, height, rgba, caption));
    }

    let outcome = apx_export::web::export_gallery(
        &photos,
        &options.title,
        theme,
        options.max_edge.unwrap_or(1600),
        Path::new(&dest_dir),
    )
    .map_err(|err| err.to_string())?;

    let uploaded_count = match &options.upload {
        None => None,
        Some(upload) => {
            let remote_dir = upload.remote_dir.clone().unwrap_or_default();
            match upload.protocol.as_str() {
                "ftp" => {
                    let target = apx_export::web::FtpTarget {
                        host: upload.host.clone(),
                        port: upload.port,
                        username: upload.username.clone(),
                        password: upload.password.clone(),
                        remote_dir,
                    };
                    Some(
                        apx_export::web::upload_via_ftp(&outcome.dest_dir, &target)
                            .map_err(|err| err.to_string())?,
                    )
                }
                "sftp" => {
                    let target = apx_export::web::SftpTarget {
                        host: upload.host.clone(),
                        port: upload.port,
                        username: upload.username.clone(),
                        password: upload.password.clone(),
                        remote_dir,
                    };
                    Some(
                        apx_export::web::upload_via_sftp(&outcome.dest_dir, &target)
                            .await
                            .map_err(|err| err.to_string())?,
                    )
                }
                other => return Err(format!("unbekanntes Upload-Protokoll '{other}'")),
            }
        }
    };

    Ok(WebGalleryOutcomeDto {
        dest_dir: outcome.dest_dir.to_string_lossy().to_string(),
        photo_count: outcome.photo_count,
        uploaded_count,
    })
}

// ---- Karte (Phase 8 Schritt 7) ---------------------------------------
//
// Reine Verdrahtung wie die übrigen Export-Module: GPS-Koordinaten liest
// bereits der Import (`apx_raw::metadata::extract_gps`) in die
// `photos`-Tabelle, hier kommt nur die Kartenansicht selbst dazu —
// geotaggte Fotos auflisten, offline reverse-geocoden, GPX-Tracks
// importieren, GPS von Hand setzen (`apx_export::map`).

/// Alle Fotos mit bekannten GPS-Koordinaten, ordnerübergreifend, für die
/// Kartenansicht.
#[tauri::command]
pub fn list_geotagged_photos(state: State<'_, AppState>) -> Result<Vec<PhotoDto>, String> {
    let photos = state
        .catalog
        .list_geotagged_photos()
        .map_err(|err| err.to_string())?;
    Ok(photos.into_iter().map(PhotoDto::from).collect())
}

#[derive(Debug, Clone, Serialize)]
pub struct GeocodedLocationDto {
    pub name: String,
    pub admin1: String,
    pub country_code: String,
    pub distance_km: f64,
}

/// Vollständig offline Reverse-Geocoding einer Koordinate (kein
/// Netzwerkaufruf, siehe `apx_export::map`s Moduldoku).
#[tauri::command]
pub fn reverse_geocode_location(lat: f64, lon: f64) -> GeocodedLocationDto {
    let location = apx_export::map::reverse_geocode(lat, lon);
    GeocodedLocationDto {
        name: location.name,
        admin1: location.admin1,
        country_code: location.country_code,
        distance_km: location.distance_km,
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct GpxTrackPointDto {
    pub lat: f64,
    pub lon: f64,
    pub elevation: Option<f64>,
    pub time: Option<String>,
}

/// Liest und parst eine GPX-Datei (Pfad kommt vom bereits vorhandenen
/// `pick_file_path`-Dialog) — gibt alle Trackpunkte für die
/// Reiserouten-Anzeige auf der Karte zurück.
#[tauri::command]
pub fn import_gpx_track(path: String) -> Result<Vec<GpxTrackPointDto>, String> {
    let xml = std::fs::read_to_string(&path)
        .map_err(|err| format!("GPX-Datei '{path}' konnte nicht gelesen werden: {err}"))?;
    let points = apx_export::map::parse_gpx(&xml).map_err(|err| err.to_string())?;
    Ok(points
        .into_iter()
        .map(|p| GpxTrackPointDto {
            lat: p.lat,
            lon: p.lon,
            elevation: p.elevation,
            time: p.time,
        })
        .collect())
}

/// Setzt oder löscht (`lat`/`lon` beide `None`) die GPS-Koordinaten eines
/// Fotos von Hand — z. B. per Klick auf die Kartenansicht platziert, weil
/// das Foto keine EXIF-GPS-Daten trug.
#[tauri::command]
pub fn set_photo_gps(
    state: State<'_, AppState>,
    photo_id: String,
    lat: Option<f64>,
    lon: Option<f64>,
) -> Result<(), String> {
    let id = parse_photo_id(photo_id)?;
    let gps = match (lat, lon) {
        (Some(lat), Some(lon)) => Some((lat, lon)),
        _ => None,
    };
    state
        .catalog
        .set_photo_gps(id, gps)
        .map_err(|err| err.to_string())
}

// ---- Vorlagen (Phase 8 Schritt 8) --------------------------------------
//
// Reine Verdrahtung: `apx_catalog::Catalog`s generische Vorlagen-Tabelle
// (`kind`+`name`+`payload_json`) deckt Export-/Layout-Vorlagen für alle
// fünf Ausgabemodule ab (dieselben `*Options`-DTOs, die die jeweiligen
// Dialoge ohnehin schon als JSON schicken) sowie Workflow-Vorlagen — die
// eigentliche Workflow-Ausführung (Preset anwenden + exportieren über
// mehrere Fotos) läuft bewusst im Frontend (siehe `store/index.ts`s
// `runWorkflowTemplate`), weil das EDL-Vorlagen-Mischen
// (`mergeEdlSubset`) bislang nur dort existiert — kein zweiter,
// serverseitiger Merge-Codepfad für denselben Vorgang.

/// Legt eine neue Vorlage an.
#[tauri::command]
pub fn save_template(
    state: State<'_, AppState>,
    kind: String,
    name: String,
    payload_json: String,
) -> Result<String, String> {
    let id = state
        .catalog
        .create_template(&kind, &name, &payload_json)
        .map_err(|err| err.to_string())?;
    Ok(id.to_string())
}

/// Alle Vorlagen einer Art, alphabetisch nach Namen.
#[tauri::command]
pub fn list_templates(
    state: State<'_, AppState>,
    kind: String,
) -> Result<Vec<TemplateDto>, String> {
    let templates = state
        .catalog
        .list_templates(&kind)
        .map_err(|err| err.to_string())?;
    Ok(templates.into_iter().map(TemplateDto::from).collect())
}

#[tauri::command]
pub fn delete_template(state: State<'_, AppState>, template_id: String) -> Result<(), String> {
    let id = parse_template_id(template_id)?;
    state
        .catalog
        .delete_template(id)
        .map_err(|err| err.to_string())
}

/// Öffnet einen Speichern-Dialog und schreibt die Vorlage als `.apxt`-Datei
/// (siehe `ApxTemplateFile`s Moduldoku). `None`, wenn der Dialog
/// abgebrochen wurde.
#[tauri::command]
pub async fn export_template_to_file(
    app: AppHandle,
    state: State<'_, AppState>,
    template_id: String,
) -> Result<Option<String>, String> {
    let id = parse_template_id(template_id)?;
    let template = state
        .catalog
        .get_template(id)
        .map_err(|err| err.to_string())?;
    let payload: serde_json::Value = serde_json::from_str(&template.payload_json)
        .map_err(|err| format!("Vorlagen-Nutzlast ist kein gültiges JSON: {err}"))?;
    let file = ApxTemplateFile {
        schema_version: APX_TEMPLATE_SCHEMA_VERSION,
        kind: template.kind,
        name: template.name.clone(),
        payload,
    };
    let json = serde_json::to_string_pretty(&file)
        .map_err(|err| format!("Vorlage nicht serialisierbar: {err}"))?;

    let (tx, rx) = tokio::sync::oneshot::channel();
    app.dialog()
        .file()
        .add_filter("Aperture X Vorlage", &["apxt"])
        .set_file_name(format!("{}.apxt", template.name))
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
    Ok(Some(path.to_string_lossy().to_string()))
}

/// Öffnet einen Öffnen-Dialog und legt die gewählte `.apxt`-Datei als neue
/// Vorlage an. `None`, wenn der Dialog abgebrochen wurde.
#[tauri::command]
pub async fn import_template_from_file(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Option<TemplateDto>, String> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    app.dialog()
        .file()
        .add_filter("Aperture X Vorlage", &["apxt"])
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
    let file: ApxTemplateFile = serde_json::from_str(&text).map_err(|err| {
        format!(
            "Datei '{}' ist keine gültige .apxt-Datei: {err}",
            path.display()
        )
    })?;
    if file.schema_version > APX_TEMPLATE_SCHEMA_VERSION {
        return Err(format!(
            "Datei '{}' hat Schema-Version {}, diese Aperture-X-Version kennt nur {}",
            path.display(),
            file.schema_version,
            APX_TEMPLATE_SCHEMA_VERSION
        ));
    }
    let payload_json = serde_json::to_string(&file.payload)
        .map_err(|err| format!("Nutzlast nicht serialisierbar: {err}"))?;
    let id = state
        .catalog
        .create_template(&file.kind, &file.name, &payload_json)
        .map_err(|err| err.to_string())?;
    let template = state
        .catalog
        .get_template(id)
        .map_err(|err| err.to_string())?;
    Ok(Some(TemplateDto::from(template)))
}

/// Einzelnes Foto per ID — u. a. für das sekundäre Display (Phase 9
/// Schritt 3), das als eigenes Fenster/Webview keinen Zugriff auf den
/// Zustand des Hauptfensters hat und sich sein Foto deshalb selbst holen
/// muss.
#[tauri::command]
pub fn get_photo(state: State<'_, AppState>, photo_id: String) -> Result<PhotoDto, String> {
    let photo_id = parse_photo_id(photo_id)?;
    state
        .catalog
        .get_photo(photo_id)
        .map(PhotoDto::from)
        .map_err(|err| err.to_string())
}

// ---- Bibliothek: Statistik, Vorschau-Cache (ab Phase 9 Schritt 3, siehe
// DECISIONS.md ADR-0035) -----------------------------------------------------

#[derive(Debug, Clone, Serialize)]
pub struct CatalogStatisticsDto {
    pub total_photos: u64,
    pub total_file_size: u64,
    pub earliest_captured_at: Option<String>,
    pub latest_captured_at: Option<String>,
    pub rating_distribution: Vec<(u8, u64)>,
    pub top_camera_models: Vec<(String, u64)>,
    pub top_lenses: Vec<(String, u64)>,
}

fn format_rfc3339(dt: Option<time::OffsetDateTime>) -> Option<String> {
    dt.and_then(|dt| {
        dt.format(&time::format_description::well_known::Rfc3339)
            .ok()
    })
}

impl From<apx_catalog::CatalogStatistics> for CatalogStatisticsDto {
    fn from(stats: apx_catalog::CatalogStatistics) -> Self {
        Self {
            total_photos: stats.total_photos,
            total_file_size: stats.total_file_size,
            earliest_captured_at: format_rfc3339(stats.earliest_captured_at),
            latest_captured_at: format_rfc3339(stats.latest_captured_at),
            rating_distribution: stats.rating_distribution,
            top_camera_models: stats.top_camera_models,
            top_lenses: stats.top_lenses,
        }
    }
}

#[tauri::command]
pub fn catalog_statistics(state: State<'_, AppState>) -> Result<CatalogStatisticsDto, String> {
    state
        .catalog
        .catalog_statistics()
        .map(CatalogStatisticsDto::from)
        .map_err(|err| err.to_string())
}

#[derive(Debug, Clone, Serialize)]
pub struct PreviewCacheStatsDto {
    pub file_count: u64,
    pub total_bytes: u64,
}

fn walk_dir_stats(dir: &std::path::Path) -> std::io::Result<(u64, u64)> {
    let mut file_count = 0u64;
    let mut total_bytes = 0u64;
    if !dir.exists() {
        return Ok((0, 0));
    }
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let metadata = entry.metadata()?;
        if metadata.is_dir() {
            let (sub_count, sub_bytes) = walk_dir_stats(&entry.path())?;
            file_count += sub_count;
            total_bytes += sub_bytes;
        } else {
            file_count += 1;
            total_bytes += metadata.len();
        }
    }
    Ok((file_count, total_bytes))
}

/// Größe des Vorschau-Caches (`AppPaths::preview_cache_dir`) — rekursiv,
/// da `apx-app` die Dateien beim Import nach den ersten zwei Zeichen der
/// Foto-ID in Unterordner aufteilt.
#[tauri::command]
pub fn preview_cache_stats(state: State<'_, AppState>) -> Result<PreviewCacheStatsDto, String> {
    let (file_count, total_bytes) =
        walk_dir_stats(&state.paths.preview_cache_dir()).map_err(|err| err.to_string())?;
    Ok(PreviewCacheStatsDto {
        file_count,
        total_bytes,
    })
}

fn clear_dir_contents(dir: &std::path::Path) -> std::io::Result<()> {
    if !dir.exists() {
        return Ok(());
    }
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            std::fs::remove_dir_all(&path)?;
        } else {
            std::fs::remove_file(&path)?;
        }
    }
    Ok(())
}

/// Leert den Vorschau-Cache vollständig (Verzeichnis selbst bleibt
/// bestehen) — Vorschauen werden beim nächsten Zugriff aus dem Original
/// neu generiert (kein Datenverlust, reiner Cache).
#[tauri::command]
pub fn clear_preview_cache(state: State<'_, AppState>) -> Result<(), String> {
    clear_dir_contents(&state.paths.preview_cache_dir()).map_err(|err| err.to_string())
}

/// Feste lange Kante für Smart Previews (Phase 11 Schritt 4, siehe
/// DECISIONS.md ADR-0038) — deutlich kleiner als ein Original, aber groß
/// genug für Betrachtung/Grob-Bearbeitung, wenn das Original selbst
/// nicht erreichbar ist.
const SMART_PREVIEW_EDGE: u32 = 2560;

/// Erzeugt Smart Previews für `photo_ids`: je eine feste, verkleinerte
/// JPEG-Zwischendatei in `AppPaths::smart_preview_dir()`, die
/// `apx-app::protocol::resolve_source_path` als Fallback nutzt, wenn die
/// Originaldatei nicht erreichbar ist (z. B. eine getrennte externe
/// Festplatte) — ermöglicht eingeschränktes Weiterarbeiten offline
/// (Anzeige/Entwickeln-Vorschau, siehe `Viewer.tsx`). Überspringt Fotos,
/// deren Original selbst schon nicht erreichbar ist (kann daraus kein
/// Smart Preview erzeugen), statt den ganzen Aufruf abzubrechen — gibt
/// die Zahl tatsächlich erzeugter Previews zurück.
#[tauri::command]
pub fn generate_smart_previews(
    state: State<'_, AppState>,
    photo_ids: Vec<String>,
) -> Result<usize, String> {
    let dir = state.paths.smart_preview_dir();
    std::fs::create_dir_all(&dir).map_err(|err| {
        format!(
            "Smart-Preview-Verzeichnis '{}' nicht anlegbar: {err}",
            dir.display()
        )
    })?;

    let mut generated = 0usize;
    for raw_id in photo_ids {
        let photo_id = parse_photo_id(raw_id)?;
        let source_path = resolve_source_path_for_ai(&state.catalog, photo_id)?;
        if !source_path.exists() {
            tracing::warn!(photo_id = %photo_id, path = %source_path.display(), "Original nicht erreichbar, überspringe Smart-Preview-Erzeugung");
            continue;
        }
        let decoded = apx_raw::decode(&source_path, Some(SMART_PREVIEW_EDGE))
            .map_err(|err| err.to_string())?;
        let image = decoded
            .into_dynamic_image()
            .ok_or_else(|| "Dekodiertes Bild hat inkonsistente Maße".to_string())?;
        let dest_path = dir.join(format!("{photo_id}.jpg"));
        image
            .save_with_format(&dest_path, image::ImageFormat::Jpeg)
            .map_err(|err| {
                format!(
                    "Smart Preview '{}' nicht schreibbar: {err}",
                    dest_path.display()
                )
            })?;
        generated += 1;
    }
    Ok(generated)
}

// ---- Entwickeln: Entrauschung, Hochskalierung (ab Phase 9 Schritt 6, siehe
// DECISIONS.md ADR-0035) — klassische, deterministische Algorithmen statt
// echter Modellinferenz (dasselbe ONNX-Beschaffungsproblem wie ADR-0033),
// deshalb bewusst nicht als „KI"/„AI" beschriftet. -------------------------

/// Rendert `photo_id` mit seinem aktuellen Bearbeitungsstand in voller
/// Auflösung — gemeinsame Grundlage für [`denoise_photo`]/[`upscale_photo`],
/// dieselbe `apx_export::engine`-Rendering-Kette wie ein echter Export
/// (kein zweiter Rendering-Codepfad).
fn render_photo_full_resolution(
    state: &AppState,
    photo_id: apx_core::PhotoId,
) -> Result<(PathBuf, u32, u32, Vec<u8>), String> {
    let source_path = resolve_source_path_for_ai(&state.catalog, photo_id)?;
    let edl = resolve_current_edl(&state.catalog, photo_id)?;
    let request = apx_export::engine::ExportRequest::new(
        source_path.clone(),
        edl,
        apx_export::format::ExportFormat::Png,
    );
    let (width, height, pixels) =
        apx_export::engine::render_to_pixels(Some(&state.pipeline), &request)
            .map_err(|err| err.to_string())?;
    Ok((source_path, width, height, pixels))
}

fn write_derived_png(
    source_path: &std::path::Path,
    suffix: &str,
    width: u32,
    height: u32,
    pixels: &[u8],
) -> Result<String, String> {
    let bytes = apx_export::format::encode_rgba8(
        width,
        height,
        pixels,
        apx_export::format::ExportFormat::Png,
        &apx_export::format::EncodeOptions::default(),
    )
    .map_err(|err| err.to_string())?;
    let stem = source_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Foto");
    let dest_path = source_path.with_file_name(format!("{stem}_{suffix}.png"));
    std::fs::write(&dest_path, bytes)
        .map_err(|err| format!("Datei '{}' nicht schreibbar: {err}", dest_path.display()))?;
    Ok(dest_path.display().to_string())
}

/// Entrauscht `photo_id` (kantenerhaltender Bilateral-Filter,
/// `apx_ai::denoise`) und schreibt das Ergebnis als neue PNG-Datei neben
/// dem Original. `range_sigma` steuert die Stärke (größer = mehr
/// Glättung) — `None` verwendet einen moderaten Vorgabewert.
#[tauri::command]
pub fn denoise_photo(
    state: State<'_, AppState>,
    photo_id: String,
    range_sigma: Option<f32>,
) -> Result<String, String> {
    let photo_id = parse_photo_id(photo_id)?;
    let (source_path, width, height, pixels) = render_photo_full_resolution(&state, photo_id)?;
    let denoised = apx_ai::denoise::bilateral_filter_rgba8(
        &pixels,
        width,
        height,
        apx_ai::denoise::DEFAULT_RADIUS,
        3.0,
        range_sigma.unwrap_or(20.0),
    );
    write_derived_png(&source_path, "entrauscht", width, height, &denoised)
}

/// Skaliert `photo_id` auf das Doppelte hoch (kantengerichtete
/// Interpolation, `apx_ai::upscale`) und schreibt das Ergebnis als neue
/// PNG-Datei neben dem Original.
#[tauri::command]
pub fn upscale_photo(state: State<'_, AppState>, photo_id: String) -> Result<String, String> {
    let photo_id = parse_photo_id(photo_id)?;
    let (source_path, width, height, pixels) = render_photo_full_resolution(&state, photo_id)?;
    let (out_width, out_height, upscaled) =
        apx_ai::upscale::edge_directed_upscale_2x_rgba8(&pixels, width, height);
    write_derived_png(
        &source_path,
        "hochskaliert",
        out_width,
        out_height,
        &upscaled,
    )
}

// ---- Import mit DNG-Konvertierung (Phase 11 Schritt 1, siehe
// DECISIONS.md ADR-0038) — schreibt eine „Linear DNG" (siehe
// `apx_export::dng`s Moduldoku) aus den unveränderten, kamera-nativen
// RAW-Daten (nicht dem entwickelten/edierten Rendering wie
// `render_photo_full_resolution` unten) — echte DNG-Konvertierung
// bewahrt den unbearbeiteten Ausgangszustand, kein zweiter
// Rendering-Codepfad nötig, da `apx_raw::decode_linear` bereits der
// Phase-2-Einstiegspunkt für `apx-pipeline` ist.
#[tauri::command]
pub fn convert_photo_to_dng(
    state: State<'_, AppState>,
    photo_id: String,
) -> Result<String, String> {
    let photo_id = parse_photo_id(photo_id)?;
    let photo = state
        .catalog
        .get_photo(photo_id)
        .map_err(|err| err.to_string())?;
    let source_path = resolve_source_path_for_ai(&state.catalog, photo_id)?;

    let linear = apx_raw::decode_linear(&source_path, None).map_err(|err| err.to_string())?;
    let camera_model = match (&photo.camera_make, &photo.camera_model) {
        (Some(make), Some(model)) => format!("{make} {model}"),
        (Some(make), None) => make.clone(),
        (None, Some(model)) => model.clone(),
        (None, None) => String::new(),
    };
    let bytes = apx_export::dng::encode_linear_dng(&linear, &camera_model)
        .map_err(|err| err.to_string())?;

    let stem = source_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Foto");
    let dest_path = source_path.with_file_name(format!("{stem}.dng"));
    std::fs::write(&dest_path, bytes)
        .map_err(|err| format!("Datei '{}' nicht schreibbar: {err}", dest_path.display()))?;
    Ok(dest_path.display().to_string())
}

// ---- Fortgeschrittenes: Fokus-/HDR-/Panorama-/Astro-Stacking (Phase 9
// Schritt 8, siehe PLAN.md, DECISIONS.md ADR-0035 Punkt 2) -----------------
//
// Reine, deterministische Algorithmen leben komplett in `apx-stacking`
// (keine externe Registrierungs-/Stitching-Bibliothek) — diese Commands
// sind reine Verdrahtung: Quellfotos in voller Auflösung rendern (dieselbe
// `render_photo_full_resolution` wie Schritt 6, kein zweiter Rendering-
// Codepfad), Algorithmus aufrufen, Ergebnis als neues Katalogfoto
// importieren und per Stapel (`Catalog::create_stack`, Phase 9 Schritt 1)
// mit den Quellbildern verknüpfen.

/// Das Ergebnis eines Stacking-Commands — die neu importierte Foto-ID und
/// der Stapel, der sie mit den Quellfotos verknüpft.
#[derive(Debug, Clone, Serialize)]
pub struct StackResultDto {
    pub photo_id: String,
    pub stack_id: String,
    pub width: u32,
    pub height: u32,
}

fn parse_photo_ids(photo_ids: Vec<String>) -> Result<Vec<apx_core::PhotoId>, String> {
    photo_ids.into_iter().map(parse_photo_id).collect()
}

/// Rendert jedes Foto in `photo_ids` in voller Auflösung — gemeinsame
/// Grundlage für alle vier Stacking-Commands unten.
fn render_photos_full_resolution(
    state: &AppState,
    photo_ids: &[apx_core::PhotoId],
) -> Result<Vec<(u32, u32, Vec<u8>)>, String> {
    photo_ids
        .iter()
        .map(|&id| {
            let (_, width, height, pixels) = render_photo_full_resolution(state, id)?;
            Ok((width, height, pixels))
        })
        .collect()
}

/// Importiert ein synthetisiertes Stacking-Ergebnis als neues Katalogfoto
/// im selben Ordner wie das erste Quellfoto und verknüpft es per Stapel
/// mit allen Quellfotos — die neue Zeile bekommt selbst kein EDL/keine
/// EXIF-Aufnahmewerte (ein synthetisiertes Bild wurde nicht fotografiert),
/// nur Breite/Höhe/Dateigröße/Hash wie jedes andere importierte Foto.
fn import_stack_result_photo(
    state: &AppState,
    source_ids: &[apx_core::PhotoId],
    stack_name: &str,
    filename_suffix: &str,
    width: u32,
    height: u32,
    pixels: &[u8],
) -> Result<StackResultDto, String> {
    let first_source = source_ids
        .first()
        .ok_or_else(|| "Stacking braucht mindestens ein Quellfoto".to_string())?;
    let first_photo = state
        .catalog
        .get_photo(*first_source)
        .map_err(|err| err.to_string())?;
    let folder = state
        .catalog
        .get_folder(first_photo.folder_id)
        .map_err(|err| err.to_string())?;

    let bytes = apx_export::format::encode_rgba8(
        width,
        height,
        pixels,
        apx_export::format::ExportFormat::Png,
        &apx_export::format::EncodeOptions::default(),
    )
    .map_err(|err| err.to_string())?;

    let stem = first_photo
        .filename
        .rsplit_once('.')
        .map(|(stem, _)| stem)
        .unwrap_or(first_photo.filename.as_str());
    let mut dest_path = folder.path.join(format!("{stem}_{filename_suffix}.png"));
    let mut counter = 1u32;
    while dest_path.exists() {
        dest_path = folder
            .path
            .join(format!("{stem}_{filename_suffix}_{counter}.png"));
        counter += 1;
    }
    std::fs::write(&dest_path, &bytes)
        .map_err(|err| format!("Datei '{}' nicht schreibbar: {err}", dest_path.display()))?;

    let content_hash = crate::import::compute_content_hash(&dest_path)?;
    let file_size = std::fs::metadata(&dest_path)
        .map_err(|err| err.to_string())?
        .len();
    let filename = dest_path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| "Dateiname ist kein gültiges UTF-8".to_string())?
        .to_string();

    let new_photo = apx_catalog::NewPhoto {
        folder_id: first_photo.folder_id,
        filename,
        file_size,
        file_mtime: time::OffsetDateTime::now_utc(),
        content_hash: Some(content_hash),
        width: Some(width),
        height: Some(height),
        orientation: 1,
        camera_make: None,
        camera_model: None,
        lens: None,
        iso: None,
        shutter: None,
        aperture: None,
        focal_length: None,
        captured_at: Some(time::OffsetDateTime::now_utc()),
        gps_lat: None,
        gps_lon: None,
    };
    let (result_photo_id, _) = state
        .catalog
        .upsert_photo(&new_photo)
        .map_err(|err| err.to_string())?;

    let mut stack_photo_ids = vec![result_photo_id];
    stack_photo_ids.extend_from_slice(source_ids);
    let stack_id = state
        .catalog
        .create_stack(Some(stack_name), &stack_photo_ids)
        .map_err(|err| err.to_string())?;

    Ok(StackResultDto {
        photo_id: result_photo_id.to_string(),
        stack_id: stack_id.to_string(),
        width,
        height,
    })
}

/// Fokus-Stacking (`apx_stacking::focus`) über bereits ausgerichtete
/// Aufnahmen — für jeden Pixel wird die schärfste Quelle übernommen.
#[tauri::command]
pub fn stack_focus(
    state: State<'_, AppState>,
    photo_ids: Vec<String>,
) -> Result<StackResultDto, String> {
    let ids = parse_photo_ids(photo_ids)?;
    let rendered = render_photos_full_resolution(&state, &ids)?;
    let (width, height) = rendered
        .first()
        .map(|(w, h, _)| (*w, *h))
        .ok_or_else(|| "keine Fotos übergeben".to_string())?;
    let refs: Vec<&[u8]> = rendered.iter().map(|(_, _, px)| px.as_slice()).collect();
    let stacked = apx_stacking::focus::focus_stack_rgba8(&refs, width, height)
        .map_err(|err| err.to_string())?;
    import_stack_result_photo(
        &state,
        &ids,
        "Fokus-Stack",
        "fokus_stack",
        width,
        height,
        &stacked,
    )
}

/// HDR-Zusammenführung (`apx_stacking::hdr`) über eine Belichtungsreihe —
/// jedes Quellfoto braucht eine EXIF-Belichtungszeit.
#[tauri::command]
pub fn stack_hdr(
    state: State<'_, AppState>,
    photo_ids: Vec<String>,
) -> Result<StackResultDto, String> {
    let ids = parse_photo_ids(photo_ids)?;
    let rendered = render_photos_full_resolution(&state, &ids)?;
    let (width, height) = rendered
        .first()
        .map(|(w, h, _)| (*w, *h))
        .ok_or_else(|| "keine Fotos übergeben".to_string())?;

    let mut exposure_seconds = Vec::with_capacity(ids.len());
    for &id in &ids {
        let photo = state.catalog.get_photo(id).map_err(|err| err.to_string())?;
        let shutter = photo.shutter.ok_or_else(|| {
            format!(
                "Foto '{}' hat keine EXIF-Belichtungszeit — HDR-Zusammenführung braucht sie für jede Aufnahme",
                photo.filename
            )
        })?;
        exposure_seconds.push(shutter);
    }
    let exposures: Vec<apx_stacking::hdr::Exposure> = rendered
        .iter()
        .zip(exposure_seconds.iter())
        .map(|((_, _, px), &seconds)| apx_stacking::hdr::Exposure {
            pixels: px.as_slice(),
            exposure_seconds: seconds,
        })
        .collect();
    let merged = apx_stacking::hdr::hdr_merge_rgba8(&exposures, width, height)
        .map_err(|err| err.to_string())?;
    import_stack_result_photo(
        &state,
        &ids,
        "HDR-Zusammenführung",
        "hdr",
        width,
        height,
        &merged,
    )
}

/// Panorama-Zusammenführung (`apx_stacking::panorama`) — **v1 nur
/// Verschiebungs-Registrierung** (siehe dessen Moduldoku), jedes Foto
/// nach dem ersten wird per Phasenkorrelation gegen das erste
/// ausgerichtet.
#[tauri::command]
pub fn stack_panorama(
    state: State<'_, AppState>,
    photo_ids: Vec<String>,
) -> Result<StackResultDto, String> {
    let ids = parse_photo_ids(photo_ids)?;
    let rendered = render_photos_full_resolution(&state, &ids)?;
    let (width, height) = rendered
        .first()
        .map(|(w, h, _)| (*w, *h))
        .ok_or_else(|| "keine Fotos übergeben".to_string())?;

    let reference = rendered[0].2.as_slice();
    let mut offsets: Vec<(i32, i32)> = vec![(0, 0)];
    for (_, _, pixels) in rendered.iter().skip(1) {
        let offset = apx_stacking::panorama::estimate_shift_rgba8(reference, pixels, width, height)
            .map_err(|err| err.to_string())?;
        offsets.push(offset);
    }
    let images: Vec<apx_stacking::panorama::PositionedImage> = rendered
        .iter()
        .zip(offsets.iter())
        .map(
            |((_, _, px), &(offset_x, offset_y))| apx_stacking::panorama::PositionedImage {
                pixels: px.as_slice(),
                offset_x,
                offset_y,
            },
        )
        .collect();
    let (out_width, out_height, stitched) =
        apx_stacking::panorama::stitch_shift_rgba8(&images, width, height)
            .map_err(|err| err.to_string())?;
    import_stack_result_photo(
        &state, &ids, "Panorama", "panorama", out_width, out_height, &stitched,
    )
}

/// Astro-Stacking (`apx_stacking::astro`) — Sigma-geclipptes Mittel über
/// viele Kurzbelichtungen, registriert per Phasenkorrelation gegen die
/// erste. `sigma` steuert die Ausreißer-Schwelle (größer = toleranter),
/// `None` verwendet einen moderaten Vorgabewert.
#[tauri::command]
pub fn stack_astro(
    state: State<'_, AppState>,
    photo_ids: Vec<String>,
    sigma: Option<f32>,
) -> Result<StackResultDto, String> {
    let ids = parse_photo_ids(photo_ids)?;
    let rendered = render_photos_full_resolution(&state, &ids)?;
    let (width, height) = rendered
        .first()
        .map(|(w, h, _)| (*w, *h))
        .ok_or_else(|| "keine Fotos übergeben".to_string())?;
    let refs: Vec<&[u8]> = rendered.iter().map(|(_, _, px)| px.as_slice()).collect();
    let stacked = apx_stacking::astro::register_and_stack_astro_rgba8(
        &refs,
        width,
        height,
        sigma.unwrap_or(2.5),
    )
    .map_err(|err| err.to_string())?;
    import_stack_result_photo(
        &state,
        &ids,
        "Astro-Stack",
        "astro_stack",
        width,
        height,
        &stacked,
    )
}

// ---- Fortgeschrittenes: Skript-API (Rhai) + Plugin-System (Phase 9
// Schritt 9, siehe PLAN.md, DECISIONS.md ADR-0035 Punkt 3) ------------------

/// Führt ein Rhai-Skript gegen den aktuellen Bearbeitungsstand von
/// `photo_id` aus (`apx_script::run_script` — schmale, primitiv-
/// typisierte API auf die Grundeinstellungs-Regler, siehe dessen
/// Moduldoku) und committet das Ergebnis wie jede andere Entwickeln-
/// Bearbeitung.
#[tauri::command]
pub fn run_develop_script(
    state: State<'_, AppState>,
    photo_id: String,
    script: String,
) -> Result<(), String> {
    let photo_id = parse_photo_id(photo_id)?;
    let edl = resolve_current_edl(&state.catalog, photo_id)?;
    let updated = apx_script::run_script(edl, &script).map_err(|err| err.to_string())?;
    let envelope = apx_pipeline::edl::to_envelope(&updated).map_err(|err| err.to_string())?;
    state
        .catalog
        .commit_edit(photo_id, &envelope, Some("Skript"))
        .map_err(|err| err.to_string())?;
    Ok(())
}

/// Lädt ein Plugin (`apx-plugin-host`, prüft die ABI-Version hart, siehe
/// dessen Moduldoku) und wendet dessen Custom-Effekt auf `photo_id` in
/// voller Auflösung an — schreibt das Ergebnis als neue PNG-Datei neben
/// dem Original (derselbe `write_derived_png`-Mechanismus wie Schritt 6:
/// EDL/Katalogeintrag des Originalfotos bleiben unverändert).
#[tauri::command]
pub fn run_plugin_custom_effect(
    state: State<'_, AppState>,
    photo_id: String,
    plugin_path: String,
    param: f32,
) -> Result<String, String> {
    let photo_id = parse_photo_id(photo_id)?;
    let (source_path, width, height, mut pixels) = render_photo_full_resolution(&state, photo_id)?;
    let plugin = apx_plugin_host::LoadedPlugin::load(std::path::Path::new(&plugin_path))
        .map_err(|err| err.to_string())?;
    plugin
        .apply_custom_effect_rgba8(&mut pixels, width, height, param)
        .map_err(|err| err.to_string())?;
    write_derived_png(&source_path, "plugin", width, height, &pixels)
}

// ---- Fortgeschrittenes: Kollaborationsmodus (Phase 9 Schritt 10, siehe
// PLAN.md, DECISIONS.md ADR-0035 Punkt 4) -----------------------------------
//
// Asynchroner Export→Weitergabe→Import→Konfliktauflösung-Ablauf statt
// Echtzeit-Mehrbenutzer-Modus (kein Live-Cursor/keine Präsenz/kein CRDT) —
// `apx-catalog` bleibt dabei unverändert ein einzelner `Mutex<Connection>`
// (ADR-0008). Eine `.apxs`-Datei enthält **keine Pixel-Bytes**, nur den
// committeten EDL-Stand je Foto, gematcht über den bereits vorhandenen
// `content_hash` — spiegelt `ApxPresetFile`/`ApxTemplateFile` oben.

/// Ein einzelnes Foto in einer `.apxs`-Freigabedatei — `edl` ist bewusst
/// eingebettetes JSON (wie `ApxPresetFile::edl_subset`), kein noch einmal
/// string-kodierter String.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ApxSharePhoto {
    content_hash: String,
    filename: String,
    edl: apx_core::EdlEnvelope,
    /// RFC3339 — der `edits.created_at`-Zeitstempel des exportierten
    /// Bearbeitungsstands, Grundlage für die "zuletzt geändert
    /// gewinnt"-Standardregel beim Import.
    edited_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ApxShareFile {
    schema_version: u32,
    name: String,
    photos: Vec<ApxSharePhoto>,
}

const APX_SHARE_SCHEMA_VERSION: u32 = 1;

fn format_share_timestamp(at: time::OffsetDateTime) -> Result<String, String> {
    at.format(&time::format_description::well_known::Rfc3339)
        .map_err(|err| format!("Zeitstempel nicht formatierbar: {err}"))
}

/// Öffnet einen Speichern-Dialog und schreibt die aktuellen Bearbeitungs-
/// stände von `photo_ids` als `.apxs`-Datei. Fotos ohne `content_hash`
/// (z. B. vor Schritt 8.2 importiert) können nicht gematcht werden und
/// werden übersprungen — bleiben am Ende keine übrig, schlägt der Befehl
/// fehl statt eine leere Datei zu schreiben. `None`, wenn der Dialog
/// abgebrochen wurde.
#[tauri::command]
pub async fn export_catalog_share(
    app: AppHandle,
    state: State<'_, AppState>,
    photo_ids: Vec<String>,
    name: String,
) -> Result<Option<String>, String> {
    let ids = parse_photo_ids(photo_ids)?;
    let mut photos = Vec::new();
    for id in ids {
        let photo = state.catalog.get_photo(id).map_err(|err| err.to_string())?;
        let Some(content_hash) = photo.content_hash.clone() else {
            continue;
        };
        let (edl, edited_at) = match state
            .catalog
            .current_edit(id)
            .map_err(|err| err.to_string())?
        {
            apx_catalog::HistoryPosition::Neutral => (
                apx_pipeline::edl::to_envelope(&apx_pipeline::edl::EdlV4::default())
                    .map_err(|err| err.to_string())?,
                time::OffsetDateTime::UNIX_EPOCH,
            ),
            apx_catalog::HistoryPosition::At(entry) => (entry.edl, entry.created_at),
        };
        photos.push(ApxSharePhoto {
            content_hash,
            filename: photo.filename,
            edl,
            edited_at: format_share_timestamp(edited_at)?,
        });
    }
    if photos.is_empty() {
        return Err(
            "Keines der ausgewählten Fotos hat einen Inhalts-Hash — keine Freigabe möglich"
                .to_string(),
        );
    }

    let file = ApxShareFile {
        schema_version: APX_SHARE_SCHEMA_VERSION,
        name: name.clone(),
        photos,
    };
    let json = serde_json::to_string_pretty(&file)
        .map_err(|err| format!("Freigabe nicht serialisierbar: {err}"))?;

    let (tx, rx) = tokio::sync::oneshot::channel();
    app.dialog()
        .file()
        .add_filter("Aperture X Freigabe", &["apxs"])
        .set_file_name(format!("{name}.apxs"))
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
    Ok(Some(path.to_string_lossy().to_string()))
}

/// Ein Foto aus der Freigabedatei, zu dem kein lokales Foto mit demselben
/// `content_hash` gefunden wurde.
#[derive(Debug, Clone, Serialize)]
pub struct UnmatchedSharePhotoDto {
    pub filename: String,
    pub content_hash: String,
}

/// Ein Foto, bei dem der lokale und der importierte Bearbeitungsstand
/// voneinander abweichen. `incoming_edl_json` wird unverändert an
/// [`resolve_share_conflict`] zurückgereicht — das Frontend muss den
/// Inhalt nicht selbst verstehen.
#[derive(Debug, Clone, Serialize)]
pub struct ShareConflictDto {
    pub photo_id: String,
    pub filename: String,
    pub incoming_edl_json: String,
    /// Vorschlag nach der Standardregel „zuletzt geändert gewinnt" — nur
    /// eine Anzeige-Empfehlung, keine automatische Anwendung.
    pub prefer_incoming: bool,
    pub local_edited_at: String,
    pub incoming_edited_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ImportShareResultDto {
    pub name: String,
    pub unmatched: Vec<UnmatchedSharePhotoDto>,
    /// Dateinamen mit identischem EDL-Inhalt — kein Konflikt, keine Aktion.
    pub unchanged: Vec<String>,
    pub conflicts: Vec<ShareConflictDto>,
}

/// Öffnet einen Öffnen-Dialog, liest eine `.apxs`-Datei und berechnet den
/// Abgleich gegen den lokalen Katalog (`Catalog::find_photo_by_content_hash`
/// + `Catalog::diff_share_edit`) — **schreibt dabei nichts** in den
/// Katalog. Konflikte müssen einzeln über [`resolve_share_conflict`]
/// aufgelöst werden. `None`, wenn der Dialog abgebrochen wurde.
#[tauri::command]
pub async fn import_catalog_share(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Option<ImportShareResultDto>, String> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    app.dialog()
        .file()
        .add_filter("Aperture X Freigabe", &["apxs"])
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
    let file: ApxShareFile = serde_json::from_str(&text).map_err(|err| {
        format!(
            "Datei '{}' ist keine gültige .apxs-Datei: {err}",
            path.display()
        )
    })?;
    if file.schema_version > APX_SHARE_SCHEMA_VERSION {
        return Err(format!(
            "Datei '{}' hat Schema-Version {}, diese Aperture-X-Version kennt nur {}",
            path.display(),
            file.schema_version,
            APX_SHARE_SCHEMA_VERSION
        ));
    }

    let mut unmatched = Vec::new();
    let mut unchanged = Vec::new();
    let mut conflicts = Vec::new();
    for shared in file.photos {
        let Some(local) = state
            .catalog
            .find_photo_by_content_hash(&shared.content_hash)
            .map_err(|err| err.to_string())?
        else {
            unmatched.push(UnmatchedSharePhotoDto {
                filename: shared.filename,
                content_hash: shared.content_hash,
            });
            continue;
        };
        let (local_edl, local_edited_at) = match state
            .catalog
            .current_edit(local.id)
            .map_err(|err| err.to_string())?
        {
            apx_catalog::HistoryPosition::Neutral => (
                apx_pipeline::edl::to_envelope(&apx_pipeline::edl::EdlV4::default())
                    .map_err(|err| err.to_string())?,
                time::OffsetDateTime::UNIX_EPOCH,
            ),
            apx_catalog::HistoryPosition::At(entry) => (entry.edl, entry.created_at),
        };
        let incoming_edited_at = time::OffsetDateTime::parse(
            &shared.edited_at,
            &time::format_description::well_known::Rfc3339,
        )
        .map_err(|err| format!("Ungültiger Zeitstempel in Freigabedatei: {err}"))?;

        match state.catalog.diff_share_edit(
            &local_edl,
            local_edited_at,
            &shared.edl,
            incoming_edited_at,
        ) {
            apx_catalog::ShareDiff::Identical => unchanged.push(shared.filename),
            apx_catalog::ShareDiff::Conflict { prefer_incoming } => {
                let incoming_edl_json =
                    shared.edl.to_json_string().map_err(|err| err.to_string())?;
                conflicts.push(ShareConflictDto {
                    photo_id: local.id.to_string(),
                    filename: shared.filename,
                    incoming_edl_json,
                    prefer_incoming,
                    local_edited_at: format_share_timestamp(local_edited_at)?,
                    incoming_edited_at: format_share_timestamp(incoming_edited_at)?,
                });
            }
        }
    }

    Ok(Some(ImportShareResultDto {
        name: file.name,
        unmatched,
        unchanged,
        conflicts,
    }))
}

/// Löst einen einzelnen von [`import_catalog_share`] gemeldeten Konflikt
/// auf. `resolution`: `"mine"` (nichts tun — der lokale Stand bleibt
/// aktiv), `"theirs"` (den importierten Stand committen, wie jede andere
/// Entwickeln-Bearbeitung) oder `"virtual_copy"` (eine neue virtuelle
/// Kopie anlegen — Schritt 1 — und den importierten Stand dort committen,
/// sodass beide Bearbeitungen erhalten bleiben).
#[tauri::command]
pub fn resolve_share_conflict(
    state: State<'_, AppState>,
    photo_id: String,
    incoming_edl_json: String,
    resolution: String,
) -> Result<(), String> {
    let photo_id = parse_photo_id(photo_id)?;
    let envelope =
        apx_core::EdlEnvelope::from_json_str(&incoming_edl_json).map_err(|err| err.to_string())?;
    match resolution.as_str() {
        "mine" => Ok(()),
        "theirs" => {
            state
                .catalog
                .commit_edit(photo_id, &envelope, Some("Kollaboration: übernommen"))
                .map_err(|err| err.to_string())?;
            Ok(())
        }
        "virtual_copy" => {
            let copy_id = state
                .catalog
                .create_virtual_copy(photo_id)
                .map_err(|err| err.to_string())?;
            state
                .catalog
                .commit_edit(
                    copy_id,
                    &envelope,
                    Some("Kollaboration: als virtuelle Kopie"),
                )
                .map_err(|err| err.to_string())?;
            Ok(())
        }
        other => Err(format!(
            "Unbekannte Konfliktauflösung '{other}' — erwartet 'mine'/'theirs'/'virtual_copy'"
        )),
    }
}

// ---- Fortgeschrittenes: Tethered Shooting (Phase 9 Schritt 11, siehe
// PLAN.md, DECISIONS.md ADR-0035 Punkt 5) ------------------------------------
//
// Ablauf: Kamera erkennen (`tether_connect`) → auslösen + herunterladen +
// automatisches Import-Preset anwenden (`tether_capture`, wiederverwendet
// `import::run_with_mode` aus Phase 3/5 unverändert). `apx_tether`s
// `Gphoto2Backend` ist nur mit dem Cargo-Feature `tethering` kompiliert
// (standardmäßig aus, siehe `THIRD_PARTY.md`) — ohne das Feature (dieser
// Build) läuft ausschließlich `FakeBackend`, klar als Simulation markiert.

#[derive(Debug, Clone, Serialize)]
pub struct CameraInfoDto {
    pub model: String,
    pub port: String,
    /// `true`, wenn dieser Build ohne das `tethering`-Feature kompiliert
    /// wurde (oder — mit Feature — keine echte Kamera gefunden wurde) und
    /// daher `apx_tether::FakeBackend` statt echter Hardware antwortet.
    /// Das Frontend zeigt das explizit an, statt eine echte
    /// Kameraverbindung vorzutäuschen.
    pub simulated: bool,
}

#[cfg(feature = "tethering")]
fn new_tether_backend() -> (Box<dyn apx_tether::TetherBackend>, bool) {
    match apx_tether::gphoto2_backend::Gphoto2Backend::new() {
        Ok(backend) => (Box::new(backend), false),
        Err(_) => (Box::new(apx_tether::FakeBackend::disconnected()), true),
    }
}

#[cfg(not(feature = "tethering"))]
fn new_tether_backend() -> (Box<dyn apx_tether::TetherBackend>, bool) {
    (
        Box::new(apx_tether::FakeBackend::connected("Simulierte Kamera")),
        true,
    )
}

/// (Neu-)Verbindet zu einer Kamera und erkennt sie — `None`, wenn keine
/// gefunden wurde. Speichert das Backend in `AppState::tether`, damit
/// [`tether_capture`] dieselbe Verbindung (und deren Aufnahmezähler beim
/// `FakeBackend`) wiederverwendet.
#[tauri::command]
pub fn tether_connect(state: State<'_, AppState>) -> Result<Option<CameraInfoDto>, String> {
    let (mut backend, simulated) = new_tether_backend();
    let detected = backend.detect_camera().map_err(|err| err.to_string())?;
    let dto = detected.as_ref().map(|info| CameraInfoDto {
        model: info.model.clone(),
        port: info.port.clone(),
        simulated,
    });
    let mut guard = state
        .tether
        .lock()
        .map_err(|_| "Tethering-Status ist blockiert (vergiftete Sperre)".to_string())?;
    *guard = Some(backend);
    Ok(dto)
}

/// Löst den Import-Modus/das Umbenennungsmuster für [`tether_capture`]
/// auf: `None` (kein Preset gewählt) bleibt beim bisherigen Verhalten
/// (Datei bleibt im `tether_download_dir`); ein benanntes Preset (Phase 3
/// Schritt 4/Phase 5 Schritt 9) wählt Kopieren/Verschieben in den dort
/// hinterlegten Zielordner plus optionales Umbenennungsmuster.
fn resolve_tether_import_settings(
    paths: &apx_core::AppPaths,
    preset_name: Option<&str>,
) -> Result<(crate::import::ImportMode, Option<String>), String> {
    let Some(name) = preset_name else {
        return Ok((crate::import::ImportMode::AddInPlace, None));
    };
    let presets = crate::import::presets::load_presets(&paths.import_presets_file())?;
    let preset = presets
        .into_iter()
        .find(|p| p.name == name)
        .ok_or_else(|| format!("Import-Preset '{name}' nicht gefunden"))?;
    let mode = match preset.mode {
        crate::import::presets::PresetMode::AddInPlace => crate::import::ImportMode::AddInPlace,
        crate::import::presets::PresetMode::Copy { target_dir } => {
            crate::import::ImportMode::Copy(target_dir)
        }
        crate::import::presets::PresetMode::Move { target_dir } => {
            crate::import::ImportMode::Move(target_dir)
        }
    };
    Ok((mode, preset.rename_pattern))
}

/// Löst über das verbundene Backend aus, lädt die Aufnahme in
/// `AppPaths::tether_download_dir` herunter und importiert sie über den
/// bestehenden Import-Pfad (`import::run_with_mode`, Phase 3/5 —
/// derselbe Scan-/Metadaten-/Thumbnail-Ablauf wie ein normaler
/// Ordner-Import, hier auf ein Ein-Datei-Verzeichnis angewandt). Ein
/// Fehler, wenn zuvor kein [`tether_connect`] mit erkannter Kamera lief.
#[tauri::command]
pub async fn tether_capture(
    app: AppHandle,
    state: State<'_, AppState>,
    preset_name: Option<String>,
) -> Result<Option<PhotoDto>, String> {
    let dest_dir = state.paths.tether_download_dir();
    let downloaded_path = {
        let mut guard = state
            .tether
            .lock()
            .map_err(|_| "Tethering-Status ist blockiert (vergiftete Sperre)".to_string())?;
        let backend = guard
            .as_deref_mut()
            .ok_or_else(|| "Keine Kamera verbunden — zuerst tether_connect aufrufen".to_string())?;
        backend
            .capture_and_download(&dest_dir)
            .map_err(|err| err.to_string())?
    };

    let (mode, rename_pattern) =
        resolve_tether_import_settings(&state.paths, preset_name.as_deref())?;

    let catalog = state.catalog.clone();
    let cache_root = state.paths.preview_cache_dir();
    let app_for_blocking = app.clone();
    let dest_dir_for_blocking = dest_dir.clone();

    tokio::task::spawn_blocking(move || {
        let events = crate::import::TauriEvents(&app_for_blocking);
        let cancel = tokio_util::sync::CancellationToken::new();
        crate::import::run_with_mode(
            &events,
            &catalog,
            &cache_root,
            &dest_dir_for_blocking,
            &cancel,
            &mode,
            rename_pattern.as_deref(),
        );
    })
    .await
    .map_err(|err| format!("Import-Task ist abgestürzt: {err}"))?;

    let content_hash = crate::import::compute_content_hash(&downloaded_path)?;
    let photo = state
        .catalog
        .find_photo_by_content_hash(&content_hash)
        .map_err(|err| err.to_string())?;
    Ok(photo.map(PhotoDto::from))
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
        let edl = apx_pipeline::edl::EdlV4 {
            basic: apx_pipeline::edl::BasicAdjustments {
                exposure_ev: marker,
                ..apx_pipeline::edl::BasicAdjustments::NEUTRAL
            },
            ..apx_pipeline::edl::EdlV4::neutral()
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
                    .expect("sollte gültiges EdlV4 ergeben");
                assert_eq!(parsed.basic.exposure_ev, 0.7);
            }
            HistoryPositionDto::Neutral => panic!("sollte nicht neutral sein"),
        }
    }

    #[test]
    fn edit_history_entry_dto_carries_sequence_and_roundtrips_edl() {
        let entry = apx_catalog::EditHistoryEntry {
            id: apx_core::EditHistoryId::new(),
            photo_id: apx_core::PhotoId::new(),
            sequence: 3,
            label: Some("Vor Weißabgleich".to_string()),
            edl: sample_envelope(0.4),
            created_at: time::OffsetDateTime::now_utc(),
        };
        let dto = EditHistoryEntryDto::try_from(entry).expect("sollte gelingen");
        assert_eq!(dto.sequence, 3);
        assert_eq!(dto.label.as_deref(), Some("Vor Weißabgleich"));
        assert!(!dto.created_at.is_empty());
        let roundtripped =
            apx_core::EdlEnvelope::from_json_str(&dto.edl_json).expect("sollte wieder parsen");
        let parsed =
            apx_pipeline::edl::from_envelope(&roundtripped).expect("sollte gültiges EdlV4 ergeben");
        assert_eq!(parsed.basic.exposure_ev, 0.4);
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

    // ---- Tethered Shooting (Phase 9 Schritt 11) ----------------------------

    #[test]
    fn tether_import_settings_default_to_add_in_place_without_a_preset() {
        let tmp = tempfile::tempdir().expect("Temp-Verzeichnis");
        let paths = apx_core::AppPaths::rooted_at(tmp.path()).expect("sollte anlegen");
        let (mode, rename_pattern) =
            resolve_tether_import_settings(&paths, None).expect("sollte auflösen");
        assert!(matches!(mode, crate::import::ImportMode::AddInPlace));
        assert_eq!(rename_pattern, None);
    }

    #[test]
    fn tether_import_settings_apply_a_named_preset() {
        let tmp = tempfile::tempdir().expect("Temp-Verzeichnis");
        let paths = apx_core::AppPaths::rooted_at(tmp.path()).expect("sollte anlegen");
        let target_dir = tmp.path().join("bibliothek");
        crate::import::presets::upsert_preset(
            &paths.import_presets_file(),
            crate::import::presets::ImportPreset {
                name: "Studio".to_string(),
                mode: crate::import::presets::PresetMode::Copy {
                    target_dir: target_dir.clone(),
                },
                rename_pattern: Some("{date}_{seq}".to_string()),
            },
        )
        .expect("sollte speichern");

        let (mode, rename_pattern) =
            resolve_tether_import_settings(&paths, Some("Studio")).expect("sollte auflösen");
        assert!(matches!(mode, crate::import::ImportMode::Copy(dir) if dir == target_dir));
        assert_eq!(rename_pattern.as_deref(), Some("{date}_{seq}"));
    }

    #[test]
    fn tether_import_settings_reject_an_unknown_preset_name() {
        let tmp = tempfile::tempdir().expect("Temp-Verzeichnis");
        let paths = apx_core::AppPaths::rooted_at(tmp.path()).expect("sollte anlegen");
        assert!(resolve_tether_import_settings(&paths, Some("Unbekannt")).is_err());
    }

    #[test]
    fn tether_backend_without_the_tethering_feature_is_clearly_marked_simulated() {
        // Dieser Build hat das `tethering`-Feature nicht aktiv (siehe
        // apx-app/Cargo.toml — Standard-CI/Sandbox ohne libgphoto2) —
        // `new_tether_backend` muss das ehrlich als Simulation
        // kennzeichnen, nicht stillschweigend eine echte Kamera
        // vortäuschen.
        let (mut backend, simulated) = new_tether_backend();
        assert!(simulated);
        assert!(backend.detect_camera().expect("ok").is_some());
    }

    #[test]
    fn export_format_parses_all_seven_known_strings() {
        // Phase 11 Schritt 2 (siehe DECISIONS.md ADR-0038): "psd"/"jxl"
        // kamen hinzu, HEIF bleibt bewusst außen vor.
        use apx_export::format::ExportFormat;
        let cases = [
            ("jpeg", ExportFormat::Jpeg),
            ("png", ExportFormat::Png),
            ("tiff", ExportFormat::Tiff),
            ("webp", ExportFormat::WebP),
            ("avif", ExportFormat::Avif),
            ("psd", ExportFormat::Psd),
            ("jxl", ExportFormat::Jxl),
        ];
        for (raw, expected) in cases {
            assert_eq!(parse_export_format(raw).expect("sollte parsen"), expected);
        }
    }

    #[test]
    fn export_format_rejects_unknown_string() {
        let err = parse_export_format("heif").expect_err("sollte fehlschlagen");
        assert!(err.contains("heif"));
    }
}
