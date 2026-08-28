//! `apx-app` — Tauri-Binary für Aperture X.
//!
//! Reine Verdrahtung: verbindet `apx-core`, `apx-raw` und `apx-catalog`
//! über Tauri-Commands, IPC-Events und (ab Schritt 7 des Phase-1-Plans)
//! einen Custom-Protokoll-Handler mit dem Frontend. Enthält selbst keine
//! Geschäftslogik — siehe `ARCHITECTURE.md` Abschnitt 4.

mod commands;
mod import;
mod state;

use std::sync::{Arc, Mutex};

use apx_catalog::Catalog;
use apx_core::AppPaths;
use state::AppState;

fn main() {
    // Fehler beim Ermitteln der Systempfade, beim Initialisieren des
    // Loggings oder beim Öffnen des Katalogs sind an dieser Stelle
    // unrecoverable — es existiert noch kein Fenster, in dem man dem
    // Nutzer einen Fehler anzeigen könnte. `expect()` (nicht `unwrap()`,
    // siehe DECISIONS.md ADR-0006) mit einer klaren Meldung ist hier der
    // richtige, bewusste Ausnahmefall.
    let paths = AppPaths::discover()
        .expect("Systempfade (Katalog/Cache/Logs/Settings) konnten nicht ermittelt werden");

    let _log_guard =
        apx_core::init_logging(paths.log_dir()).expect("Logging konnte nicht initialisiert werden");

    tracing::info!(catalog = %paths.default_catalog_file().display(), "Aperture X startet");

    let catalog = Catalog::open(&paths.default_catalog_file())
        .expect("Katalog konnte nicht geöffnet/angelegt werden");

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState {
            paths,
            catalog: Arc::new(catalog),
            active_import: Arc::new(Mutex::new(None)),
        })
        .invoke_handler(tauri::generate_handler![
            commands::select_folder,
            commands::catalog_status,
            commands::list_folders,
            commands::import_folder,
            commands::cancel_import,
        ])
        .run(tauri::generate_context!())
        .expect("Fehler beim Starten von Aperture X");
}
