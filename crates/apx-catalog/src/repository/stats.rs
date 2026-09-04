//! Katalog-Statistik-Dashboard (Phase 9 Schritt 3, siehe `PLAN.md`/
//! `DECISIONS.md` ADR-0035) — reine Aggregatabfragen über `photos`, keine
//! neue Tabelle. Absichtlich in einem eigenen Modul statt in `photos.rs`,
//! weil es rein lesend/aggregierend ist statt einzelne Zeilen zu
//! manipulieren.

use apx_core::Result;
use rusqlite::Connection;

use crate::error::map_sqlite_err;
use crate::models::CatalogStatistics;

const TOP_N: usize = 8;

fn top_value_counts(conn: &Connection, column: &str) -> Result<Vec<(String, u64)>> {
    // `column` kommt ausschließlich aus dieser Datei fest verdrahtet
    // (nie aus Nutzereingabe) — String-Interpolation hier ist deshalb
    // sicher, ein gebundener Parameter wäre für einen Spaltennamen ohnehin
    // nicht möglich (SQLite bindet nur Werte, keine Bezeichner).
    let sql = format!(
        "SELECT {column}, COUNT(*) as cnt FROM photos \
         WHERE {column} IS NOT NULL AND source_photo_id IS NULL \
         GROUP BY {column} ORDER BY cnt DESC, {column} ASC LIMIT {TOP_N}"
    );
    let mut stmt = conn.prepare(&sql).map_err(map_sqlite_err)?;
    let rows = stmt
        .query_map([], |row| {
            let value: String = row.get(0)?;
            let count: i64 = row.get(1)?;
            Ok((value, count as u64))
        })
        .map_err(map_sqlite_err)?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(map_sqlite_err)
}

fn rating_distribution(conn: &Connection) -> Result<Vec<(u8, u64)>> {
    let mut stmt = conn
        .prepare(
            "SELECT rating, COUNT(*) FROM photos WHERE source_photo_id IS NULL \
             GROUP BY rating ORDER BY rating ASC",
        )
        .map_err(map_sqlite_err)?;
    let rows = stmt
        .query_map([], |row| {
            let rating: i64 = row.get(0)?;
            let count: i64 = row.get(1)?;
            Ok((rating as u8, count as u64))
        })
        .map_err(map_sqlite_err)?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(map_sqlite_err)
}

/// Aggregierte Katalog-Statistik — schließt virtuelle Kopien
/// (`source_photo_id IS NOT NULL`) konsequent aus, damit z. B. die
/// Foto-Gesamtzahl den tatsächlichen Datei-Bestand zeigt, nicht durch
/// zusätzliche Bearbeitungsstände desselben Fotos aufgebläht wird.
pub(crate) fn compute(conn: &Connection) -> Result<CatalogStatistics> {
    let (total_photos, total_file_size): (i64, i64) = conn
        .query_row(
            "SELECT COUNT(*), COALESCE(SUM(file_size), 0) FROM photos WHERE source_photo_id IS NULL",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(map_sqlite_err)?;

    let (earliest, latest): (Option<i64>, Option<i64>) = conn
        .query_row(
            "SELECT MIN(captured_at), MAX(captured_at) FROM photos \
             WHERE source_photo_id IS NULL AND captured_at IS NOT NULL",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(map_sqlite_err)?;

    Ok(CatalogStatistics {
        total_photos: total_photos as u64,
        total_file_size: total_file_size as u64,
        earliest_captured_at: earliest.map(crate::models::from_unix).transpose()?,
        latest_captured_at: latest.map(crate::models::from_unix).transpose()?,
        rating_distribution: rating_distribution(conn)?,
        top_camera_models: top_value_counts(conn, "camera_model")?,
        top_lenses: top_value_counts(conn, "lens")?,
    })
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

    fn insert_photo(conn: &Connection, filename: &str, camera_model: Option<&str>, rating: u8) {
        let folder_id =
            folders::find_or_create(conn, Path::new("/fotos"), None, OffsetDateTime::now_utc())
                .expect("Ordner");
        let photo = NewPhoto {
            media_kind: "photo".to_string(),
            duration_ms: None,
            video_codec: None,
            has_audio: None,
            frame_rate: None,
            folder_id,
            filename: filename.to_string(),
            file_size: 1000,
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
        let (photo_id, _) =
            photos::upsert(conn, &photo, OffsetDateTime::now_utc()).expect("Foto anlegen");
        photos::set_rating(conn, photo_id, rating).expect("Bewertung setzen");
    }

    #[test]
    fn empty_catalog_has_zero_totals_and_empty_distributions() {
        let conn = setup();
        let stats = compute(&conn).expect("Statistik");
        assert_eq!(stats.total_photos, 0);
        assert_eq!(stats.total_file_size, 0);
        assert!(stats.top_camera_models.is_empty());
    }

    #[test]
    fn counts_photos_and_groups_by_camera_model() {
        let conn = setup();
        insert_photo(&conn, "a.cr2", Some("Canon EOS R5"), 5);
        insert_photo(&conn, "b.cr2", Some("Canon EOS R5"), 3);
        insert_photo(&conn, "c.cr2", Some("Nikon Z9"), 0);

        let stats = compute(&conn).expect("Statistik");
        assert_eq!(stats.total_photos, 3);
        assert_eq!(stats.total_file_size, 3000);
        assert_eq!(stats.top_camera_models[0], ("Canon EOS R5".to_string(), 2));
        assert!(stats.rating_distribution.contains(&(5, 1)));
        assert!(stats.rating_distribution.contains(&(0, 1)));
    }
}
