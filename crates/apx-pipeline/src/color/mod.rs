//! Farbmanagement: ProPhoto-RGB-Arbeitsraum-Matrizen und die
//! `lcms2`-Anbindung für die abschließende Anzeige-Transformation
//! (ProPhoto → sRGB) — siehe `SPEC.md` §2.2, `ARCHITECTURE.md` §5.
//!
//! Wird in Phase-2-Schritt 4 gefüllt, zusammen mit den Regler-Modulen in
//! `crate::stages`, die auf diesem Arbeitsraum aufbauen. Noch keine Typen
//! hier — Teil des in Schritt 1 angelegten Crate-Skeletts.
