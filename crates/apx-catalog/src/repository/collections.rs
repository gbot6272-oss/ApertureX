//! SQL für `collections`/`collection_photos` (siehe
//! `migrations/0003_library.sql`, `DECISIONS.md` ADR-0023) — rein manuelle
//! Sammlungen mit fester Reihenfolge über `position`, keine intelligenten/
//! verschachtelten Sammlungen.

use apx_core::{AppError, CollectionId, PhotoId, Result};
use rusqlite::{params, Connection};
use time::OffsetDateTime;

use crate::error::map_sqlite_err;
use crate::models::{from_unix, to_unix, Collection};
use crate::repository::photos::{raw_to_photo, row_to_raw, SELECT_COLUMNS};
use crate::Photo;

pub(crate) fn create(
    conn: &Connection,
    name: &str,
    created_at: OffsetDateTime,
) -> Result<CollectionId> {
    let id = CollectionId::new();
    conn.execute(
        "INSERT INTO collections (id, name, created_at) VALUES (?1, ?2, ?3)",
        params![id.to_string(), name, to_unix(created_at)],
    )
    .map_err(map_sqlite_err)?;
    Ok(id)
}

pub(crate) fn rename(conn: &Connection, id: CollectionId, name: &str) -> Result<()> {
    let changed = conn
        .execute(
            "UPDATE collections SET name = ?2 WHERE id = ?1",
            params![id.to_string(), name],
        )
        .map_err(map_sqlite_err)?;
    if changed == 0 {
        return Err(AppError::not_found("Sammlung", id.to_string()));
    }
    Ok(())
}

pub(crate) fn delete(conn: &Connection, id: CollectionId) -> Result<()> {
    let changed = conn
        .execute(
            "DELETE FROM collections WHERE id = ?1",
            params![id.to_string()],
        )
        .map_err(map_sqlite_err)?;
    if changed == 0 {
        return Err(AppError::not_found("Sammlung", id.to_string()));
    }
    Ok(())
}

pub(crate) fn list_all(conn: &Connection) -> Result<Vec<Collection>> {
    let mut stmt = conn
        .prepare("SELECT id, name, created_at FROM collections ORDER BY created_at")
        .map_err(map_sqlite_err)?;
    let rows = stmt
        .query_map([], |row| {
            let id: String = row.get(0)?;
            let name: String = row.get(1)?;
            let created_at: i64 = row.get(2)?;
            Ok((id, name, created_at))
        })
        .map_err(map_sqlite_err)?;
    let mut result = Vec::new();
    for row in rows {
        let (id, name, created_at) = row.map_err(map_sqlite_err)?;
        result.push(Collection {
            id: id.parse()?,
            name,
            created_at: from_unix(created_at)?,
        });
    }
    Ok(result)
}

/// Fügt `photo_id` ans Ende von `collection_id` an (höchste `position` + 1).
/// Erneutes Hinzufügen desselben Fotos ist ein No-Op (Primärschlüssel
/// `(collection_id, photo_id)`), verschiebt es also nicht ans Ende.
pub(crate) fn add_photo(
    conn: &Connection,
    collection_id: CollectionId,
    photo_id: PhotoId,
) -> Result<()> {
    let next_position: i64 = conn
        .query_row(
            "SELECT COALESCE(MAX(position), -1) + 1 FROM collection_photos WHERE collection_id = ?1",
            params![collection_id.to_string()],
            |row| row.get(0),
        )
        .map_err(map_sqlite_err)?;
    conn.execute(
        "INSERT OR IGNORE INTO collection_photos (collection_id, photo_id, position) VALUES (?1, ?2, ?3)",
        params![collection_id.to_string(), photo_id.to_string(), next_position],
    )
    .map_err(map_sqlite_err)?;
    Ok(())
}

pub(crate) fn remove_photo(
    conn: &Connection,
    collection_id: CollectionId,
    photo_id: PhotoId,
) -> Result<()> {
    conn.execute(
        "DELETE FROM collection_photos WHERE collection_id = ?1 AND photo_id = ?2",
        params![collection_id.to_string(), photo_id.to_string()],
    )
    .map_err(map_sqlite_err)?;
    Ok(())
}

