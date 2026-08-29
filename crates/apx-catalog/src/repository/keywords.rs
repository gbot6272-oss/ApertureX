//! SQL für `keywords`/`photo_keywords` (siehe `migrations/0003_library.sql`,
//! `DECISIONS.md` ADR-0022) — flache Schlagwort-Liste ohne Hierarchie oder
//! Synonyme.

use apx_core::{KeywordId, PhotoId, Result};
use rusqlite::{params, Connection, OptionalExtension};

use crate::error::map_sqlite_err;
use crate::models::Keyword;

fn find_by_name(conn: &Connection, name: &str) -> Result<Option<KeywordId>> {
    let id: Option<String> = conn
        .query_row(
            "SELECT id FROM keywords WHERE name = ?1",
            params![name],
            |row| row.get(0),
        )
        .optional()
        .map_err(map_sqlite_err)?;
    id.map(|id| id.parse()).transpose()
}

fn find_or_create(conn: &Connection, name: &str) -> Result<KeywordId> {
    if let Some(id) = find_by_name(conn, name)? {
        return Ok(id);
    }
    let id = KeywordId::new();
    conn.execute(
        "INSERT INTO keywords (id, name) VALUES (?1, ?2)",
        params![id.to_string(), name],
    )
    .map_err(map_sqlite_err)?;
    Ok(id)
}

/// Verknüpft `photo_id` mit dem Schlagwort `name` — legt das Schlagwort an,
/// falls es noch nicht existiert. Wiederholtes Hinzufügen desselben
/// Schlagworts ist ein No-Op (`INSERT OR IGNORE`, `photo_keywords` hat
/// `(photo_id, keyword_id)` als Primärschlüssel).
pub(crate) fn add(conn: &Connection, photo_id: PhotoId, name: &str) -> Result<KeywordId> {
    let keyword_id = find_or_create(conn, name)?;
    conn.execute(
        "INSERT OR IGNORE INTO photo_keywords (photo_id, keyword_id) VALUES (?1, ?2)",
        params![photo_id.to_string(), keyword_id.to_string()],
    )
    .map_err(map_sqlite_err)?;
    Ok(keyword_id)
}

/// Löst die Verknüpfung zwischen `photo_id` und `keyword_id` — das
/// Schlagwort selbst bleibt bestehen (kann an anderen Fotos hängen).
pub(crate) fn remove(conn: &Connection, photo_id: PhotoId, keyword_id: KeywordId) -> Result<()> {
    conn.execute(
        "DELETE FROM photo_keywords WHERE photo_id = ?1 AND keyword_id = ?2",
        params![photo_id.to_string(), keyword_id.to_string()],
    )
    .map_err(map_sqlite_err)?;
    Ok(())
}

pub(crate) fn list_for_photo(conn: &Connection, photo_id: PhotoId) -> Result<Vec<Keyword>> {
    let mut stmt = conn
        .prepare(
            "SELECT k.id, k.name FROM keywords k \
             JOIN photo_keywords pk ON pk.keyword_id = k.id \
             WHERE pk.photo_id = ?1 ORDER BY k.name",
        )
        .map_err(map_sqlite_err)?;
    let rows = stmt
        .query_map(params![photo_id.to_string()], |row| {
            let id: String = row.get(0)?;
            let name: String = row.get(1)?;
            Ok((id, name))
        })
        .map_err(map_sqlite_err)?;
    let mut result = Vec::new();
    for row in rows {
        let (id, name) = row.map_err(map_sqlite_err)?;
        result.push(Keyword {
            id: id.parse()?,
            name,
        });
    }
    Ok(result)
}

pub(crate) fn list_all(conn: &Connection) -> Result<Vec<Keyword>> {
    let mut stmt = conn
        .prepare("SELECT id, name FROM keywords ORDER BY name")
        .map_err(map_sqlite_err)?;
    let rows = stmt
        .query_map([], |row| {
            let id: String = row.get(0)?;
            let name: String = row.get(1)?;
            Ok((id, name))
        })
        .map_err(map_sqlite_err)?;
    let mut result = Vec::new();
    for row in rows {
        let (id, name) = row.map_err(map_sqlite_err)?;
        result.push(Keyword {
            id: id.parse()?,
            name,
        });
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
    use time::OffsetDateTime;

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

    #[test]
    fn adding_the_same_keyword_twice_is_idempotent() {
        let (conn, photo_id) = setup();
        add(&conn, photo_id, "Sonnenuntergang").expect("ok");
        add(&conn, photo_id, "Sonnenuntergang").expect("ok");

        let keywords = list_for_photo(&conn, photo_id).expect("ok");
        assert_eq!(keywords.len(), 1);
        assert_eq!(keywords[0].name, "Sonnenuntergang");
    }

    #[test]
    fn remove_detaches_keyword_but_keeps_it_in_catalog() {
        let (conn, photo_id) = setup();
        let keyword_id = add(&conn, photo_id, "Strand").expect("ok");

        remove(&conn, photo_id, keyword_id).expect("ok");

        assert!(list_for_photo(&conn, photo_id).expect("ok").is_empty());
        assert_eq!(
            list_all(&conn).expect("ok").len(),
            1,
            "Schlagwort selbst bleibt im Katalog bestehen"
        );
    }

    #[test]
    fn deleting_photo_cascades_to_photo_keywords_but_keeps_keyword() {
        let (conn, photo_id) = setup();
        add(&conn, photo_id, "Berge").expect("ok");

        conn.execute(
            "DELETE FROM photos WHERE id = ?1",
            params![photo_id.to_string()],
        )
        .expect("Foto löschen");

        let count: i64 = conn
            .query_row("SELECT count(*) FROM photo_keywords", [], |row| row.get(0))
            .expect("lesbar");
        assert_eq!(count, 0, "Verknüpfung muss per Kaskade gelöscht sein");
        assert_eq!(
            list_all(&conn).expect("ok").len(),
            1,
            "Schlagwort selbst bleibt bestehen"
        );
    }
}
