//! Zwischenspeicher für gerenderte Vorschau-Kacheln, damit ein
//! Regler-Wechsel nicht bei jedem Tick die komplette Kette (RAW-Decode,
//! Demosaicing) neu durchläuft — siehe `SPEC.md` §5 ("Tile-Cache") und
//! `ARCHITECTURE.md` §5.
//!
//! Wird in Phase-2-Schritt 4 gefüllt (Cache-Schlüssel-Design als Teil der
//! Regler-Implementierung, siehe `PLAN.md` Phase 2 Schritt 4). Noch keine
//! Typen hier — Teil des in Schritt 1 angelegten Crate-Skeletts.
