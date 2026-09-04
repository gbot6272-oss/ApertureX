//! SQL für `stacks`/`stack_photos` (siehe
//! `migrations/0007_library_backlog.sql`s Moduldoku, Phase 9 Schritt 1).

use apx_core::{AppError, PhotoId, Result, StackId};
use rusqlite::{params, Connection};
use time::OffsetDateTime;

use crate::error::map_sqlite_err;
use crate::models::{from_unix, to_unix, Stack};
use crate::repository::photos::get as get_photo;

/// Legt einen Stapel aus `photo_ids` an (Reihenfolge = Einfüge-
/// Reihenfolge), Titelbild ist standardmäßig das erste Foto.
pub(crate) fn create(
    conn: &Connection,
    name: Option<&str>,
    photo_ids: &[PhotoId],
    created_at: OffsetDateTime,
) -> Result<StackId> {
    if photo_ids.is_empty() {
        return Err(AppError::validation(
            "Ein Stapel braucht mindestens ein Foto".to_string(),
        ));
    }
    let id = StackId::new();
    conn.execute(
        "INSERT INTO stacks (id, name, cover_photo_id, created_at) VALUES (?1, ?2, ?3, ?4)",
        params![
            id.to_string(),
            name,
            photo_ids[0].to_string(),
            to_unix(created_at)
        ],
    )
    .map_err(map_sqlite_err)?;
    for (position, photo_id) in photo_ids.iter().enumerate() {
        conn.execute(
            "INSERT INTO stack_photos (stack_id, photo_id, position) VALUES (?1, ?2, ?3)",
            params![id.to_string(), photo_id.to_string(), position as i64],
        )
        .map_err(map_sqlite_err)?;
    }
    Ok(id)
}

pub(crate) fn delete(conn: &Connection, id: StackId) -> Result<()> {
    let changed = conn
        .execute("DELETE FROM stacks WHERE id = ?1", params![id.to_string()])
        .map_err(map_sqlite_err)?;
    if changed == 0 {
        return Err(AppError::not_found("Stapel", id.to_string()));
    }
    Ok(())
}

pub(crate) fn set_cover(conn: &Connection, id: StackId, cover_photo_id: PhotoId) -> Result<()> {
    let is_member: bool = conn
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM stack_photos WHERE stack_id = ?1 AND photo_id = ?2)",
            params![id.to_string(), cover_photo_id.to_string()],
            |row| row.get(0),
        )
        .map_err(map_sqlite_err)?;
    if !is_member {
        return Err(AppError::validation(
            "Das Titelbild muss Mitglied des Stapels sein".to_string(),
        ));
    }
    conn.execute(
        "UPDATE stacks SET cover_photo_id = ?2 WHERE id = ?1",
        params![id.to_string(), cover_photo_id.to_string()],
    )
    .map_err(map_sqlite_err)?;
    Ok(())
}

pub(crate) fn list_all(conn: &Connection) -> Result<Vec<Stack>> {
    let mut stmt = conn
        .prepare("SELECT id, name, cover_photo_id, created_at FROM stacks ORDER BY created_at")
        .map_err(map_sqlite_err)?;
    let rows = stmt
        .query_map([], |row| {
            let id: String = row.get(0)?;
            let name: Option<String> = row.get(1)?;
            let cover_photo_id: Option<String> = row.get(2)?;
            let created_at: i64 = row.get(3)?;
            Ok((id, name, cover_photo_id, created_at))
        })
        .map_err(map_sqlite_err)?;

    let mut result = Vec::new();
    for row in rows {
        let (id, name, cover_photo_id, created_at) = row.map_err(map_sqlite_err)?;
        let stack_id: StackId = id.parse()?;
        let mut photo_stmt = conn
            .prepare("SELECT photo_id FROM stack_photos WHERE stack_id = ?1 ORDER BY position")
            .map_err(map_sqlite_err)?;
        let photo_ids: Vec<PhotoId> = photo_stmt
            .query_map(params![stack_id.to_string()], |row| row.get::<_, String>(0))
            .map_err(map_sqlite_err)?
            .collect::<rusqlite::Result<Vec<String>>>()
            .map_err(map_sqlite_err)?
            .into_iter()
            .map(|s| s.parse())
            .collect::<std::result::Result<Vec<_>, _>>()?;
        result.push(Stack {
            id: stack_id,
            name,
            cover_photo_id: cover_photo_id.map(|s| s.parse()).transpose()?,
            created_at: from_unix(created_at)?,
            photo_ids,
        });
    }
    Ok(result)
}

