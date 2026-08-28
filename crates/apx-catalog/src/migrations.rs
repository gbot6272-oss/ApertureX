//! Migrationssystem: nummerierte SQL-Dateien, angewendet über
//! `PRAGMA user_version`. Migrationen werden nie nachträglich geändert,
//! nur ergänzt — siehe Kommentar am Kopf von `migrations/0001_initial.sql`.

use apx_core::{AppError, Result};
use rusqlite::Connection;

use crate::error::map_sqlite_err;

/// Migrationen in Anwendungsreihenfolge. Index 0 = Version 1, Index 1 =
/// Version 2, usw. — `user_version` zählt ab 1, nicht ab 0.
const MIGRATIONS: &[&str] = &[include_str!("../migrations/0001_initial.sql")];

/// Wendet alle noch fehlenden Migrationen auf `conn` an.
pub(crate) fn apply(conn: &Connection) -> Result<()> {
    let current_version: i64 = conn
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .map_err(map_sqlite_err)?;

    if current_version < 0 || current_version as usize > MIGRATIONS.len() {
        return Err(AppError::Database {
            message: format!(
                "Katalog hat Schema-Version {current_version}, diese Aperture-X-Version kennt nur \
                 {} Migration(en) — vermutlich wurde die Datei mit einer neueren Version erstellt.",
                MIGRATIONS.len()
            ),
        });
    }

    for (index, sql) in MIGRATIONS.iter().enumerate() {
        let version = (index + 1) as i64;
        if version <= current_version {
            continue;
        }
        conn.execute_batch(sql).map_err(map_sqlite_err)?;
        // PRAGMA user_version akzeptiert kein gebundenes Parameter — der
        // Wert kommt direkt aus dieser Schleife, nicht von außen, daher
        // ist das direkte Einsetzen hier unbedenklich.
        conn.execute_batch(&format!("PRAGMA user_version = {version}"))
            .map_err(map_sqlite_err)?;
        tracing::info!(version, "Katalog-Migration angewendet");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn applies_all_migrations_from_scratch() {
        let conn = Connection::open_in_memory().expect("In-Memory-DB");
        apply(&conn).expect("Migration darf nicht scheitern");

        let version: i64 = conn
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .expect("lesbar");
        assert_eq!(version as usize, MIGRATIONS.len());

        let table_count: i64 = conn
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE type='table' AND name IN ('folders','photos','previews')",
                [],
                |row| row.get(0),
            )
            .expect("lesbar");
        assert_eq!(table_count, 3);
    }

    #[test]
    fn applying_twice_is_idempotent() {
        let conn = Connection::open_in_memory().expect("In-Memory-DB");
        apply(&conn).expect("erste Anwendung darf nicht scheitern");
        apply(&conn).expect("zweite Anwendung darf nicht scheitern (keine Migration mehr fällig)");
    }

    #[test]
    fn future_schema_version_is_rejected() {
        let conn = Connection::open_in_memory().expect("In-Memory-DB");
        conn.execute_batch("PRAGMA user_version = 9999")
            .expect("setzbar");
        let result = apply(&conn);
        assert!(result.is_err());
    }
}