pub(crate) fn list_photos(conn: &Connection, collection_id: CollectionId) -> Result<Vec<Photo>> {
    let sql = format!(
        "SELECT {SELECT_COLUMNS} FROM collection_photos cp \
         JOIN photos ON photos.id = cp.photo_id \
         WHERE cp.collection_id = ?1 ORDER BY cp.position"
    );
    let mut stmt = conn.prepare(&sql).map_err(map_sqlite_err)?;
    let rows = stmt
        .query_map(params![collection_id.to_string()], row_to_raw)
        .map_err(map_sqlite_err)?;
    let mut result = Vec::new();
    for row in rows {
        result.push(raw_to_photo(row.map_err(map_sqlite_err)?)?);
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::migrations;
    use crate::models::NewPhoto;
    use crate::repository::{folders, photos};
    use std::path::Path;

    fn setup() -> (Connection, PhotoId) {
        let conn = Connection::open_in_memory().expect("In-Memory-DB");
        conn.execute_batch("PRAGMA foreign_keys = ON")
            .expect("FKs an");
        migrations::apply(&conn).expect("Migration");
        let folder_id =
            folders::insert(&conn, Path::new("/fotos"), None, OffsetDateTime::now_utc())
                .expect("Ordner");
        let photo = NewPhoto {
            folder_id,
            filename: "a.cr2".to_string(),
            file_size: 100,
            file_mtime: OffsetDateTime::now_utc()
                .replace_nanosecond(0)
                .expect("gültig"),
            content_hash: None,
            width: None,
            height: None,
            orientation: 1,
            camera_make: None,
            camera_model: None,
            lens: None,
            iso: None,
            shutter: None,
            aperture: None,
            focal_length: None,
            captured_at: None,
            gps_lat: None,
            gps_lon: None,
        };
        let (photo_id, _) =
            photos::upsert(&conn, &photo, OffsetDateTime::now_utc()).expect("Foto anlegen");
        (conn, photo_id)
    }

    fn second_photo(conn: &Connection, folder_id: apx_core::FolderId) -> PhotoId {
        let photo = NewPhoto {
            folder_id,
            filename: "b.cr2".to_string(),
            file_size: 200,
            file_mtime: OffsetDateTime::now_utc()
                .replace_nanosecond(0)
                .expect("gültig"),
            content_hash: None,
            width: None,
            height: None,
            orientation: 1,
            camera_make: None,
            camera_model: None,
            lens: None,
            iso: None,
            shutter: None,
            aperture: None,
            focal_length: None,
            captured_at: None,
            gps_lat: None,
            gps_lon: None,
        };
        photos::upsert(conn, &photo, OffsetDateTime::now_utc())
            .expect("Foto anlegen")
            .0
    }

    #[test]
    fn create_rename_delete_roundtrip() {
        let (conn, _) = setup();
        let id = create(&conn, "Urlaub 2026", OffsetDateTime::now_utc()).expect("ok");
        assert_eq!(list_all(&conn).expect("ok").len(), 1);

        rename(&conn, id, "Urlaub 2026 (Auswahl)").expect("ok");
        assert_eq!(
            list_all(&conn).expect("ok")[0].name,
            "Urlaub 2026 (Auswahl)"
        );

        delete(&conn, id).expect("ok");
        assert!(list_all(&conn).expect("ok").is_empty());
        assert!(rename(&conn, id, "x").is_err(), "gelöschte Sammlung");
    }

    #[test]
    fn add_photo_keeps_insertion_order_via_position() {
        let (conn, photo_a) = setup();
        let folder_id = photos::get(&conn, photo_a).expect("ok").folder_id;
        let photo_b = second_photo(&conn, folder_id);
        let collection_id =
            create(&conn, "Reihenfolge-Test", OffsetDateTime::now_utc()).expect("ok");

        add_photo(&conn, collection_id, photo_b).expect("ok");
        add_photo(&conn, collection_id, photo_a).expect("ok");

        let photos_in_order = list_photos(&conn, collection_id).expect("ok");
        assert_eq!(photos_in_order.len(), 2);
        assert_eq!(photos_in_order[0].id, photo_b, "zuerst hinzugefügt zuerst");
        assert_eq!(photos_in_order[1].id, photo_a);
    }

    #[test]
    fn adding_same_photo_twice_is_idempotent() {
        let (conn, photo_id) = setup();
        let collection_id = create(&conn, "Test", OffsetDateTime::now_utc()).expect("ok");

        add_photo(&conn, collection_id, photo_id).expect("ok");
        add_photo(&conn, collection_id, photo_id).expect("ok");

        assert_eq!(list_photos(&conn, collection_id).expect("ok").len(), 1);
    }

    #[test]
    fn remove_photo_detaches_without_deleting_collection_or_photo() {
        let (conn, photo_id) = setup();
        let collection_id = create(&conn, "Test", OffsetDateTime::now_utc()).expect("ok");
        add_photo(&conn, collection_id, photo_id).expect("ok");

        remove_photo(&conn, collection_id, photo_id).expect("ok");

        assert!(list_photos(&conn, collection_id).expect("ok").is_empty());
        assert_eq!(list_all(&conn).expect("ok").len(), 1);
    }

    #[test]
    fn deleting_collection_cascades_to_collection_photos() {
        let (conn, photo_id) = setup();
        let collection_id = create(&conn, "Test", OffsetDateTime::now_utc()).expect("ok");
        add_photo(&conn, collection_id, photo_id).expect("ok");

        delete(&conn, collection_id).expect("ok");

        let count: i64 = conn
            .query_row("SELECT count(*) FROM collection_photos", [], |row| {
                row.get(0)
            })
            .expect("lesbar");
        assert_eq!(count, 0, "Verknüpfung muss per Kaskade gelöscht sein");
    }

    #[test]
    fn deleting_photo_cascades_to_collection_photos_but_keeps_collection() {
        let (conn, photo_id) = setup();
        let collection_id = create(&conn, "Test", OffsetDateTime::now_utc()).expect("ok");
        add_photo(&conn, collection_id, photo_id).expect("ok");

        conn.execute(
            "DELETE FROM photos WHERE id = ?1",
            params![photo_id.to_string()],
        )
        .expect("Foto löschen");

        assert!(list_photos(&conn, collection_id).expect("ok").is_empty());
        assert_eq!(list_all(&conn).expect("ok").len(), 1);
    }
}
