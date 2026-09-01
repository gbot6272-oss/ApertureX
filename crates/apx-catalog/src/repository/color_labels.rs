//! SQL für `color_label_definitions` (siehe
//! `migrations/0007_library_backlog.sql`s Moduldoku, Phase 9 Schritt 1)
//! — ersetzt die frühere fest verdrahtete `ALLOWED_COLOR_LABELS`-Palette
//! durch eine benutzerdefinierbare Tabelle, mit den fünf früheren Werten
//! als Migrations-Seed.

use apx_core::{AppError, Result};
use rusqlite::{params, Connection};

use crate::error::map_sqlite_err;
use crate::models::ColorLabelDefinition;

pub(crate) fn list_all(conn: &Connection) -> Result<Vec<ColorLabelDefinition>> {
    let mut stmt = conn
        .prepare("SELECT name, display_name, hex, position FROM color_label_definitions ORDER BY position")
        .map_err(map_sqlite_err)?;
    let rows = stmt
        .query_map([], |row| {
            Ok(ColorLabelDefinition {
                name: row.get(0)?,
                display_name: row.get(1)?,
                hex: row.get(2)?,
                position: row.get(3)?,
            })
        })
        .map_err(map_sqlite_err)?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(map_sqlite_err)
}

/// Legt eine neue Farbmarkierung an — `name` ist der interne Schlüssel
/// (wird in `photos.color_label` gespeichert), muss eindeutig sein.
pub(crate) fn create(conn: &Connection, name: &str, display_name: &str, hex: &str) -> Result<()> {
    let next_position: i64 = conn
        .query_row(
            "SELECT COALESCE(MAX(position), -1) + 1 FROM color_label_definitions",
            [],
            |row| row.get(0),
        )
        .map_err(map_sqlite_err)?;
    conn.execute(
        "INSERT INTO color_label_definitions (name, display_name, hex, position) VALUES (?1, ?2, ?3, ?4)",
        params![name, display_name, hex, next_position],
    )
    .map_err(|err| match err {
        rusqlite::Error::SqliteFailure(e, _) if e.code == rusqlite::ErrorCode::ConstraintViolation => {
            AppError::validation(format!("Farbmarkierung '{name}' existiert bereits"))
        }
        other => map_sqlite_err(other),
    })?;
    Ok(())
}

/// Löscht eine Farbmarkierungs-Definition — Fotos, die sie tragen,
/// behalten den Zeichenketten-Wert in `color_label` (kein `ON DELETE`-
/// Fremdschlüssel, da `photos.color_label` bewusst kein `REFERENCES`
/// hat, um bestehende Werte nicht rückwirkend ungültig zu machen).
pub(crate) fn delete(conn: &Connection, name: &str) -> Result<()> {
    let changed = conn
        .execute(
            "DELETE FROM color_label_definitions WHERE name = ?1",
            params![name],
        )
        .map_err(map_sqlite_err)?;
    if changed == 0 {
        return Err(AppError::not_found("Farbmarkierung", name.to_string()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::migrations;

    fn setup() -> Connection {
        let conn = Connection::open_in_memory().expect("In-Memory-DB");
        migrations::apply(&conn).expect("Migration");
        conn
    }

    #[test]
    fn migration_seeds_the_five_original_labels() {
        let conn = setup();
        let labels = list_all(&conn).expect("liste");
        assert_eq!(labels.len(), 5);
        assert_eq!(labels[0].name, "red");
    }

    #[test]
    fn create_then_delete_roundtrip() {
        let conn = setup();
        create(&conn, "orange", "Orange", "#dd6b20").expect("anlegen");
        assert_eq!(list_all(&conn).expect("liste").len(), 6);

        delete(&conn, "orange").expect("löschen");
        assert_eq!(list_all(&conn).expect("liste").len(), 5);
    }

    #[test]
    fn create_rejects_duplicate_name() {
        let conn = setup();
        assert!(create(&conn, "red", "Erneut Rot", "#000000").is_err());
    }

    #[test]
    fn delete_unknown_label_fails() {
        let conn = setup();
        assert!(delete(&conn, "does-not-exist").is_err());
    }
}
