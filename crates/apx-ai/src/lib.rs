//! Aperture X — KI-Funktionen (Phase 7, `SPEC.md` §5: „KI-Funktionen.
//! Motiv-/Himmel-/Personen-Segmentierung (ONNX-Runtime, Modelle lokal),
//! Preset-Generator per LLM, Referenzbild-Matching, Auto-Tagging.").
//!
//! **Scope-Präzisierung (`DECISIONS.md` ADR-0033):** echte ONNX-Runtime-
//! Modellinferenz galt zum damaligen Rechercheergebnis als in dieser
//! Umgebung nicht seriös umsetzbar — kein legitimer Weg, echte
//! Segmentierungs-Modellgewichte zu beschaffen und mitzuliefern. Die fünf
//! KI-Masken (`segmentation`-Modul) sind deshalb echte, deterministische,
//! klassische Bildverarbeitungsheuristiken statt echter tiefer neuronaler
//! Netze — jede eine genuine, unit-getestete Fähigkeit, kein Platzhalter.
//! Der Preset-Generator (`preset_generator`-Modul) nutzt dagegen einen
//! echten Anthropic-Messages-API-Client für seinen LLM-Modus.
//!
//! **Korrektur (Phase 13, `DECISIONS.md` ADR-0040):** eine echte
//! ONNX-Laufzeit (`ort`/`tract-onnx`) ist inzwischen real verfügbar, und
//! für mindestens ein Modell (LaMa-Inpainting) auch echte, lizenzgeklärte
//! Gewichte — siehe [`inpaint`]. Das ändert die Segmentierungs-Heuristiken
//! oben nicht rückwirkend, öffnet aber den Weg für echte Modellinferenz
//! dort, wo reale Gewichte tatsächlich beschaffbar sind.
//!
//! Modulübersicht:
//! - [`color`]/[`blur`]: gemeinsame, reine Bildanalyse-Bausteine, von den
//!   übrigen Modulen wiederverwendet (das bilineare Alpha-Resampling
//!   selbst lebt in `apx_core::raster`, siehe dessen Moduldoku).
//! - `segmentation` (Schritt 2): die fünf KI-Masken.
//! - `repair_analysis` (Schritt 3): Auto-Quellenfindung, Sensorflecken-
//!   Visualisierung — einmalige Analyse-Befehle, im Unterschied zum
//!   render-zeitlichen `RepairMode::ContentAwareFill`, das in
//!   `apx-pipeline::stages::repair` bleibt (siehe ADR-0033 Punkt 4).
//! - `preset_generator` (Schritt 4): LLM-Anfrage, Referenzbild-Modus,
//!   Variationen-Generator, Preset aus Bearbeitung lernen.
//! - `tagging` (Schritt 5): regelbasierte Auto-Tagging-Vorschläge.

pub mod blur;
pub mod color;
pub mod denoise;
pub mod error;
pub mod faces;
pub mod inpaint;
pub mod lens_calibration;
pub mod preset_generator;
pub mod repair_analysis;
pub mod segmentation;
pub mod tagging;
pub mod upscale;

pub use error::{AiError, Result};
