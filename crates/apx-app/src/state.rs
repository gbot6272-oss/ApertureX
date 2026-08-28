//! Geteilter Anwendungszustand, von Tauri als "managed state" verwaltet.
//! Reine Verdrahtung — keine Geschäftslogik, siehe Modul-Doku in `main.rs`.

use std::sync::Arc;

use apx_catalog::Catalog;
use apx_core::AppPaths;

pub struct AppState {
    pub paths: AppPaths,
    pub catalog: Arc<Catalog>,
}
