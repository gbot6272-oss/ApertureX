//! SQL für die `photos`-Tabelle.

use apx_core::{AppError, FolderId, PhotoId, Result};
use rusqlite::{params, Connection, OptionalExtension};
use time::OffsetDateTime;

use crate::error::map_sqlite_err;
use crate::models::{from_unix, from_unix_opt, to_unix, to_unix_opt, NewPhoto, Photo};

const SELECT_COLUMNS: &str =
    "id, folder_id, filename, file_size, file_mtime, content_hash, width, \
     height, orientation, camera_make, camera_model, lens, iso, shutter, aperture, focal_length, \
     captured_at, gps_lat, gps_lon, imported_at, missing";

#[allow(clippy::type_complexity)]
struct PhotoRow {
    id: String,
    folder_id: String,
    filename: String,
    file_size: i64,
    file_mtime: i64,
    content_hash: Option<String>,
    width: Option<i64>,
    height: Option<i64>,
    orientation: i64,
    camera_make: Option<String>,
    camera_model: Option<String>,
    lens: Option<String>,
    iso: Option<i64>,
    shutter: Option<f64>,
    aperture: Option<f64>,
    focal_length: Option<f64>,
    captured_at: Option<i64>,
    gps_lat: Option<f64>,
    gps_lon: Option<f64>,
    imported_at: i64,
    missing: i64,
}

fn row_to_raw(row: &rusqlite::Row) -> rusqlite::Result<PhotoRow> {
    Ok(PhotoRow {
        id: row.get(0)?,
        folder_id: row.get(1)?,
        filename: row.get(2)?,
        file_size: row.get(3)?,
        file_mtime: row.get(4)?,
        content_hash: row.get(5)?,
        width: row.get(6)?,
        height: row.get(7)?,
        orientation: row.get(8)?,
        camera_make: row.get(9)?,
        camera_model: row.get(10)?,
        lens: row.get(11)?,
        iso: row.get(12)?,
        shutter: row.get(13)?,
        aperture: row.get(14)?,
        focal_length: row.get(15)?,
        captured_at: row.get(16)?,
        gps_lat: row.get(17)?,
        gps_lon: row.get(18)?,
        imported_at: row.get(19)?,
        missing: row.get(20)?,
    })
}

fn raw_to_photo(raw: PhotoRow) -> Result<Photo> {
    Ok(Photo {
        id: raw.id.parse()?,
        folder_id: raw.folder_id.parse()?,
        filename: raw.filename,
        file_size: raw.file_size as u64,
        file_mtime: from_unix(raw.file_mtime)?,
        content_hash: raw.content_hash,
        width: raw.width.map(|v| v as u32),
        height: raw.height.map(|v| v as u32),
        orientation: raw.orientation as u16,
        camera_make: raw.camera_make,
        camera_model: raw.camera_model,
        lens: raw.lens,
        iso: raw.iso.map(|v| v as u32),
        shutter: raw.shutter.map(|v| v as f32),
        aperture: raw.aperture.map(|v| v as f32),
        focal_length: raw.focal_length.map(|v| v as f32),
        captured_at: from_unix_opt(raw.captured_at)?,
        gps_lat: raw.gps_lat,
        gps_lon: raw.gps_lon,
        imported_at: from_unix(raw.imported_at)?,
        missing: raw.missing != 0,
    })
}

pub(crate) fn find_by_folder_and_filename(
    conn: &Connection,
    folder_id: FolderId,
    filename: &str,
) -> Result<Option<Photo>> {
    let sql = format!("SELECT {SELECT_COLUMNS} FROM photos WHERE folder_id = ?1 AND filename = ?2");
    let raw: Option<PhotoRow> = conn
        .query_row(&sql, params![folder_id.to_string(), filename], row_to_raw)
        .optional()
        .map_err(map_sqlite_err)?;
    raw.map(raw_to_photo).transpose()
}

pub(crate) fn get(conn: &Connection, id: PhotoId) -> Result<Photo> {
    let sql = format!("SELECT {SELECT_COLUMNS} FROM photos WHERE id = ?1");
    let raw = conn
        .query_row(&sql, params![id.to_string()], row_to_raw)
        .map_err(|err| match err {
            rusqlite::Error::QueryReturnedNoRows => AppError::not_found("Foto", id.to_string()),
            other => map_sqlite_err(other),
        })?;
    raw_to_photo(raw)
}

pub(crate) fn list_by_folder(conn: &Connection, folder_id: FolderId) -> Result<Vec<Photo>> {
    let sql = format!("SELECT {SELECT_COLUMNS} FROM photos WHERE folder_id = ?1 ORDER BY filename");
    let mut stmt = conn.prepare(&sql).map_err(map_sqlite_err)?;
    let rows = stmt
        .query_map(params![folder_id.to_string()], row_to_raw)
        .map_err(map_sqlite_err)?;
    let mut result = Vec::new();
    for row in rows {
        result.push(raw_to_photo(row.map_err(map_sqlite_err)?)?);
    }
    Ok(result)
}

