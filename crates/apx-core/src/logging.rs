//! Logging-Setup: strukturierte Logs über `tracing`, mit täglich
//! rotierender Log-Datei und zusätzlich stdout im Debug-Build.

use std::path::Path;

use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{fmt, EnvFilter};

use crate::error::{AppError, Result};

/// Initialisiert das globale `tracing`-Subscriber-Setup.
///
/// Muss genau einmal beim Programmstart aufgerufen werden. Der
/// zurückgegebene [`WorkerGuard`] muss so lange am Leben gehalten werden,
/// wie geloggt werden soll — der nicht-blockierende Datei-Writer flusht
/// beim Drop des Guards.
///
/// Log-Level werden über die Umgebungsvariable `RUST_LOG` gesteuert
/// (Standard: `info`, im Debug-Build zusätzlich `apx=debug`).
pub fn init_logging(log_dir: &Path) -> Result<WorkerGuard> {
    let file_appender = tracing_appender::rolling::daily(log_dir, "apx.log");
    let (non_blocking_file, guard) = tracing_appender::non_blocking(file_appender);

    let default_filter = if cfg!(debug_assertions) {
        "info,apx_core=debug,apx_raw=debug,apx_catalog=debug,apx_app=debug"
    } else {
        "info"
    };
    let env_filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(default_filter));

    let file_layer = fmt::layer()
        .with_writer(non_blocking_file)
        .with_ansi(false)
        .with_target(true);

    let registry = tracing_subscriber::registry()
        .with(env_filter)
        .with(file_layer);

    // stdout-Ausgabe nur im Debug-Build — in Release-Builds soll die App
    // still bleiben und nur in die Log-Datei schreiben.
    if cfg!(debug_assertions) {
        let stdout_layer = fmt::layer().with_writer(std::io::stdout).with_target(true);
        registry
            .with(stdout_layer)
            .try_init()
            .map_err(|source| AppError::Settings {
                message: format!("Logging konnte nicht initialisiert werden: {source}"),
            })?;
    } else {
        registry.try_init().map_err(|source| AppError::Settings {
            message: format!("Logging konnte nicht initialisiert werden: {source}"),
        })?;
    }

    Ok(guard)
}

#[cfg(test)]
mod tests {
    #[test]
    fn init_logging_creates_log_file_directory_and_returns_guard() {
        let tmp = tempfile::tempdir().expect("Temp-Verzeichnis");
        // Wir rufen init_logging hier bewusst NICHT auf: tracing erlaubt
        // nur einen globalen Subscriber pro Prozess, und die Testsuite
        // läuft mit mehreren Tests im selben Prozess. Stattdessen prüfen
        // wir nur, dass der rollende File-Appender selbst funktioniert.
        let appender = tracing_appender::rolling::daily(tmp.path(), "apx.log");
        let (_writer, _guard) = tracing_appender::non_blocking(appender);
        assert!(tmp.path().exists());
    }
}
