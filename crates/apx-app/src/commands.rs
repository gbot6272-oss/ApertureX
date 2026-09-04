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
    /// Voller EXIF/IPTC-Editor (Phase 12 Schritt 4, siehe `DECISIONS.md`
    /// ADR-0039) — frei benannte Zusatzfelder, siehe
    /// `apx_catalog::Photo::custom_metadata`.
    pub custom_metadata: std::collections::BTreeMap<String, String>,
    /// Video als Katalog-Asset (Phase 16 Schritt 4, siehe `DECISIONS.md`
    /// ADR-0043) — `"photo"` oder `"video"`, siehe
    /// `apx_catalog::NewPhoto::media_kind`s Moduldoku.
    pub media_kind: String,
    pub duration_ms: Option<i64>,
    pub video_codec: Option<String>,
    pub has_audio: Option<bool>,
    pub frame_rate: Option<f32>,
}

impl From<apx_catalog::Photo> for PhotoDto {
    fn from(photo: apx_catalog::Photo) -> Self {
        Self {
            media_kind: photo.media_kind,
            duration_ms: photo.duration_ms,
            video_codec: photo.video_codec,
            has_audio: photo.has_audio,
            frame_rate: photo.frame_rate,
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
            custom_metadata: photo.custom_metadata,
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

#[derive(Debug, Clone, Serialize)]
pub struct LensProfileSuggestionDto {
    id: String,
    display_name: String,
}

/// Ordnet einen EXIF-Objektiv-String automatisch einem Objektivprofil zu
/// (Phase 12 Schritt 3 Teil A, siehe `DECISIONS.md` ADR-0039) — dünner
/// Wrapper um `apx_pipeline::lens_profiles::match_profile_for_lens_string`,
/// das jetzt gegen die echte LensFun-Datenbank sucht statt gegen drei
/// handgepflegte Beispielprofile. Kein DB-/State-Zugriff nötig, reine
/// Funktionsauswertung.
#[tauri::command]
pub fn resolve_lens_profile(lens: Option<String>) -> Option<LensProfileSuggestionDto> {
    let lens = lens?;
    let lens = lens.trim();
    if lens.is_empty() {
        return None;
    }
    apx_pipeline::lens_profiles::match_profile_for_lens_string(lens).map(|profile| {
        LensProfileSuggestionDto {
            id: profile.id,
            display_name: profile.display_name,
        }
    })
}

#[derive(Debug, Clone, Deserialize)]
pub struct CalibrationPointDto {
    x: f32,
    y: f32,
}

/// Berechnet aus vom Nutzer markierten, in der Realität geraden Linien
/// einen Verzeichnungskoeffizienten (Phase 12 Schritt 3 Teil B, siehe
/// `DECISIONS.md` ADR-0039) — dünner Wrapper um
/// `apx_ai::lens_calibration::calibrate_distortion_k1`. Direkt als
/// `LensCorrectionAdjustment::custom_distortion_k1` im EDL speicherbar,
/// keine separate Profildatenbank/-datei nötig.
#[tauri::command]
pub fn calibrate_lens_distortion(lines: Vec<Vec<CalibrationPointDto>>) -> Result<f32, String> {
    let lines: Vec<Vec<apx_ai::lens_calibration::StraightLinePoint>> = lines
        .into_iter()
        .map(|line| {
            line.into_iter()
                .map(|p| apx_ai::lens_calibration::StraightLinePoint { x: p.x, y: p.y })
                .collect()
        })
        .collect();
    apx_ai::lens_calibration::calibrate_distortion_k1(&lines).map_err(|err| err.to_string())
}

// ---- Perspektive/Upright: automatische Kantenerkennung (Phase 13 Schritt 4,
// siehe DECISIONS.md ADR-0040-Nachtrag II) -----------------------------------

fn parse_upright_mode(mode: &str) -> Result<apx_pipeline::edl::UprightMode, String> {
    match mode {
        "Off" => Ok(apx_pipeline::edl::UprightMode::Off),
        "Auto" => Ok(apx_pipeline::edl::UprightMode::Auto),
        "Level" => Ok(apx_pipeline::edl::UprightMode::Level),
        "Vertical" => Ok(apx_pipeline::edl::UprightMode::Vertical),
        "Full" => Ok(apx_pipeline::edl::UprightMode::Full),
        "Guided" => Ok(apx_pipeline::edl::UprightMode::Guided),
        other => Err(format!("unbekannter Upright-Modus '{other}'")),
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct UprightCorrectionDto {
    pub rotate_degrees: f32,
    pub horizontal: f32,
}

/// Automatische Perspektive/Upright-Kantenerkennung — dünner Wrapper um
/// `apx_ai::upright::detect_from_linear_rgb` (dasselbe Analyse-Auflösung-
/// über-`TileCache`-Muster wie `generate_ai_mask` oben, obwohl dies keine
/// KI-Funktion im engeren Sinn ist, siehe dessen Moduldoku zu Canny/Hough).
/// `mode` muss einer von `UprightMode`s sechs Werten sein; für `"Off"`/
/// `"Guided"` liefert die Analyse ohnehin nur Nullen (siehe
/// `apx_ai::upright::detect`s Doku) — der Befehl nimmt sie trotzdem an,
/// statt sie als Fehler abzulehnen, für einen einfacheren Aufrufer.
#[tauri::command]
pub fn detect_upright_correction(
    state: State<'_, AppState>,
    photo_id: String,
    mode: String,
) -> Result<UprightCorrectionDto, String> {
    let photo_id = parse_photo_id(photo_id)?;
    let mode = parse_upright_mode(&mode)?;
    let source_path = resolve_source_path_for_ai(&state.catalog, photo_id)?;

    let max_edge = Some(apx_ai::segmentation::ANALYSIS_MAX_EDGE);
    let linear = state
        .tile_cache
        .get_or_decode(photo_id, max_edge, || {
            apx_raw::decode_linear(&source_path, max_edge)
        })
        .map_err(|err| err.to_string())?;

    let correction =
        apx_ai::upright::detect_from_linear_rgb(&linear.pixels, linear.width, linear.height, mode);
    Ok(UprightCorrectionDto {
        rotate_degrees: correction.rotate_degrees,
        horizontal: correction.horizontal,
    })
}

// ---- Adobe-DCP-Farbprofil-Import (Phase 13 Schritt 3) ----------------------

#[derive(Debug, Clone, Serialize)]
pub struct DcpProfileDataDto {
    pub name: String,
    pub hue_divisions: u32,
    pub sat_divisions: u32,
    pub val_divisions: u32,
    pub hue_sat_map: Vec<[f32; 3]>,
    pub tone_curve: Vec<[f32; 2]>,
}

impl From<apx_pipeline::edl::DcpProfileData> for DcpProfileDataDto {
    fn from(data: apx_pipeline::edl::DcpProfileData) -> Self {
        Self {
            name: data.name,
            hue_divisions: data.hue_divisions,
            sat_divisions: data.sat_divisions,
            val_divisions: data.val_divisions,
            hue_sat_map: data.hue_sat_map,
            tone_curve: data.tone_curve,
        }
    }
}

/// Öffnet einen Datei-Dialog für eine `.dcp`-Datei, parst sie
/// (`apx_pipeline::dcp_profile::parse_dcp_bytes`, siehe dessen Moduldoku)
/// und liefert die für `stages::calibration` relevanten Profildaten
/// zurück — `None`, wenn der Dialog abgebrochen wurde. Das Frontend
/// speichert das Ergebnis direkt in `CalibrationAdjustment::dcp_profile`
/// (derselbe „einmal auflösen, als Zahlen im EDL ablegen"-Ansatz wie bei
/// KI-Ausfüllen, Phase 13 Schritt 1) — dieser Command liest die Datei
/// nur, schreibt nichts in den Katalog.
///
/// **Kein eingebautes Profil enthalten** — der Nutzer bringt eine eigene
/// `.dcp`-Datei mit (z. B. Adobes kostenlos herunterladbare
/// Kameraprofile), genau wie beim LensFun-Kalibrier-Assistenten
/// (Phase 12 Schritt 3) niemand die LensFun-Datenbank selbst mitliefert,
/// sondern nur den Code, sie zu nutzen.
#[tauri::command]
pub async fn import_dcp_profile(app: AppHandle) -> Result<Option<DcpProfileDataDto>, String> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    app.dialog()
        .file()
        .add_filter("Adobe-Kameraprofil", &["dcp"])
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
    let bytes = std::fs::read(&path)
        .map_err(|err| format!("Datei '{}' nicht lesbar: {err}", path.display()))?;
    let profile =
        apx_pipeline::dcp_profile::parse_dcp_bytes(&bytes).map_err(|err| err.to_string())?;
    let Some(hue_sat_map) = profile.hue_sat_map else {
        return Err(format!(
            "'{}' enthält keine HueSatMap-Look-Daten (nur Farbmatrizen) — diese Datei liefert derzeit keinen sichtbaren Effekt, siehe apx-pipeline::dcp_profile-Moduldoku",
            path.display()
        ));
    };
    Ok(Some(hue_sat_map.into()))
}

// ---- Filter-/LUT-Bibliothek (Phase 16 Schritt 1) ---------------------------

// `Deserialize` zusätzlich zum bisherigen `Serialize` (Phase 16
// Schritt 9): `apply_lut_filter_to_video` nimmt einen bereits im
// Frontend gewählten Filter (Bibliotheks-Eintrag oder eigener
// `.cube`-Import) als Parameter entgegen statt ihn erneut zu berechnen.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LutFilterDataDto {
    pub name: String,
    pub size: u32,
    pub table: Vec<f32>,
    pub domain_min: [f32; 3],
    pub domain_max: [f32; 3],
}

impl From<LutFilterDataDto> for apx_pipeline::edl::LutFilterData {
    fn from(dto: LutFilterDataDto) -> Self {
        Self {
            name: dto.name,
            size: dto.size,
            table: dto.table,
            domain_min: dto.domain_min,
            domain_max: dto.domain_max,
        }
    }
}

impl From<apx_pipeline::lut_cube::ParsedLut> for LutFilterDataDto {
    fn from(parsed: apx_pipeline::lut_cube::ParsedLut) -> Self {
        Self {
            name: parsed
                .title
                .unwrap_or_else(|| "Unbenannter Filter".to_string()),
            size: parsed.size,
            table: parsed.table,
            domain_min: parsed.domain_min,
            domain_max: parsed.domain_max,
        }
    }
}

/// Öffnet einen Datei-Dialog für eine `.cube`-3D-LUT-Datei, parst sie
/// (`apx_pipeline::lut_cube::parse_cube_bytes`, siehe dessen Moduldoku)
/// und liefert das Ergebnis zurück — `None`, wenn der Dialog abgebrochen
/// wurde. Dasselbe „Dialog öffnen, Datei parsen, fertige Daten
/// zurückgeben — nur das Frontend legt sie im EDL ab"-Muster wie
/// `import_dcp_profile`.
///
/// Wenn kein eigener Dateiname im Dokument steht (keine `TITLE`-Zeile),
/// wird der Dateiname ohne Endung als Anzeigename verwendet — dieselbe
/// Bequemlichkeit, die die meisten frei verfügbaren `.cube`-Dateien
/// ohnehin ohne `TITLE`-Zeile ausliefern.
#[tauri::command]
pub async fn import_lut_cube_file(app: AppHandle) -> Result<Option<LutFilterDataDto>, String> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    app.dialog()
        .file()
        .add_filter("3D-LUT (.cube)", &["cube"])
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
    let bytes = std::fs::read(&path)
        .map_err(|err| format!("Datei '{}' nicht lesbar: {err}", path.display()))?;
    let mut parsed =
        apx_pipeline::lut_cube::parse_cube_bytes(&bytes).map_err(|err| err.to_string())?;
    if parsed.title.is_none() {
        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Unbenannter Filter");
        parsed.title = Some(stem.to_string());
    }
    Ok(Some(parsed.into()))
}

impl From<apx_pipeline::edl::LutFilterData> for LutFilterDataDto {
    fn from(data: apx_pipeline::edl::LutFilterData) -> Self {
        Self {
            name: data.name,
            size: data.size,
            table: data.table,
            domain_min: data.domain_min,
            domain_max: data.domain_max,
        }
    }
}

/// Liefert die fünf eingebauten, selbst erstellten Filter-Looks
/// (`apx_pipeline::builtin_luts`, siehe dessen Moduldoku — original
/// erstellt statt von einer externen Quelle heruntergeladen, dieselbe
/// Rolle wie Lightrooms eigene mitgelieferte "Creative"-Profile). Reine
/// Berechnung, kein Datei-/Netzwerkzugriff — anders als
/// `import_lut_cube_file` kein `async`.
#[tauri::command]
pub fn list_builtin_lut_filters() -> Vec<LutFilterDataDto> {
    apx_pipeline::builtin_luts::BuiltinLut::ALL
        .into_iter()
        .map(|kind| apx_pipeline::builtin_luts::generate(kind, 17).into())
        .collect()
}

// ---- Video-Bearbeitung (Phase 16 Schritt 6) --------------------------------

/// Schneidet `[start_ms, end_ms)` aus einem Video-Asset — nicht
/// destruktiv (siehe `DECISIONS.md` ADR-0043): das Original bleibt
/// unverändert, das Ergebnis wird als **neues** Katalog-Asset im selben
/// Ordner abgelegt (`<stem>_trim[_N].<ext>`), dieselbe Konvention wie
/// gespeicherte Stapel-/Panorama-Ergebnisse an anderer Stelle in dieser
/// Datei. Erst ein schneller `-c copy`-Stream-Kopier-Versuch (verlustfrei,
/// aber an den nächsten Keyframes statt frame-genau — dieselbe
/// Einschränkung wie bei jedem Videoschnittprogramm im "schnellen"
/// Modus), bei Fehlschlag ein vollständiger Re-Encode
/// (`libx264`/`aac`) — siehe PLAN.md Schritt 6: "verlustfreier
/// ffmpeg-Stream-Copy wo möglich, sonst Re-Encode".
#[tauri::command]
pub fn trim_video(
    state: State<'_, AppState>,
    photo_id: String,
    start_ms: i64,
    end_ms: i64,
) -> Result<PhotoDto, String> {
    let photo_id = parse_photo_id(photo_id)?;
    if start_ms < 0 || end_ms <= start_ms {
        return Err("Ungültiger Zeitbereich: Ende muss nach Anfang liegen".to_string());
    }

    let photo = state
        .catalog
        .get_photo(photo_id)
        .map_err(|err| err.to_string())?;
    if photo.media_kind != "video" {
        return Err("Nur Videos können geschnitten werden".to_string());
    }
    let folder = state
        .catalog
        .get_folder(photo.folder_id)
        .map_err(|err| err.to_string())?;
    let source_path = folder.path.join(&photo.filename);

    let dest_path = unique_sibling_video_path(&folder.path, &source_path, "trim");

    run_ffmpeg_trim(&source_path, &dest_path, start_ms, end_ms, true)
        .or_else(|_| run_ffmpeg_trim(&source_path, &dest_path, start_ms, end_ms, false))?;

    register_video_result_as_new_photo(&state, photo.folder_id, &dest_path)
}

/// `<stem>_<suffix>.<ext>`, mit `_<n>`-Zähler bei Namenskollision — die
/// gemeinsame nicht-destruktive Zielpfad-Konvention aller Phase-16-
/// Video-Bearbeitungs-Commands (`trim_video`, `denoise_video_audio`,
/// `add_video_audio_track`): das Ergebnis landet immer als neue Datei
/// im selben Ordner wie die Quelle, niemals als Überschreiben.
fn unique_sibling_video_path(folder_path: &Path, source_path: &Path, suffix: &str) -> PathBuf {
    let stem = source_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("video");
    let ext = source_path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("mp4");
    let mut dest_path = folder_path.join(format!("{stem}_{suffix}.{ext}"));
    let mut counter = 1u32;
    while dest_path.exists() {
        dest_path = folder_path.join(format!("{stem}_{suffix}_{counter}.{ext}"));
        counter += 1;
    }
    dest_path
}

/// Legt eine bereits fertig auf der Platte liegende Video-Datei
/// (Ergebnis von `trim_video`/`denoise_video_audio`/
/// `add_video_audio_track`) als **neues** Katalog-Asset an — dieselbe
/// Metadaten-Extraktion+Thumbnail-Erzeugung, die vorher in `trim_video`
/// inline stand, jetzt geteilt zwischen allen drei Video-Bearbeitungs-
/// Commands.
fn register_video_result_as_new_photo(
    state: &State<'_, AppState>,
    folder_id: apx_core::FolderId,
    dest_path: &Path,
) -> Result<PhotoDto, String> {
    let file_size = std::fs::metadata(dest_path)
        .map_err(|err| format!("Ergebnisdatei nicht lesbar: {err}"))?
        .len();
    let content_hash = crate::import::compute_content_hash(dest_path)?;
    let video_meta = crate::import::video::extract_video_metadata(dest_path)?;
    let filename = dest_path
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| "Ungültiger Dateiname".to_string())?
        .to_string();

    let new_photo = apx_catalog::NewPhoto {
        folder_id,
        filename,
        file_size,
        file_mtime: time::OffsetDateTime::now_utc(),
        content_hash: Some(content_hash),
        width: video_meta.width,
        height: video_meta.height,
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
        media_kind: "video".to_string(),
        duration_ms: video_meta.duration_ms,
        video_codec: video_meta.codec,
        has_audio: video_meta.has_audio,
        frame_rate: video_meta.frame_rate,
    };
    let (new_photo_id, _) = state
        .catalog
        .upsert_photo(&new_photo)
        .map_err(|err| err.to_string())?;

    if let Err(err) = crate::import::thumbnails::generate_one(
        &state.catalog,
        &state.paths.preview_cache_dir(),
        new_photo_id,
        dest_path,
    ) {
        tracing::warn!(%err, "Thumbnail für neues Video-Asset nicht erzeugbar");
    }

    let saved = state
        .catalog
        .get_photo(new_photo_id)
        .map_err(|err| err.to_string())?;
    Ok(saved.into())
}

/// `stream_copy = true`: `-c copy` (schnell, verlustfrei, an den
/// nächsten Keyframes) — `false`: vollständiger Re-Encode (langsamer,
/// aber frame-genau, funktioniert auch, wenn Stream-Copy am
/// Container/Codec scheitert). `-ss` **vor** `-i` (schnelles Grobsuchen)
/// ist bei `-c copy` sogar erforderlich — ffmpeg kann einen
/// kopierten Stream nur an Paketgrenzen (Keyframes) schneiden, ein
/// Suchen nach dem Dekodieren würde daran nichts ändern.
fn run_ffmpeg_trim(
    source: &std::path::Path,
    dest: &std::path::Path,
    start_ms: i64,
    end_ms: i64,
    stream_copy: bool,
) -> Result<(), String> {
    let start_secs = start_ms as f64 / 1000.0;
    let duration_secs = (end_ms - start_ms) as f64 / 1000.0;

    let mut cmd = std::process::Command::new("ffmpeg");
    cmd.args(["-y", "-ss", &format!("{start_secs}"), "-i"])
        .arg(source);
    cmd.args(["-t", &format!("{duration_secs}")]);
    if stream_copy {
        cmd.args(["-c", "copy"]);
    } else {
        cmd.args([
            "-c:v", "libx264", "-crf", "18", "-preset", "medium", "-c:a", "aac",
        ]);
    }
    cmd.arg(dest);

    let output = cmd
        .output()
        .map_err(|err| format!("ffmpeg nicht startbar (ist ffmpeg installiert?): {err}"))?;
    if !output.status.success() {
        let _ = std::fs::remove_file(dest); // unvollständige Datei nicht liegen lassen
        return Err(format!(
            "ffmpeg-Schnitt fehlgeschlagen: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(())
}

/// Automatisches Zuschneiden (Phase 16 Schritt 7, siehe `DECISIONS.md`
/// ADR-0043): erkennt Szenenwechsel über ffmpegs nativen `scdet`-Filter
/// (seit ffmpeg 4.3, keine externe Modell-Abhängigkeit — echte, gegen
/// die ffmpeg-Dokumentation verifizierte Bordfunktion statt eines
/// eigenen Bild-Differenz-Algorithmus). Gibt sortierte, deduplizierte
/// Zeitstempel (Millisekunden) jedes erkannten Wechsels zurück; das
/// Frontend nutzt diese als Sprungmarken auf der Zeitleiste und um den
/// "aktuellen Szenenabschnitt" automatisch als Trimm-Vorschlag
/// vorzubelegen (siehe `VideoPlayer.tsx`). `threshold` folgt `scdet`s
/// eigener Skala (0–100, Standardwert des Filters ist 10.0 — niedriger
/// = empfindlicher/mehr erkannte Wechsel).
#[tauri::command]
pub fn detect_video_scene_changes(
    state: State<'_, AppState>,
    photo_id: String,
    threshold: Option<f32>,
) -> Result<Vec<i64>, String> {
    let photo_id = parse_photo_id(photo_id)?;
    let photo = state
        .catalog
        .get_photo(photo_id)
        .map_err(|err| err.to_string())?;
    if photo.media_kind != "video" {
        return Err("Szenenerkennung funktioniert nur bei Videos".to_string());
    }
    let folder = state
        .catalog
        .get_folder(photo.folder_id)
        .map_err(|err| err.to_string())?;
    let source_path = folder.path.join(&photo.filename);

    run_ffmpeg_scene_detect(&source_path, threshold.unwrap_or(10.0))
}

/// `scdet` protokolliert jeden erkannten Wechsel als eine
/// `av_log`-Info-Zeile auf `stderr` in der Form
/// `lavfi.scd.score: <wert>, lavfi.scd.time: <sekunden>` — `-f null -`
/// verwirft die eigentliche Bildausgabe (nur die Metadaten interessieren
/// hier), `-an` überspringt die Tonspur (unnötig für Bild-Szenenerkennung,
/// spart Laufzeit).
fn run_ffmpeg_scene_detect(source: &std::path::Path, threshold: f32) -> Result<Vec<i64>, String> {
    let output = std::process::Command::new("ffmpeg")
        .args(["-hide_banner", "-nostats", "-v", "info", "-i"])
        .arg(source)
        .args([
            "-an",
            "-filter:v",
            &format!("scdet=threshold={threshold}"),
            "-f",
            "null",
            "-",
        ])
        .output()
        .map_err(|err| format!("ffmpeg nicht startbar (ist ffmpeg installiert?): {err}"))?;

    // scdet meldet Erfolg über den regulären Nulldevice-Encode-Pfad —
    // ein Fehlschlag hier bedeutet i. d. R. eine nicht dekodierbare
    // Datei, nicht "keine Szenenwechsel gefunden" (das liefert einfach
    // eine leere Liste).
    if !output.status.success() {
        return Err(format!(
            "Szenenerkennung fehlgeschlagen: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    let mut timestamps: Vec<i64> = stderr
        .lines()
        .filter_map(|line| {
            let marker = "lavfi.scd.time:";
            let idx = line.find(marker)?;
            let rest = line[idx + marker.len()..].trim();
            let value = rest
                .split(|c: char| !(c.is_ascii_digit() || c == '.'))
                .next()?;
            value.parse::<f64>().ok()
        })
        .map(|secs| (secs * 1000.0).round() as i64)
        .collect();
    timestamps.sort_unstable();
    timestamps.dedup();
    Ok(timestamps)
}

/// Geräuschreduktion (Phase 16 Schritt 8, siehe `DECISIONS.md`
/// ADR-0043) — nativer ffmpeg-Filter `afftdn` (reine FFT-Spektral-
/// Subtraktion, kein externes Modell nötig, anders als das
/// RNN-basierte `arnndn`, das laut ADR-0043-Recherche einen separaten
/// Modell-Download voraussetzen würde). Nicht destruktiv wie
/// `trim_video`: Ergebnis landet als neues Katalog-Asset
/// (`<stem>_denoise[_N].<ext>`), der Video-Stream bleibt per `-c:v copy`
/// unverändert — nur die Tonspur wird neu kodiert. `strength` steuert
/// `afftdn`s `nr`-Parameter (0.01–97; `afftdn`s eigener Standardwert ist
/// 12, hier als `"medium"` übernommen).
#[tauri::command]
pub fn denoise_video_audio(
    state: State<'_, AppState>,
    photo_id: String,
    strength: String,
) -> Result<PhotoDto, String> {
    let photo_id = parse_photo_id(photo_id)?;
    let photo = state
        .catalog
        .get_photo(photo_id)
        .map_err(|err| err.to_string())?;
    if photo.media_kind != "video" {
        return Err("Geräuschreduktion funktioniert nur bei Videos".to_string());
    }
    if photo.has_audio != Some(true) {
        return Err("Dieses Video hat keine Tonspur".to_string());
    }
    let folder = state
        .catalog
        .get_folder(photo.folder_id)
        .map_err(|err| err.to_string())?;
    let source_path = folder.path.join(&photo.filename);

    let nr: f32 = match strength.as_str() {
        "low" => 6.0,
        "high" => 24.0,
        _ => 12.0, // "medium" — zugleich afftdns eigener Standardwert
    };

    let dest_path = unique_sibling_video_path(&folder.path, &source_path, "denoise");
    let output = std::process::Command::new("ffmpeg")
        .args(["-y", "-i"])
        .arg(&source_path)
        .args([
            "-c:v",
            "copy",
            "-af",
            &format!("afftdn=nr={nr}"),
            "-c:a",
            "aac",
        ])
        .arg(&dest_path)
        .output()
        .map_err(|err| format!("ffmpeg nicht startbar (ist ffmpeg installiert?): {err}"))?;
    if !output.status.success() {
        let _ = std::fs::remove_file(&dest_path);
        return Err(format!(
            "Geräuschreduktion fehlgeschlagen: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    register_video_result_as_new_photo(&state, photo.folder_id, &dest_path)
}

/// Musik/Sounds zu einem Video hinzufügen (Phase 16 Schritt 8) —
/// dieselbe Audio-Mix-Technik wie die Diashow-Musikuntermalung
/// (`export_slideshow_video`, ADR-0034 Punkt 3), hier auf ein bereits
/// bestehendes Video-Asset angewendet statt beim Rendern einer neuen
/// Diashow. Nicht destruktiv: neues Katalog-Asset
/// (`<stem>_audio[_N].<ext>`), Video-Stream per `-c:v copy` unverändert.
///
/// `mode == "mix"` mischt die neue Spur zur vorhandenen Tonspur dazu
/// (`amix`, `duration=first` — die Ausgabelänge folgt der *Original*-
/// Tonspur, damit eine kürzere/längere Musikdatei die Videolänge nicht
/// verändert) und fällt automatisch auf `"replace"` zurück, wenn das
/// Video gar keine Tonspur hat (nichts zum Mischen da). `mode ==
/// "replace"` ersetzt die Tonspur vollständig; ein explizites `-t` auf
/// die aus dem Katalog bekannte Originallänge verhindert hier, dass
/// eine längere Musikdatei die Ausgabe über das Video hinaus verlängert
/// (kürzere Musik lässt den Rest einfach stumm, dasselbe Verhalten wie
/// bei den meisten Schnittprogrammen). `music_volume` skaliert nur die
/// neu hinzugefügte Spur (1.0 = unverändert).
#[tauri::command]
pub fn add_video_audio_track(
    state: State<'_, AppState>,
    photo_id: String,
    audio_path: String,
    mode: String,
    music_volume: Option<f32>,
) -> Result<PhotoDto, String> {
    let photo_id = parse_photo_id(photo_id)?;
    let photo = state
        .catalog
        .get_photo(photo_id)
        .map_err(|err| err.to_string())?;
    if photo.media_kind != "video" {
        return Err("Tonspur hinzufügen funktioniert nur bei Videos".to_string());
    }
    let folder = state
        .catalog
        .get_folder(photo.folder_id)
        .map_err(|err| err.to_string())?;
    let source_path = folder.path.join(&photo.filename);

    let audio_source = Path::new(&audio_path);
    if !audio_source.is_file() {
        return Err(format!("Audiodatei '{audio_path}' nicht gefunden"));
    }

    let volume = music_volume.unwrap_or(1.0).max(0.0);
    let should_mix = mode == "mix" && photo.has_audio == Some(true);

    let dest_path = unique_sibling_video_path(&folder.path, &source_path, "audio");
    let mut cmd = std::process::Command::new("ffmpeg");
    cmd.args(["-y", "-i"])
        .arg(&source_path)
        .arg("-i")
        .arg(audio_source);
    if should_mix {
        cmd.args([
            "-filter_complex",
            &format!(
                "[1:a]volume={volume}[music];[0:a][music]amix=inputs=2:duration=first:dropout_transition=0[aout]"
            ),
            "-map",
            "0:v",
            "-map",
            "[aout]",
        ]);
    } else {
        cmd.args([
            "-map",
            "0:v",
            "-map",
            "1:a",
            "-af",
            &format!("volume={volume}"),
        ]);
    }
    cmd.args(["-c:v", "copy", "-c:a", "aac"]);
    if let Some(duration_ms) = photo.duration_ms {
        cmd.args(["-t", &format!("{}", duration_ms as f64 / 1000.0)]);
    }
    cmd.arg(&dest_path);

    let output = cmd
        .output()
        .map_err(|err| format!("ffmpeg nicht startbar (ist ffmpeg installiert?): {err}"))?;
    if !output.status.success() {
        let _ = std::fs::remove_file(&dest_path);
        return Err(format!(
            "Tonspur hinzufügen fehlgeschlagen: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    register_video_result_as_new_photo(&state, photo.folder_id, &dest_path)
}

/// Filter/LUT auf Video anwenden (Phase 16 Schritt 9, siehe
/// `DECISIONS.md` ADR-0043) — wendet dieselbe trilineare `.cube`-LUT-
/// Interpolation an, die Schritt 1 für Fotos gebaut hat
/// (`apx_pipeline::stages::lut_filter::apply`), framegenau auf jedes
/// Bild eines Videos. Bewusst **global** (keine Pinselstriche wie bei
/// Fotos — eine pro-Frame-Maske wäre für ein bewegtes Bild ein
/// eigenständiges, deutlich größeres Feature und nicht Teil des
/// "Basis-Videoschnitt"-Anspruchs dieser Phase). Nicht destruktiv:
/// neues Katalog-Asset (`<stem>_lut[_N].<ext>`), Original unverändert;
/// die Original-Tonspur wird unangetastet in die Ausgabe übernommen
/// (`-c:a copy`), nur das Bild durchläuft die LUT.
///
/// **Bewusst kein GPU-Pfad** (siehe ADR-0043: `apx-pipeline` ist reines
/// CPU-Rust) — bei langen/hochauflösenden Videos entsprechend langsam,
/// siehe die Performance-Messung in Schritt 11.
#[tauri::command]
pub fn apply_lut_filter_to_video(
    state: State<'_, AppState>,
    photo_id: String,
    lut: LutFilterDataDto,
    strength: f32,
) -> Result<PhotoDto, String> {
    let photo_id = parse_photo_id(photo_id)?;
    let photo = state
        .catalog
        .get_photo(photo_id)
        .map_err(|err| err.to_string())?;
    if photo.media_kind != "video" {
        return Err("Filter/LUT auf Video funktioniert nur bei Videos".to_string());
    }
    let (Some(width), Some(height)) = (photo.width, photo.height) else {
        return Err("Video-Auflösung unbekannt (fehlende Metadaten)".to_string());
    };
    let fps = photo.frame_rate.unwrap_or(30.0).max(1.0);
    let folder = state
        .catalog
        .get_folder(photo.folder_id)
        .map_err(|err| err.to_string())?;
    let source_path = folder.path.join(&photo.filename);

    let adjustment = apx_pipeline::edl::LutFilterAdjustment {
        strength: strength.clamp(0.0, 1.0),
        lut: Some(lut.into()),
        strokes: Vec::new(),
    };

    let dest_path = unique_sibling_video_path(&folder.path, &source_path, "lut");
    run_ffmpeg_apply_lut_to_video(&source_path, &dest_path, width, height, fps, adjustment)?;

    register_video_result_as_new_photo(&state, photo.folder_id, &dest_path)
}

/// Zwei gekoppelte `ffmpeg`-Subprozesse: der erste dekodiert `source`
/// zu rohen RGBA8-Frames auf `stdout` (`-f rawvideo -pix_fmt rgba`),
/// ein eigener Thread liest sie framegenau, wendet
/// `stages::lut_filter::apply` darauf an und schreibt das Ergebnis in
/// `stdin` des zweiten `ffmpeg`, der die transformierten Frames zu
/// `dest` re-kodiert und dabei per zweitem Input (`source` erneut,
/// `-map 1:a?`) die Original-Tonspur unverändert hinüberkopiert —
/// dasselbe zwei-Prozesse-Pipe-Muster wie ein klassischer
/// Video-Filterpipeline-Aufbau, hier in Rust statt einer einzigen
/// `-vf`-ffmpeg-Filterkette, weil die LUT-Logik in
/// `apx_pipeline::stages::lut_filter` liegt (dieselbe Implementierung
/// wie bei Fotos, keine zweite LUT-Anwendung in einer ffmpeg-eigenen
/// Filtersprache).
fn run_ffmpeg_apply_lut_to_video(
    source: &Path,
    dest: &Path,
    width: u32,
    height: u32,
    fps: f32,
    // Wert statt Referenz: der Frame-Pumpen-Thread braucht `'static`
    // (`std::thread::spawn`), ein geklonter Wert ist einfacher als
    // `std::thread::scope` für diesen einen Aufrufort — die LUT-Tabelle
    // ist mit höchstens einigen zehntausend Floats klein genug, dass das
    // Klonen nicht ins Gewicht fällt.
    adjustment: apx_pipeline::edl::LutFilterAdjustment,
) -> Result<(), String> {
    use std::io::{Read, Write};

    let mut decode = std::process::Command::new("ffmpeg")
        .args(["-v", "error", "-i"])
        .arg(source)
        .args(["-f", "rawvideo", "-pix_fmt", "rgba", "-"])
        .stdout(std::process::Stdio::piped())
        .spawn()
        .map_err(|err| format!("ffmpeg (Dekodieren) nicht startbar: {err}"))?;
    let mut decode_stdout = decode
        .stdout
        .take()
        .ok_or_else(|| "ffmpeg-Dekodier-Ausgabe nicht verfügbar".to_string())?;

    let mut encode = std::process::Command::new("ffmpeg")
        .args(["-v", "error", "-y", "-f", "rawvideo", "-pix_fmt", "rgba"])
        .args([
            "-s",
            &format!("{width}x{height}"),
            "-r",
            &format!("{fps}"),
            "-i",
            "-",
        ])
        .arg("-i")
        .arg(source)
        .args([
            "-map", "0:v", "-map", "1:a?", "-c:v", "libx264", "-crf", "18", "-preset", "medium",
            "-c:a", "copy",
        ])
        .arg(dest)
        .stdin(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|err| format!("ffmpeg (Kodieren) nicht startbar: {err}"))?;
    let mut encode_stdin = encode
        .stdin
        .take()
        .ok_or_else(|| "ffmpeg-Kodier-Eingabe nicht verfügbar".to_string())?;

    let frame_bytes = (width as usize) * (height as usize) * 4;
    let pump = std::thread::spawn(move || -> Result<(), String> {
        let mut frame = vec![0u8; frame_bytes];
        loop {
            match decode_stdout.read_exact(&mut frame) {
                Ok(()) => {
                    let filtered =
                        apx_pipeline::stages::lut_filter::apply(&frame, width, height, &adjustment);
                    encode_stdin
                        .write_all(&filtered)
                        .map_err(|err| format!("Schreiben an ffmpeg fehlgeschlagen: {err}"))?;
                }
                Err(err) if err.kind() == std::io::ErrorKind::UnexpectedEof => break,
                Err(err) => return Err(format!("Lesen von ffmpeg fehlgeschlagen: {err}")),
            }
        }
        Ok(()) // `encode_stdin` fällt hier aus dem Gültigkeitsbereich → EOF für ffmpeg
    });

    let pump_result = pump
        .join()
        .map_err(|_| "Frame-Pipeline-Thread abgestürzt".to_string())?;

    let decode_status = decode
        .wait()
        .map_err(|err| format!("Warten auf ffmpeg (Dekodieren) fehlgeschlagen: {err}"))?;
    let encode_output = {
        let status = encode
            .wait()
            .map_err(|err| format!("Warten auf ffmpeg (Kodieren) fehlgeschlagen: {err}"))?;
        let mut stderr = String::new();
        if let Some(mut pipe) = encode.stderr.take() {
            let _ = pipe.read_to_string(&mut stderr);
        }
        (status, stderr)
    };

    pump_result?;
    if !decode_status.success() {
        let _ = std::fs::remove_file(dest);
        return Err("ffmpeg (Dekodieren) fehlgeschlagen".to_string());
    }
    if !encode_output.0.success() {
        let _ = std::fs::remove_file(dest);
        return Err(format!(
            "ffmpeg (Kodieren) fehlgeschlagen: {}",
            encode_output.1
        ));
    }
    Ok(())
}

/// Ein einzelner Zeitachsen-Eintrag (Phase 17 Schritt 1, siehe
/// `DECISIONS.md` ADR-0045) — `photo_id` referenziert entweder ein
/// Video (dann sind `in_ms`/`out_ms` Pflicht) oder ein Foto (dann ist
/// `hold_seconds` maßgeblich, `in_ms`/`out_ms` werden ignoriert). Ein
/// eigener DTO statt Wiederverwendung von `SlideshowTitleCardOptions`/
/// `-VideoOptions`, weil eine Zeitachse Video- und Foto-Einträge
/// gemischt in einer Liste trägt.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TimelineItemInput {
    pub photo_id: String,
    pub in_ms: Option<i64>,
    pub out_ms: Option<i64>,
    pub hold_seconds: Option<f32>,
    /// Tempo-Faktor für Video-Einträge (Phase 17 Schritt 2, siehe
    /// `DECISIONS.md` ADR-0045) — `None`/fehlend = `1.0` (unverändert).
    /// Wird für Foto-/Titel-Einträge ignoriert (deren "Tempo" ist
    /// bereits `hold_seconds`).
    pub speed: Option<f32>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoTimelineOptions {
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    /// Je Übergang einer von `"cut"`/`"fade"`/`"dissolve"`/`"wipe_left"`/
    /// `"wipe_right"`/`"slide_up"`/`"slide_down"`/`"circle_open"` (Phase
    /// 17 Schritt 3, siehe `parse_timeline_transition_kind`) — Länge
    /// muss `items.len() - 1` sein.
    pub transitions: Vec<String>,
    pub transition_seconds: Option<f32>,
    pub music_path: Option<String>,
    /// Text-/Titel-Overlays (Phase 17 Schritt 4, siehe `DECISIONS.md`
    /// ADR-0045) — Zeiten beziehen sich auf die fertige, verkettete
    /// Sequenz.
    pub text_overlays: Option<Vec<TimelineTextOverlayInput>>,
    /// Bild-in-Bild-/Split-Screen-Overlays (Phase 17 Schritt 7, siehe
    /// `DECISIONS.md` ADR-0045) — Zeiten beziehen sich ebenfalls auf die
    /// fertige, verkettete Sequenz.
    pub pip_overlays: Option<Vec<TimelinePipOverlayInput>>,
}

/// Ein Text-Overlay-Eintrag (Phase 17 Schritt 4) — `position` folgt
/// [`parse_watermark_position`]s Vertrag (`"top_left"` u. Ä.), `font_path`
/// ist wie bei den Diashow-Intro-/Outro-Titelkarten Pflicht (keine
/// eingebettete Schriftart, siehe `watermark`s Moduldoku).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TimelineTextOverlayInput {
    pub text: String,
    pub position: String,
    pub start_seconds: f32,
    pub end_seconds: f32,
    pub font_path: String,
    pub font_size: Option<f32>,
    pub color_rgb: Option<[u8; 3]>,
}

fn build_timeline_text_overlays(
    overlays: &[TimelineTextOverlayInput],
) -> Result<Vec<apx_export::timeline::TimelineTextOverlay>, String> {
    overlays
        .iter()
        .map(|overlay| {
            if overlay.end_seconds <= overlay.start_seconds {
                return Err("Text-Overlay: Ende muss nach dem Start liegen".to_string());
            }
            let font_bytes = std::fs::read(&overlay.font_path).map_err(|err| {
                format!(
                    "Schriftdatei '{}' konnte nicht gelesen werden: {err}",
                    overlay.font_path
                )
            })?;
            Ok(apx_export::timeline::TimelineTextOverlay {
                text: overlay.text.clone(),
                position: parse_watermark_position(&overlay.position)?,
                start_seconds: overlay.start_seconds,
                end_seconds: overlay.end_seconds,
                font_bytes,
                font_size_px: overlay.font_size.unwrap_or(48.0),
                text_color: overlay.color_rgb.unwrap_or([255, 255, 255]),
            })
        })
        .collect()
}

/// Baut aus den rohen Eingabefeldern eines Zeitachsen-Eintrags (Foto-ID +
/// optionale Trim-/Haltedauer-/Tempo-Felder) das passende
/// `TimelineItem` — gemeinsam genutzt von der Haupt-Zeitachse
/// (`items`) und den Bild-in-Bild-Overlays (Phase 17 Schritt 7,
/// [`build_timeline_pip_overlays`]), damit die Video-/Foto-Erkennungs-
/// und Validierungslogik nicht zweimal existiert. Gibt zusätzlich die
/// `FolderId` zurück (die Haupt-Zeitachse braucht sie für den
/// Ziel-Ordner, Bild-in-Bild-Overlays ignorieren sie).
fn build_timeline_item(
    state: &AppState,
    photo_id: &str,
    in_ms: Option<i64>,
    out_ms: Option<i64>,
    hold_seconds: Option<f32>,
    speed: Option<f32>,
) -> Result<(apx_core::FolderId, apx_export::timeline::TimelineItem), String> {
    let parsed_id = parse_photo_id(photo_id.to_string())?;
    let photo = state
        .catalog
        .get_photo(parsed_id)
        .map_err(|err| err.to_string())?;
    let folder = state
        .catalog
        .get_folder(photo.folder_id)
        .map_err(|err| err.to_string())?;
    let source_path = folder.path.join(&photo.filename);

    if photo.media_kind == "video" {
        let in_ms = in_ms.ok_or_else(|| "Video-Eintrag ohne Start-Zeitpunkt".to_string())?;
        let out_ms = out_ms.ok_or_else(|| "Video-Eintrag ohne End-Zeitpunkt".to_string())?;
        if in_ms < 0 || out_ms <= in_ms {
            return Err("Ungültiger Zeitbereich in der Zeitachse".to_string());
        }
        let speed = speed.unwrap_or(1.0);
        if !(0.1..=8.0).contains(&speed) {
            return Err("Tempo-Faktor muss zwischen 0,1 und 8,0 liegen".to_string());
        }
        Ok((
            photo.folder_id,
            apx_export::timeline::TimelineItem::VideoClip {
                source_path,
                in_ms,
                out_ms,
                speed,
            },
        ))
    } else {
        let edl = resolve_current_edl(&state.catalog, parsed_id)?;
        let request = apx_export::engine::ExportRequest::new(
            source_path,
            edl,
            apx_export::format::ExportFormat::Jpeg,
        );
        let (width, height, rgba) =
            apx_export::engine::render_to_pixels(Some(&state.pipeline), &request)
                .map_err(|err| err.to_string())?;
        Ok((
            photo.folder_id,
            apx_export::timeline::TimelineItem::Photo {
                width,
                height,
                rgba,
                hold_seconds: hold_seconds.unwrap_or(3.0).max(0.1),
            },
        ))
    }
}

/// Ein Bild-in-Bild-/Split-Screen-Overlay (Phase 17 Schritt 7, siehe
/// `DECISIONS.md` ADR-0045) — die Quelle (`photo_id` + Trim-/Halte-/
/// Tempo-Felder) folgt demselben Vertrag wie [`TimelineItemInput`],
/// zusätzlich Zeitspanne/Position/Größe für die Einblendung. Split-
/// Screen ist bewusst kein eigener Mechanismus, sondern derselbe
/// Bild-in-Bild-Overlay mit `scale` nahe `1.0` und zwei Einträgen an
/// gegenüberliegenden Positionen (siehe Moduldoku-Verweis in ADR-0045).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TimelinePipOverlayInput {
    pub photo_id: String,
    pub in_ms: Option<i64>,
    pub out_ms: Option<i64>,
    pub hold_seconds: Option<f32>,
    pub speed: Option<f32>,
    pub start_seconds: f32,
    pub end_seconds: f32,
    pub position: String,
    /// Anteil an der Ziel-Videobreite/-höhe (`0.05..=1.0`) — die
    /// Einblendung behält dasselbe Seitenverhältnis wie die
    /// Ziel-Auflösung selbst (siehe `apx_export::timeline::
    /// apply_pip_overlays`s Moduldoku).
    pub scale: f32,
}

fn build_timeline_pip_overlays(
    state: &AppState,
    overlays: &[TimelinePipOverlayInput],
) -> Result<Vec<apx_export::timeline::TimelinePipOverlay>, String> {
    overlays
        .iter()
        .map(|overlay| {
            if overlay.end_seconds <= overlay.start_seconds {
                return Err("Bild-in-Bild-Overlay: Ende muss nach dem Start liegen".to_string());
            }
            if !(0.05..=1.0).contains(&overlay.scale) {
                return Err(
                    "Bild-in-Bild-Overlay: Größe muss zwischen 0,05 und 1,0 liegen".to_string(),
                );
            }
            let (_, source) = build_timeline_item(
                state,
                &overlay.photo_id,
                overlay.in_ms,
                overlay.out_ms,
                overlay.hold_seconds,
                overlay.speed,
            )?;
            Ok(apx_export::timeline::TimelinePipOverlay {
                source,
                start_seconds: overlay.start_seconds,
                end_seconds: overlay.end_seconds,
                position: parse_watermark_position(&overlay.position)?,
                scale: overlay.scale,
            })
        })
        .collect()
}

/// Rendert `items` zu einer neuen Video-Zeitachse (siehe
/// `apx_export::timeline`s Moduldoku für den zweistufigen Rendering-
/// Ansatz) und legt das Ergebnis als neues Katalog-Video im Ordner des
/// ersten Eintrags an (`register_video_result_as_new_photo`,
/// wiederverwendet aus Schritt 6) — nicht-destruktiv wie jeder andere
/// Video-Bearbeitungs-Command dieser Datei: keiner der Quell-Clips
/// wird verändert.
#[tauri::command]
pub fn render_video_timeline(
    state: State<'_, AppState>,
    items: Vec<TimelineItemInput>,
    options: VideoTimelineOptions,
) -> Result<PhotoDto, String> {
    if items.is_empty() {
        return Err("Zeitachse enthält keine Einträge".to_string());
    }
    let transitions: Vec<apx_export::timeline::TimelineTransitionKind> = options
        .transitions
        .iter()
        .map(|t| parse_timeline_transition_kind(t))
        .collect::<Result<_, _>>()?;
    if transitions.len() != items.len() - 1 {
        return Err(format!(
            "Erwartete {} Übergänge für {} Einträge, {} übergeben",
            items.len() - 1,
            items.len(),
            transitions.len()
        ));
    }
    if options.width == 0 || options.height == 0 || options.fps == 0 {
        return Err("Video-Auflösung/Bildrate muss größer null sein".to_string());
    }

    let mut timeline_items = Vec::with_capacity(items.len());
    let mut first_folder_id = None;
    for item in &items {
        let (folder_id, timeline_item) = build_timeline_item(
            &state,
            &item.photo_id,
            item.in_ms,
            item.out_ms,
            item.hold_seconds,
            item.speed,
        )?;
        if first_folder_id.is_none() {
            first_folder_id = Some(folder_id);
        }
        timeline_items.push(timeline_item);
    }

    let folder_id = first_folder_id.ok_or_else(|| "Kein Eintrag in der Zeitachse".to_string())?;
    let folder = state
        .catalog
        .get_folder(folder_id)
        .map_err(|err| err.to_string())?;
    let dest_path = unique_timeline_dest_path(&folder.path);

    let text_overlays =
        build_timeline_text_overlays(options.text_overlays.as_deref().unwrap_or_default())?;
    let pip_overlays =
        build_timeline_pip_overlays(&state, options.pip_overlays.as_deref().unwrap_or_default())?;
    let timeline_options = apx_export::timeline::TimelineExportOptions {
        output_width: options.width,
        output_height: options.height,
        fps: options.fps,
        audio_path: options.music_path.as_ref().map(PathBuf::from),
        text_overlays,
        pip_overlays,
    };

    apx_export::timeline::render_video_timeline(
        &timeline_items,
        &transitions,
        options.transition_seconds.unwrap_or(1.0),
        &timeline_options,
        &dest_path,
    )
    .map_err(|err| err.to_string())?;

    register_video_result_as_new_photo(&state, folder_id, &dest_path)
}

/// `Zeitachse.mp4`, mit `_<n>`-Zähler bei Namenskollision — dieselbe
/// Konvention wie [`unique_sibling_video_path`], nur ohne einen
/// einzelnen Quell-Clip, aus dem ein Dateiname abgeleitet werden
/// könnte (eine Zeitachse kombiniert mehrere Quellen).
fn unique_timeline_dest_path(folder_path: &Path) -> PathBuf {
    let mut dest_path = folder_path.join("Zeitachse.mp4");
    let mut counter = 1u32;
    while dest_path.exists() {
        dest_path = folder_path.join(format!("Zeitachse_{counter}.mp4"));
        counter += 1;
    }
    dest_path
}

// ---- Automatische Untertitel (Phase 17 Schritt 5, siehe DECISIONS.md ------
// ADR-0045) — lokal per Whisper (`apx_ai::subtitles`), hinter dem
// Cargo-Feature `subtitles` (siehe dessen Moduldoku für die Begründung).
// Opt-in-Modell-Download wie das MiDaS-/LaMa-Modell oben.

/// Öffentliche Download-URL des `ggml-base.en`-Modells (whisper.cpp), real
/// aus dessen eigenem `models/download-ggml-model.sh` übernommen
/// (`src="https://huggingface.co/ggerganov/whisper.cpp"`,
/// `pfx="resolve/main/ggml"` -> `ggml-base.en.bin`). **Nicht in dieser
/// Sitzung erreichbar/verifiziert** — `huggingface.co` ist von dieser
/// Entwicklungs-Sandbox aus blockiert, genau wie beim LaMa-Modell
/// (Phase 13, [`LAMA_MODEL_URL`]).
const WHISPER_MODEL_URL: &str =
    "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.en.bin";
/// SHA1 der `base.en`-Modelldatei — anders als beim LaMa-Modell **real
/// aus `whisper.cpp`s eigenem `models/README.md` übernommen** (dessen
/// Prüfsummen-Tabelle, `github.com` ist in dieser Sitzung erreichbar,
/// `huggingface.co` selbst nicht). **Nur SHA1** (40 Hex-Zeichen) — anders
/// als [`MIDAS_MODEL_SHA256`] u. Ä. veröffentlicht `whisper.cpp` für seine
/// Modelle ausschließlich SHA1, keine erfundene SHA-256-Ersatzprüfsumme.
const WHISPER_MODEL_SHA1: &str = "137c40403d78fd54d454da0f9bd998f78703390c";

/// Lädt das Whisper-`base.en`-Untertitel-Modell herunter, prüft die
/// Prüfsumme gegen [`WHISPER_MODEL_SHA1`] und hinterlegt den Pfad in den
/// Einstellungen — dieselbe Verwerfen-bei-Fehlschlag-Logik wie
/// [`download_depth_model`].
#[tauri::command]
pub async fn download_whisper_model(state: State<'_, AppState>) -> Result<String, String> {
    let response = reqwest::get(WHISPER_MODEL_URL)
        .await
        .map_err(|err| format!("Download von '{WHISPER_MODEL_URL}' fehlgeschlagen: {err}"))?;
    if !response.status().is_success() {
        return Err(format!(
            "Download von '{WHISPER_MODEL_URL}' fehlgeschlagen: HTTP {}",
            response.status()
        ));
    }
    let bytes = response
        .bytes()
        .await
        .map_err(|err| format!("Antwort konnte nicht gelesen werden: {err}"))?;

    use sha1::Digest as _;
    let mut hasher = sha1::Sha1::new();
    hasher.update(&bytes);
    // `sha1`s `Digest::finalize()` gibt kein `LowerHex`-fähiges Array
    // zurück (anders als `sha2` oben) — Byte-für-Byte selbst formatieren.
    let actual_hash = hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    if actual_hash != WHISPER_MODEL_SHA1 {
        return Err(format!(
            "Prüfsumme stimmt nicht überein (erwartet {WHISPER_MODEL_SHA1}, erhalten {actual_hash}) — Download verworfen."
        ));
    }

    let dest_dir = state.paths.models_dir();
    std::fs::create_dir_all(&dest_dir).map_err(|err| err.to_string())?;
    let dest_path = dest_dir.join("ggml-base.en.bin");
    std::fs::write(&dest_path, &bytes).map_err(|err| err.to_string())?;

    let path_string = dest_path.to_string_lossy().to_string();
    let settings_path = state.paths.settings_file();
    let mut settings =
        apx_core::Settings::load_or_default(&settings_path).map_err(|err| err.to_string())?;
    settings.ai.whisper_model_path = Some(path_string.clone());
    settings
        .save(&settings_path)
        .map_err(|err| err.to_string())?;

    Ok(path_string)
}

/// Entfernt den hinterlegten Modellpfad (löscht die Datei selbst nicht,
/// siehe [`clear_depth_model_path`]s Begründung).
#[tauri::command]
pub fn clear_whisper_model_path(state: State<'_, AppState>) -> Result<(), String> {
    let path = state.paths.settings_file();
    let mut settings = apx_core::Settings::load_or_default(&path).map_err(|err| err.to_string())?;
    settings.ai.whisper_model_path = None;
    settings.save(&path).map_err(|err| err.to_string())
}

/// Extrahiert die Tonspur von `source_path` als rohes `f32`-PCM, 16 kHz,
/// mono — Whisper verlangt exakt dieses Format (siehe
/// `apx_ai::subtitles`s Moduldoku). Läuft über `ffmpeg`s Stdout-Pipe
/// (`-f f32le -` statt einer Zwischendatei) — dieselbe Subprozess-Technik
/// wie jeder andere `ffmpeg`-Aufruf dieser Datei, nur mit `stdout` statt
/// einer Zieldatei als Ausgabe.
fn extract_audio_pcm_f32(source_path: &Path) -> Result<Vec<f32>, String> {
    let output = std::process::Command::new("ffmpeg")
        .arg("-i")
        .arg(source_path)
        .args(["-vn", "-ac", "1", "-ar", "16000", "-f", "f32le", "-"])
        .output()
        .map_err(|err| format!("ffmpeg nicht startbar (ist ffmpeg installiert?): {err}"))?;
    if !output.status.success() {
        return Err(format!(
            "Tonspur-Extraktion fehlgeschlagen: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    if output.stdout.len() % 4 != 0 {
        return Err("ffmpeg-Ausgabe ist keine gültige f32-PCM-Folge".to_string());
    }
    Ok(output
        .stdout
        .chunks_exact(4)
        .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
        .collect())
}

#[derive(Debug, Clone, Serialize)]
pub struct SubtitleSegmentDto {
    pub start_ms: i64,
    pub end_ms: i64,
    pub text: String,
}

#[cfg(feature = "subtitles")]
fn transcribe_pcm(
    model_path: &Path,
    samples: &[f32],
    language: Option<&str>,
) -> Result<Vec<SubtitleSegmentDto>, String> {
    let session =
        apx_ai::subtitles::WhisperSession::load(model_path).map_err(|err| err.to_string())?;
    let segments = session
        .transcribe(samples, language)
        .map_err(|err| err.to_string())?;
    Ok(segments
        .into_iter()
        .map(|s| SubtitleSegmentDto {
            start_ms: s.start_ms,
            end_ms: s.end_ms,
            text: s.text,
        })
        .collect())
}

#[cfg(not(feature = "subtitles"))]
fn transcribe_pcm(
    _model_path: &Path,
    _samples: &[f32],
    _language: Option<&str>,
) -> Result<Vec<SubtitleSegmentDto>, String> {
    Err(
        "Diese Aperture-X-Build wurde ohne automatische Untertitel kompiliert (Cargo-Feature \"subtitles\" fehlt — baut whisper.cpp lokal per cmake, siehe apx-ai::subtitles)."
            .to_string(),
    )
}

/// Transkribiert die Tonspur des Videos `photo_id` per Whisper zu
/// zeitversehenen Text-Abschnitten — direkt weiterverwendbar als
/// Text-Overlays (Phase 17 Schritt 4, `TimelineTextOverlayInput`), das
/// Frontend übernimmt die Zeitstempel/Texte unverändert in den
/// Overlay-Editor statt sie hier schon als fertige Overlays anzulegen
/// (Position/Schriftart/Größe bleiben bewusst manuell wählbar).
/// `language`: ISO-639-1-Kürzel oder `None` für Auto-Erkennung.
#[tauri::command]
pub fn transcribe_video_audio(
    state: State<'_, AppState>,
    photo_id: String,
    language: Option<String>,
) -> Result<Vec<SubtitleSegmentDto>, String> {
    let photo_id = parse_photo_id(photo_id)?;
    let photo = state
        .catalog
        .get_photo(photo_id)
        .map_err(|err| err.to_string())?;
    if photo.media_kind != "video" {
        return Err("Automatische Untertitel funktionieren nur bei Videos".to_string());
    }
    let folder = state
        .catalog
        .get_folder(photo.folder_id)
        .map_err(|err| err.to_string())?;
    let source_path = folder.path.join(&photo.filename);

    let settings = apx_core::Settings::load_or_default(&state.paths.settings_file())
        .map_err(|err| err.to_string())?;
    let model_path = settings
        .ai
        .whisper_model_path
        .filter(|p| !p.trim().is_empty())
        .ok_or_else(|| {
            "Kein Untertitel-Modell heruntergeladen — siehe Einstellungen → KI.".to_string()
        })?;

    let samples = extract_audio_pcm_f32(&source_path)?;
    transcribe_pcm(Path::new(&model_path), &samples, language.as_deref())
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

/// Ersetzt die frei benannten IPTC-Zusatzfelder für eine oder mehrere
/// Fotos (Phase 12 Schritt 4, voller EXIF/IPTC-Editor, siehe
/// `DECISIONS.md` ADR-0039) — wie `set_photo_metadata` deckt das auch
/// Stapel-Metadatenbearbeitung ab.
#[tauri::command]
pub fn set_photo_custom_metadata(
    state: State<'_, AppState>,
    photo_id: String,
    metadata: std::collections::BTreeMap<String, String>,
) -> Result<(), String> {
    let photo_id = parse_photo_id(photo_id)?;
    state
        .catalog
        .set_photo_custom_metadata(photo_id, &metadata)
        .map_err(|err| err.to_string())
}

/// Die wohlbekannten IPTC-Kernfeld-Schlüssel, die das Frontend fest
/// anbietet (siehe `apx_catalog::iptc::WELL_KNOWN_FIELDS`) — reine
/// Konstante, kein State-Zugriff nötig.
#[tauri::command]
pub fn list_well_known_iptc_fields() -> Vec<(String, String)> {
    apx_catalog::iptc::WELL_KNOWN_FIELDS
        .iter()
        .map(|(key, label)| (key.to_string(), label.to_string()))
        .collect()
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
        custom_metadata: photo.custom_metadata.clone(),
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
/// Moduldoku. `criteria_json` ist seit Phase 13 Schritt 7 der serialisierte
/// UND/ODER-Regelbaum (`apx_catalog::FilterNode`, vom Frontend-
/// `RuleTreeEditor.tsx` erzeugt) statt der alten flachen `FilterCriteriaDto`
/// — als opakes JSON durchgereicht, wie `conditions_json` bei Presets.
#[tauri::command]
pub fn create_smart_collection(
    state: State<'_, AppState>,
    name: String,
    folder_id: Option<String>,
    criteria_json: String,
) -> Result<String, String> {
    let folder_id = folder_id.map(parse_collection_folder_id).transpose()?;
    let node = apx_catalog::parse_filter_node(&criteria_json).map_err(|err| err.to_string())?;
    let id = state
        .catalog
        .create_smart_collection(&name, folder_id, &node)
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

/// Ähnliche Videos finden (Phase 16 Schritt 10, siehe `DECISIONS.md`
/// ADR-0043) — arbeitet **exakt** wie
/// [`list_perceptual_duplicate_groups`] (derselbe Hasher, dieselbe
/// O(n²)-Gruppierung gegen den jeweils ersten Gruppen-Vertreter),
/// beschränkt auf `media_kind == "video"` und deren bereits bei Import
/// per `ffmpeg` extrahiertes Vorschau-Frame (`extract_video_frame`,
/// Phase 16 Schritt 4) als Hash-Grundlage — kein neuer Hashing-
/// Algorithmus, kein zweiter Keyframe-Extraktionsweg. Nur ein einzelnes
/// Frame pro Video (dieselbe Vereinfachung wie bei Fotos: "genügt für
/// einen auf Abruf gestarteten Assistenten", keine Szenen-übergreifende
/// Analyse).
#[tauri::command]
pub fn list_similar_video_groups(
    state: State<'_, AppState>,
    max_distance: u32,
) -> Result<Vec<Vec<SimilarVideoDto>>, String> {
    let photos = state
        .catalog
        .search_and_filter_photos(None, &apx_catalog::FilterCriteria::default())
        .map_err(|err| err.to_string())?
        .into_iter()
        .filter(|photo| photo.media_kind == "video");

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
                .map(|i| {
                    let photo = hashed[i].0.clone();
                    SimilarVideoDto {
                        folder_id: photo.folder_id.to_string(),
                        photo: photo.into(),
                    }
                })
                .collect()
        })
        .collect())
}

/// Ein Video innerhalb einer [`list_similar_video_groups`]-Gruppe —
/// `PhotoDto` selbst trägt bewusst kein `folder_id` (in dieser breit
/// genutzten zentralen Struktur wäre das eine deutlich größere
/// Änderung mit vielen Testfixture-Anpassungen, siehe Phase 16
/// Schritt 4s Erfahrung mit den 23 `NewPhoto`/`Photo`-Konstruktions-
/// stellen) — hier reicht ein schlanker Wrapper, weil das Frontend nur
/// für *diese eine* Funktion (zu einem ähnlichen Video in einem anderen
/// Ordner springen) wissen muss, in welchem Ordner es liegt.
#[derive(Debug, Clone, Serialize)]
pub struct SimilarVideoDto {
    pub photo: PhotoDto,
    pub folder_id: String,
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

/// Ein Foto innerhalb eines analysierten Shootings, siehe
/// [`analyze_style_consistency`].
#[derive(Debug, Clone, Serialize)]
pub struct StylePhotoAnalysisDto {
    pub photo: PhotoDto,
    pub mean_l: f32,
    pub mean_a: f32,
    pub mean_b: f32,
    pub distance_from_group: f32,
    pub is_outlier: bool,
    pub suggested_exposure_ev_delta: f32,
    pub suggested_temp_shift_kelvin_delta: f32,
    pub suggested_tint_shift_delta: f32,
}

/// Automatischer Stil-Konsistenz-Check fürs Shooting (Phase 14 Schritt 5,
/// siehe `DECISIONS.md` ADR-0041 Nachtrag V): Lightroom hat dafür kein
/// Äquivalent, nur das manuelle "Sync Settings" zwischen zwei Fotos. Die
/// eigentliche Lab-Statistik lebt in `apx_ai::style_consistency` (rein,
/// unit-getestet) — dieser Command löst nur die Fotos eines Ordners auf
/// und arbeitet wie [`list_perceptual_duplicate_groups`]/
/// [`list_people_groups`] auf dem bereits vorhandenen Thumbnail-
/// Vorschau-Cache statt jedes Foto neu von der RAW-Datei zu dekodieren.
/// Fotos ohne bereits generierte Miniaturansicht werden übersprungen wie
/// bei den beiden genannten Vorbildern.
#[tauri::command]
pub fn analyze_style_consistency(
    state: State<'_, AppState>,
    folder_id: String,
) -> Result<Vec<StylePhotoAnalysisDto>, String> {
    let folder_id: apx_core::FolderId = folder_id
        .parse()
        .map_err(|err: apx_core::AppError| err.to_string())?;
    let photos = state
        .catalog
        .list_photos_by_folder(folder_id)
        .map_err(|err| err.to_string())?;

    let mut with_signatures: Vec<(
        apx_catalog::Photo,
        apx_ai::style_consistency::StyleSignature,
    )> = Vec::new();
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
        let signature = apx_ai::style_consistency::compute_style_signature(&pixels, width, height);
        with_signatures.push((photo, signature));
    }

    let signatures: Vec<apx_ai::style_consistency::StyleSignature> = with_signatures
        .iter()
        .map(|(_, signature)| *signature)
        .collect();
    let analyses = apx_ai::style_consistency::analyze_group(&signatures);

    Ok(with_signatures
        .into_iter()
        .zip(analyses)
        .map(|((photo, _), analysis)| StylePhotoAnalysisDto {
            photo: PhotoDto::from(photo),
            mean_l: analysis.signature.mean_l,
            mean_a: analysis.signature.mean_a,
            mean_b: analysis.signature.mean_b,
            distance_from_group: analysis.distance_from_group,
            is_outlier: analysis.is_outlier,
            suggested_exposure_ev_delta: analysis.suggestion.exposure_ev_delta,
            suggested_temp_shift_kelvin_delta: analysis.suggestion.temp_shift_kelvin_delta,
            suggested_tint_shift_delta: analysis.suggestion.tint_shift_delta,
        })
        .collect())
}

/// Eine dominante Farbe der extrahierten Palette, siehe
/// [`extract_color_palette`].
#[derive(Debug, Clone, Serialize)]
pub struct PaletteColorDto {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub hue_degrees: f32,
    pub chroma: f32,
    pub lightness: f32,
    pub percentage: f32,
}

/// Farb-Harmonie-Rad: automatische Paletten-Extraktion (Phase 14
/// Schritt 7, siehe `DECISIONS.md` ADR-0041 Nachtrag VII). Die eigentliche
/// k-means-Analyse lebt in `apx_ai::palette` (rein, unit-getestet) —
/// dieser Command arbeitet wie [`list_perceptual_duplicate_groups`]/
/// [`analyze_style_consistency`] auf dem bereits vorhandenen Thumbnail-
/// Vorschau-Cache statt jedes Mal neu von der RAW-Datei zu dekodieren.
/// Dieselbe ehrliche Grenze wie bei jeder anderen Analyse auf Basis
/// dieses Caches: spiegelt bereits im Entwickeln-Modul gesetzte, aber
/// noch nicht in eine neue Vorschau gebackene Anpassungen nicht wider.
#[tauri::command]
pub fn extract_color_palette(
    state: State<'_, AppState>,
    photo_id: String,
    k: Option<usize>,
) -> Result<Vec<PaletteColorDto>, String> {
    let photo_id = parse_photo_id(photo_id)?;
    let preview = state
        .catalog
        .get_preview(photo_id, apx_catalog::PreviewLevel::Thumbnail)
        .map_err(|err| err.to_string())?
        .ok_or_else(|| "Keine Vorschau für dieses Foto vorhanden".to_string())?;
    let img = image::open(&preview.path).map_err(|err| err.to_string())?;
    let rgb = img.to_rgb8();
    let (width, height) = rgb.dimensions();
    let pixels: Vec<f32> = rgb
        .into_raw()
        .iter()
        .map(|&v| f32::from(v) / 255.0)
        .collect();

    let k = k.unwrap_or(apx_ai::palette::DEFAULT_PALETTE_SIZE);
    let colors = apx_ai::palette::extract_palette(&pixels, width, height, k);

    Ok(colors
        .into_iter()
        .map(|color| PaletteColorDto {
            r: color.r,
            g: color.g,
            b: color.b,
            hue_degrees: color.hue_degrees,
            chroma: color.chroma,
            lightness: color.lightness,
            percentage: color.percentage,
        })
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
    /// `apx_core::AiSettings`s Moduldoku. Wird dem Frontend
    /// unverändert zurückgegeben, damit das Eingabefeld den hinterlegten
    /// Schlüssel zur Kontrolle/Bearbeitung zeigen kann (maskiert per
    /// `type="password"`, nicht serverseitig verborgen — ein lokaler,
    /// nicht synchronisierter Einzelnutzer-Schlüssel).
    pub anthropic_api_key: Option<String>,
    /// `Some`, sobald der Nutzer den Download bestätigt und er
    /// erfolgreich war (Phase 13 Schritt 1, siehe [`download_inpainting_model`]).
    pub inpainting_model_path: Option<String>,
    /// `Some`, sobald der Nutzer den Download beider Personen-
    /// Wiedererkennungs-Modelle bestätigt hat und er erfolgreich war
    /// (Phase 13 Schritt 8, siehe [`download_people_models`]).
    pub people_landmark_model_path: Option<String>,
    pub people_encoder_model_path: Option<String>,
    /// `true`, wenn diese Build mit dem Cargo-Feature `people` kompiliert
    /// wurde — das Frontend zeigt sonst einen Hinweis statt der Download-
    /// /Erkennungs-Aktionen (siehe `apx-ai::people`s Moduldoku).
    pub people_feature_compiled: bool,
    /// `Some`, sobald der Nutzer den Download des MiDaS-Tiefenschätzungs-
    /// Modells bestätigt hat und er erfolgreich war (Phase 14 Schritt 8,
    /// siehe [`download_depth_model`]).
    pub depth_model_path: Option<String>,
    /// Je Stil (`apx_ai::style_transfer::StyleKind::id()` als Schlüssel)
    /// der lokale Pfad, sobald der Nutzer dessen Download bestätigt hat
    /// und er erfolgreich war (Phase 14 Schritt 9, siehe
    /// [`download_style_transfer_model`]) — ein fehlender Schlüssel
    /// heißt „dieser Stil noch nicht heruntergeladen".
    pub style_transfer_model_paths: std::collections::BTreeMap<String, String>,
    /// `Some`, sobald der Nutzer den Download des Whisper-Untertitel-
    /// Modells bestätigt hat und er erfolgreich war (Phase 17 Schritt 5,
    /// siehe [`download_whisper_model`]).
    pub whisper_model_path: Option<String>,
    /// `true`, wenn diese Build mit dem Cargo-Feature `subtitles`
    /// kompiliert wurde (siehe `apx-ai::subtitles`s Moduldoku) —
    /// analog zu `people_feature_compiled` oben.
    pub subtitles_feature_compiled: bool,
}

#[tauri::command]
pub fn get_ai_settings(state: State<'_, AppState>) -> Result<AiSettingsDto, String> {
    let settings = apx_core::Settings::load_or_default(&state.paths.settings_file())
        .map_err(|err| err.to_string())?;
    Ok(AiSettingsDto {
        anthropic_api_key: settings.ai.anthropic_api_key,
        inpainting_model_path: settings.ai.inpainting_model_path,
        people_landmark_model_path: settings.ai.people_landmark_model_path,
        people_encoder_model_path: settings.ai.people_encoder_model_path,
        people_feature_compiled: cfg!(feature = "people"),
        depth_model_path: settings.ai.depth_model_path,
        style_transfer_model_paths: settings.ai.style_transfer_model_paths,
        whisper_model_path: settings.ai.whisper_model_path,
        subtitles_feature_compiled: cfg!(feature = "subtitles"),
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

// ---- KI: Ausfüllen (LaMa-Inpainting, Phase 13 Schritt 1) -------------------
//
// Opt-in, kein Bundling im Installer (siehe `DECISIONS.md` ADR-0040 und
// `apx_core::AiSettings::inpainting_model_path`s Moduldoku) —
// derselbe Ansatz wie der Anthropic-API-Schlüssel oben: der Nutzer
// bestätigt den ~208-MB-Download ausdrücklich im Einstellungsdialog,
// bevor irgendetwas heruntergeladen wird.

/// Öffentliche Download-URL des von `DECISIONS.md` ADR-0040 recherchierten
/// Modells (`Carve/LaMa-ONNX`, Apache-2.0, Hugging Face). **Nicht in
/// dieser Sitzung erreichbar/verifiziert** — `huggingface.co` ist von
/// dieser Entwicklungs-Sandbox aus blockiert (siehe `apx-ai::inpaint`s
/// Moduldoku) — die URL folgt Hugging Faces dokumentiertem
/// `resolve/main/<datei>`-Schema für Rohdatei-Downloads, wurde aber nicht
/// tatsächlich abgerufen. Vor Produktivnutzung mit erreichbarem
/// `huggingface.co` einmal nachprüfen.
const LAMA_MODEL_URL: &str = "https://huggingface.co/Carve/LaMa-ONNX/resolve/main/lama_fp32.onnx";

/// Lädt das LaMa-Inpainting-Modell herunter (siehe [`LAMA_MODEL_URL`]s
/// Vorbehalt) nach `AppPaths::models_dir()` und hinterlegt den Pfad in den
/// Einstellungen. **Keine Hash-Prüfung** — der auf Hugging Face
/// veröffentlichte Datei-Hash wurde in dieser Sitzung nie tatsächlich
/// abgerufen (siehe oben), eine erfundene Prüfsumme wäre schlimmer als
/// keine (stiller Fabrikations-Bug statt einer ehrlichen Lücke). Wer
/// diesen Command auf einer Maschine mit erreichbarem `huggingface.co`
/// zuerst nutzt, sollte den echten Hash ergänzen.
#[tauri::command]
pub async fn download_inpainting_model(state: State<'_, AppState>) -> Result<String, String> {
    let response = reqwest::get(LAMA_MODEL_URL)
        .await
        .map_err(|err| format!("Download von '{LAMA_MODEL_URL}' fehlgeschlagen: {err}"))?;
    if !response.status().is_success() {
        return Err(format!(
            "Download von '{LAMA_MODEL_URL}' fehlgeschlagen: HTTP {}",
            response.status()
        ));
    }
    let bytes = response
        .bytes()
        .await
        .map_err(|err| format!("Antwort konnte nicht gelesen werden: {err}"))?;

    let dest_dir = state.paths.models_dir();
    std::fs::create_dir_all(&dest_dir).map_err(|err| err.to_string())?;
    let dest_path = dest_dir.join("lama_fp32.onnx");
    std::fs::write(&dest_path, &bytes).map_err(|err| err.to_string())?;

    let path_string = dest_path.to_string_lossy().to_string();
    let settings_path = state.paths.settings_file();
    let mut settings =
        apx_core::Settings::load_or_default(&settings_path).map_err(|err| err.to_string())?;
    settings.ai.inpainting_model_path = Some(path_string.clone());
    settings
        .save(&settings_path)
        .map_err(|err| err.to_string())?;

    Ok(path_string)
}

/// Entfernt den hinterlegten Modellpfad (löscht die Datei selbst nicht —
/// der Nutzer kann sie manuell entfernen, dieselbe Zurückhaltung wie beim
/// Löschen anderer nutzergesteuerter lokaler Dateien in diesem Projekt).
#[tauri::command]
pub fn clear_inpainting_model_path(state: State<'_, AppState>) -> Result<(), String> {
    let path = state.paths.settings_file();
    let mut settings = apx_core::Settings::load_or_default(&path).map_err(|err| err.to_string())?;
    settings.ai.inpainting_model_path = None;
    settings.save(&path).map_err(|err| err.to_string())
}

#[derive(Debug, Clone, Serialize)]
pub struct AiFillPatchDto {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub bitmap_width: u32,
    pub bitmap_height: u32,
    /// Base64-kodiertes interleaved-RGB-`u8`-Ergebnis, `bitmap_width *
    /// bitmap_height * 3` Bytes nach dem Dekodieren — dasselbe
    /// Übertragungsmuster wie `AiMaskAlphaDto::alpha_base64`.
    pub pixels_base64: String,
}

/// Führt echte LaMa-Inferenz für ein normiertes Rechteck (`x`/`y`/`width`/
/// `height`, `0.0..=1.0`) auf `photo_id` aus (Phase 13 Schritt 1, siehe
/// `DECISIONS.md` ADR-0040) — das Frontend ruft dies erst nach
/// ausdrücklichem „Anwenden" auf einem gemalten `RepairMode::AiInpaint`-
/// Strich auf (siehe `apx-pipeline::edl::v2::AiFillPatch`s Moduldoku),
/// nicht bei jedem Regler-Tick.
///
/// Läuft auf derselben capped Analyse-Auflösung (`apx_ai::segmentation::
/// ANALYSIS_MAX_EDGE`) wie jede andere KI-Bildanalyse in diesem Projekt —
/// das komplette Rechteck gilt als „auszufüllen" (Maske `255` überall
/// innerhalb, kein zusätzliches Federn: LaMa selbst lernt einen weichen
/// Übergang, siehe `apx_ai::inpaint`s Moduldoku).
///
/// **Ehrliche Grenze:** läuft auf dem linearen Kamera-RGB-Dekodierergebnis
/// (`decode_linear`, derselbe Farbraum wie [`generate_ai_mask`]/
/// [`suggest_repair_source`]), nicht auf entwickelten sRGB-Pixeln — LaMa
/// wurde vermutlich auf gewöhnlichen (sRGB-artigen) Fotos trainiert, ein
/// linearer Farbraum ist eine Näherung, keine exakte Übereinstimmung mit
/// den Trainingsdaten (dieselbe Art Kompromiss wie bei jeder anderen
/// KI-Heuristik dieses Projekts, die auf demselben Dekodierergebnis
/// arbeitet).
#[tauri::command]
pub fn run_ai_inpaint(
    state: State<'_, AppState>,
    photo_id: String,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
) -> Result<AiFillPatchDto, String> {
    let photo_id = parse_photo_id(photo_id)?;
    let settings = apx_core::Settings::load_or_default(&state.paths.settings_file())
        .map_err(|err| err.to_string())?;
    let model_path = settings
        .ai
        .inpainting_model_path
        .filter(|path| !path.trim().is_empty())
        .ok_or_else(|| {
            "Kein KI-Ausfüllen-Modell heruntergeladen — siehe Einstellungen → KI.".to_string()
        })?;

    let source_path = resolve_source_path_for_ai(&state.catalog, photo_id)?;
    let max_edge = Some(apx_ai::segmentation::ANALYSIS_MAX_EDGE);
    let linear = state
        .tile_cache
        .get_or_decode(photo_id, max_edge, || {
            apx_raw::decode_linear(&source_path, max_edge)
        })
        .map_err(|err| err.to_string())?;

    let px = (x * linear.width as f32)
        .round()
        .clamp(0.0, linear.width as f32 - 1.0) as u32;
    let py = (y * linear.height as f32)
        .round()
        .clamp(0.0, linear.height as f32 - 1.0) as u32;
    let pw = (width * linear.width as f32)
        .round()
        .max(1.0)
        .min((linear.width - px) as f32) as u32;
    let ph = (height * linear.height as f32)
        .round()
        .max(1.0)
        .min((linear.height - py) as f32) as u32;

    let mut crop_u8 = vec![0u8; (pw as usize) * (ph as usize) * 3];
    for row in 0..ph {
        for col in 0..pw {
            let src_idx = ((py + row) as usize * linear.width as usize + (px + col) as usize) * 3;
            let dst_idx = (row as usize * pw as usize + col as usize) * 3;
            for c in 0..3 {
                let value = linear.pixels[src_idx + c].clamp(0.0, 1.0);
                crop_u8[dst_idx + c] = (value * 255.0).round() as u8;
            }
        }
    }
    // Vollflächige Maske — das gesamte übergebene Rechteck gilt als
    // auszufüllen (siehe Funktionsdoku).
    let mask = vec![255u8; (pw as usize) * (ph as usize)];

    // Session wird pro Aufruf frisch geladen statt in `AppState` gehalten
    // — einfacher, aber langsamer bei wiederholter Nutzung (das Modell
    // wird bei jedem „Anwenden"-Klick neu von der Platte gelesen und der
    // ONNX-Graph neu aufgebaut). Für einen Nutzer-ausgelösten, nicht
    // performance-kritischen Ein-Klick-Vorgang akzeptabel; eine gehaltene
    // Session in `AppState` wäre eine mögliche spätere Optimierung.
    let mut session = apx_ai::inpaint::InpaintSession::load(Path::new(&model_path))
        .map_err(|err| err.to_string())?;
    let filled = session
        .fill_rgb8(&crop_u8, pw, ph, &mask)
        .map_err(|err| err.to_string())?;

    Ok(AiFillPatchDto {
        x,
        y,
        width,
        height,
        bitmap_width: pw,
        bitmap_height: ph,
        pixels_base64: base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &filled),
    })
}

// ---- Photoshop-Funktion: Content-Aware Move (Phase 15 Schritt 1, siehe
// DECISIONS.md ADR-0042) -----------------------------------------------------
//
// Baut auf zwei bereits bestehenden Primitiven auf, kein neuer EDL-Typ:
// die Ausgangsstelle wird — wie `run_ai_inpaint` oben — über einen
// `AiFillPatch` gefüllt, hier aber fürs GESAMTE Bild statt eines
// Rechtecks (damit LaMa echten Bildkontext ringsum sieht, nicht nur den
// unmittelbaren Rand des ausgeschnittenen Rechtecks). Da `fill_rgb8`
// unmaskierte Pixel laut seiner eigenen Moduldoku unverändert
// zurückgibt, lässt sich das Ergebnis gefahrlos als Vollbild-Patch
// (`x = y = 0`, `width = height = 1`) zurückgeben. Das verschobene
// Objekt selbst wird als neue `CompositeLayer` (Phase 14 Schritt 3) an
// der vom Nutzer gewählten Zielposition platziert — die eigentliche
// Platzierung (Ziel-`offset_x`/`offset_y`) entscheidet ausschließlich
// das Frontend beim Aufbau der beiden neuen EDL-Einträge, dieser
// Command liefert nur die beiden fertigen Bitmaps.
#[derive(Debug, Clone, Serialize)]
pub struct ContentAwareMoveDto {
    /// Vollbild-Fill-Patch für die Ausgangsstelle — direkt als
    /// `RepairStroke::ai_fill` verwendbar.
    pub fill: AiFillPatchDto,
    /// Der ausgeschnittene, verschobene Bildausschnitt — direkt als
    /// `CompositeLayer::source` verwendbar.
    pub moved: CompositeLayerSourceDto,
    /// Bester Startwert für `CompositeLayer::scale`, damit die
    /// verschobene Bitmap ungefähr in ihrer ursprünglichen Pixelgröße
    /// erscheint (`Auswahlbreite / Bildbreite`). `CompositeLayer::scale`
    /// skaliert Breite und Höhe der Ebene immer gleich relativ zur
    /// Leinwand (siehe `stages::composite`s Moduldoku) — bei einem von
    /// der Leinwand abweichenden Seitenverhältnis der Auswahl kommt es
    /// deshalb zu einer leichten Verzerrung, über den bereits
    /// bestehenden Skalierungs-Regler der Ebene danach manuell
    /// korrigierbar.
    pub dest_scale: f32,
}

#[tauri::command]
pub fn content_aware_move(
    state: State<'_, AppState>,
    photo_id: String,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
) -> Result<ContentAwareMoveDto, String> {
    let photo_id = parse_photo_id(photo_id)?;
    let settings = apx_core::Settings::load_or_default(&state.paths.settings_file())
        .map_err(|err| err.to_string())?;
    let model_path = settings
        .ai
        .inpainting_model_path
        .filter(|path| !path.trim().is_empty())
        .ok_or_else(|| {
            "Kein KI-Ausfüllen-Modell heruntergeladen — siehe Einstellungen → KI.".to_string()
        })?;

    let source_path = resolve_source_path_for_ai(&state.catalog, photo_id)?;
    let max_edge = Some(apx_ai::segmentation::ANALYSIS_MAX_EDGE);
    let linear = state
        .tile_cache
        .get_or_decode(photo_id, max_edge, || {
            apx_raw::decode_linear(&source_path, max_edge)
        })
        .map_err(|err| err.to_string())?;

    let px = (x * linear.width as f32)
        .round()
        .clamp(0.0, linear.width as f32 - 1.0) as u32;
    let py = (y * linear.height as f32)
        .round()
        .clamp(0.0, linear.height as f32 - 1.0) as u32;
    let pw = (width * linear.width as f32)
        .round()
        .max(1.0)
        .min((linear.width - px) as f32) as u32;
    let ph = (height * linear.height as f32)
        .round()
        .max(1.0)
        .min((linear.height - py) as f32) as u32;

    // Vollbild als linearer u8-RGB-Puffer (für den Fill-Patch — derselbe
    // Farbraum wie `run_ai_inpaint`, das Reparatur-Stufe läuft noch vor
    // der sRGB-Konvertierung im Renderpfad).
    let pixel_count = (linear.width as usize) * (linear.height as usize);
    let full_linear_u8: Vec<u8> = linear.pixels[..pixel_count * 3]
        .iter()
        .map(|&v| (v.clamp(0.0, 1.0) * 255.0).round() as u8)
        .collect();

    let mut mask = vec![0u8; pixel_count];
    for row in py..(py + ph) {
        for col in px..(px + pw) {
            mask[row as usize * linear.width as usize + col as usize] = 255;
        }
    }

    // Session wird pro Aufruf frisch geladen (dieselbe Begründung wie
    // `run_ai_inpaint`).
    let mut session = apx_ai::inpaint::InpaintSession::load(Path::new(&model_path))
        .map_err(|err| err.to_string())?;
    let filled = session
        .fill_rgb8(&full_linear_u8, linear.width, linear.height, &mask)
        .map_err(|err| err.to_string())?;

    // Der verschobene Ausschnitt selbst wird als `CompositeLayer`
    // gerendert — die läuft im bereits entwickelten sRGB-Bild, deshalb
    // hier bewusst *nicht* derselbe lineare Puffer wie oben, sondern
    // dieselbe sRGB-Konvertierung wie `prepare_composite_layer_source`s
    // Foto-Zweig.
    let rgba_srgb =
        apx_pipeline::color::linear_camera_rgb_to_srgb_rgba8(&linear.pixels, linear.cam_to_srgb);
    let mut moved_rgb = vec![0u8; (pw as usize) * (ph as usize) * 3];
    for row in 0..ph {
        for col in 0..pw {
            let src_idx = ((py + row) as usize * linear.width as usize + (px + col) as usize) * 4;
            let dst_idx = (row as usize * pw as usize + col as usize) * 3;
            moved_rgb[dst_idx] = rgba_srgb[src_idx];
            moved_rgb[dst_idx + 1] = rgba_srgb[src_idx + 1];
            moved_rgb[dst_idx + 2] = rgba_srgb[src_idx + 2];
        }
    }

    Ok(ContentAwareMoveDto {
        fill: AiFillPatchDto {
            x: 0.0,
            y: 0.0,
            width: 1.0,
            height: 1.0,
            bitmap_width: linear.width,
            bitmap_height: linear.height,
            pixels_base64: base64::Engine::encode(
                &base64::engine::general_purpose::STANDARD,
                &filled,
            ),
        },
        moved: CompositeLayerSourceDto {
            bitmap_width: pw,
            bitmap_height: ph,
            pixels_base64: base64::Engine::encode(
                &base64::engine::general_purpose::STANDARD,
                &moved_rgb,
            ),
        },
        dest_scale: pw as f32 / linear.width as f32,
    })
}

// ---- KI: Leinwand-Erweiterung (Outpainting, Phase 14 Schritt 1, siehe
// DECISIONS.md ADR-0041) -----------------------------------------------------
//
// Dieselbe LaMa-Session wie `run_ai_inpaint` oben (kein zweites Modell,
// kein zweiter Download-Command) — nur eine andere Maskenform: statt
// eines gemalten Pinselstrichs ist die gesamte neue Randfläche rund um
// das zentrierte Original als „auszufüllen" markiert. Das Ergebnis ist
// bereits die vollständig zusammengesetzte, erweiterte Leinwand (Original
// + KI-Rand) — `stages::geometry::extend_canvas` braucht den Rand-Anteil
// nicht separat, weil sie den Original-Bereich beim Rendern ohnehin durch
// die exakten Original-Pixel ersetzt (siehe deren Doku) und das
// zurückgegebene Bitmap unverändert als `CanvasExtensionPatch::pixels`
// übernommen werden kann — dasselbe „einmal berechnen, bei jedem Rendern
// nur noch skalieren"-Muster wie `AiFillPatch`.

#[derive(Debug, Clone, Serialize)]
pub struct CanvasExtensionPatchDto {
    pub margin_left: f32,
    pub margin_top: f32,
    pub margin_right: f32,
    pub margin_bottom: f32,
    pub bitmap_width: u32,
    pub bitmap_height: u32,
    /// Base64-kodiertes interleaved-RGB-`u8`-Ergebnis, `bitmap_width *
    /// bitmap_height * 3` Bytes nach dem Dekodieren — dasselbe
    /// Übertragungsmuster wie `AiFillPatchDto::pixels_base64`.
    pub pixels_base64: String,
}

/// Ersetzt den Rand einer um `margin_left`/`margin_top`/`margin_right`/
/// `margin_bottom` (normierte Bruchteile der aktuellen Bildbreite/-höhe,
/// `0.0..=1.0`, dieselbe Konvention wie `CanvasExtension`s Feldtypen)
/// erweiterten Leinwand durch echte LaMa-Inferenz (Phase 14 Schritt 1).
/// Läuft — wie `run_ai_inpaint` — auf dem linearen, auf
/// `apx_ai::segmentation::ANALYSIS_MAX_EDGE` gedeckelten Dekodierergebnis,
/// **ohne** eine bereits im Entwickeln-Modul gesetzte Drehung/Zuschnitt zu
/// berücksichtigen — dieselbe ehrliche Vereinfachung wie bei jeder
/// anderen KI-Analyse in diesem Projekt (`generate_ai_mask`,
/// `suggest_repair_source`, …), die ebenfalls auf dem rohen
/// Dekodierergebnis statt der entwickelten Vorschau arbeitet.
#[tauri::command]
pub fn run_ai_outpaint(
    state: State<'_, AppState>,
    photo_id: String,
    margin_left: f32,
    margin_top: f32,
    margin_right: f32,
    margin_bottom: f32,
) -> Result<CanvasExtensionPatchDto, String> {
    let photo_id = parse_photo_id(photo_id)?;
    let settings = apx_core::Settings::load_or_default(&state.paths.settings_file())
        .map_err(|err| err.to_string())?;
    let model_path = settings
        .ai
        .inpainting_model_path
        .filter(|path| !path.trim().is_empty())
        .ok_or_else(|| {
            "Kein KI-Ausfüllen-Modell heruntergeladen — siehe Einstellungen → KI.".to_string()
        })?;

    let source_path = resolve_source_path_for_ai(&state.catalog, photo_id)?;
    let max_edge = Some(apx_ai::segmentation::ANALYSIS_MAX_EDGE);
    let linear = state
        .tile_cache
        .get_or_decode(photo_id, max_edge, || {
            apx_raw::decode_linear(&source_path, max_edge)
        })
        .map_err(|err| err.to_string())?;

    let width = linear.width;
    let height = linear.height;
    let ml = (margin_left.max(0.0) * width as f32).round() as u32;
    let mr = (margin_right.max(0.0) * width as f32).round() as u32;
    let mt = (margin_top.max(0.0) * height as f32).round() as u32;
    let mb = (margin_bottom.max(0.0) * height as f32).round() as u32;
    let new_width = width + ml + mr;
    let new_height = height + mt + mb;
    if new_width == 0 || new_height == 0 || (ml == 0 && mr == 0 && mt == 0 && mb == 0) {
        return Err("Leinwand-Erweiterung braucht mindestens einen Rand größer als 0.".to_string());
    }

    // Original als `u8`-RGB in die Bildmitte der neuen Leinwand kopieren;
    // der neue Rand wird per Kanten-Klemmung (nächstliegender Original-
    // Pixel) vorbefüllt statt mit einer harten Flächenfarbe — reduziert
    // sichtbare Kanten-Artefakte, bevor LaMa den Rand tatsächlich neu
    // erzeugt (dieselbe Näherung, mit der auch andere Inpainting-Tools
    // ihre Startfüllung wählen).
    let (nw, nh) = (new_width as usize, new_height as usize);
    let mut extended_u8 = vec![0u8; nw * nh * 3];
    let mut mask = vec![255u8; nw * nh];
    for y in 0..new_height {
        for x in 0..new_width {
            let src_x = x.saturating_sub(ml).min(width.saturating_sub(1));
            let src_y = y.saturating_sub(mt).min(height.saturating_sub(1));
            let src = (src_y as usize * width as usize + src_x as usize) * 3;
            let dst = (y as usize * nw + x as usize) * 3;
            for c in 0..3 {
                let value = linear.pixels[src + c].clamp(0.0, 1.0);
                extended_u8[dst + c] = (value * 255.0).round() as u8;
            }
            let in_center = x >= ml && x < ml + width && y >= mt && y < mt + height;
            if in_center {
                mask[y as usize * nw + x as usize] = 0;
            }
        }
    }

    // Session wird pro Aufruf frisch geladen — dieselbe akzeptierte
    // Vereinfachung wie in `run_ai_inpaint` oben (siehe dort).
    let mut session = apx_ai::inpaint::InpaintSession::load(Path::new(&model_path))
        .map_err(|err| err.to_string())?;
    let filled = session
        .fill_rgb8(&extended_u8, new_width, new_height, &mask)
        .map_err(|err| err.to_string())?;

    Ok(CanvasExtensionPatchDto {
        margin_left,
        margin_top,
        margin_right,
        margin_bottom,
        bitmap_width: new_width,
        bitmap_height: new_height,
        pixels_base64: base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &filled),
    })
}

// ---- Inhaltssensitives Skalieren (Content-Aware Scale / Seam Carving,
// Phase 15 Schritt 4, siehe DECISIONS.md ADR-0042) — klassischer
// Algorithmus (`apx_ai::seam_carving`), kein ONNX-Modell. --------------------

#[derive(Debug, Clone, Serialize)]
pub struct ContentAwareScalePatchDto {
    pub width_fraction: f32,
    pub height_fraction: f32,
    pub bitmap_width: u32,
    pub bitmap_height: u32,
    /// Base64-kodiertes interleaved-RGB-`u8`-Ergebnis — dasselbe
    /// Übertragungsmuster wie `CanvasExtensionPatchDto::pixels_base64`.
    pub pixels_base64: String,
}

/// Berechnet das seam-carvte Ergebnis für `width_fraction`/
/// `height_fraction` (Bruchteile der aktuellen, auf
/// `apx_ai::segmentation::ANALYSIS_MAX_EDGE` gedeckelten Dekodier-
/// Auflösung — dieselbe ehrliche Vereinfachung wie `run_ai_outpaint`:
/// arbeitet auf dem rohen Dekodierergebnis, nicht der entwickelten
/// Vorschau). Schützt erkannte Personen/Gesichter automatisch vor
/// Verzerrung (`apx_ai::segmentation::person_alpha` als Schutzmaske,
/// siehe `apx_ai::seam_carving`s Moduldoku) — ein `person_alpha`-Fehler
/// (z. B. leeres Bild) lässt die Schutzmaske einfach entfallen statt den
/// ganzen Befehl fehlschlagen zu lassen, da sie nur eine Qualitäts-
/// verbesserung, keine Voraussetzung ist.
#[tauri::command]
pub fn content_aware_scale(
    state: State<'_, AppState>,
    photo_id: String,
    width_fraction: f32,
    height_fraction: f32,
) -> Result<ContentAwareScalePatchDto, String> {
    let photo_id = parse_photo_id(photo_id)?;
    let source_path = resolve_source_path_for_ai(&state.catalog, photo_id)?;
    let max_edge = Some(apx_ai::segmentation::ANALYSIS_MAX_EDGE);
    let linear = state
        .tile_cache
        .get_or_decode(photo_id, max_edge, || {
            apx_raw::decode_linear(&source_path, max_edge)
        })
        .map_err(|err| err.to_string())?;

    let target_width = ((linear.width as f32) * width_fraction.max(0.01))
        .round()
        .max(1.0) as u32;
    let target_height = ((linear.height as f32) * height_fraction.max(0.01))
        .round()
        .max(1.0) as u32;
    if target_width == linear.width && target_height == linear.height {
        return Err("Zielgröße entspricht bereits der aktuellen Bildgröße.".to_string());
    }

    let rgba =
        apx_pipeline::color::linear_camera_rgb_to_srgb_rgba8(&linear.pixels, linear.cam_to_srgb);
    let pixel_count = (linear.width as usize) * (linear.height as usize);
    let mut rgb = vec![0u8; pixel_count * 3];
    for i in 0..pixel_count {
        rgb[i * 3] = rgba[i * 4];
        rgb[i * 3 + 1] = rgba[i * 4 + 1];
        rgb[i * 3 + 2] = rgba[i * 4 + 2];
    }

    let protect =
        apx_ai::segmentation::person_alpha(&linear.pixels, linear.width, linear.height).ok();

    let (bitmap_width, bitmap_height, pixels) = apx_ai::seam_carving::resize_rgb8(
        &rgb,
        linear.width,
        linear.height,
        target_width,
        target_height,
        protect.as_deref(),
    );

    Ok(ContentAwareScalePatchDto {
        width_fraction,
        height_fraction,
        bitmap_width,
        bitmap_height,
        pixels_base64: base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &pixels),
    })
}

// ---- Automatisches Hautglätten (Phase 15 Schritt 5, siehe DECISIONS.md
// ADR-0042) — kombiniert Gesichtserkennung + Frequenztrennung, kein
// zusätzliches ONNX-Modell. --------------------------------------------------

#[derive(Debug, Clone, Serialize)]
pub struct SkinSmoothingPatchDto {
    pub bitmap_width: u32,
    pub bitmap_height: u32,
    /// Base64-kodiertes interleaved-RGB-`u8`-Ergebnis.
    pub pixels_base64: String,
}

/// Erkennt Gesichtsregionen (`apx_ai::faces::detect_face_regions`),
/// schneidet die Hautton-Alpha-Maske (`apx_ai::segmentation::
/// person_alpha`) auf diese Regionen zu (verhindert Glätten hautfarbener
/// Bildbereiche außerhalb von Gesichtern), zerlegt das Bild per
/// Frequenztrennung mit einem kleineren Radius als `stages::repair`s
/// Standardwert (feinere Poren-/Textur-Frequenz) und weichzeichnet nur
/// die Hochfrequenz-Ebene innerhalb der Gesichtsmaske — dieselbe
/// Retusche-Technik wie Photoshops "Frequenztrennung", hier automatisiert
/// ohne manuelles Maskieren (siehe `PLAN.md` Phase 15 Schritt 5). Läuft
/// — wie jede andere KI-Analyse dieses Projekts — auf dem rohen, auf
/// `apx_ai::segmentation::ANALYSIS_MAX_EDGE` gedeckelten
/// Dekodierergebnis.
#[tauri::command]
pub fn smooth_skin(
    state: State<'_, AppState>,
    photo_id: String,
) -> Result<SkinSmoothingPatchDto, String> {
    let photo_id = parse_photo_id(photo_id)?;
    let source_path = resolve_source_path_for_ai(&state.catalog, photo_id)?;
    let max_edge = Some(apx_ai::segmentation::ANALYSIS_MAX_EDGE);
    let linear = state
        .tile_cache
        .get_or_decode(photo_id, max_edge, || {
            apx_raw::decode_linear(&source_path, max_edge)
        })
        .map_err(|err| err.to_string())?;

    let faces = apx_ai::faces::detect_face_regions(&linear.pixels, linear.width, linear.height)
        .map_err(|err| err.to_string())?;
    if faces.is_empty() {
        return Err("Keine Gesichter erkannt.".to_string());
    }
    let skin_alpha =
        apx_ai::segmentation::person_alpha(&linear.pixels, linear.width, linear.height)
            .map_err(|err| err.to_string())?;

    let width = linear.width;
    let height = linear.height;
    let pixel_count = (width as usize) * (height as usize);

    let mut face_mask = vec![0u8; pixel_count];
    for y in 0..height {
        for x in 0..width {
            let nx = x as f32 / width as f32;
            let ny = y as f32 / height as f32;
            let inside = faces
                .iter()
                .any(|f| nx >= f.x && nx < f.x + f.width && ny >= f.y && ny < f.y + f.height);
            if inside {
                let idx = (y as usize) * (width as usize) + x as usize;
                face_mask[idx] = skin_alpha[idx];
            }
        }
    }

    let rgba =
        apx_pipeline::color::linear_camera_rgb_to_srgb_rgba8(&linear.pixels, linear.cam_to_srgb);
    let mut rgb_f32 = vec![0f32; pixel_count * 3];
    for i in 0..pixel_count {
        rgb_f32[i * 3] = rgba[i * 4] as f32 / 255.0;
        rgb_f32[i * 3 + 1] = rgba[i * 4 + 1] as f32 / 255.0;
        rgb_f32[i * 3 + 2] = rgba[i * 4 + 2] as f32 / 255.0;
    }

    use apx_pipeline::stages::frequency_separation::{
        combine, default_split_radius_px, low_pass, HIGH_FREQUENCY_OFFSET,
    };
    // Kleinerer Trennradius als der für allgemeine Retusche gewählte
    // Standardwert — feinere Poren-/Textur-Frequenz (siehe Moduldoku).
    let split_radius = (default_split_radius_px(width) / 2).max(1);
    let low = low_pass(&rgb_f32, width, height, split_radius);
    let high: Vec<f32> = rgb_f32
        .iter()
        .zip(low.iter())
        .map(|(&original, &blurred)| (original - blurred + HIGH_FREQUENCY_OFFSET).clamp(0.0, 1.0))
        .collect();

    // Größerer Radius als die Trennung selbst — verwischt genau die
    // feine Poren-/Textur-Information, die die Hochfrequenz-Ebene trägt.
    let smoothing_radius = (split_radius * 4).max(2);
    let smoothed_high = low_pass(&high, width, height, smoothing_radius);

    // Nur innerhalb der Gesichtsmaske durch die geglättete Hochfrequenz
    // ersetzen, weich gewichtet nach `person_alpha`s Alpha-Wert.
    let mut final_high = high.clone();
    for (i, &mask_value) in face_mask.iter().enumerate() {
        let weight = mask_value as f32 / 255.0;
        if weight <= 0.0 {
            continue;
        }
        for c in 0..3 {
            let idx = i * 3 + c;
            final_high[idx] = high[idx] + (smoothed_high[idx] - high[idx]) * weight;
        }
    }

    let smoothed = combine(&low, &final_high);
    let mut pixels = vec![0u8; pixel_count * 3];
    for (i, value) in smoothed.iter().enumerate() {
        pixels[i] = (value.clamp(0.0, 1.0) * 255.0).round() as u8;
    }

    Ok(SkinSmoothingPatchDto {
        bitmap_width: width,
        bitmap_height: height,
        pixels_base64: base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &pixels),
    })
}

// ---- Mehrfachbelichtung/Layer-Compositing (Phase 14 Schritt 3, siehe
// DECISIONS.md ADR-0041) ------------------------------------------------------
//
// Kein KI-Modell nötig — reine Verdrahtung + Bilddekodierung. Löst
// entweder ein weiteres Katalog-Foto (`photo_id`) oder eine vom Nutzer
// per `pick_file_path` gewählte Textur-Datei (`texture_path`) EINMALIG
// zu einer fertigen RGB-Bitmap auf, die das Frontend danach unverändert
// in `CompositeLayer::source` ablegt — dasselbe „einmal auflösen, bei
// jedem Rendern nur noch skalieren"-Muster wie [`run_ai_inpaint`]/
// [`run_ai_outpaint`] oben. `apx-pipeline` selbst hat keinen Katalog-/
// Dateisystemzugriff (siehe `edl::v4::CompositeLayerSource`s Moduldoku)
// — das Auflösen passiert bewusst hier, nicht dort.

fn downsample_rgb_image(img: image::RgbImage, max_edge: u32) -> image::RgbImage {
    let (width, height) = (img.width(), img.height());
    if width.max(height) <= max_edge {
        return img;
    }
    let scale = max_edge as f32 / width.max(height) as f32;
    let new_width = ((width as f32) * scale).round().max(1.0) as u32;
    let new_height = ((height as f32) * scale).round().max(1.0) as u32;
    image::imageops::resize(
        &img,
        new_width,
        new_height,
        image::imageops::FilterType::Triangle,
    )
}

#[derive(Debug, Clone, Serialize)]
pub struct CompositeLayerSourceDto {
    pub bitmap_width: u32,
    pub bitmap_height: u32,
    /// Base64-kodiertes interleaved-RGB-`u8`-Ergebnis, `bitmap_width *
    /// bitmap_height * 3` Bytes nach dem Dekodieren — dasselbe
    /// Übertragungsmuster wie `AiFillPatchDto::pixels_base64`.
    pub pixels_base64: String,
}

/// Löst genau eine der beiden Quellen zu einer fertigen RGB-Bitmap auf
/// (`photo_id` **oder** `texture_path`, nie beide/keines) — auf
/// `apx_ai::segmentation::ANALYSIS_MAX_EDGE` gedeckelt, dieselbe
/// Auflösungsgrenze wie jede andere in der EDL gespeicherte Bitmap
/// dieses Projekts.
#[tauri::command]
pub fn prepare_composite_layer_source(
    state: State<'_, AppState>,
    photo_id: Option<String>,
    texture_path: Option<String>,
) -> Result<CompositeLayerSourceDto, String> {
    let max_edge = apx_ai::segmentation::ANALYSIS_MAX_EDGE;

    let (width, height, rgb) = match (photo_id, texture_path) {
        (Some(photo_id), None) => {
            let photo_id = parse_photo_id(photo_id)?;
            let source_path = resolve_source_path_for_ai(&state.catalog, photo_id)?;
            let linear = apx_raw::decode_linear(&source_path, Some(max_edge))
                .map_err(|err| err.to_string())?;
            let rgba = apx_pipeline::color::linear_camera_rgb_to_srgb_rgba8(
                &linear.pixels,
                linear.cam_to_srgb,
            );
            let pixel_count = (linear.width as usize) * (linear.height as usize);
            let mut rgb = vec![0u8; pixel_count * 3];
            for i in 0..pixel_count {
                rgb[i * 3] = rgba[i * 4];
                rgb[i * 3 + 1] = rgba[i * 4 + 1];
                rgb[i * 3 + 2] = rgba[i * 4 + 2];
            }
            (linear.width, linear.height, rgb)
        }
        (None, Some(texture_path)) => {
            let img = image::open(&texture_path)
                .map_err(|err| {
                    format!("Textur '{texture_path}' konnte nicht geladen werden: {err}")
                })?
                .into_rgb8();
            let img = downsample_rgb_image(img, max_edge);
            let (width, height) = (img.width(), img.height());
            (width, height, img.into_raw())
        }
        _ => {
            return Err(
                "Entweder photo_id oder texture_path muss angegeben werden (nicht beides, nicht keines)."
                    .to_string(),
            );
        }
    };

    Ok(CompositeLayerSourceDto {
        bitmap_width: width,
        bitmap_height: height,
        pixels_base64: base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &rgb),
    })
}

// ---- KI: Tiefenschärfe-Simulator "Virtuelle Blende" (Phase 14 Schritt 8,
// siehe DECISIONS.md ADR-0041 Nachtrag VIII) ---------------------------------
//
// Opt-in, kein Bundling im Installer (dasselbe Muster wie das LaMa-
// Inpainting-Modell oben): der Nutzer bestätigt den ~64-MB-Download
// ausdrücklich im Einstellungsdialog, bevor irgendetwas heruntergeladen
// wird.

/// Öffentliche Download-URL des in `DECISIONS.md` ADR-0041 (Schritt 0 +
/// Nachtrag VIII) recherchierten Modells (`isl-org/MiDaS`, MIT,
/// GitHub-Release-Asset). **Anders als beim LaMa-Modell in dieser
/// Sitzung tatsächlich real heruntergeladen und geprüft** — siehe
/// [`MIDAS_MODEL_SHA256`].
const MIDAS_MODEL_URL: &str =
    "https://github.com/isl-org/MiDaS/releases/download/v2_1/model-small.onnx";
/// SHA-256 der real heruntergeladenen Datei (siehe `apx_ai::depth`s
/// Moduldoku für den genauen Verifikationsweg) — anders als beim
/// LaMa-Modell (`huggingface.co` aus dieser Sandbox nicht erreichbar)
/// hier eine echte Prüfsumme statt einer offenen Lücke.
const MIDAS_MODEL_SHA256: &str = "2d8c6cb8f415229daf1eb041024208e2608c9f98e17c81cc7c6ecb449c56fd58";

/// Lädt das MiDaS-Tiefenschätzungs-Modell herunter, prüft die Prüfsumme
/// gegen [`MIDAS_MODEL_SHA256`] und hinterlegt den Pfad in den
/// Einstellungen — nur bei erfolgreicher Prüfung, sonst wird die Datei
/// verworfen (kein potenziell manipuliertes/beschädigtes Modell landet
/// je in den Einstellungen).
#[tauri::command]
pub async fn download_depth_model(state: State<'_, AppState>) -> Result<String, String> {
    let response = reqwest::get(MIDAS_MODEL_URL)
        .await
        .map_err(|err| format!("Download von '{MIDAS_MODEL_URL}' fehlgeschlagen: {err}"))?;
    if !response.status().is_success() {
        return Err(format!(
            "Download von '{MIDAS_MODEL_URL}' fehlgeschlagen: HTTP {}",
            response.status()
        ));
    }
    let bytes = response
        .bytes()
        .await
        .map_err(|err| format!("Antwort konnte nicht gelesen werden: {err}"))?;

    use sha2::Digest as _;
    let mut hasher = sha2::Sha256::new();
    hasher.update(&bytes);
    let actual_hash = format!("{:x}", hasher.finalize());
    if actual_hash != MIDAS_MODEL_SHA256 {
        return Err(format!(
            "Prüfsumme stimmt nicht überein (erwartet {MIDAS_MODEL_SHA256}, erhalten {actual_hash}) — Download verworfen."
        ));
    }

    let dest_dir = state.paths.models_dir();
    std::fs::create_dir_all(&dest_dir).map_err(|err| err.to_string())?;
    let dest_path = dest_dir.join("midas_v21_small.onnx");
    std::fs::write(&dest_path, &bytes).map_err(|err| err.to_string())?;

    let path_string = dest_path.to_string_lossy().to_string();
    let settings_path = state.paths.settings_file();
    let mut settings =
        apx_core::Settings::load_or_default(&settings_path).map_err(|err| err.to_string())?;
    settings.ai.depth_model_path = Some(path_string.clone());
    settings
        .save(&settings_path)
        .map_err(|err| err.to_string())?;

    Ok(path_string)
}

/// Entfernt den hinterlegten Modellpfad (löscht die Datei selbst nicht —
/// dieselbe Zurückhaltung wie [`clear_inpainting_model_path`]).
#[tauri::command]
pub fn clear_depth_model_path(state: State<'_, AppState>) -> Result<(), String> {
    let path = state.paths.settings_file();
    let mut settings = apx_core::Settings::load_or_default(&path).map_err(|err| err.to_string())?;
    settings.ai.depth_model_path = None;
    settings.save(&path).map_err(|err| err.to_string())
}

#[derive(Debug, Clone, Serialize)]
pub struct DepthMapDto {
    pub bitmap_width: u32,
    pub bitmap_height: u32,
    /// Base64-kodierte `0..=255`-Tiefenkarte (`255` = am nächsten), ein
    /// Byte je Pixel — dieselbe Übertragungskonvention wie
    /// `AiMaskAlphaDto::alpha_base64`.
    pub depth_base64: String,
}

/// Berechnet einmalig eine echte monokulare Tiefenkarte für `photo_id`
/// per MiDaS v2.1 small (Phase 14 Schritt 8) — das Frontend ruft diesen
/// Command nur auf ausdrücklichen Nutzerwunsch ("Tiefenkarte berechnen")
/// auf, nicht bei jedem Regler-Tick, und speichert das Ergebnis als
/// `VirtualApertureAdjustment::depth_map` in der EDL (dasselbe „einmal
/// berechnen"-Muster wie [`run_ai_inpaint`]).
///
/// **Ehrliche Grenze, dieselbe wie [`run_ai_inpaint`]:** läuft auf dem
/// linearen Kamera-RGB-Dekodierergebnis (`decode_linear`), nicht auf
/// entwickelten sRGB-Pixeln — MiDaS wurde vermutlich auf gewöhnlichen
/// (sRGB-artigen) Fotos trainiert, ein linearer Farbraum ist eine
/// Näherung, keine exakte Übereinstimmung mit den Trainingsdaten
/// (dieselbe Art Kompromiss wie bei jeder anderen KI-Heuristik dieses
/// Projekts, die auf demselben Dekodierergebnis arbeitet).
#[tauri::command]
pub fn estimate_photo_depth(
    state: State<'_, AppState>,
    photo_id: String,
) -> Result<DepthMapDto, String> {
    let photo_id = parse_photo_id(photo_id)?;
    let settings = apx_core::Settings::load_or_default(&state.paths.settings_file())
        .map_err(|err| err.to_string())?;
    let model_path = settings
        .ai
        .depth_model_path
        .filter(|path| !path.trim().is_empty())
        .ok_or_else(|| {
            "Kein Tiefenschätzungs-Modell heruntergeladen — siehe Einstellungen → KI.".to_string()
        })?;

    let source_path = resolve_source_path_for_ai(&state.catalog, photo_id)?;
    let max_edge = Some(apx_ai::segmentation::ANALYSIS_MAX_EDGE);
    let linear = state
        .tile_cache
        .get_or_decode(photo_id, max_edge, || {
            apx_raw::decode_linear(&source_path, max_edge)
        })
        .map_err(|err| err.to_string())?;

    let pixel_count = (linear.width as usize) * (linear.height as usize);
    let rgb_u8: Vec<u8> = linear.pixels[..pixel_count * 3]
        .iter()
        .map(|&v| (v.clamp(0.0, 1.0) * 255.0).round() as u8)
        .collect();

    // Session wird pro Aufruf frisch geladen (dieselbe Begründung wie
    // `run_ai_inpaint`: einfacher, für einen Ein-Klick-Vorgang
    // akzeptabel).
    let mut session =
        apx_ai::depth::DepthSession::load(Path::new(&model_path)).map_err(|err| err.to_string())?;
    let depth = session
        .estimate_rgb8(&rgb_u8, linear.width, linear.height)
        .map_err(|err| err.to_string())?;
    let depth_u8: Vec<u8> = depth
        .iter()
        .map(|&v| (v.clamp(0.0, 1.0) * 255.0).round() as u8)
        .collect();

    Ok(DepthMapDto {
        bitmap_width: linear.width,
        bitmap_height: linear.height,
        depth_base64: base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &depth_u8),
    })
}

// ---- KI: Stiltransfer zwischen Fotos (Phase 14 Schritt 9, siehe
// DECISIONS.md ADR-0041 Nachtrag IX) -----------------------------------------
//
// Opt-in, kein Bundling im Installer (dasselbe Muster wie MiDaS/LaMa
// oben) — fünf unabhängig voneinander herunterladbare feste Stile
// (`apx_ai::style_transfer::StyleKind`), jeweils real per SHA-256
// geprüft.

/// Basis-URL für alle fünf `fast_neural_style`-Modelle — derselbe echte
/// Git-LFS-Auslieferungs-Host wie bei MiDaS' Recherche identifiziert
/// (`media.githubusercontent.com/media/...`, `raw.githubusercontent.com`
/// liefert für LFS-Dateien nur den Zeiger-Text, siehe `DECISIONS.md`
/// ADR-0041).
const STYLE_TRANSFER_BASE_URL: &str = "https://media.githubusercontent.com/media/onnx/models/main/validated/vision/style_transfer/fast_neural_style/model";

/// Real berechnete SHA-256-Hashes je Stil (alle fünf Dateien in dieser
/// Sitzung tatsächlich heruntergeladen, je exakt 6 728 029 Byte — siehe
/// `apx_ai::style_transfer`s Moduldoku) — wie bei MiDaS eine echte
/// Prüfsumme statt der bei LaMa dokumentierten offenen Lücke.
fn style_transfer_model_sha256(style: apx_ai::style_transfer::StyleKind) -> &'static str {
    use apx_ai::style_transfer::StyleKind;
    match style {
        StyleKind::Candy => "9d11a3529d1e547da6ae07201d93484dbab2ec0a3614535752c8f40f0fe2968a",
        StyleKind::Mosaic => "fa646dedade881243f8d5a2ceb7de2b93675b21fc24f7482894ac4851a9a0a47",
        StyleKind::RainPrincess => {
            "4162912e6f75fedef6f810ae989b9e10d3d5d43308dab34b027c850cf255e152"
        }
        StyleKind::Udnie => "8656b6ce7dec8f22ee13c2d557d6b67bd6f550dde88d0f2e7c9972aeb765cc0d",
        StyleKind::Pointilism => "5ee2b8d4d6bc60a777f54e0fe96a1b717360a004b79d56c67390d4a975b14d98",
    }
}

/// Lädt das ONNX-Modell für genau einen Stil herunter, prüft die
/// Prüfsumme gegen [`style_transfer_model_sha256`] und hinterlegt den
/// Pfad in den Einstellungen (unter `style.id()` als Schlüssel) — nur
/// bei erfolgreicher Prüfung, sonst wird die Datei verworfen.
#[tauri::command]
pub async fn download_style_transfer_model(
    state: State<'_, AppState>,
    style: String,
) -> Result<String, String> {
    let kind = apx_ai::style_transfer::StyleKind::from_id(&style)
        .ok_or_else(|| format!("Unbekannter Stil '{style}'."))?;
    let url = format!("{STYLE_TRANSFER_BASE_URL}/{}-9.onnx", kind.id());
    let response = reqwest::get(&url)
        .await
        .map_err(|err| format!("Download von '{url}' fehlgeschlagen: {err}"))?;
    if !response.status().is_success() {
        return Err(format!(
            "Download von '{url}' fehlgeschlagen: HTTP {}",
            response.status()
        ));
    }
    let bytes = response
        .bytes()
        .await
        .map_err(|err| format!("Antwort konnte nicht gelesen werden: {err}"))?;

    use sha2::Digest as _;
    let mut hasher = sha2::Sha256::new();
    hasher.update(&bytes);
    let actual_hash = format!("{:x}", hasher.finalize());
    let expected_hash = style_transfer_model_sha256(kind);
    if actual_hash != expected_hash {
        return Err(format!(
            "Prüfsumme stimmt nicht überein (erwartet {expected_hash}, erhalten {actual_hash}) — Download verworfen."
        ));
    }

    let dest_dir = state.paths.models_dir();
    std::fs::create_dir_all(&dest_dir).map_err(|err| err.to_string())?;
    let dest_path = dest_dir.join(format!("style_transfer_{}.onnx", kind.id()));
    std::fs::write(&dest_path, &bytes).map_err(|err| err.to_string())?;

    let path_string = dest_path.to_string_lossy().to_string();
    let settings_path = state.paths.settings_file();
    let mut settings =
        apx_core::Settings::load_or_default(&settings_path).map_err(|err| err.to_string())?;
    settings
        .ai
        .style_transfer_model_paths
        .insert(kind.id().to_string(), path_string.clone());
    settings
        .save(&settings_path)
        .map_err(|err| err.to_string())?;

    Ok(path_string)
}

/// Entfernt den hinterlegten Pfad für genau einen Stil (löscht die Datei
/// selbst nicht — dieselbe Zurückhaltung wie [`clear_depth_model_path`]).
#[tauri::command]
pub fn clear_style_transfer_model_path(
    state: State<'_, AppState>,
    style: String,
) -> Result<(), String> {
    let kind = apx_ai::style_transfer::StyleKind::from_id(&style)
        .ok_or_else(|| format!("Unbekannter Stil '{style}'."))?;
    let path = state.paths.settings_file();
    let mut settings = apx_core::Settings::load_or_default(&path).map_err(|err| err.to_string())?;
    settings.ai.style_transfer_model_paths.remove(kind.id());
    settings.save(&path).map_err(|err| err.to_string())
}

#[derive(Debug, Clone, Serialize)]
pub struct StyleTransferPatchDto {
    pub bitmap_width: u32,
    pub bitmap_height: u32,
    /// Base64-kodiertes interleaved-RGB-`u8`-Ergebnis — dasselbe
    /// Übertragungsmuster wie `CompositeLayerSourceDto::pixels_base64`.
    pub pixels_base64: String,
}

/// Berechnet einmalig das stilisierte Ergebnis für `photo_id` mit dem
/// gewählten `style` (Phase 14 Schritt 9) — das Frontend ruft diesen
/// Command nur auf ausdrücklichen Nutzerwunsch auf, nicht bei jedem
/// Regler-Tick, und speichert das Ergebnis als
/// `StyleTransferAdjustment::patch` in der EDL (dasselbe „einmal
/// berechnen"-Muster wie [`estimate_photo_depth`]/[`run_ai_inpaint`]).
///
/// **Anders als `estimate_photo_depth`/`run_ai_inpaint`: läuft auf dem
/// bereits nach sRGB konvertierten Dekodierergebnis**, nicht auf rohen
/// linearen Pixeln — dieselbe Farbraum-Wahl wie
/// [`prepare_composite_layer_source`] (Schritt 3), weil das Ergebnis
/// hier direkt als sichtbares Bild ins fertig entwickelte Foto
/// zurückgeblendet wird (`stages::style_transfer::apply`), nicht nur
/// als Zwischengröße für eine weitere Berechnung dient wie eine
/// Tiefenkarte — ein linearer Kompromiss wäre hier ein sichtbarer
/// Tonwert-Fehler, keine bloße Ungenauigkeit.
#[tauri::command]
pub fn stylize_photo(
    state: State<'_, AppState>,
    photo_id: String,
    style: String,
) -> Result<StyleTransferPatchDto, String> {
    let kind = apx_ai::style_transfer::StyleKind::from_id(&style)
        .ok_or_else(|| format!("Unbekannter Stil '{style}'."))?;
    let photo_id = parse_photo_id(photo_id)?;
    let settings = apx_core::Settings::load_or_default(&state.paths.settings_file())
        .map_err(|err| err.to_string())?;
    let model_path = settings
        .ai
        .style_transfer_model_paths
        .get(kind.id())
        .filter(|path| !path.trim().is_empty())
        .ok_or_else(|| {
            format!(
                "Stil '{}' noch nicht heruntergeladen — siehe Einstellungen → KI.",
                kind.id()
            )
        })?
        .clone();

    let max_edge = apx_ai::segmentation::ANALYSIS_MAX_EDGE;
    let source_path = resolve_source_path_for_ai(&state.catalog, photo_id)?;
    let linear =
        apx_raw::decode_linear(&source_path, Some(max_edge)).map_err(|err| err.to_string())?;
    let rgba =
        apx_pipeline::color::linear_camera_rgb_to_srgb_rgba8(&linear.pixels, linear.cam_to_srgb);
    let pixel_count = (linear.width as usize) * (linear.height as usize);
    let mut rgb = vec![0u8; pixel_count * 3];
    for i in 0..pixel_count {
        rgb[i * 3] = rgba[i * 4];
        rgb[i * 3 + 1] = rgba[i * 4 + 1];
        rgb[i * 3 + 2] = rgba[i * 4 + 2];
    }

    // Session wird pro Aufruf frisch geladen (dieselbe Begründung wie
    // `run_ai_inpaint`/`estimate_photo_depth`: einfacher, für einen
    // Ein-Klick-Vorgang akzeptabel).
    let mut session = apx_ai::style_transfer::StyleTransferSession::load(Path::new(&model_path))
        .map_err(|err| err.to_string())?;
    let styled = session
        .stylize_rgb8(&rgb, linear.width, linear.height)
        .map_err(|err| err.to_string())?;

    Ok(StyleTransferPatchDto {
        bitmap_width: linear.width,
        bitmap_height: linear.height,
        pixels_base64: base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &styled),
    })
}

// ---- Himmelsaustausch mit automatischer Neubelichtung (Phase 14
// Schritt 10) — klassischer Algorithmus, kein ONNX-Modell. -----------------

#[derive(Debug, Clone, Serialize)]
pub struct SkyReplacePatchDto {
    pub bitmap_width: u32,
    pub bitmap_height: u32,
    pub pixels_base64: String,
}

/// Ersetzt den Himmel in `photo_id` durch das Foto unter `sky_image_path`
/// und gleicht den Vordergrund grob an dessen Farbtemperatur/Helligkeit
/// an (`apx_ai::sky_replace::composite`).
#[tauri::command]
pub fn replace_sky(
    state: State<'_, AppState>,
    photo_id: String,
    sky_image_path: String,
) -> Result<SkyReplacePatchDto, String> {
    let photo_id = parse_photo_id(photo_id)?;
    let max_edge = apx_ai::segmentation::ANALYSIS_MAX_EDGE;
    let source_path = resolve_source_path_for_ai(&state.catalog, photo_id)?;
    let linear =
        apx_raw::decode_linear(&source_path, Some(max_edge)).map_err(|err| err.to_string())?;
    let alpha = apx_ai::segmentation::sky_alpha(&linear.pixels, linear.width, linear.height)
        .map_err(|err| err.to_string())?;
    let rgba =
        apx_pipeline::color::linear_camera_rgb_to_srgb_rgba8(&linear.pixels, linear.cam_to_srgb);
    let pixel_count = (linear.width as usize) * (linear.height as usize);
    let mut rgb = vec![0u8; pixel_count * 3];
    for i in 0..pixel_count {
        rgb[i * 3] = rgba[i * 4];
        rgb[i * 3 + 1] = rgba[i * 4 + 1];
        rgb[i * 3 + 2] = rgba[i * 4 + 2];
    }

    let sky_img = image::open(&sky_image_path)
        .map_err(|err| {
            format!("Himmel-Foto '{sky_image_path}' konnte nicht geladen werden: {err}")
        })?
        .into_rgb8();
    let sky_img = downsample_rgb_image(sky_img, max_edge);
    let (sky_w, sky_h) = (sky_img.width(), sky_img.height());

    let composited = apx_ai::sky_replace::composite(
        &rgb,
        linear.width,
        linear.height,
        &alpha,
        sky_img.as_raw(),
        sky_w,
        sky_h,
    );

    Ok(SkyReplacePatchDto {
        bitmap_width: linear.width,
        bitmap_height: linear.height,
        pixels_base64: base64::Engine::encode(
            &base64::engine::general_purpose::STANDARD,
            &composited,
        ),
    })
}

// ---- KI: Echte Personen-Wiedererkennung (Phase 13 Schritt 8, siehe
// DECISIONS.md ADR-0040-Nachtrag VI) -----------------------------------------
//
// Opt-in, kein Bundling im Installer (dasselbe Muster wie das LaMa-
// Inpainting-Modell oben): der Nutzer bestätigt den Download der beiden
// gemeinfreien `dlib`-Modelldateien ausdrücklich im Einstellungsdialog.
// Die eigentliche `dlib`-Bindung steckt hinter dem standardmäßig
// ausgeschalteten Cargo-Feature `people` (siehe `apx-ai::people`s
// Moduldoku, `apx-tether`s `tethering`-Feature für dieselbe Konvention);
// `detect_faces_for_photo` gibt ohne dieses Feature einen klaren Fehler
// statt eines stillen No-Ops zurück.

const PEOPLE_LANDMARK_MODEL_URL: &str =
    "http://dlib.net/files/shape_predictor_5_face_landmarks.dat.bz2";
const PEOPLE_ENCODER_MODEL_URL: &str =
    "http://dlib.net/files/dlib_face_recognition_resnet_model_v1.dat.bz2";

async fn download_and_decompress_bz2(url: &str, dest: &Path) -> Result<(), String> {
    let response = reqwest::get(url)
        .await
        .map_err(|err| format!("Download von '{url}' fehlgeschlagen: {err}"))?;
    if !response.status().is_success() {
        return Err(format!(
            "Download von '{url}' fehlgeschlagen: HTTP {}",
            response.status()
        ));
    }
    let compressed = response
        .bytes()
        .await
        .map_err(|err| format!("Antwort konnte nicht gelesen werden: {err}"))?;
    let mut decoder = bzip2::read::BzDecoder::new(compressed.as_ref());
    let mut bytes = Vec::new();
    std::io::Read::read_to_end(&mut decoder, &mut bytes)
        .map_err(|err| format!("bz2-Dekompression fehlgeschlagen: {err}"))?;
    std::fs::write(dest, &bytes).map_err(|err| err.to_string())?;
    Ok(())
}

/// Lädt beide für Personen-Wiedererkennung nötigen Modelle herunter
/// (siehe [`PEOPLE_LANDMARK_MODEL_URL`]/[`PEOPLE_ENCODER_MODEL_URL`]s
/// Herkunft in `apx-ai::people`s Moduldoku — **nicht in dieser Sitzung
/// erreichbar/verifiziert**, `dlib.net` ist von dieser Entwicklungs-
/// Sandbox aus blockiert, siehe `apx-ai::people`s Moduldoku) und
/// hinterlegt beide Pfade in den Einstellungen. **Keine Hash-Prüfung**
/// — dieselbe ehrliche Lücke wie beim LaMa-Modell oben, aus demselben
/// Grund (kein erreichbarer, verifizierbarer Hash in dieser Sitzung).
#[tauri::command]
pub async fn download_people_models(state: State<'_, AppState>) -> Result<(), String> {
    let dest_dir = state.paths.models_dir();
    std::fs::create_dir_all(&dest_dir).map_err(|err| err.to_string())?;
    let landmark_path = dest_dir.join("shape_predictor_5_face_landmarks.dat");
    let encoder_path = dest_dir.join("dlib_face_recognition_resnet_model_v1.dat");

    download_and_decompress_bz2(PEOPLE_LANDMARK_MODEL_URL, &landmark_path).await?;
    download_and_decompress_bz2(PEOPLE_ENCODER_MODEL_URL, &encoder_path).await?;

    let settings_path = state.paths.settings_file();
    let mut settings =
        apx_core::Settings::load_or_default(&settings_path).map_err(|err| err.to_string())?;
    settings.ai.people_landmark_model_path = Some(landmark_path.to_string_lossy().to_string());
    settings.ai.people_encoder_model_path = Some(encoder_path.to_string_lossy().to_string());
    settings.save(&settings_path).map_err(|err| err.to_string())
}

/// Entfernt beide hinterlegten Modellpfade (löscht die Dateien selbst
/// nicht, siehe [`clear_inpainting_model_path`]s Begründung).
#[tauri::command]
pub fn clear_people_model_paths(state: State<'_, AppState>) -> Result<(), String> {
    let path = state.paths.settings_file();
    let mut settings = apx_core::Settings::load_or_default(&path).map_err(|err| err.to_string())?;
    settings.ai.people_landmark_model_path = None;
    settings.ai.people_encoder_model_path = None;
    settings.save(&path).map_err(|err| err.to_string())
}

#[cfg(feature = "people")]
fn build_person_embedder(
    settings: &apx_core::AiSettings,
) -> Result<apx_ai::people::PersonEmbedder, String> {
    let landmark_path = settings
        .people_landmark_model_path
        .as_deref()
        .filter(|p| !p.trim().is_empty())
        .ok_or_else(|| {
            "Kein Landmarken-Modell heruntergeladen — siehe Einstellungen → KI.".to_string()
        })?;
    let encoder_path = settings
        .people_encoder_model_path
        .as_deref()
        .filter(|p| !p.trim().is_empty())
        .ok_or_else(|| {
            "Kein Embedding-Modell heruntergeladen — siehe Einstellungen → KI.".to_string()
        })?;
    apx_ai::people::PersonEmbedder::new(Path::new(landmark_path), Path::new(encoder_path))
        .map_err(|err| err.to_string())
}

#[cfg(not(feature = "people"))]
fn build_person_embedder(
    _settings: &apx_core::AiSettings,
) -> Result<std::convert::Infallible, String> {
    Err(
        "Diese Aperture-X-Build wurde ohne echte Personen-Wiedererkennung kompiliert (Cargo-Feature \"people\" fehlt — libdlib/libblas/liblapack sind nicht in jeder Umgebung installiert)."
            .to_string(),
    )
}

#[cfg(feature = "people")]
fn detect_faces_with_embedder(
    embedder: &apx_ai::people::PersonEmbedder,
    img: &image::RgbImage,
) -> Result<Vec<(apx_catalog::FaceRect, Vec<f64>)>, String> {
    let detected = embedder
        .detect_and_embed(img.as_raw(), img.width(), img.height())
        .map_err(|err| err.to_string())?;
    Ok(detected
        .into_iter()
        .map(|face| {
            (
                (face.left, face.top, face.right, face.bottom),
                face.embedding,
            )
        })
        .collect())
}

#[cfg(not(feature = "people"))]
fn detect_faces_with_embedder(
    embedder: &std::convert::Infallible,
    _img: &image::RgbImage,
) -> Result<Vec<(apx_catalog::FaceRect, Vec<f64>)>, String> {
    match *embedder {}
}

#[derive(Debug, Clone, Serialize)]
pub struct FaceDetectionDto {
    pub id: String,
    pub photo_id: String,
    pub person_id: Option<String>,
    pub rect_left: i64,
    pub rect_top: i64,
    pub rect_right: i64,
    pub rect_bottom: i64,
}

impl From<apx_catalog::FaceDetection> for FaceDetectionDto {
    fn from(face: apx_catalog::FaceDetection) -> Self {
        Self {
            id: face.id.to_string(),
            photo_id: face.photo_id.to_string(),
            person_id: face.person_id.map(|id| id.to_string()),
            rect_left: face.rect_left,
            rect_top: face.rect_top,
            rect_right: face.rect_right,
            rect_bottom: face.rect_bottom,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct PersonDto {
    pub id: String,
    pub name: Option<String>,
    pub cover_face_id: Option<String>,
}

impl From<apx_catalog::Person> for PersonDto {
    fn from(person: apx_catalog::Person) -> Self {
        Self {
            id: person.id.to_string(),
            name: person.name,
            cover_face_id: person.cover_face_id.map(|id| id.to_string()),
        }
    }
}

/// Erkennt alle Gesichter in `photo_id`s bereits gerenderter Standard-
/// Vorschau (`apx_catalog::PreviewLevel::Standard` — dieselbe
/// Auflösungsbegrenzung wie jede andere KI-Analyse in diesem Projekt),
/// speichert sie (ersetzt frühere Erkennungen desselben Fotos) und
/// ordnet neue Gesichter automatisch bereits benannten Personen zu, wenn
/// deren Embedding-Abstand unter der Schwelle liegt (siehe
/// `apx_catalog::Catalog::save_face_detections`s Moduldoku).
#[tauri::command]
pub fn detect_faces_for_photo(
    state: State<'_, AppState>,
    photo_id: String,
) -> Result<Vec<FaceDetectionDto>, String> {
    let photo_id = parse_photo_id(photo_id)?;
    let settings = apx_core::Settings::load_or_default(&state.paths.settings_file())
        .map_err(|err| err.to_string())?;
    let embedder = build_person_embedder(&settings.ai)?;

    let preview = state
        .catalog
        .get_preview(photo_id, apx_catalog::PreviewLevel::Standard)
        .map_err(|err| err.to_string())?
        .ok_or_else(|| {
            "Keine Vorschau für dieses Foto vorhanden — zuerst öffnen/entwickeln.".to_string()
        })?;
    let img = image::open(&preview.path)
        .map_err(|err| format!("Vorschau konnte nicht gelesen werden: {err}"))?
        .to_rgb8();

    let detections = detect_faces_with_embedder(&embedder, &img)?;

    let saved = state
        .catalog
        .save_face_detections(photo_id, &detections)
        .map_err(|err| err.to_string())?;
    Ok(saved.into_iter().map(FaceDetectionDto::from).collect())
}

#[tauri::command]
pub fn list_faces_for_photo(
    state: State<'_, AppState>,
    photo_id: String,
) -> Result<Vec<FaceDetectionDto>, String> {
    let photo_id = parse_photo_id(photo_id)?;
    let faces = state
        .catalog
        .list_faces_for_photo(photo_id)
        .map_err(|err| err.to_string())?;
    Ok(faces.into_iter().map(FaceDetectionDto::from).collect())
}

#[tauri::command]
pub fn list_people(state: State<'_, AppState>) -> Result<Vec<PersonDto>, String> {
    let people = state.catalog.list_people().map_err(|err| err.to_string())?;
    Ok(people.into_iter().map(PersonDto::from).collect())
}

/// Alle Fotos, die mindestens ein `person_id` zugeordnetes Gesicht
/// enthalten — nach Dateiname sortiert wie andere Foto-Listen in diesem
/// Projekt, doppelte Fotos (mehrere Gesichter derselben Person auf einem
/// Foto) werden herausgefiltert.
#[tauri::command]
pub fn list_photos_for_person(
    state: State<'_, AppState>,
    person_id: String,
) -> Result<Vec<PhotoDto>, String> {
    let person_id: apx_core::PersonId = person_id
        .parse()
        .map_err(|_| "Ungültige Personen-ID".to_string())?;
    let faces = state
        .catalog
        .list_faces_for_person(person_id)
        .map_err(|err| err.to_string())?;
    let mut seen = std::collections::HashSet::new();
    let mut photos = Vec::new();
    for face in faces {
        if !seen.insert(face.photo_id) {
            continue;
        }
        if let Ok(photo) = state.catalog.get_photo(face.photo_id) {
            photos.push(PhotoDto::from(photo));
        }
    }
    photos.sort_by(|a, b| a.filename.cmp(&b.filename));
    Ok(photos)
}

#[tauri::command]
pub fn create_person(state: State<'_, AppState>, name: Option<String>) -> Result<String, String> {
    let id = state
        .catalog
        .create_person(name.as_deref().filter(|n| !n.trim().is_empty()))
        .map_err(|err| err.to_string())?;
    Ok(id.to_string())
}

#[tauri::command]
pub fn rename_person(
    state: State<'_, AppState>,
    person_id: String,
    name: Option<String>,
) -> Result<(), String> {
    let person_id: apx_core::PersonId = person_id
        .parse()
        .map_err(|_| "Ungültige Personen-ID".to_string())?;
    state
        .catalog
        .rename_person(person_id, name.as_deref().filter(|n| !n.trim().is_empty()))
        .map_err(|err| err.to_string())
}

#[tauri::command]
pub fn delete_person(state: State<'_, AppState>, person_id: String) -> Result<(), String> {
    let person_id: apx_core::PersonId = person_id
        .parse()
        .map_err(|_| "Ungültige Personen-ID".to_string())?;
    state
        .catalog
        .delete_person(person_id)
        .map_err(|err| err.to_string())
}

/// Ordnet ein Gesicht manuell einer Person zu — `person_id: None` legt
/// eine neue, noch unbenannte Person an und ordnet das Gesicht dieser
/// zu (derselbe Ablauf wie „Als neue Person markieren" in Adobe
/// Lightroom Classics Personenansicht).
#[tauri::command]
pub fn assign_face_to_person(
    state: State<'_, AppState>,
    face_id: String,
    person_id: Option<String>,
) -> Result<String, String> {
    let face_id: apx_core::FaceDetectionId = face_id
        .parse()
        .map_err(|_| "Ungültige Gesichts-ID".to_string())?;
    let person_id = match person_id {
        Some(id) => id
            .parse()
            .map_err(|_| "Ungültige Personen-ID".to_string())?,
        None => state
            .catalog
            .create_person(None)
            .map_err(|err| err.to_string())?,
    };
    state
        .catalog
        .assign_face_to_person(face_id, person_id)
        .map_err(|err| err.to_string())?;
    Ok(person_id.to_string())
}

#[tauri::command]
pub fn unassign_face(state: State<'_, AppState>, face_id: String) -> Result<(), String> {
    let face_id: apx_core::FaceDetectionId = face_id
        .parse()
        .map_err(|_| "Ungültige Gesichts-ID".to_string())?;
    state
        .catalog
        .unassign_face(face_id)
        .map_err(|err| err.to_string())
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

// ---- Beobachteter Ordner / Auto-Import (Phase 12 Schritt 7) ---------------
//
// Dasselbe Lade-/Speicher-Muster wie `get_ai_settings`/`get_ui_settings`
// oben. Der eigentliche Hintergrund-Worker (`watched_folder_worker` in
// `main.rs`) liest dieselbe Datei direkt, ohne über diese Commands zu
// gehen — hier nur die Frontend-Verdrahtung zum Anzeigen/Ändern.

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WatchedFolderSettingsDto {
    pub path: Option<String>,
    pub enabled: bool,
    pub poll_seconds: u32,
}

impl From<apx_core::WatchedFolderSettings> for WatchedFolderSettingsDto {
    fn from(wf: apx_core::WatchedFolderSettings) -> Self {
        Self {
            path: wf.path,
            enabled: wf.enabled,
            poll_seconds: wf.poll_seconds,
        }
    }
}

#[tauri::command]
pub fn get_watched_folder_settings(
    state: State<'_, AppState>,
) -> Result<WatchedFolderSettingsDto, String> {
    let settings = apx_core::Settings::load_or_default(&state.paths.settings_file())
        .map_err(|err| err.to_string())?;
    Ok(settings.watched_folder.into())
}

#[tauri::command]
pub fn set_watched_folder_settings(
    state: State<'_, AppState>,
    settings: WatchedFolderSettingsDto,
) -> Result<(), String> {
    let path = state.paths.settings_file();
    let mut all = apx_core::Settings::load_or_default(&path).map_err(|err| err.to_string())?;
    all.watched_folder = apx_core::WatchedFolderSettings {
        path: settings.path.filter(|p| !p.trim().is_empty()),
        enabled: settings.enabled,
        // Ein zu kleines Intervall würde den beobachteten Ordner bei
        // jedem Poll komplett neu scannen (`run_with_mode` scannt den
        // gesamten Baum, kein inkrementeller Dateisystem-Watcher, siehe
        // `main.rs`s Moduldoku) — 5 Sekunden als niedrigste sinnvolle
        // Untergrenze.
        poll_seconds: settings.poll_seconds.max(5),
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

pub(crate) fn parse_icc_target(
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

/// Übergänge für die Video-Zeitachse (Phase 17 Schritt 3, siehe
/// `DECISIONS.md` ADR-0045) — eigener Parser statt Wiederverwendung
/// von [`parse_transition_kind`], weil `TimelineTransitionKind` mehr
/// Varianten kennt als die Diashow (`cut`/`cross_fade`).
fn parse_timeline_transition_kind(
    transition: &str,
) -> Result<apx_export::timeline::TimelineTransitionKind, String> {
    use apx_export::timeline::TimelineTransitionKind as T;
    match transition {
        "cut" => Ok(T::Cut),
        "fade" => Ok(T::Fade),
        "dissolve" => Ok(T::Dissolve),
        "wipe_left" => Ok(T::WipeLeft),
        "wipe_right" => Ok(T::WipeRight),
        "slide_up" => Ok(T::SlideUp),
        "slide_down" => Ok(T::SlideDown),
        "circle_open" => Ok(T::CircleOpen),
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

// ---- Mehrere Kataloge + Katalog-Wartung (Phase 13 Schritt 6, siehe
// DECISIONS.md ADR-0040-Nachtrag IV) -----------------------------------
//
// **Kein Hot-Swap der offenen Katalogverbindung im laufenden Prozess:**
// `AppState::catalog` ist ein `Arc<Catalog>`, von dem praktisch jeder
// Command in dieser Datei eine Kopie hält oder direkt referenziert — ein
// echtes Austauschen der Verbindung würde entweder jeden dieser
// Zugriffe hinter ein zusätzliches Lock verlegen (invasiv, hohes
// Fehlerrisiko quer durch die ganze Datei) oder den `Arc` selbst
// austauschbar machen (bringt dieselbe Umbau-Größe). Stattdessen exakt
// dieselbe UX wie Adobe Lightroom Classics eigener Katalogwechsel:
// „Diese Änderung erfordert einen Neustart" — Wechseln/Neuanlegen
// speichert den Zielpfad in den Einstellungen und startet die App über
// `AppHandle::request_restart` neu, die beim nächsten Start (siehe
// `main.rs`) automatisch den neuen Pfad öffnet.

/// Informationen zum aktuell geöffneten Katalog — für eine
/// "Katalog-Informationen"-Anzeige, ergänzend zu [`catalog_statistics`].
#[derive(Debug, Clone, Serialize)]
pub struct CatalogInfoDto {
    pub path: String,
    /// `None`, wenn die Dateigröße nicht ermittelbar ist (sollte für den
    /// aktuell offenen Katalog praktisch nie vorkommen).
    pub file_size_bytes: Option<u64>,
}

#[tauri::command]
pub fn get_active_catalog_info(state: State<'_, AppState>) -> CatalogInfoDto {
    CatalogInfoDto {
        path: state.catalog_path.display().to_string(),
        file_size_bytes: std::fs::metadata(&state.catalog_path)
            .map(|meta| meta.len())
            .ok(),
    }
}

/// Ein Eintrag der "Zuletzt geöffnet"-Liste (`Settings::catalog::
/// recent_catalogs`) — `exists`/`file_size_bytes` live vom Dateisystem
/// abgefragt statt in den Einstellungen mitgeführt, damit ein seit dem
/// letzten Öffnen verändertes/gelöschtes/wieder aufgetauchtes Netzlaufwerk
/// sich sofort korrekt widerspiegelt.
#[derive(Debug, Clone, Serialize)]
pub struct RecentCatalogDto {
    pub path: String,
    pub file_name: String,
    pub exists: bool,
    pub is_current: bool,
    pub file_size_bytes: Option<u64>,
}

#[tauri::command]
pub fn list_recent_catalogs(state: State<'_, AppState>) -> Result<Vec<RecentCatalogDto>, String> {
    let settings = apx_core::Settings::load_or_default(&state.paths.settings_file())
        .map_err(|err| err.to_string())?;
    Ok(settings
        .catalog
        .recent_catalogs
        .into_iter()
        .map(|path| {
            let path_buf = PathBuf::from(&path);
            let metadata = std::fs::metadata(&path_buf).ok();
            RecentCatalogDto {
                file_name: path_buf
                    .file_name()
                    .map(|name| name.to_string_lossy().into_owned())
                    .unwrap_or_else(|| path.clone()),
                exists: metadata.is_some(),
                is_current: path_buf == state.catalog_path,
                file_size_bytes: metadata.map(|meta| meta.len()),
                path,
            }
        })
        .collect())
}

/// Öffnet `path` kurz zur Prüfung (das lässt bei einer bestehenden
/// fremden, nicht-Aperture-X-SQLite-Datei den Wechsel ehrlich mit einem
/// echten SQL-Fehler scheitern, statt sie erst beim nächsten Start
/// kaputtzumachen), trägt ihn dann als zuletzt geöffneten Katalog ein und
/// startet die App neu.
fn persist_catalog_choice_and_restart(
    app: &AppHandle,
    state: &State<'_, AppState>,
    path: &str,
) -> Result<(), String> {
    apx_catalog::Catalog::open(Path::new(path)).map_err(|err| err.to_string())?;

    let settings_path = state.paths.settings_file();
    let mut settings =
        apx_core::Settings::load_or_default(&settings_path).map_err(|err| err.to_string())?;
    settings.catalog.record_opened(path);
    settings
        .save(&settings_path)
        .map_err(|err| err.to_string())?;

    app.request_restart();
    Ok(())
}

/// Legt einen neuen, leeren Katalog unter `path` an (frisches Schema per
/// Migrationen, siehe `apx_catalog::Catalog::open`) und wechselt per
/// Neustart zu ihm. Lehnt einen bereits existierenden Pfad ab — sonst
/// könnte ein Nutzer versehentlich eine fremde Datei auswählen und sie
/// überschreiben; zum Öffnen eines bestehenden Katalogs ist
/// [`switch_active_catalog`] da.
#[tauri::command]
pub fn create_new_catalog(
    app: AppHandle,
    state: State<'_, AppState>,
    path: String,
) -> Result<(), String> {
    if Path::new(&path).exists() {
        return Err(format!(
            "'{path}' existiert bereits — zum Öffnen eines bestehenden Katalogs „Katalog öffnen…“ verwenden"
        ));
    }
    persist_catalog_choice_and_restart(&app, &state, &path)
}

/// Wechselt per Neustart zu einem bestehenden Katalog unter `path`.
#[tauri::command]
pub fn switch_active_catalog(
    app: AppHandle,
    state: State<'_, AppState>,
    path: String,
) -> Result<(), String> {
    persist_catalog_choice_and_restart(&app, &state, &path)
}

/// Führt `PRAGMA integrity_check` auf dem aktuell geöffneten Katalog aus
/// (siehe `apx_catalog::Catalog::integrity_check`s Doku) — leere Liste =
/// keine Probleme gefunden.
#[tauri::command]
pub fn run_catalog_integrity_check(state: State<'_, AppState>) -> Result<Vec<String>, String> {
    state
        .catalog
        .integrity_check()
        .map_err(|err| err.to_string())
}

/// Führt `VACUUM` auf dem aktuell geöffneten Katalog aus (siehe
/// `apx_catalog::Catalog::vacuum`s Doku) — gibt durch Löschungen
/// freigewordenen Speicherplatz zurück und defragmentiert die Datei.
#[tauri::command]
pub fn run_catalog_optimize(state: State<'_, AppState>) -> Result<(), String> {
    state.catalog.vacuum().map_err(|err| err.to_string())
}

/// Sichert den aktuell geöffneten Katalog nach `destination_path` (siehe
/// `apx_catalog::Catalog::backup_to`s Doku) — der Zielpfad wird über den
/// bereits bestehenden generischen `pick_save_file_path`-Command im
/// Frontend ausgewählt.
#[tauri::command]
pub fn run_catalog_backup(
    state: State<'_, AppState>,
    destination_path: String,
) -> Result<(), String> {
    state
        .catalog
        .backup_to(Path::new(&destination_path))
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
        media_kind: "photo".to_string(),
        duration_ms: None,
        video_codec: None,
        has_audio: None,
        frame_rate: None,
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

/// Versucht das echte merkmalsbasierte Homographie-Stitching
/// (`apx_stacking::homography_stitch`, Phase 13 Schritt 5) für die ganze
/// Bilderserie — `None`, wenn für mindestens eines der Fotos ab dem
/// zweiten keine verlässliche Homografie gefunden wurde (zu wenige/zu
/// unzuverlässige Merkmalsübereinstimmungen). Bewusst alles-oder-nichts
/// für die ganze Serie statt eines Fotos mit Homografie und eines mit
/// reiner Verschiebung gemischt auf derselben Leinwand — eine echte
/// Mischkomposition bräuchte eine gemeinsame Canvas-Berechnung über
/// beide Positionierungsarten hinweg, siehe `stack_panorama`s Rückfall
/// auf die gesamte Serie stattdessen.
fn try_homography_panorama(
    rendered: &[(u32, u32, Vec<u8>)],
    width: u32,
    height: u32,
) -> Option<(u32, u32, Vec<u8>)> {
    let reference = rendered[0].2.as_slice();
    let others: Vec<&[u8]> = rendered[1..]
        .iter()
        .map(|(_, _, px)| px.as_slice())
        .collect();
    let homographies = apx_stacking::homography_stitch::estimate_pairwise_homographies_rgba8(
        reference, &others, width, height,
    );
    let mut images = Vec::with_capacity(rendered.len());
    images.push(apx_stacking::homography_stitch::HomographyPositionedImage {
        pixels: reference,
        homography: nalgebra::Matrix3::identity(),
    });
    for (pixels, homography) in others.into_iter().zip(homographies) {
        images.push(apx_stacking::homography_stitch::HomographyPositionedImage {
            pixels,
            homography: homography?,
        });
    }
    apx_stacking::homography_stitch::stitch_homography_rgba8(&images, width, height).ok()
}

/// Panorama-Zusammenführung — versucht zuerst echtes merkmalsbasiertes
/// Homographie-Stitching (`apx_stacking::homography_stitch`, Phase 13
/// Schritt 5, siehe `DECISIONS.md` ADR-0040-Nachtrag III), geeignet für
/// Freihandaufnahmen mit Rotation/Perspektive/Parallaxe. Fällt auf die
/// einfachere reine Verschiebungs-Registrierung (`apx_stacking::
/// panorama`, Phasenkorrelation) zurück, wenn keine verlässliche
/// Homografie gefunden wird — z. B. bei zu wenig Bildstruktur/
/// Überlappung für genug Merkmalsübereinstimmungen, dann aber weiterhin
/// für reine Stativaufnahmen ohne Kamerarotation geeignet.
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

    if let Some((out_width, out_height, stitched)) =
        try_homography_panorama(&rendered, width, height)
    {
        return import_stack_result_photo(
            &state, &ids, "Panorama", "panorama", out_width, out_height, &stitched,
        );
    }

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

// ---- Direktimport von Speicherkarte/Kamera (Phase 13 Schritt 2) -----------
//
// Zwei unabhängige Wege: (1) Wechseldatenträger-Erkennung per `sysinfo`
// (reine Bequemlichkeit — der Nutzer bestätigt weiterhin per Klick, kein
// neuer Berechtigungsrahmen), (2) bereits auf einer verbundenen Kamera
// vorhandene Dateien über `apx_tether`s `list_camera_files`/
// `download_camera_file` (Phase 13 Schritt 2, dieselbe Kamera-Verbindung
// wie beim Tethering oben, hier zum Abholen bereits aufgenommener Dateien
// statt Live-Auslösen). Beide münden im bestehenden Import-Pfad
// (`import::run_with_mode`, Phase 3/5), unverändert.

#[derive(Debug, Clone, Serialize)]
pub struct RemovableVolumeDto {
    pub mount_point: String,
    pub name: String,
    /// `true`, wenn der Datenträger einen `DCIM`-Ordner (groß- oder
    /// kleingeschrieben) im Wurzelverzeichnis hat — die übliche
    /// Speicherkarten-Konvention (DCF, siehe `sysinfo`s
    /// `is_removable()`-Grenzen: nicht jede Plattform meldet SD-Karten-
    /// Adapter zuverlässig als Wechseldatenträger, der `DCIM`-Fund ist
    /// deshalb das stärkere Signal). Das Frontend sortiert/markiert
    /// danach, filtert aber nicht hart heraus — ein Wechseldatenträger
    /// ohne `DCIM` bleibt wählbar (z. B. eine bereits sortierte Karte).
    pub has_dcim: bool,
}

/// Listet Wechseldatenträger auf (Phase 13 Schritt 2) — reine
/// Erkennungs-Bequemlichkeit für `ImportDialog.tsx`, ersetzt keinen
/// bestehenden Import-Weg (der Nutzer kann weiterhin jeden Ordner manuell
/// wählen). Läuft synchron (`sysinfo::Disks::new_with_refreshed_list` ist
/// ein schneller, lokaler Systemaufruf, kein Netzwerk/keine große E/A).
#[tauri::command]
pub fn list_removable_volumes() -> Vec<RemovableVolumeDto> {
    let disks = sysinfo::Disks::new_with_refreshed_list();
    disks
        .list()
        .iter()
        .filter(|disk| disk.is_removable())
        .map(|disk| {
            let mount_point = disk.mount_point().to_path_buf();
            let has_dcim = ["DCIM", "dcim"]
                .iter()
                .any(|name| mount_point.join(name).is_dir());
            RemovableVolumeDto {
                mount_point: mount_point.to_string_lossy().to_string(),
                name: disk.name().to_string_lossy().to_string(),
                has_dcim,
            }
        })
        .collect()
}

#[derive(Debug, Clone, Serialize)]
pub struct CameraFileEntryDto {
    pub folder: String,
    pub name: String,
}

impl From<apx_tether::CameraFileEntry> for CameraFileEntryDto {
    fn from(entry: apx_tether::CameraFileEntry) -> Self {
        Self {
            folder: entry.folder,
            name: entry.name,
        }
    }
}

/// Listet bereits aufgenommene Dateien auf der über [`tether_connect`]
/// verbundenen Kamera (Phase 13 Schritt 2) — im Unterschied zu
/// [`tether_capture`], das eine **neue** Aufnahme auslöst. Ein Fehler,
/// wenn zuvor kein `tether_connect` mit erkannter Kamera lief.
#[tauri::command]
pub fn list_camera_files(state: State<'_, AppState>) -> Result<Vec<CameraFileEntryDto>, String> {
    let mut guard = state
        .tether
        .lock()
        .map_err(|_| "Tethering-Status ist blockiert (vergiftete Sperre)".to_string())?;
    let backend = guard
        .as_deref_mut()
        .ok_or_else(|| "Keine Kamera verbunden — zuerst tether_connect aufrufen".to_string())?;
    Ok(backend
        .list_camera_files()
        .map_err(|err| err.to_string())?
        .into_iter()
        .map(CameraFileEntryDto::from)
        .collect())
}

/// Lädt eine per [`list_camera_files`] gefundene Datei herunter und
/// importiert sie über den bestehenden Import-Pfad (`import::
/// run_with_mode`, Phase 3/5) — derselbe Ablauf wie [`tether_capture`],
/// nur mit einer bereits vorhandenen statt einer neu ausgelösten Aufnahme.
#[tauri::command]
pub async fn import_from_camera(
    app: AppHandle,
    state: State<'_, AppState>,
    folder: String,
    name: String,
    preset_name: Option<String>,
) -> Result<Option<PhotoDto>, String> {
    let dest_dir = state.paths.tether_download_dir();
    let entry = apx_tether::CameraFileEntry { folder, name };
    let downloaded_path = {
        let mut guard = state
            .tether
            .lock()
            .map_err(|_| "Tethering-Status ist blockiert (vergiftete Sperre)".to_string())?;
        let backend = guard
            .as_deref_mut()
            .ok_or_else(|| "Keine Kamera verbunden — zuerst tether_connect aufrufen".to_string())?;
        backend
            .download_camera_file(&entry, &dest_dir)
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
