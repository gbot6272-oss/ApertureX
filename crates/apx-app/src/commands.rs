//! Tauri-Commands: die einzige Schnittstelle, über die das Frontend mit
//! dem Backend spricht (siehe `ARCHITECTURE.md` Abschnitt 4 — Frontend
//! kennt kein SQL, keine Dateisystempfade, keine Bildpuffer). Fehler
//! werden für Phase 1 als `String` an das Frontend gereicht; eine
//! strukturierte Fehler-DTO kommt bei Bedarf in einer späteren Phase.

use serde::Serialize;
use tauri::State;
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
