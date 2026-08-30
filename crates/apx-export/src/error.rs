//! Fehlertyp für `apx-export`, in derselben Form wie `apx_pipeline::error`/
//! `apx_ai::error` — eigene Varianten für exportspezifische Fehlerfälle,
//! mit einer `From`-Umwandlung in `apx_core::AppError`, damit `apx-app`s
//! Tauri-Commands weiterhin einheitlich Fehler als `String` ans Frontend
//! reichen (siehe `DECISIONS.md` ADR-0012/ADR-0034).

use apx_core::AppError;
use thiserror::Error;

/// Kurzform für `Result<T, ExportError>`.
pub type Result<T> = std::result::Result<T, ExportError>;

#[derive(Debug, Error)]
pub enum ExportError {
    /// Ein angefragtes Ausgabeformat/eine Bit-Tiefen-Kombination wird nicht
    /// unterstützt (z. B. 16-Bit-JPEG).
    #[error("Nicht unterstützt: {0}")]
    Unsupported(String),

    /// Kodierung durch das zugrunde liegende `image`-Crate fehlgeschlagen.
    #[error("Kodierung fehlgeschlagen: {message}")]
    Encode { message: String },

    /// Ein-/Ausgabefehler beim Schreiben der Export-Datei oder beim
    /// Hoch-/Herunterladen über FTP/SFTP.
    #[error("Ein-/Ausgabefehler bei '{path}': {message}")]
    Io { path: String, message: String },

    /// ICC-Profil konnte nicht geladen oder auf das Bild angewendet werden.
    #[error("ICC-Farbmanagement fehlgeschlagen: {message}")]
    Icc { message: String },

    /// FTP/SFTP-Upload fehlgeschlagen (Verbindung, Authentifizierung,
    /// Übertragung).
    #[error("Upload fehlgeschlagen: {message}")]
    Upload { message: String },

    /// PDF-Erzeugung (Buch-/Druck-Modul) fehlgeschlagen.
    #[error("PDF-Erzeugung fehlgeschlagen: {message}")]
    Pdf { message: String },

    /// GPX-Tracklog konnte nicht geparst werden.
    #[error("GPX-Import fehlgeschlagen: {message}")]
    Gpx { message: String },

    /// Ein zugrunde liegender Pipeline-/Katalog-/RAW-Fehler.
    #[error(transparent)]
    App(#[from] AppError),
}

impl From<ExportError> for AppError {
    fn from(err: ExportError) -> Self {
        match err {
            ExportError::App(app_err) => app_err,
            other => AppError::export(other.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_messages_are_human_readable() {
        let err = ExportError::Unsupported("16-Bit-JPEG".to_string());
        assert_eq!(err.to_string(), "Nicht unterstützt: 16-Bit-JPEG");
    }

    #[test]
    fn converts_into_app_error() {
        let err = ExportError::Upload {
            message: "Verbindung abgelehnt".to_string(),
        };
        let app_err: AppError = err.into();
        assert!(app_err.to_string().contains("Verbindung abgelehnt"));
    }

    #[test]
    fn wrapped_app_error_passes_through_unchanged() {
        let inner = AppError::not_found("Foto", "abc");
        let err: ExportError = inner.into();
        let app_err: AppError = err.into();
        assert!(matches!(app_err, AppError::NotFound { .. }));
    }
}
