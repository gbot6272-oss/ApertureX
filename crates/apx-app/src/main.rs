//! `apx-app` — Tauri-Binary für Aperture X.
//!
//! Reine Verdrahtung: verbindet `apx-core`, `apx-raw` und `apx-catalog`
//! über Tauri-Commands, IPC-Events und (ab Schritt 7 des Phase-1-Plans)
//! einen Custom-Protokoll-Handler mit dem Frontend. Enthält selbst keine
//! Geschäftslogik — siehe `ARCHITECTURE.md` Abschnitt 4.

#![deny(clippy::unwrap_used)]

mod commands;
mod import;
mod protocol;
mod reconcile;
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

    // Siehe DECISIONS.md ADR-0012: schlägt sowohl der bevorzugte
    // Hardware- als auch der Software-Fallback-Adapter fehl, gibt es
    // buchstäblich keine wgpu-Ausführungsumgebung — an dieser Stelle (vor
    // dem ersten Fenster) genauso unrecoverable wie ein fehlender
    // Katalog, daher derselbe bewusste `expect()`-Ausnahmefall wie oben.
    let pipeline = apx_pipeline::GpuContext::new_blocking()
        .expect("wgpu-Gerätekontext konnte nicht aufgebaut werden (weder Hardware- noch Software-Adapter verfügbar)");
    tracing::info!(adapter = %pipeline.adapter_info.name, backend = ?pipeline.adapter_info.backend, "wgpu-Gerätekontext bereit");

    let builder = protocol::register(tauri::Builder::default());

    builder
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState {
            paths,
            catalog: Arc::new(catalog),
            active_import: Arc::new(Mutex::new(None)),
            pipeline: Arc::new(pipeline),
            tile_cache: Arc::new(apx_pipeline::tile_cache::TileCache::new()),
        })
        .invoke_handler(tauri::generate_handler![
            commands::select_folder,
            commands::catalog_status,
            commands::list_folders,
            commands::relink_folder,
            commands::import_folder,
            commands::import_folder_with_mode,
            commands::cancel_import,
            commands::list_import_presets,
            commands::save_import_preset,
            commands::delete_import_preset,
            commands::list_photos_in_folder,
            commands::apply_develop_edit,
            commands::current_develop_edit,
            commands::undo_develop_edit,
            commands::redo_develop_edit,
            commands::create_snapshot,
            commands::list_snapshots,
            commands::rename_snapshot,
            commands::delete_snapshot,
            commands::set_photo_rating,
            commands::set_photo_flag,
            commands::set_photo_color_label,
            commands::add_photo_keyword,
            commands::remove_photo_keyword,
            commands::list_photo_keywords,
            commands::list_all_keywords,
            commands::create_collection,
            commands::rename_collection,
            commands::delete_collection,
            commands::list_collections,
            commands::add_to_collection,
            commands::remove_from_collection,
            commands::list_photos_in_collection,
            commands::create_preset_folder,
            commands::rename_preset_folder,
            commands::delete_preset_folder,
            commands::list_preset_folders,
            commands::create_preset,
            commands::update_preset_metadata,
            commands::set_preset_favorite,
            commands::delete_preset,
            commands::list_presets,
            commands::add_preset_version,
            commands::list_preset_versions,
            commands::latest_preset_version,
            commands::export_preset_to_apx_file,
            commands::import_preset_from_apx_file,
            commands::search_photos,
            commands::filter_photos,
            commands::search_and_filter_photos,
            commands::list_duplicate_photo_groups,
            commands::generate_ai_mask,
            commands::suggest_repair_source,
            commands::detect_sensor_spots,
        ])
        .run(tauri::generate_context!())
        .expect("Fehler beim Starten von Aperture X");
}
