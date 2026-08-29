//! `apx-raw` — RAW-Dekodierung, Metadaten-Extraktion und
//! Vorschau-Generierung für Aperture X.
//!
//! Deckt in Phase 1 CR2, CR3, NEF, ARW, RAF, ORF, RW2 und DNG über
//! [`rawler`] ab, plus JPEG/PNG/TIFF als Fallback über die `image`-Crate.
//! Die Dekodierungskette ist absichtlich minimal und provisorisch — siehe
//! die Modul-Dokumentation in `pipeline` — und wird in Phase 2 durch die
//! GPU-Pipeline ersetzt.
//!
//! **Lizenzhinweis:** `rawler` ist LGPL-2.1 lizenziert. Das ist eine
//! bewusste, dokumentierte Ausnahme von der sonst geltenden
//! "kein GPL im Kern"-Regel — siehe `DECISIONS.md`, ADR-0002.
//!
//! `apx-raw` hängt nur von `apx-core` ab, nicht von `apx-catalog` (siehe
//! `ARCHITECTURE.md` Abschnitt 4) und greift nicht auf die Datenbank oder
//! Tauri-APIs zu.

#![deny(clippy::unwrap_used)]

mod fallback;
mod format;
mod metadata;
mod orientation;
mod pipeline;
mod preview;

pub use format::is_supported_extension;
pub use metadata::{read_metadata, RawMetadata};
pub use orientation::Orientation;
pub use pipeline::{decode, decode_linear, DecodedImage, LinearImage};
pub use preview::extract_embedded_preview;

// Re-Export, damit Aufrufer Vorschaubilder direkt weiterverarbeiten können,
// ohne selbst eine passende `image`-Version im eigenen Cargo.toml zu
// pinnen (siehe `Cargo.toml` — Version ist im Workspace vereinheitlicht).
pub use image::DynamicImage;
