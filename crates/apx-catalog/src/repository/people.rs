//! SQL für `people`/`face_detections` (siehe `migrations/0011_people.sql`s
//! Moduldoku, Phase 13 Schritt 8).

use apx_core::{AppError, FaceDetectionId, PersonId, PhotoId, Result};
use rusqlite::{params, Connection};
use time::OffsetDateTime;

use crate::error::map_sqlite_err;
use crate::models::{
    embedding_distance, from_unix, to_unix, FaceDetection, FaceRect, Person,
    SAME_PERSON_EMBEDDING_THRESHOLD,
};

#[allow(clippy::type_complexity)]
fn row_to_face(
    row: &rusqlite::Row,
) -> rusqlite::Result<(
    String,
    String,
    Option<String>,
    i64,
    i64,
    i64,
    i64,
    String,
    i64,
)> {
    Ok((
        row.get(0)?,
        row.get(1)?,
        row.get(2)?,
        row.get(3)?,
        row.get(4)?,
        row.get(5)?,
        row.get(6)?,
        row.get(7)?,
        row.get(8)?,
    ))
}

const FACE_COLUMNS: &str =
    "id, photo_id, person_id, rect_left, rect_top, rect_right, rect_bottom, embedding_json, created_at";

#[allow(clippy::type_complexity)]
fn raw_to_face(
    raw: (
        String,
        String,
        Option<String>,
        i64,
        i64,
        i64,
        i64,
        String,
        i64,
    ),
) -> Result<FaceDetection> {
    let (id, photo_id, person_id, left, top, right, bottom, embedding_json, created_at) = raw;
    let embedding: Vec<f64> =
        serde_json::from_str(&embedding_json).map_err(|err| AppError::Database {
            message: format!("Embedding kaputt: {err}"),
        })?;
    Ok(FaceDetection {
        id: id.parse()?,
        photo_id: photo_id.parse()?,
        person_id: person_id.map(|s| s.parse()).transpose()?,
        rect_left: left,
        rect_top: top,
        rect_right: right,
        rect_bottom: bottom,
        embedding,
        created_at: from_unix(created_at)?,
    })
}

/// Ersetzt alle bisherigen Gesichtserkennungen von `photo_id` durch
/// `detections` (erneutes Erkennen überschreibt den alten Stand
/// vollständig statt zu duplizieren) und ordnet jedes neue Gesicht
/// automatisch der nächstliegenden bereits bekannten Person zu, wenn
/// deren Abstand unter [`SAME_PERSON_EMBEDDING_THRESHOLD`] liegt
/// (einfaches Schwellenwert-Clustering auf dem euklidischen Abstand,
/// wie von `PLAN.md` Phase 13 Schritt 8 vorgesehen — in-memory über
/// alle bereits zugeordneten Gesichter, keine dynamische SQL-Suche,
/// dieselbe Vereinfachung wie bei den intelligenten Sammlungen aus
/// Schritt 7, siehe `DECISIONS.md` ADR-0040-Nachtrag V/VI: Kataloge hier
/// sind Einzelnutzer-Bibliotheken). Gibt die neu angelegten Gesichter
/// zurück.
pub(crate) fn save_detections_for_photo(
    conn: &Connection,
    photo_id: PhotoId,
    detections: &[(FaceRect, Vec<f64>)],
    created_at: OffsetDateTime,
) -> Result<Vec<FaceDetection>> {
    conn.execute(
        "DELETE FROM face_detections WHERE photo_id = ?1",
        params![photo_id.to_string()],
    )
    .map_err(map_sqlite_err)?;

    // Alle bereits einer Person zugeordneten Gesichter einmal laden —
    // Grundlage für die Nächste-Nachbar-Zuordnung unten.
    let assigned = list_assigned(conn)?;

    let mut result = Vec::new();
    for ((left, top, right, bottom), embedding) in detections {
        let id = FaceDetectionId::new();
        let embedding_json = serde_json::to_string(embedding).map_err(|err| {
            AppError::validation(format!("Embedding nicht serialisierbar: {err}"))
        })?;

        let matched_person = assigned
            .iter()
            .map(|face| {
                (
                    face.person_id,
                    embedding_distance(embedding, &face.embedding),
                )
            })
            .filter(|(person_id, distance)| {
                person_id.is_some() && *distance < SAME_PERSON_EMBEDDING_THRESHOLD
            })
            .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
            .and_then(|(person_id, _)| person_id);

        conn.execute(
            "INSERT INTO face_detections (id, photo_id, person_id, rect_left, rect_top, rect_right, rect_bottom, embedding_json, created_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                id.to_string(),
                photo_id.to_string(),
                matched_person.map(|p| p.to_string()),
                left,
                top,
                right,
                bottom,
                embedding_json,
                to_unix(created_at),
            ],
        )
        .map_err(map_sqlite_err)?;

        result.push(FaceDetection {
            id,
            photo_id,
            person_id: matched_person,
            rect_left: *left,
            rect_top: *top,
            rect_right: *right,
            rect_bottom: *bottom,
            embedding: embedding.clone(),
            created_at,
        });
    }
    Ok(result)
}

