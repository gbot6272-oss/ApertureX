//! Die sieben Grundeinstellungs-Regler, je ein Modul mit eigenem
//! WGSL-Shader, eigenem CPU-Fallback und eigenen Tests (`SPEC.md` §6:
//! „jede Operation ein eigenes Modul mit eigenem Shader, eigenem Test"),
//! plus ein fusionierter Shader für den interaktiven Vorschau-Pfad
//! (siehe `DECISIONS.md` ADR-0017).
//!
//! Wird in Phase-2-Schritt 4 gefüllt: `white_balance`, `exposure`,
//! `contrast`, `highlights_shadows`, `whites_blacks`, `basic_fused`. Noch
//! keine Module hier — Teil des in Schritt 1 angelegten Crate-Skeletts.
