//! Tauri-Commands: die einzige Schnittstelle, über die das Frontend mit
//! dem Backend spricht (siehe `ARCHITECTURE.md` Abschnitt 4 — Frontend
//! kennt kein SQL, keine Dateisystempfade, keine Bildpuffer). Fehler
//! werden für Phase 1 als `String` an das Frontend gereicht; eine
//! strukturierte Fehler-DTO kommt bei Bedarf in einer späteren Phase.

use std::path::PathBuf;

use serde::Serialize;
use tauri::{AppHandle, State};
use tauri_plugin_dialog::DialogExt;

use crate::state::AppState;

#[derive(Debug, Clone, Serialize)]
pub struct FolderDto {
    pub id: String,
    pub path: String,
    pub photo_count: u64,
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
}

impl From<apx_catalog::Photo> for PhotoDto {
    fn from(photo: apx_catalog::Photo) -> Self {
        Self {
            id: photo.id.to_string(),
            filename: photo.filename,
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
                path: folder.path.to_string_lossy().to_string(),
                photo_count,
            })
        })
        .collect()
}

/// Startet den Import-Job für `path` im Hintergrund und kehrt sofort
/// zurück — Fortschritt läuft über die Events `import:progress`,
/// `import:error`, `import:finished` (siehe `import`-Modul). Es kann
/// jeweils nur ein Import gleichzeitig laufen.
#[tauri::command]
pub async fn import_folder(
    app: AppHandle,
    state: State<'_, AppState>,
    path: String,
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
            crate::import::run(
                &events,
                &catalog,
                &cache_root,
                &folder,
                &cancel_for_blocking,
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
        let edl = apx_pipeline::EdlV1 {
            basic: apx_pipeline::edl::BasicAdjustments {
                exposure_ev: marker,
                ..apx_pipeline::edl::BasicAdjustments::NEUTRAL
            },
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
                    .expect("sollte gültiges EdlV1 ergeben");
                assert_eq!(parsed.basic.exposure_ev, 0.7);
            }
            HistoryPositionDto::Neutral => panic!("sollte nicht neutral sein"),
        }
    }

    #[test]
    fn parse_photo_id_rejects_invalid_string() {
        assert!(parse_photo_id("nicht-valide".to_string()).is_err());
    }
}
