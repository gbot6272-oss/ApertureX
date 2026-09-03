//! Aperture X — Fokus-/HDR-/Panorama-/Astro-Stacking (Phase 9 Schritt 8,
//! `SPEC.md` §3.6/§5, siehe `PLAN.md`, `DECISIONS.md` ADR-0035 Punkt 2).
//!
//! Reine, deterministische Bildverarbeitung — kein `opencv` (bräuchte
//! eine in dieser Sandbox fehlende Systembibliothek), aber seit Phase 13
//! Schritt 5 echtes merkmalsbasiertes Stitching über reine-Rust-Crates
//! (`akaze`/`homography`, siehe [`homography_stitch`]). Fünf Algorithmen:
//!
//! - [`focus`]: Fokus-Stacking über bereits ausgerichtete Aufnahmen
//!   (Laplacian-Schärfemaß, schärfste Quelle je Pixel).
//! - [`hdr`]: HDR-Zusammenführung über eine Belichtungsreihe (Debevec-
//!   artige gewichtete Fusion im linearen Raum + Reinhard-Tonemap).
//! - [`panorama`]: reine Verschiebungs-Registrierung per 2D-
//!   Phasenkorrelation (`rustfft`) + einfaches Überlapp-Mitteln — für
//!   Stativ-/gleicher-Blickpunkt-Aufnahmen ohne Kamerarotation.
//! - [`homography_stitch`] (Phase 13 Schritt 5): echtes merkmalsbasiertes
//!   Homographie-Stitching (AKAZE + eigener RANSAC-Loop über `homography`)
//!   für Freihandaufnahmen mit Rotation/Perspektive/Parallaxe — siehe
//!   dessen Moduldoku. `apx-app`s `stack_panorama`-Command versucht dies
//!   zuerst und fällt auf [`panorama`] zurück, wenn keine verlässliche
//!   Homografie gefunden wird.
//! - [`astro`]: Sigma-geclipptes Mittel über viele Kurzbelichtungen,
//!   registriert mit derselben Phasenkorrelation wie `panorama`.
//!
//! `apx-stacking` hängt nur von `apx-core` ab (dieselbe Crate-Ebene wie
//! `apx-ai`/`apx-export`) — `apx-app` orchestriert (lädt Quellbilder,
//! ruft hier auf, schreibt/importiert das Ergebnis), siehe
//! `ARCHITECTURE.md` §4 „apx-app bleibt reine Verdrahtung".

pub mod astro;
pub mod error;
pub mod focus;
pub mod hdr;
pub mod homography_stitch;
mod luma;
pub mod panorama;

pub use error::{Result, StackingError};
