//! Volltextsuche (`photos_fts`, FTS5) und kombinierbarer Attributfilter
//! über `photos` — siehe `migrations/0003_library.sql`, `DECISIONS.md`
//! ADR-0023 und `PLAN.md` Phase 3, Schritt 2.

use apx_core::Result;
use rusqlite::types::ToSql;
use rusqlite::Connection;

use crate::error::map_sqlite_err;
use crate::models::FilterCriteria;
use crate::repository::photos::{raw_to_photo, row_to_raw, SELECT_COLUMNS};
use crate::Photo;

/// Volltextsuche über Dateiname, Kamerahersteller/-modell und Objektiv.
/// `query` wird unverändert als FTS5-Match-Ausdruck durchgereicht (erlaubt
/// also z. B. `filename:sonnenuntergang*` oder mehrere Wörter per UND) —
/// Ergebnisse nach FTS5-Relevanz (`rank`) sortiert.
pub(crate) fn search_photos(conn: &Connection, query: &str) -> Result<Vec<Photo>> {
    let sql = format!(
        "SELECT {SELECT_COLUMNS} FROM photos_fts \
         JOIN photos ON photos.rowid = photos_fts.rowid \
         WHERE photos_fts MATCH ?1 ORDER BY rank"
    );
    let mut stmt = conn.prepare(&sql).map_err(map_sqlite_err)?;
    let rows = stmt
        .query_map(rusqlite::params![query], row_to_raw)
        .map_err(map_sqlite_err)?;
    let mut result = Vec::new();
    for row in rows {
        result.push(raw_to_photo(row.map_err(map_sqlite_err)?)?);
    }
    Ok(result)
}

/// Kombiniert alle gesetzten Felder von `criteria` per UND. Ein komplett
/// leeres `criteria` (alle Felder `None`) liefert alle Fotos, sortiert nach
/// Dateiname — konsistent mit [`crate::repository::photos::list_by_folder`].
pub(crate) fn filter_photos(conn: &Connection, criteria: &FilterCriteria) -> Result<Vec<Photo>> {
    let mut clauses: Vec<String> = Vec::new();
    let mut values: Vec<Box<dyn ToSql>> = Vec::new();

    if let Some(min) = criteria.rating_at_least {
        values.push(Box::new(min as i64));
        clauses.push(format!("photos.rating >= ?{}", values.len()));
    }
    if let Some(flag) = criteria.flag {
        values.push(Box::new(flag as i64));
        clauses.push(format!("photos.flag = ?{}", values.len()));
    }
    if let Some(color) = &criteria.color_label {
        values.push(Box::new(color.clone()));
        clauses.push(format!("photos.color_label = ?{}", values.len()));
    }
    if let Some(model) = &criteria.camera_model {
        values.push(Box::new(model.clone()));
        clauses.push(format!("photos.camera_model = ?{}", values.len()));
    }

    let where_clause = if clauses.is_empty() {
        "1 = 1".to_string()
    } else {
        clauses.join(" AND ")
    };
    let sql = format!("SELECT {SELECT_COLUMNS} FROM photos WHERE {where_clause} ORDER BY filename");

    let mut stmt = conn.prepare(&sql).map_err(map_sqlite_err)?;
    let param_refs: Vec<&dyn ToSql> = values.iter().map(|v| v.as_ref()).collect();
    let rows = stmt
        .query_map(param_refs.as_slice(), row_to_raw)
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
    use time::OffsetDateTime;

    fn setup() -> Connection {
        let conn = Connection::open_in_memory().expect("In-Memory-DB");
        migrations::apply(&conn).expect("Migration");
        conn
    }

    fn insert_photo(
        conn: &Connection,
        filename: &str,
        camera_model: Option<&str>,
    ) -> apx_core::PhotoId {
        let folder_id =
            folders::find_or_create(conn, Path::new("/fotos"), None, OffsetDateTime::now_utc())
                .expect("Ordner");
        let photo = NewPhoto {
            folder_id,
            filename: filename.to_string(),
            file_size: 100,
            file_mtime: OffsetDateTime::now_utc()
                .replace_nanosecond(0)
                .expect("gültig"),
            content_hash: None,
            width: None,
            height: None,
            orientation: 1,
            camera_make: None,
            camera_model: camera_model.map(|s| s.to_string()),
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
    fn search_finds_photo_by_filename_substring_token() {
        let conn = setup();
        let id = insert_photo(&conn, "Sonnenuntergang_Strand.CR2", None);
        insert_photo(&conn, "Bergwanderung.CR2", None);

        let results = search_photos(&conn, "Sonnenuntergang*").expect("ok");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, id);
    }

    #[test]
    fn search_finds_photo_by_camera_model() {
        let conn = setup();
        let id = insert_photo(&conn, "a.cr2", Some("EOS R5"));
        insert_photo(&conn, "b.cr2", Some("Z9"));

        let results = search_photos(&conn, "R5").expect("ok");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, id);
    }

    #[test]
    fn search_reflects_updates_via_sync_trigger() {
        let conn = setup();
        let id = insert_photo(&conn, "vorher.cr2", None);

        assert!(search_photos(&conn, "nachher").expect("ok").is_empty());

        conn.execute(
            "UPDATE photos SET filename = 'nachher.cr2' WHERE id = ?1",
            rusqlite::params![id.to_string()],
        )
        .expect("Update");

        let results = search_photos(&conn, "nachher").expect("ok");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, id);
        assert!(
            search_photos(&conn, "vorher").expect("ok").is_empty(),
            "alter Dateiname darf nach Umbenennung nicht mehr treffen"
        );
    }

    #[test]
    fn empty_filter_returns_all_photos() {
        let conn = setup();
        insert_photo(&conn, "a.cr2", None);
        insert_photo(&conn, "b.cr2", None);

        let results = filter_photos(&conn, &FilterCriteria::default()).expect("ok");
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn filter_combines_rating_and_camera_model_with_and() {
        let conn = setup();
        let matching = insert_photo(&conn, "a.cr2", Some("EOS R5"));
        let wrong_camera = insert_photo(&conn, "b.cr2", Some("Z9"));
        photos::set_rating(&conn, matching, 4).expect("ok");
        photos::set_rating(&conn, wrong_camera, 4).expect("ok");

        let criteria = FilterCriteria {
            rating_at_least: Some(3),
            camera_model: Some("EOS R5".to_string()),
            ..Default::default()
        };
        let results = filter_photos(&conn, &criteria).expect("ok");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, matching);
    }
}
