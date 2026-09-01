//! Fehler-Mapping von `rusqlite::Error` auf `apx_core::AppError`.

use apx_core::AppError;

pub(crate) fn map_sqlite_err(err: rusqlite::Error) -> AppError {
    AppError::Database {
        message: err.to_string(),
    }
}
