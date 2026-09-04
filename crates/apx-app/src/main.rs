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

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use apx_catalog::Catalog;
use apx_core::AppPaths;
use state::AppState;

/// Sucht die ONNX-Runtime-Bibliothek für `apx_ai::inpaint::
/// init_environment` (Phase 13 Schritt 1, siehe `DECISIONS.md` ADR-0040)
/// — `apx-ai` nutzt `ort`s `load-dynamic`-Feature statt `download-binaries`
/// (siehe `apx-ai/Cargo.toml`s Begründung: funktioniert überall, verlangt
/// aber, dass die Bibliothek zur Laufzeit gefunden wird):
/// 1. `ORT_DYLIB_PATH`, falls gesetzt und die Datei existiert — derselbe
///    Override, den `ort` selbst dokumentiert, z. B. für Entwicklung oder
///    diese Sandbox (siehe `apx-ai/src/inpaint.rs`s Testhilfsfunktion).
/// 2. Eine Datei mit dem plattformüblichen Namen direkt neben der
///    ausführbaren Datei — der vorgesehene Ort für ein künftiges
///    Installer-Bundling (siehe `PLAN.md` Phase 10 Schritt 11).
///
/// **Ehrliche Lücke:** das eigentliche Bundling (die Laufzeitbibliothek
/// tatsächlich in den Installer packen) ist noch nicht umgesetzt — ohne
/// diesen Schritt findet ein frisch installiertes Aperture X keine
/// Laufzeit. `None` ist deshalb ein erwarteter, kein fehlerhafter Zustand:
/// `main()` initialisiert die ONNX-Umgebung dann einfach nicht, KI-
/// Ausfüllen bleibt mit einer klaren Fehlermeldung aus statt abzustürzen
/// (derselbe „fehlt halt"-Umgang wie bei einem fehlenden GPU-Adapter an
/// anderer Stelle in diesem Projekt).
fn find_onnx_runtime_dylib() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("ORT_DYLIB_PATH") {
        let path = PathBuf::from(path);
        if path.exists() {
            return Some(path);
        }
    }
    let exe_dir = std::env::current_exe().ok()?.parent()?.to_path_buf();
    let filename = if cfg!(target_os = "windows") {
        "onnxruntime.dll"
    } else if cfg!(target_os = "macos") {
        "libonnxruntime.dylib"
    } else {
        "libonnxruntime.so"
    };
    let candidate = exe_dir.join(filename);
    candidate.exists().then_some(candidate)
}

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