pub(crate) fn count_by_folder(conn: &Connection, folder_id: FolderId) -> Result<u64> {
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM photos WHERE folder_id = ?1",
            params![folder_id.to_string()],
            |row| row.get(0),
        )
        .map_err(map_sqlite_err)?;
    Ok(count as u64)
}

pub(crate) fn set_missing(conn: &Connection, id: PhotoId, missing: bool) -> Result<()> {
    let changed = conn
        .execute(
            "UPDATE photos SET missing = ?2 WHERE id = ?1",
            params![id.to_string(), missing as i64],
        )
        .map_err(map_sqlite_err)?;
    if changed == 0 {
        return Err(AppError::not_found("Foto", id.to_string()));
    }
    Ok(())
}

/// Legt ein Foto an oder aktualisiert es, falls unter demselben
/// `(folder_id, filename)` bereits eines existiert (siehe `UNIQUE`-Constraint
/// in Migration 1). Gibt `(id, war_neu_oder_geändert)` zurück — Aufrufer
/// (der Import-Job) nutzt das für seine Fortschrittsstatistik.
///
/// - Existiert noch kein Eintrag: neue Zeile, `changed = true`.
/// - Existiert einer mit identischer Größe/Änderungszeit: unverändert,
///   `changed = false` — das ist der Fall "Datei schon importiert".
/// - Existiert einer, aber Größe/Änderungszeit weichen ab (Datei wurde
///   außerhalb von Aperture X ersetzt): Zeile wird aktualisiert, ID bleibt
///   erhalten, `changed = true`.
pub(crate) fn upsert(
    conn: &Connection,
    new_photo: &NewPhoto,
    imported_at: OffsetDateTime,
) -> Result<(PhotoId, bool)> {
    if let Some(existing) =
        find_by_folder_and_filename(conn, new_photo.folder_id, &new_photo.filename)?
    {
        let unchanged = existing.file_size == new_photo.file_size
            && existing.file_mtime == new_photo.file_mtime;
        if unchanged {
            return Ok((existing.id, false));
        }
        update_row(conn, existing.id, new_photo)?;
        return Ok((existing.id, true));
    }

    let id = PhotoId::new();
    insert_row(conn, id, new_photo, imported_at)?;
    Ok((id, true))
}

fn insert_row(
    conn: &Connection,
    id: PhotoId,
    p: &NewPhoto,
    imported_at: OffsetDateTime,
) -> Result<()> {
    conn.execute(
        "INSERT INTO photos (
            id, folder_id, filename, file_size, file_mtime, content_hash, width, height,
            orientation, camera_make, camera_model, lens, iso, shutter, aperture, focal_length,
            captured_at, gps_lat, gps_lon, imported_at, missing
        ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20,0)",
        params![
            id.to_string(),
            p.folder_id.to_string(),
            p.filename,
            p.file_size as i64,
            to_unix(p.file_mtime),
            p.content_hash,
            p.width.map(|v| v as i64),
            p.height.map(|v| v as i64),
            p.orientation as i64,
            p.camera_make,
            p.camera_model,
            p.lens,
            p.iso.map(|v| v as i64),
            p.shutter.map(|v| v as f64),
            p.aperture.map(|v| v as f64),
            p.focal_length.map(|v| v as f64),
            to_unix_opt(p.captured_at),
            p.gps_lat,
            p.gps_lon,
            to_unix(imported_at),
        ],
    )
    .map_err(map_sqlite_err)?;
    Ok(())
}

