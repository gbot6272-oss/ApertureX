//! Fehlertyp für `apx-ai`, in derselben Form wie `apx_core::AppError` und
//! `apx_pipeline::PipelineError` (siehe dort) — eigene Varianten für die
//! KI-/Analyse-spezifischen Fehlerfälle, mit einer `From`-Umwandlung in
//! den gemeinsamen Fehlertyp.

use apx_core::AppError;
use thiserror::Error;

/// Kurzform für `Result<T, AiError>`.
pub type Result<T> = std::result::Result<T, AiError>;

#[derive(Debug, Error)]
pub enum AiError {
    /// Ein Bild ist zu klein oder anderweitig ungeeignet für den
    /// angeforderten Analyse-Algorithmus (z. B. 0×0 nach Zuschnitt).
    #[error("Bildanalyse fehlgeschlagen: {message}")]
    Analysis { message: String },

    /// Der LLM-Preset-Generator wurde ohne hinterlegten
    /// Anthropic-API-Schlüssel aufgerufen (siehe `apx-core::Settings::ai`).
    #[error("Kein Anthropic-API-Schlüssel in den Einstellungen hinterlegt")]
    MissingApiKey,

    /// Der HTTP-Aufruf gegen die Anthropic-API ist fehlgeschlagen
    /// (Netzwerkfehler, Zeitüberschreitung, HTTP-Fehlerstatus).
    #[error("Anthropic-API-Aufruf fehlgeschlagen: {message}")]
    LlmRequest { message: String },

    /// Die Antwort des LLM ließ sich nicht als das erwartete
    /// `PresetEdlSubset`-JSON parsen — wird als Fehler statt eines
    /// stillschweigend übernommenen Unsinns-Presets behandelt.
    #[error("LLM-Antwort ließ sich nicht als gültiges Preset parsen: {message}")]
    LlmResponseUnparsable { message: String },

    /// Ein ONNX-Modell (Phase 13, siehe `inpaint`-Moduldoku) konnte nicht
    /// geladen werden oder die Inferenz ist fehlgeschlagen — Laufzeit-
    /// Bibliothek fehlt, Modelldatei ist keine gültige ONNX-Datei, oder
    /// die Ein-/Ausgabeform passt nicht zum geladenen Graphen.
    #[error("ONNX-Modell: {message}")]
    Model { message: String },
}

impl From<AiError> for AppError {
    fn from(err: AiError) -> Self {
        AppError::ai(err.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_messages_are_human_readable() {
        let err = AiError::Analysis {
            message: "Bild ist 0×0".to_string(),
        };
        assert_eq!(err.to_string(), "Bildanalyse fehlgeschlagen: Bild ist 0×0");
    }

    #[test]
    fn converts_into_app_error() {
        let err = AiError::MissingApiKey;
        let app_err: AppError = err.into();
        assert!(app_err.to_string().contains("Anthropic-API-Schlüssel"));
    }
}