/// Beobachteter Ordner (Phase 12 Schritt 7, siehe `DECISIONS.md`
/// ADR-0039-Nachtrag III): pollt alle `poll_seconds` (aus den
/// Einstellungen, live bei jedem Durchlauf neu gelesen, damit ein
/// Umschalten in den Einstellungen ohne Neustart wirkt) den konfigurierten
/// Ordner und stößt bei Fund denselben `import::run_with_mode`-Pfad an wie
/// ein manueller Import — **kein** natives Datei-System-Watcher-Crate
/// nötig, Polling ist für dieses Projekt an anderer Stelle schon der
/// bewusst gewählte einfache Weg (siehe `export_queue_worker` oben).
/// `run_with_mode` überspringt bereits katalogisierte Dateien von selbst
/// (`SingleFileOutcome::Unchanged`, siehe `import`-Moduldoku) — ein
/// wiederholter Lauf über denselben Ordner ist daher von sich aus billig
/// und idempotent, kein eigener "bereits gesehen"-Zustand nötig. Teilt
/// sich `active_import` mit den Tauri-Commands (`start_import`), damit ein
/// manueller und ein automatischer Import sich nie überschneiden.
async fn watched_folder_worker(
    app: tauri::AppHandle,
    catalog: Arc<Catalog>,
    paths: AppPaths,
    active_import: Arc<Mutex<Option<tokio_util::sync::CancellationToken>>>,
) {
    loop {
        let settings = apx_core::Settings::load_or_default(&paths.settings_file())
            .unwrap_or_else(|err| {
                tracing::warn!(%err, "Einstellungen für den beobachteten Ordner nicht lesbar, überspringe diesen Durchlauf");
                apx_core::Settings::default()
            });
        let watched = settings.watched_folder;
        let poll_seconds = watched.poll_seconds.max(5) as u64;

        let folder = watched
            .enabled
            .then_some(watched.path)
            .flatten()
            .filter(|p| !p.trim().is_empty())
            .map(std::path::PathBuf::from)
            .filter(|p| p.is_dir());

        if let Some(folder) = folder {
            let token = {
                let mut guard = active_import
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
                if guard.is_some() {
                    None // Ein manueller (oder ein vorheriger automatischer) Import läuft bereits.
                } else {
                    let token = tokio_util::sync::CancellationToken::new();
                    *guard = Some(token.clone());
                    Some(token)
                }
            };
            if let Some(token) = token {
                let catalog = catalog.clone();
                let cache_root = paths.preview_cache_dir();
                let app_for_blocking = app.clone();
                let join_result = tokio::task::spawn_blocking(move || {
                    let events = crate::import::TauriEvents(&app_for_blocking);
                    crate::import::run_with_mode(
                        &events,
                        &catalog,
                        &cache_root,
                        &folder,
                        &token,
                        &crate::import::ImportMode::AddInPlace,
                        None,
                    );
                })
                .await;
                if let Ok(mut guard) = active_import.lock() {
                    *guard = None;
                }
                if let Err(join_err) = join_result {
                    tracing::error!(error = %join_err, "Automatischer Import (beobachteter Ordner) ist abgestürzt");
                }
            }
        }

        tokio::time::sleep(Duration::from_secs(poll_seconds)).await;
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

    // Mehrere Kataloge (Phase 13 Schritt 6, siehe `DECISIONS.md`
    // ADR-0040-Nachtrag IV): welcher Katalog beim Start geöffnet wird,
    // ist seit dieser Sitzung tatsächlich `settings.catalog.
    // last_opened_catalog` (das Feld existierte bereits seit Phase 10,
    // wurde aber nie gelesen — reine Attrappe). Fällt auf den
    // Standard-Katalog zurück, wenn kein Pfad hinterlegt ist ODER das
    // Öffnen scheitert (Datei verschoben/gelöscht seit dem letzten
    // Start) — ein Absturz beim Start wegen eines veralteten Pfads wäre
    // schlimmer als eine stille, ehrliche Rückkehr zum Standardkatalog.
    let startup_settings = apx_core::Settings::load_or_default(&paths.settings_file())
        .unwrap_or_else(|err| {
            tracing::warn!(%err, "Einstellungen beim Start nicht lesbar, verwende Standard-Katalog");
            apx_core::Settings::default()
        });
    let requested_catalog_path = startup_settings
        .catalog
        .last_opened_catalog
        .as_ref()
        .filter(|p| !p.trim().is_empty())
        .map(std::path::PathBuf::from);

    let catalog_path = requested_catalog_path
        .clone()
        .unwrap_or_else(|| paths.default_catalog_file());
    tracing::info!(catalog = %catalog_path.display(), "Aperture X startet");

    // Fällt bei einem Fehler auf den Standardpfad zurück (siehe oben) —
    // `catalog_path` muss dann ebenfalls den tatsächlich geöffneten Pfad
    // widerspiegeln, nicht den ursprünglich angeforderten.
    let mut catalog_path = catalog_path;
    let catalog = Catalog::open(&catalog_path).unwrap_or_else(|err| {
        if requested_catalog_path.is_some() {
            tracing::warn!(
                %err,
                requested = %catalog_path.display(),
                fallback = %paths.default_catalog_file().display(),
                "hinterlegter Katalog konnte nicht geöffnet werden, verwende den Standard-Katalog"
            );
            catalog_path = paths.default_catalog_file();
            Catalog::open(&catalog_path)
                .expect("Standard-Katalog konnte nicht geöffnet/angelegt werden")
        } else {
            panic!("Katalog konnte nicht geöffnet/angelegt werden: {err}");
        }
    });

    // Siehe DECISIONS.md ADR-0012: schlägt sowohl der bevorzugte
    // Hardware- als auch der Software-Fallback-Adapter fehl, gibt es
    // buchstäblich keine wgpu-Ausführungsumgebung — an dieser Stelle (vor
    // dem ersten Fenster) genauso unrecoverable wie ein fehlender
    // Katalog, daher derselbe bewusste `expect()`-Ausnahmefall wie oben.
    let pipeline = apx_pipeline::GpuContext::new_blocking()
        .expect("wgpu-Gerätekontext konnte nicht aufgebaut werden (weder Hardware- noch Software-Adapter verfügbar)");
    tracing::info!(adapter = %pipeline.adapter_info.name, backend = ?pipeline.adapter_info.backend, "wgpu-Gerätekontext bereit");

    // KI-Ausfüllen (Phase 13 Schritt 1, siehe `find_onnx_runtime_dylib`s
    // Moduldoku) — bewusst kein `expect()`: eine fehlende ONNX-Laufzeit
    // ist ein erwarteter Zustand (noch kein Installer-Bundling), keiner,
    // der den ganzen Programmstart verhindern sollte.
    match find_onnx_runtime_dylib() {
        Some(dylib) => match apx_ai::inpaint::init_environment(&dylib) {
            Ok(()) => tracing::info!(dylib = %dylib.display(), "ONNX-Laufzeit initialisiert (KI-Ausfüllen verfügbar)"),
            Err(err) => tracing::warn!(%err, "ONNX-Laufzeit konnte nicht initialisiert werden — KI-Ausfüllen bleibt deaktiviert"),
        },
        None => tracing::info!(
            "keine ONNX-Laufzeit gefunden — KI-Ausfüllen bleibt deaktiviert (siehe find_onnx_runtime_dylib-Moduldoku)"
        ),
    }

    let builder = protocol::register(tauri::Builder::default());

    let catalog = Arc::new(catalog);
    let active_import = Arc::new(Mutex::new(None));
    let pipeline = Arc::new(pipeline);
    let export_queue = Arc::new(Mutex::new(apx_export::queue::ExportQueue::new()));
    let worker_pipeline = pipeline.clone();
    let worker_queue = export_queue.clone();
    let watched_folder_catalog = catalog.clone();
    let watched_folder_paths = paths.clone();
    let watched_folder_active_import = active_import.clone();

    builder
        .plugin(tauri_plugin_dialog::init())
        .setup(move |app| {
            tauri::async_runtime::spawn(export_queue_worker(worker_pipeline, worker_queue));
            tauri::async_runtime::spawn(watched_folder_worker(
                app.handle().clone(),
                watched_folder_catalog,
                watched_folder_paths,
                watched_folder_active_import,
            ));
            Ok(())
        })
        .manage(AppState {
            paths,
            catalog,
            catalog_path,
            active_import,
            pipeline,
            tile_cache: Arc::new(apx_pipeline::tile_cache::TileCache::new()),
            export_queue,
            tether: Arc::new(Mutex::new(None)),
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
            commands::resolve_lens_profile,
            commands::calibrate_lens_distortion,
            commands::detect_upright_correction,
            commands::import_dcp_profile,
            commands::import_lut_cube_file,
            commands::list_builtin_lut_filters,
            commands::trim_video,
            commands::detect_video_scene_changes,
            commands::denoise_video_audio,
            commands::add_video_audio_track,
            commands::apply_lut_filter_to_video,
            commands::undo_develop_edit,
            commands::redo_develop_edit,
            commands::list_develop_history,
            commands::goto_develop_edit,
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
            commands::set_photo_custom_metadata,
            commands::list_well_known_iptc_fields,
            commands::export_xmp_sidecar,
            commands::import_xmp_develop_settings,
            commands::import_xmp_sidecar_from_file,
            commands::get_photo,
            commands::catalog_statistics,
            commands::get_active_catalog_info,
            commands::list_recent_catalogs,
            commands::create_new_catalog,
            commands::switch_active_catalog,
            commands::run_catalog_integrity_check,
            commands::run_catalog_optimize,
            commands::run_catalog_backup,
            commands::preview_cache_stats,
            commands::clear_preview_cache,
            commands::generate_smart_previews,
            commands::denoise_photo,
            commands::convert_photo_to_dng,
            commands::upscale_photo,
            commands::stack_focus,
            commands::stack_hdr,
            commands::stack_panorama,
            commands::stack_astro,
            commands::run_develop_script,
            commands::run_plugin_custom_effect,
            commands::export_catalog_share,
            commands::import_catalog_share,
            commands::resolve_share_conflict,
            commands::tether_connect,
            commands::tether_capture,
            commands::list_removable_volumes,
            commands::list_camera_files,
            commands::import_from_camera,
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
            commands::export_preset_to_lrtemplate_file,
            commands::import_preset_from_apx_file,
            commands::search_photos,
            commands::filter_photos,
            commands::search_and_filter_photos,
            commands::preview_batch_rule,
            commands::apply_batch_rule,
            commands::undo_batch_operation,
            commands::list_duplicate_photo_groups,
            commands::list_perceptual_duplicate_groups,
            commands::list_people_groups,
            commands::analyze_style_consistency,
            commands::extract_color_palette,
            commands::generate_ai_mask,
            commands::suggest_repair_source,
            commands::detect_sensor_spots,
            commands::get_ai_settings,
            commands::set_anthropic_api_key,
            commands::download_inpainting_model,
            commands::clear_inpainting_model_path,
            commands::run_ai_inpaint,
            commands::content_aware_move,
            commands::content_aware_scale,
            commands::smooth_skin,
            commands::run_ai_outpaint,
            commands::prepare_composite_layer_source,
            commands::download_depth_model,
            commands::clear_depth_model_path,
            commands::estimate_photo_depth,
            commands::download_style_transfer_model,
            commands::clear_style_transfer_model_path,
            commands::stylize_photo,
            commands::replace_sky,
            commands::download_people_models,
            commands::clear_people_model_paths,
            commands::detect_faces_for_photo,
            commands::list_faces_for_photo,
            commands::list_people,
            commands::list_photos_for_person,
            commands::create_person,
            commands::rename_person,
            commands::delete_person,
            commands::assign_face_to_person,
            commands::unassign_face,
            commands::get_ui_settings,
            commands::set_ui_settings,
            commands::get_watched_folder_settings,
            commands::set_watched_folder_settings,
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
