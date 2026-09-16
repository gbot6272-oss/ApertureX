//! Notizen am Foto (Phase 32 F6, siehe `migrations/0013_photo_notes.sql`).
//!
//! Eine Notiz hängt an einer Stelle im Bild — normierte Koordinaten
//! (0..1) aufs unbeschnittene Original, siehe die Begründung in der
//! Migration. `done` macht daraus eine abhakbare Aufgabe.

use apx_core::{AppError, PhotoId, Result};
use rusqlite::{params, Connection};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::error::map_sqlite_err;
use crate::models::{from_unix, to_unix, PhotoNote};

const SELECT_COLUMNS: &str = "id, photo_id, x, y, body, done, created_at, updated_at";

fn row_to_note(
    row: &rusqlite::Row,
) -> rusqlite::Result<(String, String, f64, f64, String, i64, i64, i64)> {
    Ok((
        row.get(0)?,
        row.get(1)?,
        row.get(2)?,
        row.get(3)?,
        row.get(4)?,
        row.get(5)?,
        row.get(6)?,
        row.get(7)?,
    ))
}

fn build(raw: (String, String, f64, f64, String, i64, i64, i64)) -> Result<PhotoNote> {
    Ok(PhotoNote {
        id: raw.0.parse()?,
        photo_id: raw.1.parse()?,
        x: raw.2,
        y: raw.3,
        body: raw.4,
        done: raw.5 != 0,
        created_at: from_unix(raw.6)?,
        updated_at: from_unix(raw.7)?,
    })
}

/// Verwirft Koordinaten außerhalb des Bildes.
///
/// Ein Klick kann durch Rundung minimal daneben landen (0.0000001 unter
/// null); alles darüber hinaus ist ein Fehler im Aufrufer und wird
/// abgelehnt statt stillschweigend geklemmt — eine Notiz, die
/// irgendwohin rutscht, wäre schlimmer als eine, die gar nicht erst
/// angelegt wird.
fn validate_position(x: f64, y: f64) -> Result<(f64, f64)> {
    const TOLERANCE: f64 = 1e-6;
    if !(-TOLERANCE..=1.0 + TOLERANCE).contains(&x) || !(-TOLERANCE..=1.0 + TOLERANCE).contains(&y)
    {
        return Err(AppError::validation(format!(
            "Notiz-Position muss zwischen 0 und 1 liegen, war ({x}, {y})"
        )));
    }
    Ok((x.clamp(0.0, 1.0), y.clamp(0.0, 1.0)))
}

pub(crate) fn create(
    conn: &Connection,
    photo_id: PhotoId,
    x: f64,
    y: f64,
    body: &str,
    now: OffsetDateTime,
) -> Result<PhotoNote> {
    let (x, y) = validate_position(x, y)?;
    let body = body.trim();
    if body.is_empty() {
        return Err(AppError::validation("Notiz darf nicht leer sein"));
    }
    let id = Uuid::now_v7();
    conn.execute(
        "INSERT INTO photo_notes (id, photo_id, x, y, body, done, created_at, updated_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, 0, ?6, ?6)",
        params![
            id.to_string(),
            photo_id.to_string(),
            x,
            y,
            body,
            to_unix(now)
        ],
    )
    .map_err(map_sqlite_err)?;
    get(conn, &id.to_string())
}

pub(crate) fn get(conn: &Connection, note_id: &str) -> Result<PhotoNote> {
    let sql = format!("SELECT {SELECT_COLUMNS} FROM photo_notes WHERE id = ?1");
    let raw = conn
        .query_row(&sql, params![note_id], row_to_note)
        .map_err(|err| match err {
            rusqlite::Error::QueryReturnedNoRows => {
                AppError::not_found("Notiz", note_id.to_string())
            }
            other => map_sqlite_err(other),
        })?;
    build(raw)
}

