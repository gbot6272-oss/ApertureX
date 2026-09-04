//! Migrationssystem: nummerierte SQL-Dateien, angewendet über
//! `PRAGMA user_version`. Migrationen werden nie nachträglich geändert,
//! nur ergänzt — siehe Kommentar am Kopf von `migrations/0001_initial.sql`.

use apx_core::{AppError, Result};
use rusqlite::Connection;

use crate::error::map_sqlite_err;

/// Migrationen in Anwendungsreihenfolge. Index 0 = Version 1, Index 1 =
/// Version 2, usw. — `user_version` zählt ab 1, nicht ab 0.
const MIGRATIONS: &[&str] = &[
    include_str!("../migrations/0001_initial.sql"),
    include_str!("../migrations/0002_edits.sql"),
    include_str!("../migrations/0003_library.sql"),
    include_str!("../migrations/0004_presets.sql"),
    include_str!("../migrations/0005_snapshots.sql"),
    include_str!("../migrations/0006_templates.sql"),
    include_str!("../migrations/0007_library_backlog.sql"),
    include_str!("../migrations/0008_metadata_keywords.sql"),
    include_str!("../migrations/0009_batch_operations.sql"),
    include_str!("../migrations/0010_custom_metadata.sql"),
    include_str!("../migrations/0011_people.sql"),
    include_str!("../migrations/0012_video.sql"),
];

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
                "SELECT count(*) FROM sqlite_master WHERE type='table' AND name IN \
                 ('folders','photos','previews','edit_history','edit_current', \
                 'keywords','photo_keywords','collections','collection_photos', \
                 'preset_folders','presets','preset_versions','templates', \
                 'stacks','stack_photos','collection_folders','color_label_definitions', \
                 'tag_rules')",
                [],
                |row| row.get(0),
            )
            .expect("lesbar");
        assert_eq!(table_count, 18);

        let fts_count: i64 = conn
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE name = 'photos_fts'",
                [],
                |row| row.get(0),
            )
            .expect("lesbar");
        assert_eq!(fts_count, 1, "photos_fts-Virtualtabelle sollte existieren");
    }

    #[test]
    fn upgrades_a_catalog_that_only_has_migration_one() {
        // Simuliert einen mit einer älteren Aperture-X-Version angelegten
        // Katalog (nur Migration 1 angewendet) — muss beim erneuten
        // `apply()` sauber auf Migration 2/3 nachziehen, ohne die
        // Phase-1-Tabellen anzutasten (SPEC.md §2.1: "Schema-Migration
        // muss alte Kataloge öffnen können").
        let conn = Connection::open_in_memory().expect("In-Memory-DB");
        conn.execute_batch(MIGRATIONS[0])
            .expect("Migration 1 anwendbar");
        conn.execute_batch("PRAGMA user_version = 1")
            .expect("setzbar");

        apply(&conn).expect("sollte auf Migration 2/3 nachziehen");

        let version: i64 = conn
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .expect("lesbar");
        assert_eq!(version as usize, MIGRATIONS.len());

        let table_count: i64 = conn
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE type='table' AND name IN \
                 ('edit_history','edit_current','keywords','collections')",
                [],
                |row| row.get(0),
            )
            .expect("lesbar");
        assert_eq!(
            table_count, 4,
            "Migrationen 2 und 3 sollten nachträglich angewendet worden sein"
        );
    }

    #[test]
    fn upgrades_a_catalog_that_only_has_migrations_one_and_two() {
        // Wie oben, aber mit einem Foto, das schon vor Migration 3
        // existierte — Backfill muss es in photos_fts aufnehmen.
        let conn = Connection::open_in_memory().expect("In-Memory-DB");
        conn.execute_batch(MIGRATIONS[0])
            .expect("Migration 1 anwendbar");
        conn.execute_batch(MIGRATIONS[1])
            .expect("Migration 2 anwendbar");
        conn.execute_batch("PRAGMA user_version = 2")
            .expect("setzbar");
        conn.execute_batch(
            "INSERT INTO folders (id, path, parent_id, added_at) VALUES ('f1', '/fotos', NULL, 0);
             INSERT INTO photos (id, folder_id, filename, file_size, file_mtime, imported_at)
             VALUES ('p1', 'f1', 'vor_migration_3.cr2', 100, 0, 0);",
        )
        .expect("Testdaten einfügbar");

        apply(&conn).expect("sollte auf Migration 3 nachziehen");

        let fts_count: i64 = conn
            .query_row(
                "SELECT count(*) FROM photos_fts WHERE photos_fts MATCH 'vor_migration_3'",
                [],
                |row| row.get(0),
            )
            .expect("lesbar");
        assert_eq!(
            fts_count, 1,
            "Backfill sollte das vor Migration 3 angelegte Foto in photos_fts aufnehmen"
        );
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
