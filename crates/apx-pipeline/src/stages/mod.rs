//! Die sieben Grundeinstellungs-Regler, je ein Modul mit eigenem WGSL-
//! Shader, eigenem CPU-Fallback und eigenen Tests (`SPEC.md` §6: „jede
//! Operation ein eigenes Modul mit eigenem Shader, eigenem Test"), plus
//! ein fusionierter Shader ([`basic_fused`]) für den interaktiven
//! Vorschau-Pfad (siehe `DECISIONS.md` ADR-0017).
//!
//! Lichter+Tiefen ([`highlights_shadows`]) und Weiß+Schwarz
//! ([`whites_blacks`]) sind bewusst je ein Modul mit zwei Parametern
//! statt vier getrennter Module, da sie mathematisch dieselbe
//! tonwertzonen-gewichtete Operation sind (siehe deren Modul-Doku).

pub mod basic_fused;
pub mod contrast;
pub mod curves;
pub mod exposure;
pub mod highlights_shadows;
pub mod local_contrast;
pub mod white_balance;
pub mod whites_blacks;
