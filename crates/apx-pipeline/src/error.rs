//! Fehlertyp für `apx-pipeline`, in derselben Form wie `apx_core::AppError`
//! (siehe dort) — eigene Varianten für die GPU-/EDL-spezifischen
//! Fehlerfälle, mit einer `From`-Umwandlung in den gemeinsamen Fehlertyp,
//! damit `apx-app`s Tauri-Commands weiterhin einheitlich Fehler als
//! `String` ans Frontend reichen (siehe DECISIONS.md ADR-0012/ADR-0013).

use apx_core::AppError;
use thiserror::Error;

/// Kurzform für `Result<T, PipelineError>`.
pub type Result<T> = std::result::Result<T, PipelineError>;

#[derive(Debug, Error)]
pub enum PipelineError {
    /// Es konnte kein wgpu-Adapter gefunden werden — weder ein echter
    /// GPU-Adapter noch der Software-Fallback (`force_fallback_adapter`).
    #[error("Keine GPU verfügbar: {message}")]
    GpuUnavailable { message: String },

    /// Ein WGSL-Shader konnte nicht kompiliert werden.
    #[error("Shader '{stage}' konnte nicht kompiliert werden: {message}")]
    ShaderCompile {
        stage: &'static str,
        message: String,
    },

    /// Ein GPU-Puffer hat nicht die erwartete Byte-Größe — deutet auf
    /// einen Layout-Fehler zwischen Rust-Struct und WGSL-Uniform-Block hin.
    #[error("Puffergröße stimmt nicht: erwartet {expected} Byte, erhalten {actual} Byte")]
    BufferSizeMismatch { expected: usize, actual: usize },

    /// Ein EDL-Wert ist strukturell ungültig oder hat eine unbekannte
    /// Schema-Version (siehe `edl::EDL_SCHEMA_VERSION`).
    #[error("Ungültiges EDL: {message}")]
    InvalidEdl { message: String },

    /// Eine Rendering-Operation wurde abgebrochen (z. B. weil eine neuere
    /// Anfrage für dasselbe Foto sie überholt hat).
    #[error("Abgebrochen: {0}")]
    Cancelled(String),

    /// Eine `.dcp`-Kameraprofildatei (Phase 13 Schritt 3, siehe
    /// `DECISIONS.md` ADR-0040-Nachtrag) konnte nicht gelesen werden — kein
    /// gültiges TIFF/IFD-Format oder fehlendes Pflichtfeld
    /// (`ColorMatrix1`).
    #[error("DCP-Farbprofil: {message}")]
    DcpProfile { message: String },
}

impl From<PipelineError> for AppError {
    fn from(err: PipelineError) -> Self {
        AppError::pipeline(err.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_messages_are_human_readable() {
        let err = PipelineError::GpuUnavailable {
            message: "kein Adapter gefunden".to_string(),
        };
        assert_eq!(
            err.to_string(),
            "Keine GPU verfügbar: kein Adapter gefunden"
        );
    }

    #[test]
    fn converts_into_app_error() {
        let err = PipelineError::InvalidEdl {
            message: "unbekannte Schema-Version 99".to_string(),
        };
        let app_err: AppError = err.into();
        assert!(app_err.to_string().contains("unbekannte Schema-Version 99"));
    }
}