fn update_row(conn: &Connection, id: PhotoId, p: &NewPhoto) -> Result<()> {
    conn.execute(
        "UPDATE photos SET
            file_size = ?2, file_mtime = ?3, content_hash = ?4, width = ?5, height = ?6,
            orientation = ?7, camera_make = ?8, camera_model = ?9, lens = ?10, iso = ?11,
            shutter = ?12, aperture = ?13, focal_length = ?14, captured_at = ?15,
            gps_lat = ?16, gps_lon = ?17, missing = 0
         WHERE id = ?1",
        params![
            id.to_string(),
            p.file_size as i64,
            to_unix(p.file_mtime),
            p.content_hash,
            p.width.map(|v| v as i64),
            p.height.map(|v| v as i64),
            p.orientation as i64,
            p.camera_make,
            p.camera_model,
            p.lens,
            p.iso.map(|v| v as i64),
            p.shutter.map(|v| v as f64),
            p.aperture.map(|v| v as f64),
            p.focal_length.map(|v| v as f64),
            to_unix_opt(p.captured_at),
            p.gps_lat,
            p.gps_lon,
        ],
    )
    .map_err(map_sqlite_err)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::migrations;
    use crate::repository::folders;
    use std::path::Path;

    fn setup() -> (Connection, FolderId) {
        let conn = Connection::open_in_memory().expect("In-Memory-DB");
        migrations::apply(&conn).expect("Migration");
        let folder_id =
            folders::insert(&conn, Path::new("/fotos"), None, OffsetDateTime::now_utc())
                .expect("Ordner");
        (conn, folder_id)
    }

    fn sample_photo(folder_id: FolderId, size: u64, mtime: OffsetDateTime) -> NewPhoto {
        NewPhoto {
            folder_id,
            filename: "IMG_0001.CR2".to_string(),
            file_size: size,
            file_mtime: mtime,
            content_hash: Some("abc123".to_string()),
            width: Some(6000),
            height: Some(4000),
            orientation: 1,
            camera_make: Some("Canon".to_string()),
            camera_model: Some("EOS R5".to_string()),
            lens: Some("RF 24-70mm".to_string()),
            iso: Some(400),
            shutter: Some(0.008),
            aperture: Some(2.8),
            focal_length: Some(50.0),
            captured_at: Some(
                OffsetDateTime::now_utc()
                    .replace_nanosecond(0)
                    .expect("gültig"),
            ),
            gps_lat: Some(52.5),
            gps_lon: Some(13.4),
        }
    }

    #[test]
    fn upsert_inserts_new_photo() {
        let (conn, folder_id) = setup();
        let mtime = OffsetDateTime::now_utc()
            .replace_nanosecond(0)
            .expect("gültig");
        let (id, changed) = upsert(
            &conn,
            &sample_photo(folder_id, 1000, mtime),
            OffsetDateTime::now_utc(),
        )
        .expect("ok");
        assert!(changed);

        let fetched = get(&conn, id).expect("sollte gefunden werden");
        assert_eq!(fetched.filename, "IMG_0001.CR2");
        assert_eq!(fetched.file_size, 1000);
        assert!(!fetched.missing);
    }

    #[test]
    fn second_import_of_unchanged_photo_is_not_a_duplicate() {
        let (conn, folder_id) = setup();
        let mtime = OffsetDateTime::now_utc()
            .replace_nanosecond(0)
            .expect("gültig");
        let photo = sample_photo(folder_id, 1000, mtime);

        let (first_id, first_changed) =
            upsert(&conn, &photo, OffsetDateTime::now_utc()).expect("ok");
        let (second_id, second_changed) =
            upsert(&conn, &photo, OffsetDateTime::now_utc()).expect("ok");

        assert!(first_changed);
        assert!(
            !second_changed,
            "unveränderte Datei darf beim zweiten Import nicht als Änderung zählen"
        );
        assert_eq!(first_id, second_id);
        assert_eq!(count_by_folder(&conn, folder_id).expect("ok"), 1);
    }

    #[test]
    fn changed_file_updates_existing_row_instead_of_duplicating() {
        let (conn, folder_id) = setup();
        let mtime = OffsetDateTime::now_utc()
            .replace_nanosecond(0)
            .expect("gültig");
        let (first_id, _) = upsert(
            &conn,
            &sample_photo(folder_id, 1000, mtime),
            OffsetDateTime::now_utc(),
        )
        .expect("ok");

        let later_mtime = mtime + time::Duration::seconds(60);
        let (second_id, changed) = upsert(
            &conn,
            &sample_photo(folder_id, 2000, later_mtime),
            OffsetDateTime::now_utc(),
        )
        .expect("ok");

        assert!(changed);
        assert_eq!(
            first_id, second_id,
            "ID muss stabil bleiben, keine neue Zeile"
        );
        assert_eq!(count_by_folder(&conn, folder_id).expect("ok"), 1);
        assert_eq!(get(&conn, first_id).expect("ok").file_size, 2000);
    }

    #[test]
    fn set_missing_marks_photo_and_unknown_id_fails() {
        let (conn, folder_id) = setup();
        let mtime = OffsetDateTime::now_utc()
            .replace_nanosecond(0)
            .expect("gültig");
        let (id, _) = upsert(
            &conn,
            &sample_photo(folder_id, 1000, mtime),
            OffsetDateTime::now_utc(),
        )
        .expect("ok");

        set_missing(&conn, id, true).expect("ok");
        assert!(get(&conn, id).expect("ok").missing);

        assert!(set_missing(&conn, PhotoId::new(), true).is_err());
    }

    #[test]
    fn deleting_folder_cascades_to_photos() {
        let (conn, folder_id) = setup();
        let mtime = OffsetDateTime::now_utc()
            .replace_nanosecond(0)
            .expect("gültig");
        upsert(
            &conn,
            &sample_photo(folder_id, 1000, mtime),
            OffsetDateTime::now_utc(),
        )
        .expect("ok");

        conn.execute(
            "DELETE FROM folders WHERE id = ?1",
            params![folder_id.to_string()],
        )
        .expect("Delete darf nicht scheitern");

        assert_eq!(count_by_folder(&conn, folder_id).expect("ok"), 0);
    }
}
