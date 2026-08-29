//! SQL für die `folders`-Tabelle. Wird ausschließlich über [`crate::Catalog`]
//! aufgerufen — kein SQL außerhalb von `apx-catalog`, siehe `ARCHITECTURE.md`.

use std::path::{Path, PathBuf};

use apx_core::{FolderId, Result};
use rusqlite::{params, Connection, OptionalExtension};
use time::OffsetDateTime;

use crate::error::map_sqlite_err;
use crate::models::{from_unix, to_unix, Folder};

struct FolderRow {
    id: String,
    path: String,
    parent_id: Option<String>,
    added_at: i64,
}

fn row_to_raw(row: &rusqlite::Row) -> rusqlite::Result<FolderRow> {
    Ok(FolderRow {
        id: row.get(0)?,
        path: row.get(1)?,
        parent_id: row.get(2)?,
        added_at: row.get(3)?,
    })
}

fn raw_to_folder(raw: FolderRow) -> Result<Folder> {
    Ok(Folder {
        id: raw.id.parse()?,
        path: PathBuf::from(raw.path),
        parent_id: raw.parent_id.map(|s| s.parse()).transpose()?,
        added_at: from_unix(raw.added_at)?,
    })
}

pub(crate) fn insert(
    conn: &Connection,
    path: &Path,
    parent_id: Option<FolderId>,
    added_at: OffsetDateTime,
) -> Result<FolderId> {
    let id = FolderId::new();
    conn.execute(
        "INSERT INTO folders (id, path, parent_id, added_at) VALUES (?1, ?2, ?3, ?4)",
        params![
            id.to_string(),
            path.to_string_lossy(),
            parent_id.map(|p| p.to_string()),
            to_unix(added_at)
        ],
    )
    .map_err(map_sqlite_err)?;
    Ok(id)
}

pub(crate) fn find_by_path(conn: &Connection, path: &Path) -> Result<Option<Folder>> {
    let raw: Option<FolderRow> = conn
        .query_row(
            "SELECT id, path, parent_id, added_at FROM folders WHERE path = ?1",
            params![path.to_string_lossy()],
            row_to_raw,
        )
        .optional()
        .map_err(map_sqlite_err)?;
    raw.map(raw_to_folder).transpose()
}

pub(crate) fn get(conn: &Connection, id: FolderId) -> Result<Folder> {
    let raw = conn
        .query_row(
            "SELECT id, path, parent_id, added_at FROM folders WHERE id = ?1",
            params![id.to_string()],
            row_to_raw,
        )
        .map_err(|err| match err {
            rusqlite::Error::QueryReturnedNoRows => {
                apx_core::AppError::not_found("Ordner", id.to_string())
            }
            other => map_sqlite_err(other),
        })?;
    raw_to_folder(raw)
}

pub(crate) fn list_all(conn: &Connection) -> Result<Vec<Folder>> {
    let mut stmt = conn
        .prepare("SELECT id, path, parent_id, added_at FROM folders ORDER BY path")
        .map_err(map_sqlite_err)?;
    let rows = stmt.query_map([], row_to_raw).map_err(map_sqlite_err)?;
    let mut result = Vec::new();
    for row in rows {
        result.push(raw_to_folder(row.map_err(map_sqlite_err)?)?);
    }
    Ok(result)
}

/// Aktualisiert den gespeicherten Pfad eines Ordners — z. B. nachdem der
/// Nutzer ihn im Dateisystem verschoben/umbenannt hat und ihn über den
/// Ordnerbaum neu verknüpft (siehe `PLAN.md` Phase 3, Schritt 5). Ändert
/// nur `folders.path`; welche Fotos darunter (nicht mehr) existieren,
/// gleicht der bestehende `reconcile`-Mechanismus (`apx-app`) danach ab.
pub(crate) fn update_path(conn: &Connection, id: FolderId, new_path: &Path) -> Result<()> {
    let changed = conn
        .execute(
            "UPDATE folders SET path = ?2 WHERE id = ?1",
            params![id.to_string(), new_path.to_string_lossy()],
        )
        .map_err(map_sqlite_err)?;
    if changed == 0 {
        return Err(apx_core::AppError::not_found("Ordner", id.to_string()));
    }
    Ok(())
}

/// Findet einen vorhandenen Ordner anhand des Pfads oder legt ihn neu an.
/// Wird vom Import verwendet, damit derselbe Ordner nie doppelt entsteht.
pub(crate) fn find_or_create(
    conn: &Connection,
    path: &Path,
    parent_id: Option<FolderId>,
    added_at: OffsetDateTime,
) -> Result<FolderId> {
    if let Some(existing) = find_by_path(conn, path)? {
        return Ok(existing.id);
    }
    insert(conn, path, parent_id, added_at)
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
    fn insert_and_find_by_path_roundtrip() {
        let conn = setup();
        let now = OffsetDateTime::now_utc()
            .replace_nanosecond(0)
            .expect("gültig");
        let id = insert(&conn, Path::new("/fotos/2024"), None, now)
            .expect("Insert darf nicht scheitern");

        let found = find_by_path(&conn, Path::new("/fotos/2024"))
            .expect("Query darf nicht scheitern")
            .expect("Ordner sollte gefunden werden");
        assert_eq!(found.id, id);
        assert_eq!(found.path, PathBuf::from("/fotos/2024"));
        assert_eq!(found.added_at, now);
    }

    #[test]
    fn find_or_create_does_not_duplicate() {
        let conn = setup();
        let now = OffsetDateTime::now_utc();
        let first = find_or_create(&conn, Path::new("/fotos/2024"), None, now).expect("ok");
        let second = find_or_create(&conn, Path::new("/fotos/2024"), None, now).expect("ok");
        assert_eq!(first, second);
        assert_eq!(list_all(&conn).expect("ok").len(), 1);
    }

    #[test]
    fn missing_folder_is_not_found() {
        let conn = setup();
        assert!(find_by_path(&conn, Path::new("/nirgendwo"))
            .expect("ok")
            .is_none());
    }

    #[test]
    fn get_unknown_id_returns_not_found_error() {
        let conn = setup();
        let result = get(&conn, FolderId::new());
        assert!(matches!(result, Err(apx_core::AppError::NotFound { .. })));
    }

    #[test]
    fn update_path_changes_the_stored_path() {
        let conn = setup();
        let now = OffsetDateTime::now_utc();
        let id = insert(&conn, Path::new("/alt"), None, now).expect("ok");

        update_path(&conn, id, Path::new("/neu")).expect("ok");

        let found = get(&conn, id).expect("ok");
        assert_eq!(found.path, PathBuf::from("/neu"));
    }

    #[test]
    fn update_path_of_unknown_folder_fails() {
        let conn = setup();
        assert!(update_path(&conn, FolderId::new(), Path::new("/neu")).is_err());
    }

    #[test]
    fn deleting_parent_cascades_to_child_folders() {
        let conn = setup();
        let now = OffsetDateTime::now_utc();
        let parent = insert(&conn, Path::new("/fotos"), None, now).expect("ok");
        let _child = insert(&conn, Path::new("/fotos/2024"), Some(parent), now).expect("ok");

        conn.execute(
            "DELETE FROM folders WHERE id = ?1",
            params![parent.to_string()],
        )
        .expect("Delete darf nicht scheitern");

        assert!(
            list_all(&conn).expect("ok").is_empty(),
            "Kaskade sollte auch den Kind-Ordner löschen"
        );
    }
}
