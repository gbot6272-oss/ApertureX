//! Der wgpu-Gerätekontext: `Instance`/`Adapter`/`Device`/`Queue`, einmal
//! beim App-Start aufgebaut, sowie der gemeinsame Hoch-/Ausführen-/
//! Runterladen-Helfer (`dispatch`), den alle Regler-Module in
//! `crate::stages` nutzen — siehe `ARCHITECTURE.md` §5.
//!
//! Wird in Phase-2-Schritt 3 gefüllt: `GpuContext` mit explizitem
//! Software-Fallback (`force_fallback_adapter`), bevor `PipelineError::
//! GpuUnavailable` zurückgegeben wird. Noch keine Typen hier — Teil des
//! in Schritt 1 angelegten Crate-Skeletts.
