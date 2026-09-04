//! SQL für die `previews`-Tabelle.

use std::path::{Path, PathBuf};

use apx_core::{PhotoId, Result};
use rusqlite::{params, Connection, OptionalExtension};
use time::OffsetDateTime;

use crate::error::map_sqlite_err;
use crate::models::{from_unix, to_unix, Preview, PreviewLevel};

struct PreviewRow {
    photo_id: String,
    level: i64,
    path: String,
    generated_at: i64,
}

fn row_to_raw(row: &rusqlite::Row) -> rusqlite::Result<PreviewRow> {
    Ok(PreviewRow {
        photo_id: row.get(0)?,
        level: row.get(1)?,
        path: row.get(2)?,
        generated_at: row.get(3)?,
    })
}

fn raw_to_preview(raw: PreviewRow) -> Result<Preview> {
    Ok(Preview {
        photo_id: raw.photo_id.parse()?,
        level: PreviewLevel::from_i64(raw.level)?,
        path: PathBuf::from(raw.path),
        generated_at: from_unix(raw.generated_at)?,
    })
}

/// Legt einen Vorschau-Eintrag an oder ersetzt ihn, falls für dieses Foto
/// und diese Stufe schon einer existiert (Primärschlüssel `(photo_id,
/// level)`, siehe Migration 1).
pub(crate) fn upsert(
    conn: &Connection,
    photo_id: PhotoId,
    level: PreviewLevel,
    path: &Path,
    generated_at: OffsetDateTime,
) -> Result<()> {
    conn.execute(
        "INSERT INTO previews (photo_id, level, path, generated_at) VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(photo_id, level) DO UPDATE SET path = excluded.path, generated_at = excluded.generated_at",
        params![photo_id.to_string(), level.as_i64(), path.to_string_lossy(), to_unix(generated_at)],
    )
    .map_err(map_sqlite_err)?;
    Ok(())
}

pub(crate) fn get(
    conn: &Connection,
    photo_id: PhotoId,
    level: PreviewLevel,
) -> Result<Option<Preview>> {
    let raw: Option<PreviewRow> = conn
        .query_row(
            "SELECT photo_id, level, path, generated_at FROM previews WHERE photo_id = ?1 AND level = ?2",
            params![photo_id.to_string(), level.as_i64()],
            row_to_raw,
        )
        .optional()
        .map_err(map_sqlite_err)?;
    raw.map(raw_to_preview).transpose()
}

pub(crate) fn list_for_photo(conn: &Connection, photo_id: PhotoId) -> Result<Vec<Preview>> {
    let mut stmt = conn
        .prepare("SELECT photo_id, level, path, generated_at FROM previews WHERE photo_id = ?1 ORDER BY level")
        .map_err(map_sqlite_err)?;
    let rows = stmt
        .query_map(params![photo_id.to_string()], row_to_raw)
        .map_err(map_sqlite_err)?;
    let mut result = Vec::new();
    for row in rows {
        result.push(raw_to_preview(row.map_err(map_sqlite_err)?)?);
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::migrations;
    use crate::models::NewPhoto;
    use crate::repository::{folders, photos};
    use std::path::Path as StdPath;

    fn setup() -> (Connection, PhotoId) {
        let conn = Connection::open_in_memory().expect("In-Memory-DB");
        migrations::apply(&conn).expect("Migration");
        let folder_id = folders::insert(
            &conn,
            StdPath::new("/fotos"),
            None,
            OffsetDateTime::now_utc(),
        )
        .expect("Ordner");
        let photo = NewPhoto {
            media_kind: "photo".to_string(),
            duration_ms: None,
            video_codec: None,
            has_audio: None,
            frame_rate: None,
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
    fn upsert_and_get_roundtrip() {
        let (conn, photo_id) = setup();
        let now = OffsetDateTime::now_utc()
            .replace_nanosecond(0)
            .expect("gültig");
        upsert(
            &conn,
            photo_id,
            PreviewLevel::Thumbnail,
            Path::new("/cache/ab/xyz_0.jpg"),
            now,
        )
        .expect("ok");

        let found = get(&conn, photo_id, PreviewLevel::Thumbnail)
            .expect("ok")
            .expect("sollte existieren");
        assert_eq!(found.path, PathBuf::from("/cache/ab/xyz_0.jpg"));
        assert_eq!(found.generated_at, now);
    }

    #[test]
    fn upsert_replaces_existing_entry_for_same_level() {
        let (conn, photo_id) = setup();
        let now = OffsetDateTime::now_utc()
            .replace_nanosecond(0)
            .expect("gültig");
        upsert(
            &conn,
            photo_id,
            PreviewLevel::Thumbnail,
            Path::new("/cache/old.jpg"),
            now,
        )
        .expect("ok");
        upsert(
            &conn,
            photo_id,
            PreviewLevel::Thumbnail,
            Path::new("/cache/new.jpg"),
            now,
        )
        .expect("ok");

        let all = list_for_photo(&conn, photo_id).expect("ok");
        assert_eq!(
            all.len(),
            1,
            "darf nicht zwei Zeilen für dieselbe Stufe anlegen"
        );
        assert_eq!(all[0].path, PathBuf::from("/cache/new.jpg"));
    }

    #[test]
    fn different_levels_coexist() {
        let (conn, photo_id) = setup();
        let now = OffsetDateTime::now_utc()
            .replace_nanosecond(0)
            .expect("gültig");
        upsert(
            &conn,
            photo_id,
            PreviewLevel::Thumbnail,
            Path::new("/cache/thumb.jpg"),
            now,
        )
        .expect("ok");
        upsert(
            &conn,
            photo_id,
            PreviewLevel::Standard,
            Path::new("/cache/std.jpg"),
            now,
        )
        .expect("ok");

        assert_eq!(list_for_photo(&conn, photo_id).expect("ok").len(), 2);
    }

    #[test]
    fn missing_preview_is_none() {
        let (conn, photo_id) = setup();
        assert!(get(&conn, photo_id, PreviewLevel::Full)
            .expect("ok")
            .is_none());
    }
}
