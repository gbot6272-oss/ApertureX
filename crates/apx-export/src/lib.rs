//! Aperture X — Export-Engine (Phase 8, siehe `DECISIONS.md` ADR-0034 und
//! `PLAN.md` Abschnitt „Aktuelle Phase: Phase 8").
//!
//! Gemeinsamer Unterbau für alle sechs Ausgabe-Module (`SPEC.md` §5):
//! Export-Engine selbst (`engine`, `format`, `resize`, `sharpen`, `icc`,
//! `watermark`, `metadata`, `queue`), darauf aufbauend Drucken (`print`),
//! Diashow (Übergänge/Ken-Burns/Intro-Outro laufen live im Frontend,
//! `video` bildet dieselbe Zeitachse nur für den MP4-Export in Rust nach —
//! siehe `video.rs`s Moduldoku), Buch (`book`), Web (`web`) und Karte
//! (`map`). Rendert ausschließlich über
//! `apx_pipeline::develop::render_rgba8` — kein zweiter Rendering-
//! Codepfad (siehe `engine.rs`s Moduldoku).
//!
//! `apx-app` bleibt reine Verdrahtung: Foto-/Katalogdaten auflösen, diese
//! Funktionen aufrufen, Ergebnis als DTO zurückreichen (`ARCHITECTURE.md`
//! §4).

#![deny(clippy::unwrap_used)]

pub mod book;
pub mod dng;
pub mod engine;
pub mod error;
pub mod format;
pub mod icc;
pub mod map;
pub mod metadata;
pub mod print;
pub mod queue;
pub mod resize;
pub mod sharpen;
pub mod video;
pub mod watermark;
pub mod web;
pub mod xmp;