/// Alle Notizen eines Fotos, älteste zuerst — die Reihenfolge, in der
/// sie entstanden sind, ist die, in der man sie liest.
pub(crate) fn list_for_photo(conn: &Connection, photo_id: PhotoId) -> Result<Vec<PhotoNote>> {
    let sql = format!(
        "SELECT {SELECT_COLUMNS} FROM photo_notes WHERE photo_id = ?1 ORDER BY created_at ASC, id ASC"
    );
    let mut stmt = conn.prepare(&sql).map_err(map_sqlite_err)?;
    let rows = stmt
        .query_map(params![photo_id.to_string()], row_to_note)
        .map_err(map_sqlite_err)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(map_sqlite_err)?;
    rows.into_iter().map(build).collect()
}

/// Ändert Text und/oder Erledigt-Haken und/oder Position. `None` lässt
/// das jeweilige Feld unverändert — so kann die Oberfläche einen Pin
/// verschieben, ohne den Text erneut zu schicken.
pub(crate) fn update(
    conn: &Connection,
    note_id: &str,
    body: Option<&str>,
    done: Option<bool>,
    position: Option<(f64, f64)>,
    now: OffsetDateTime,
) -> Result<PhotoNote> {
    let existing = get(conn, note_id)?;
    let body = match body {
        Some(body) => {
            let trimmed = body.trim();
            if trimmed.is_empty() {
                return Err(AppError::validation("Notiz darf nicht leer sein"));
            }
            trimmed.to_string()
        }
        None => existing.body,
    };
    let done = done.unwrap_or(existing.done);
    let (x, y) = match position {
        Some((x, y)) => validate_position(x, y)?,
        None => (existing.x, existing.y),
    };
    conn.execute(
        "UPDATE photo_notes SET body = ?2, done = ?3, x = ?4, y = ?5, updated_at = ?6 WHERE id = ?1",
        params![note_id, body, done as i64, x, y, to_unix(now)],
    )
    .map_err(map_sqlite_err)?;
    get(conn, note_id)
}

pub(crate) fn delete(conn: &Connection, note_id: &str) -> Result<()> {
    let changed = conn
        .execute("DELETE FROM photo_notes WHERE id = ?1", params![note_id])
        .map_err(map_sqlite_err)?;
    if changed == 0 {
        return Err(AppError::not_found("Notiz", note_id.to_string()));
    }
    Ok(())
}