fn list_assigned(conn: &Connection) -> Result<Vec<FaceDetection>> {
    let sql = format!("SELECT {FACE_COLUMNS} FROM face_detections WHERE person_id IS NOT NULL");
    let mut stmt = conn.prepare(&sql).map_err(map_sqlite_err)?;
    let rows = stmt.query_map([], row_to_face).map_err(map_sqlite_err)?;
    let mut result = Vec::new();
    for row in rows {
        result.push(raw_to_face(row.map_err(map_sqlite_err)?)?);
    }
    Ok(result)
}

pub(crate) fn list_for_photo(conn: &Connection, photo_id: PhotoId) -> Result<Vec<FaceDetection>> {
    let sql = format!(
        "SELECT {FACE_COLUMNS} FROM face_detections WHERE photo_id = ?1 ORDER BY rect_left"
    );
    let mut stmt = conn.prepare(&sql).map_err(map_sqlite_err)?;
    let rows = stmt
        .query_map(params![photo_id.to_string()], row_to_face)
        .map_err(map_sqlite_err)?;
    let mut result = Vec::new();
    for row in rows {
        result.push(raw_to_face(row.map_err(map_sqlite_err)?)?);
    }
    Ok(result)
}

pub(crate) fn list_for_person(
    conn: &Connection,
    person_id: PersonId,
) -> Result<Vec<FaceDetection>> {
    let sql = format!(
        "SELECT {FACE_COLUMNS} FROM face_detections WHERE person_id = ?1 ORDER BY created_at"
    );
    let mut stmt = conn.prepare(&sql).map_err(map_sqlite_err)?;
    let rows = stmt
        .query_map(params![person_id.to_string()], row_to_face)
        .map_err(map_sqlite_err)?;
    let mut result = Vec::new();
    for row in rows {
        result.push(raw_to_face(row.map_err(map_sqlite_err)?)?);
    }
    Ok(result)
}

/// Ordnet ein einzelnes Gesicht manuell einer Person zu (überschreibt eine
/// vorherige automatische oder manuelle Zuordnung) — bekommt die Person
/// noch kein Titelbild, wird dieses Gesicht eines.
pub(crate) fn assign_face(
    conn: &Connection,
    face_id: FaceDetectionId,
    person_id: PersonId,
) -> Result<()> {
    let changed = conn
        .execute(
            "UPDATE face_detections SET person_id = ?2 WHERE id = ?1",
            params![face_id.to_string(), person_id.to_string()],
        )
        .map_err(map_sqlite_err)?;
    if changed == 0 {
        return Err(AppError::not_found("Gesicht", face_id.to_string()));
    }
    conn.execute(
        "UPDATE people SET cover_face_id = ?2 WHERE id = ?1 AND cover_face_id IS NULL",
        params![person_id.to_string(), face_id.to_string()],
    )
    .map_err(map_sqlite_err)?;
    Ok(())
}

pub(crate) fn unassign_face(conn: &Connection, face_id: FaceDetectionId) -> Result<()> {
    let changed = conn
        .execute(
            "UPDATE face_detections SET person_id = NULL WHERE id = ?1",
            params![face_id.to_string()],
        )
        .map_err(map_sqlite_err)?;
    if changed == 0 {
        return Err(AppError::not_found("Gesicht", face_id.to_string()));
    }
    Ok(())
}

pub(crate) fn create_person(
    conn: &Connection,
    name: Option<&str>,
    created_at: OffsetDateTime,
) -> Result<PersonId> {
    let id = PersonId::new();
    conn.execute(
        "INSERT INTO people (id, name, cover_face_id, created_at) VALUES (?1, ?2, NULL, ?3)",
        params![id.to_string(), name, to_unix(created_at)],
    )
    .map_err(map_sqlite_err)?;
    Ok(id)
}

