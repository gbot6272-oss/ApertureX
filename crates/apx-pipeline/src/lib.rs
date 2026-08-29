//! `apx-pipeline` — GPU-gestützte, non-destruktive Bildbearbeitung für
//! Aperture X (Phase 2, „Pipeline-Kern").
//!
//! Nimmt dekodierte Pixeldaten aus `apx-raw` (`apx_raw::decode_linear()`,
//! ab Phase-2-Schritt 4) und einen EDL-Wert (Edit Decision List — wie
//! eine Bearbeitung non-destruktiv beschrieben wird, siehe `edl`)
//! entgegen und liefert gerenderte Pixel zurück. Kein Zugriff auf die
//! Datenbank, kein Zugriff auf Tauri-APIs — siehe `ARCHITECTURE.md` §4/§5.
//!
//! `apx-pipeline` hängt von `apx-core` und `apx-raw` ab, nicht von
//! `apx-catalog` (siehe `DECISIONS.md` ADR-0012).
//!
//! Dieses Modul-Skelett wurde in Phase-2-Schritt 1 angelegt; die
//! einzelnen Untermodule werden in den folgenden Schritten gefüllt (siehe
//! `PLAN.md`, Abschnitt „Aktuelle Phase: Phase 2").

#![deny(clippy::unwrap_used)]

pub mod color;
pub mod edl;
pub mod error;
pub mod gpu;
pub mod stages;
pub mod tile_cache;

pub use edl::{EdlV1, EDL_SCHEMA_VERSION};
pub use error::{PipelineError, Result};
