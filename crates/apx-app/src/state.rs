//! Geteilter Anwendungszustand, von Tauri als "managed state" verwaltet.
//! Reine Verdrahtung — keine Geschäftslogik, siehe Modul-Doku in `main.rs`.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use apx_catalog::Catalog;
use apx_core::AppPaths;
use apx_export::engine::ExportRequest;
use apx_export::queue::ExportQueue;
use apx_pipeline::{tile_cache::TileCache, GpuContext};
use tokio_util::sync::CancellationToken;

/// Ein in die Export-Warteschlange eingereihter Auftrag (Phase 8
/// Schritt 2) — das `ExportRequest` ist bereits vollständig aufgelöst
/// (EDL, Quellpfad, alle Optionen), der Hintergrund-Worker in `main.rs`
/// braucht dafür keinen weiteren Katalogzugriff mehr.
#[derive(Debug, Clone)]
pub struct QueuedExport {
    pub request: ExportRequest,
    pub dest_path: PathBuf,
}

pub struct AppState {
    pub paths: AppPaths,
    pub catalog: Arc<Catalog>,
    /// Abbruch-Token des aktuell laufenden Imports, falls einer läuft.
    /// `None` bedeutet: kein Import aktiv. Ein `Arc<Mutex<_>>`, damit es
    /// unabhängig von der `State`-Lebenszeit in die `spawn_blocking`-Task
    /// hinein geklont werden kann.
    pub active_import: Arc<Mutex<Option<CancellationToken>>>,
    /// Der wgpu-Gerätekontext für die `develop/...`-Route (Phase 2, siehe
    /// `protocol`-Modul), einmal beim App-Start aufgebaut.
    pub pipeline: Arc<GpuContext>,
    /// Zwischenspeicher für das teure `apx_raw::decode_linear`-Ergebnis
    /// pro Foto+Auflösung, siehe `apx_pipeline::tile_cache`.
    pub tile_cache: Arc<TileCache>,
    /// Export-Warteschlange (Phase 8 Schritt 2, `DECISIONS.md` ADR-0034)
    /// — ein einzelner Hintergrund-Worker (siehe `main.rs`) arbeitet sie
    /// ab; `commands.rs`s `enqueue_export_photo`/`export_queue_progress`/
    /// `pause_export_queue`/… greifen nur lesend/schreibend auf ihren
    /// Zustand zu, ohne selbst zu rendern.
    pub export_queue: Arc<Mutex<ExportQueue<QueuedExport>>>,
}