pub(crate) fn rename_person(conn: &Connection, id: PersonId, name: Option<&str>) -> Result<()> {
    let changed = conn
        .execute(
            "UPDATE people SET name = ?2 WHERE id = ?1",
            params![id.to_string(), name],
        )
        .map_err(map_sqlite_err)?;
    if changed == 0 {
        return Err(AppError::not_found("Person", id.to_string()));
    }
    Ok(())
}

pub(crate) fn delete_person(conn: &Connection, id: PersonId) -> Result<()> {
    let changed = conn
        .execute("DELETE FROM people WHERE id = ?1", params![id.to_string()])
        .map_err(map_sqlite_err)?;
    if changed == 0 {
        return Err(AppError::not_found("Person", id.to_string()));
    }
    Ok(())
}

pub(crate) fn list_people(conn: &Connection) -> Result<Vec<Person>> {
    let mut stmt = conn
        .prepare("SELECT id, name, cover_face_id, created_at FROM people ORDER BY created_at")
        .map_err(map_sqlite_err)?;
    let rows = stmt
        .query_map([], |row| {
            let id: String = row.get(0)?;
            let name: Option<String> = row.get(1)?;
            let cover_face_id: Option<String> = row.get(2)?;
            let created_at: i64 = row.get(3)?;
            Ok((id, name, cover_face_id, created_at))
        })
        .map_err(map_sqlite_err)?;
    let mut result = Vec::new();
    for row in rows {
        let (id, name, cover_face_id, created_at) = row.map_err(map_sqlite_err)?;
        result.push(Person {
            id: id.parse()?,
            name,
            cover_face_id: cover_face_id.map(|s| s.parse()).transpose()?,
            created_at: from_unix(created_at)?,
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
        let photo_id = photos::upsert(&conn, &photo, OffsetDateTime::now_utc())
            .expect("Foto anlegen")
            .0;
        (conn, photo_id)
    }

    /// Zwei Fotos derselben Person (Embeddings nah beieinander) — das
    /// zweite Gesicht wird automatisch derselben Person zugeordnet wie
    /// das erste, sobald dieses manuell benannt wurde.
    #[test]
    fn new_detection_is_auto_assigned_to_the_nearest_matching_person() {
        let (conn, photo_a) = setup();
        let embedding_a = vec![0.0f64; 128];
        let mut embedding_similar = embedding_a.clone();
        embedding_similar[0] = 0.1; // Abstand 0.1, deutlich unter der Schwelle 0.6
        let mut embedding_different = vec![5.0f64; 128]; // Abstand groß
        embedding_different[0] = 5.0;

        let faces = save_detections_for_photo(
            &conn,
            photo_a,
            &[((0, 0, 10, 10), embedding_a)],
            OffsetDateTime::now_utc(),
        )
        .expect("erstes Gesicht speichern");
        let person_id =
            create_person(&conn, Some("Alice"), OffsetDateTime::now_utc()).expect("Person anlegen");
        assign_face(&conn, faces[0].id, person_id).expect("zuordnen");

        // Zweites Foto derselben Person — automatisch zugeordnet, weil
        // `embedding_similar` innerhalb der Schwelle zum bereits
        // zugeordneten Gesicht liegt.
        let (conn2, photo_b) = (&conn, {
            let folder_id = photos::get(&conn, photo_a).expect("ok").folder_id;
            let photo = NewPhoto {
                folder_id,
                filename: "b.cr2".to_string(),
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
            photos::upsert(&conn, &photo, OffsetDateTime::now_utc())
                .expect("ok")
                .0
        });

        let faces_b = save_detections_for_photo(
            conn2,
            photo_b,
            &[
                ((0, 0, 10, 10), embedding_similar),
                ((20, 20, 30, 30), embedding_different),
            ],
            OffsetDateTime::now_utc(),
        )
        .expect("zweites Foto speichern");

        assert_eq!(
            faces_b[0].person_id,
            Some(person_id),
            "ähnliches Embedding sollte automatisch zugeordnet werden"
        );
        assert_eq!(
            faces_b[1].person_id, None,
            "unähnliches Embedding sollte unzugeordnet bleiben"
        );
    }

    #[test]
    fn rename_and_delete_person_roundtrip() {
        let (conn, _photo) = setup();
        let id = create_person(&conn, None, OffsetDateTime::now_utc()).expect("anlegen");
        assert_eq!(list_people(&conn).expect("liste")[0].name, None);

        rename_person(&conn, id, Some("Bob")).expect("umbenennen");
        assert_eq!(
            list_people(&conn).expect("liste")[0].name,
            Some("Bob".to_string())
        );

        delete_person(&conn, id).expect("löschen");
        assert!(list_people(&conn).expect("liste").is_empty());
    }
}
