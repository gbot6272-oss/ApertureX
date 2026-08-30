//! Aperture X — Export-Engine (Phase 8, siehe `DECISIONS.md` ADR-0034 und
//! `PLAN.md` Abschnitt „Aktuelle Phase: Phase 8").
//!
//! Gemeinsamer Unterbau für alle sechs Ausgabe-Module (`SPEC.md` §5):
//! Export-Engine selbst (`engine`, `format`, `resize`, `sharpen`, `icc`,
//! `watermark`, `metadata`, `queue`), darauf aufbauend Drucken (`print`),
//! Diashow (größtenteils Frontend, siehe `ARCHITECTURE.md`), Buch
//! (`book`), Web (`web`) und Karte (`map`). Rendert ausschließlich über
//! `apx_pipeline::develop::render_rgba8` — kein zweiter Rendering-
//! Codepfad (siehe `engine.rs`s Moduldoku).
//!
//! `apx-app` bleibt reine Verdrahtung: Foto-/Katalogdaten auflösen, diese
//! Funktionen aufrufen, Ergebnis als DTO zurückreichen (`ARCHITECTURE.md`
//! §4).

#![deny(clippy::unwrap_used)]

pub mod engine;
pub mod error;
pub mod format;
pub mod icc;
pub mod metadata;
pub mod queue;
pub mod resize;
pub mod sharpen;
pub mod watermark;

// Schritt 3–8 (Drucken/Diashow/Buch/Web/Karte/Templates) ergänzen hier
// jeweils ihr eigenes Modul, sobald sie umgesetzt sind (siehe `PLAN.md`).