/// Anzahl offener (nicht erledigter) Notizen je Foto — Grundlage für die
/// Markierung im Raster, ohne für jede Kachel einzeln abzufragen.
pub(crate) fn open_counts(conn: &Connection) -> Result<Vec<(PhotoId, u64)>> {
    let mut stmt = conn
        .prepare("SELECT photo_id, COUNT(*) FROM photo_notes WHERE done = 0 GROUP BY photo_id")
        .map_err(map_sqlite_err)?;
    let rows = stmt
        .query_map([], |row| {
            let photo_id: String = row.get(0)?;
            let count: i64 = row.get(1)?;
            Ok((photo_id, count as u64))
        })
        .map_err(map_sqlite_err)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(map_sqlite_err)?;
    rows.into_iter()
        .map(|(id, count)| Ok((id.parse()?, count)))
        .collect()
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
        migrations::apply(&conn).expect("Migration");
        let folder_id =
            folders::insert(&conn, Path::new("/fotos"), None, OffsetDateTime::now_utc())
                .expect("Ordner");
        let photo = NewPhoto {
            media_kind: "photo".to_string(),
            duration_ms: None,
            video_codec: None,
            has_audio: None,
            frame_rate: None,
            folder_id,
            filename: "IMG_0001.CR2".to_string(),
            file_size: 1000,
            file_mtime: OffsetDateTime::now_utc()
                .replace_nanosecond(0)
                .expect("gültig"),
            content_hash: None,
            width: Some(6000),
            height: Some(4000),
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
    fn create_and_list_keeps_position_text_and_open_state() {
        let (conn, photo_id) = setup();
        let note = create(
            &conn,
            photo_id,
            0.25,
            0.75,
            "  Staubfleck hier weg  ",
            OffsetDateTime::now_utc(),
        )
        .expect("anlegen");
        assert_eq!(note.body, "Staubfleck hier weg", "Leerraum wird getrimmt");
        assert!(!note.done);
        assert_eq!(note.x, 0.25);
        assert_eq!(note.y, 0.75);

        let notes = list_for_photo(&conn, photo_id).expect("liste");
        assert_eq!(notes.len(), 1);
        assert_eq!(notes[0].id, note.id);
    }

    #[test]
    fn a_position_outside_the_image_is_rejected_instead_of_clamped() {
        // Geklemmt läge die Notiz stillschweigend woanders als gemeint.
        let (conn, photo_id) = setup();
        assert!(create(
            &conn,
            photo_id,
            1.5,
            0.5,
            "daneben",
            OffsetDateTime::now_utc()
        )
        .is_err());
        assert!(create(
            &conn,
            photo_id,
            0.5,
            -0.4,
            "daneben",
            OffsetDateTime::now_utc()
        )
        .is_err());
        assert!(list_for_photo(&conn, photo_id).expect("liste").is_empty());
    }

    #[test]
    fn an_empty_note_is_rejected_on_create_and_on_update() {
        let (conn, photo_id) = setup();
        assert!(create(&conn, photo_id, 0.5, 0.5, "   ", OffsetDateTime::now_utc()).is_err());
        let note =
            create(&conn, photo_id, 0.5, 0.5, "Text", OffsetDateTime::now_utc()).expect("anlegen");
        assert!(update(
            &conn,
            &note.id.to_string(),
            Some("  "),
            None,
            None,
            OffsetDateTime::now_utc()
        )
        .is_err());
    }

    #[test]
    fn update_changes_only_what_is_given() {
        let (conn, photo_id) = setup();
        let note = create(
            &conn,
            photo_id,
            0.2,
            0.2,
            "Erster Text",
            OffsetDateTime::now_utc(),
        )
        .expect("anlegen");
        let id = note.id.to_string();

        // Nur abhaken — Text und Position bleiben.
        let done = update(
            &conn,
            &id,
            None,
            Some(true),
            None,
            OffsetDateTime::now_utc(),
        )
        .expect("abhaken");
        assert!(done.done);
        assert_eq!(done.body, "Erster Text");
        assert_eq!(done.x, 0.2);

        // Nur verschieben — Text und Haken bleiben.
        let moved = update(
            &conn,
            &id,
            None,
            None,
            Some((0.8, 0.9)),
            OffsetDateTime::now_utc(),
        )
        .expect("verschieben");
        assert_eq!((moved.x, moved.y), (0.8, 0.9));
        assert!(moved.done);
        assert_eq!(moved.body, "Erster Text");
    }

    #[test]
    fn open_counts_only_counts_unfinished_notes() {
        let (conn, photo_id) = setup();
        let first = create(
            &conn,
            photo_id,
            0.1,
            0.1,
            "offen",
            OffsetDateTime::now_utc(),
        )
        .expect("anlegen");
        create(
            &conn,
            photo_id,
            0.2,
            0.2,
            "auch offen",
            OffsetDateTime::now_utc(),
        )
        .expect("anlegen");
        assert_eq!(open_counts(&conn).expect("zählen")[0].1, 2);

        update(
            &conn,
            &first.id.to_string(),
            None,
            Some(true),
            None,
            OffsetDateTime::now_utc(),
        )
        .expect("abhaken");
        assert_eq!(open_counts(&conn).expect("zählen")[0].1, 1);
    }

    #[test]
    fn deleting_the_photo_takes_its_notes_with_it() {
        let (conn, photo_id) = setup();
        create(
            &conn,
            photo_id,
            0.5,
            0.5,
            "Notiz",
            OffsetDateTime::now_utc(),
        )
        .expect("anlegen");
        conn.execute(
            "DELETE FROM photos WHERE id = ?1",
            params![photo_id.to_string()],
        )
        .expect("Foto löschen");
        assert!(list_for_photo(&conn, photo_id).expect("liste").is_empty());
        assert!(open_counts(&conn).expect("zählen").is_empty());
    }

    #[test]
    fn deleting_an_unknown_note_is_an_error_not_a_silent_no_op() {
        let (conn, _) = setup();
        assert!(delete(&conn, &Uuid::now_v7().to_string()).is_err());
    }
}