/// Gruppiert `photo_ids` automatisch: nach `captured_at` sortiert,
/// aufeinanderfolgende Fotos mit höchstens `window_seconds` Abstand
/// landen im selben Stapel. Fotos ohne `captured_at` oder allein
/// stehende Fotos (kein Nachbar innerhalb des Fensters) bleiben
/// unverstapelt — ein Stapel mit nur einem Foto wäre nutzlos.
pub(crate) fn auto_stack_by_time(
    conn: &Connection,
    photo_ids: &[PhotoId],
    window_seconds: i64,
    created_at: OffsetDateTime,
) -> Result<Vec<StackId>> {
    let mut timed: Vec<(PhotoId, i64)> = Vec::new();
    for &id in photo_ids {
        if let Some(captured) = get_photo(conn, id)?.captured_at {
            timed.push((id, captured.unix_timestamp()));
        }
    }
    timed.sort_by_key(|(_, ts)| *ts);

    let mut stacks = Vec::new();
    let mut current_group: Vec<PhotoId> = Vec::new();
    let mut last_ts: Option<i64> = None;

    let flush = |group: &mut Vec<PhotoId>, stacks: &mut Vec<StackId>| -> Result<()> {
        if group.len() >= 2 {
            stacks.push(create(conn, None, group, created_at)?);
        }
        group.clear();
        Ok(())
    };

    for (id, ts) in timed {
        match last_ts {
            Some(prev) if ts - prev <= window_seconds => current_group.push(id),
            _ => {
                flush(&mut current_group, &mut stacks)?;
                current_group.push(id);
            }
        }
        last_ts = Some(ts);
    }
    flush(&mut current_group, &mut stacks)?;

    Ok(stacks)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::migrations;
    use crate::models::NewPhoto;
    use crate::repository::{folders, photos};
    use std::path::Path;

    fn setup() -> (Connection, apx_core::FolderId) {
        let conn = Connection::open_in_memory().expect("In-Memory-DB");
        conn.execute_batch("PRAGMA foreign_keys = ON")
            .expect("FKs an");
        migrations::apply(&conn).expect("Migration");
        let folder_id =
            folders::insert(&conn, Path::new("/fotos"), None, OffsetDateTime::now_utc())
                .expect("Ordner");
        (conn, folder_id)
    }

    fn photo_at(
        conn: &Connection,
        folder_id: apx_core::FolderId,
        filename: &str,
        captured_at: Option<OffsetDateTime>,
    ) -> PhotoId {
        let new_photo = NewPhoto {
            media_kind: "photo".to_string(),
            duration_ms: None,
            video_codec: None,
            has_audio: None,
            frame_rate: None,
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
            camera_model: None,
            lens: None,
            iso: None,
            shutter: None,
            aperture: None,
            focal_length: None,
            captured_at,
            gps_lat: None,
            gps_lon: None,
        };
        photos::upsert(conn, &new_photo, OffsetDateTime::now_utc())
            .expect("Foto anlegen")
            .0
    }

    #[test]
    fn create_then_list_roundtrips_members_and_default_cover() {
        let (conn, folder_id) = setup();
        let a = photo_at(&conn, folder_id, "a.cr2", None);
        let b = photo_at(&conn, folder_id, "b.cr2", None);

        let stack_id =
            create(&conn, Some("Serie"), &[a, b], OffsetDateTime::now_utc()).expect("anlegen");
        let stacks = list_all(&conn).expect("liste");
        assert_eq!(stacks.len(), 1);
        assert_eq!(stacks[0].id, stack_id);
        assert_eq!(stacks[0].photo_ids, vec![a, b]);
        assert_eq!(stacks[0].cover_photo_id, Some(a));
    }

    #[test]
    fn set_cover_rejects_non_member() {
        let (conn, folder_id) = setup();
        let a = photo_at(&conn, folder_id, "a.cr2", None);
        let outsider = photo_at(&conn, folder_id, "outsider.cr2", None);
        let stack_id = create(&conn, None, &[a], OffsetDateTime::now_utc()).expect("anlegen");
        assert!(set_cover(&conn, stack_id, outsider).is_err());
    }

    #[test]
    fn delete_removes_stack_and_memberships() {
        let (conn, folder_id) = setup();
        let a = photo_at(&conn, folder_id, "a.cr2", None);
        let stack_id = create(&conn, None, &[a], OffsetDateTime::now_utc()).expect("anlegen");

        delete(&conn, stack_id).expect("löschen");
        assert!(list_all(&conn).expect("liste").is_empty());
    }

    #[test]
    fn auto_stack_groups_close_captures_and_leaves_lone_photos_unstacked() {
        let (conn, folder_id) = setup();
        let base = OffsetDateTime::now_utc()
            .replace_nanosecond(0)
            .expect("gültig");
        let a = photo_at(&conn, folder_id, "a.cr2", Some(base));
        let b = photo_at(
            &conn,
            folder_id,
            "b.cr2",
            Some(base + time::Duration::seconds(2)),
        );
        // Weit entfernt — bleibt allein, kein Stapel für ein einzelnes Foto.
        let c = photo_at(
            &conn,
            folder_id,
            "c.cr2",
            Some(base + time::Duration::hours(5)),
        );
        // Ohne Aufnahmezeit — wird ignoriert.
        let d = photo_at(&conn, folder_id, "d.cr2", None);

        let stack_ids =
            auto_stack_by_time(&conn, &[a, b, c, d], 10, OffsetDateTime::now_utc()).expect("ok");
        assert_eq!(stack_ids.len(), 1);

        let stacks = list_all(&conn).expect("liste");
        assert_eq!(stacks.len(), 1);
        assert_eq!(stacks[0].photo_ids, vec![a, b]);
    }
}
