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
use std::time::Duration;

use apx_catalog::Catalog;
use apx_core::AppPaths;
use state::AppState;

/// Der einzige Hintergrund-Worker für die Export-Warteschlange (Phase 8
/// Schritt 2, siehe `state.rs`s Moduldoku) — fragt `queue` in einer
/// kurzen Schlaufe ab (**vereinfacht**: Abfragen statt einer Weck-
/// Benachrichtigung, siehe `commands.rs`s Moduldoku zur Warteschlange)
/// und rendert/kodiert/schreibt jeden anstehenden Auftrag in
/// `spawn_blocking` (dieselbe Begründung wie beim Import-Job: die
/// eigentliche Arbeit ist synchroner, CPU-gebundener Code).
async fn export_queue_worker(
    pipeline: Arc<apx_pipeline::GpuContext>,
    queue: Arc<Mutex<apx_export::queue::ExportQueue<state::QueuedExport>>>,
) {
    loop {
        let next = {
            let mut guard = queue
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            guard.take_next().map(|(id, job)| (id, job.clone()))
        };
        match next {
            Some((id, job)) => {
                let pipeline = pipeline.clone();
                let result = tokio::task::spawn_blocking(move || {
                    apx_export::engine::export_to_file(
                        Some(&pipeline),
                        &job.request,
                        &job.dest_path,
                    )
                })
                .await;
                let mut guard = queue
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
                match result {
                    Ok(Ok(_outcome)) => guard.mark_done(id),
                    Ok(Err(err)) => guard.mark_failed(id, err.to_string()),
                    Err(join_err) => guard.mark_failed(id, join_err.to_string()),
                }
            }
            None => tokio::time::sleep(Duration::from_millis(150)).await,
        }
    }
}

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

    let pipeline = Arc::new(pipeline);
    let export_queue = Arc::new(Mutex::new(apx_export::queue::ExportQueue::new()));
    let worker_pipeline = pipeline.clone();
    let worker_queue = export_queue.clone();

    builder
        .plugin(tauri_plugin_dialog::init())
        .setup(move |_app| {
            tauri::async_runtime::spawn(export_queue_worker(worker_pipeline, worker_queue));
            Ok(())
        })
        .manage(AppState {
            paths,
            catalog: Arc::new(catalog),
            active_import: Arc::new(Mutex::new(None)),
            pipeline,
            tile_cache: Arc::new(apx_pipeline::tile_cache::TileCache::new()),
            export_queue,
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
            commands::set_keyword_parent,
            commands::set_keyword_synonyms,
            commands::delete_keyword,
            commands::create_tag_rule,
            commands::set_tag_rule_enabled,
            commands::delete_tag_rule,
            commands::list_tag_rules,
            commands::set_photo_metadata,
            commands::export_xmp_sidecar,
            commands::import_xmp_develop_settings,
            commands::import_xmp_sidecar_from_file,
            commands::get_photo,
            commands::catalog_statistics,
            commands::preview_cache_stats,
            commands::clear_preview_cache,
            commands::denoise_photo,
            commands::upscale_photo,
            commands::create_collection,
            commands::create_smart_collection,
            commands::move_collection_to_folder,
            commands::rename_collection,
            commands::delete_collection,
            commands::list_collections,
            commands::add_to_collection,
            commands::remove_from_collection,
            commands::list_photos_in_collection,
            commands::create_collection_folder,
            commands::rename_collection_folder,
            commands::delete_collection_folder,
            commands::list_collection_folders,
            commands::create_virtual_copy,
            commands::list_virtual_copies,
            commands::create_stack,
            commands::delete_stack,
            commands::set_stack_cover,
            commands::list_stacks,
            commands::auto_stack_by_time,
            commands::list_color_label_definitions,
            commands::create_color_label_definition,
            commands::delete_color_label_definition,
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
            commands::list_perceptual_duplicate_groups,
            commands::generate_ai_mask,
            commands::suggest_repair_source,
            commands::detect_sensor_spots,
            commands::get_ai_settings,
            commands::set_anthropic_api_key,
            commands::generate_preset_from_llm,
            commands::build_preset_prompt_text,
            commands::import_preset_json,
            commands::generate_preset_from_reference,
            commands::generate_preset_variations,
            commands::learn_preset_from_photos,
            commands::suggest_tags,
            commands::export_photo,
            commands::enqueue_export_photo,
            commands::export_queue_progress,
            commands::pause_export_queue,
            commands::resume_export_queue,
            commands::cancel_export_job,
            commands::clear_finished_export_jobs,
            commands::pick_file_path,
            commands::pick_save_file_path,
            commands::print_photos,
            commands::check_ffmpeg_available,
            commands::export_slideshow_video,
            commands::export_book_pdf,
            commands::export_web_gallery,
            commands::list_geotagged_photos,
            commands::reverse_geocode_location,
            commands::import_gpx_track,
            commands::set_photo_gps,
            commands::save_template,
            commands::list_templates,
            commands::delete_template,
            commands::export_template_to_file,
            commands::import_template_from_file,
        ])
        .run(tauri::generate_context!())
        .expect("Fehler beim Starten von Aperture X");
}
