//! `apx-core` — Basistypen für Aperture X: IDs, Fehler, Pfade,
//! Einstellungen, Logging.
//!
//! Dieser Crate hängt bewusst von keinem anderen Workspace-Crate ab
//! (siehe `ARCHITECTURE.md`, Abschnitt 4) und bildet das Fundament, auf
//! dem `apx-raw`, `apx-catalog` und `apx-app` aufbauen.

#![deny(clippy::unwrap_used)]

mod edl;
mod error;
mod ids;
mod logging;
mod paths;
pub mod raster;
mod settings;

pub use edl::EdlEnvelope;
pub use error::{AppError, Result};
pub use ids::{
    CatalogId, CollectionFolderId, CollectionId, EditHistoryId, FolderId, KeywordId, PhotoId,
    PresetFolderId, PresetId, PresetVersionId, SnapshotId, StackId, TemplateId,
};
pub use logging::init_logging;
pub use paths::AppPaths;
pub use settings::{AiSettings, CatalogSettings, Settings, Theme, UiSettings};
