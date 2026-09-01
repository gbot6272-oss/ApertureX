//! Fehlertyp für `apx-stacking`, in derselben Form wie `apx_ai::AiError`
//! und `apx_export::ExportError` — eigene Varianten für die
//! Stacking-spezifischen Fehlerfälle, mit einer `From`-Umwandlung in den
//! gemeinsamen Fehlertyp.

use apx_core::AppError;
use thiserror::Error;

/// Kurzform für `Result<T, StackingError>`.
pub type Result<T> = std::result::Result<T, StackingError>;

#[derive(Debug, Error)]
pub enum StackingError {
    /// Zu wenige Quellbilder für den angeforderten Algorithmus (z. B. nur
    /// eines statt mindestens zwei).
    #[error("Zu wenige Quellbilder: {message}")]
    TooFewImages { message: String },

    /// Die Quellbilder haben unterschiedliche Abmessungen, obwohl der
    /// Algorithmus (Fokus-Stacking, Sigma-Clipping) bereits ausgerichtete
    /// Bilder gleicher Größe voraussetzt.
    #[error("Quellbilder haben unterschiedliche Abmessungen: {message}")]
    DimensionMismatch { message: String },
}

impl From<StackingError> for AppError {
    fn from(err: StackingError) -> Self {
        AppError::stacking(err.to_string())
    }
}
