//! Geteilter Anwendungszustand, von Tauri als "managed state" verwaltet.
//! Reine Verdrahtung — keine Geschäftslogik, siehe Modul-Doku in `main.rs`.

use std::sync::{Arc, Mutex};

use apx_catalog::Catalog;
use apx_core::AppPaths;
use apx_pipeline::{tile_cache::TileCache, GpuContext};
use tokio_util::sync::CancellationToken;

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
}
