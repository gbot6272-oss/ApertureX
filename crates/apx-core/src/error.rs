//! Gemeinsamer Fehlertyp für alle Aperture-X-Crates.
//!
//! Jede Funktion, die fehlschlagen kann, gibt `apx_core::Result<T>` zurück.
//! `unwrap()` außerhalb von Tests ist verboten (siehe `SPEC.md` Abschnitt 6
//! und `DECISIONS.md` ADR-0006) — dafür muss jeder Fehlerfall hier als
//! Variante abgebildet sein.

use std::path::PathBuf;

use thiserror::Error;

/// Kurzform für `Result<T, AppError>`, wird in allen Crates verwendet.
pub type Result<T> = std::result::Result<T, AppError>;

#[derive(Debug, Error)]
pub enum AppError {
    /// Dateisystem-Fehler (Lesen, Schreiben, Verzeichnis anlegen …).
    #[error("Ein-/Ausgabefehler bei '{path}': {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    /// Fehler beim Dekodieren eines Bildes (RAW oder Fallback-Format).
    #[error("Dekodierung von '{path}' fehlgeschlagen: {message}")]
    Decode { path: PathBuf, message: String },

    /// Fehler in der Katalog-Datenbank (Query, Migration, Transaktion …).
    #[error("Datenbankfehler: {message}")]
    Database { message: String },

    /// Eine erwartete Entität (Foto, Ordner, Preset …) wurde nicht gefunden.
    #[error("{kind} nicht gefunden: {id}")]
    NotFound { kind: &'static str, id: String },

    /// Eine Funktion oder ein Dateiformat wird (noch) nicht unterstützt.
    #[error("Nicht unterstützt: {0}")]
    Unsupported(String),

    /// Ein Eingabewert verletzt eine fachliche Regel (z. B. Bewertung
    /// außerhalb von 0–5, unbekannte Farbmarkierung) — im Unterschied zu
    /// [`AppError::Database`], das SQL-/Schema-Fehler abbildet.
    #[error("Ungültiger Wert: {message}")]
    Validation { message: String },

    /// Eine Operation wurde vom Nutzer oder durch ein Timeout abgebrochen.
    #[error("Abgebrochen: {0}")]
    Cancelled(String),

    /// Eine ID-Zeichenkette (z. B. aus der Datenbank) ist keine gültige UUID.
    #[error("Ungültige ID '{value}': {source}")]
    InvalidId {
        value: String,
        #[source]
        source: uuid::Error,
    },

    /// Fehler beim Laden/Speichern der Einstellungen (TOML).
    #[error("Einstellungen konnten nicht verarbeitet werden: {message}")]
    Settings { message: String },

    /// Fehler in der GPU-Bearbeitungs-Pipeline (`apx-pipeline`, ab Phase 2)
    /// — GPU nicht verfügbar, Shader-Fehler, ungültiges EDL. `apx-app`
    /// wandelt das wie alle anderen Varianten per `.to_string()` in eine
    /// Fehlermeldung fürs Frontend um (siehe `crates/apx-app/src/commands.rs`).
    #[error("Pipeline-Fehler: {message}")]
    Pipeline { message: String },

    /// Fehler in den KI-Funktionen (`apx-ai`, ab Phase 7, siehe
    /// `DECISIONS.md` ADR-0033) — fehlgeschlagene Bildanalyse, ein nicht
    /// hinterlegter/abgelehnter LLM-API-Schlüssel, eine unparsbare
    /// LLM-Antwort.
    #[error("KI-Funktion fehlgeschlagen: {message}")]
    Ai { message: String },

    /// Fehler in der Export-Engine (`apx-export`, ab Phase 8, siehe
    /// `DECISIONS.md` ADR-0034) — nicht unterstütztes Format, ICC-Profil-
    /// oder Upload-Fehler.
    #[error("Export fehlgeschlagen: {message}")]
    Export { message: String },
}

impl AppError {
    /// Baut eine [`AppError::Io`] aus einem `std::io::Error` und dem
    /// betroffenen Pfad. Kleine Hilfsfunktion, damit Aufrufer nicht überall
    /// die Struct-Syntax wiederholen müssen.
    pub fn io(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        Self::Io {
            path: path.into(),
            source,
        }
    }

    pub fn decode(path: impl Into<PathBuf>, message: impl Into<String>) -> Self {
        Self::Decode {
            path: path.into(),
            message: message.into(),
        }
    }

    pub fn not_found(kind: &'static str, id: impl Into<String>) -> Self {
        Self::NotFound {
            kind,
            id: id.into(),
        }
    }

    pub fn pipeline(message: impl Into<String>) -> Self {
        Self::Pipeline {
            message: message.into(),
        }
    }

    pub fn validation(message: impl Into<String>) -> Self {
        Self::Validation {
            message: message.into(),
        }
    }

    pub fn ai(message: impl Into<String>) -> Self {
        Self::Ai {
            message: message.into(),
        }
    }

    pub fn export(message: impl Into<String>) -> Self {
        Self::Export {
            message: message.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_messages_are_human_readable() {
        let err = AppError::not_found("Foto", "abc-123");
        assert_eq!(err.to_string(), "Foto nicht gefunden: abc-123");
    }

    #[test]
    fn io_error_includes_path() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "fehlt");
        let err = AppError::io("/tmp/beispiel.dat", io_err);
        assert!(err.to_string().contains("/tmp/beispiel.dat"));
    }

    #[test]
    fn source_chain_is_preserved() {
        use std::error::Error as _;
        let io_err = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "verweigert");
        let err = AppError::io("/tmp/x", io_err);
        assert!(err.source().is_some());
    }

    #[test]
    fn pipeline_error_includes_message() {
        let err = AppError::pipeline("GPU-Adapter nicht gefunden");
        assert_eq!(
            err.to_string(),
            "Pipeline-Fehler: GPU-Adapter nicht gefunden"
        );
    }

    #[test]
    fn validation_error_includes_message() {
        let err = AppError::validation("Bewertung muss zwischen 0 und 5 liegen");
        assert_eq!(
            err.to_string(),
            "Ungültiger Wert: Bewertung muss zwischen 0 und 5 liegen"
        );
    }

    #[test]
    fn ai_error_includes_message() {
        let err = AppError::ai("kein API-Schlüssel hinterlegt");
        assert_eq!(
            err.to_string(),
            "KI-Funktion fehlgeschlagen: kein API-Schlüssel hinterlegt"
        );
    }
}
