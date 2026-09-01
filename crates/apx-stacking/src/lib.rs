//! Aperture X — Fokus-/HDR-/Panorama-/Astro-Stacking (Phase 9 Schritt 8,
//! `SPEC.md` §3.6/§5, siehe `PLAN.md`, `DECISIONS.md` ADR-0035 Punkt 2).
//!
//! Reine, deterministische Bildverarbeitung — keine externe
//! Registrierungs-/Stitching-Bibliothek (`opencv` bräuchte eine in dieser
//! Sandbox fehlende Systembibliothek). Vier unabhängige Algorithmen:
//!
//! - [`focus`]: Fokus-Stacking über bereits ausgerichtete Aufnahmen
//!   (Laplacian-Schärfemaß, schärfste Quelle je Pixel).
//! - [`hdr`]: HDR-Zusammenführung über eine Belichtungsreihe (Debevec-
//!   artige gewichtete Fusion im linearen Raum + Reinhard-Tonemap).
//! - [`panorama`]: **v1 nur Verschiebungs-Registrierung** per 2D-
//!   Phasenkorrelation (`rustfft`) + einfaches Überlapp-Mitteln — kein
//!   Homographie-Stitching (siehe Moduldoku dort).
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
mod luma;
pub mod panorama;

pub use error::{Result, StackingError};
