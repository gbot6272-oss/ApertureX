//! SQL für `snapshots` (siehe `migrations/0005_snapshots.sql`,
//! `DECISIONS.md` ADR-0032). Anders als `edit_history` (`edits.rs`) ist
//! das kein linearer, sich selbst beschneidender Verlauf — ein
//! Schnappschuss trägt seine eigene Kopie des EDL und bleibt bestehen,
//! bis er ausdrücklich gelöscht wird (siehe Migrationsdatei für die
//! Begründung dieser Abgrenzung).

use apx_core::{EdlEnvelope, PhotoId, Result, SnapshotId};
use rusqlite::{params, Connection};
use time::OffsetDateTime;

use crate::error::map_sqlite_err;
use crate::models::{from_unix, to_unix, Snapshot};

struct SnapshotRow {
    id: String,
    photo_id: String,
    name: String,
    edl_json: String,
    created_at: i64,
}

fn row_to_raw(row: &rusqlite::Row) -> rusqlite::Result<SnapshotRow> {
    Ok(SnapshotRow {
        id: row.get(0)?,
        photo_id: row.get(1)?,
        name: row.get(2)?,
        edl_json: row.get(3)?,
        created_at: row.get(4)?,
    })
}

fn raw_to_snapshot(raw: SnapshotRow) -> Result<Snapshot> {
    Ok(Snapshot {
        id: raw.id.parse()?,
        photo_id: raw.photo_id.parse()?,
        name: raw.name,
        edl: EdlEnvelope::from_json_str(&raw.edl_json)?,
        created_at: from_unix(raw.created_at)?,
    })
}

const SELECT_COLUMNS: &str = "id, photo_id, name, edl_json, created_at";

/// Legt einen neuen Schnappschuss mit einer eigenen Kopie von `edl` an.
pub(crate) fn create(
    conn: &Connection,
    photo_id: PhotoId,
    name: &str,
    edl: &EdlEnvelope,
    created_at: OffsetDateTime,
) -> Result<SnapshotId> {
    let id = SnapshotId::new();
    let edl_json = edl.to_json_string()?;
    conn.execute(
        "INSERT INTO snapshots (id, photo_id, name, edl_json, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            id.to_string(),
            photo_id.to_string(),
            name,
            edl_json,
            to_unix(created_at)
        ],
    )
    .map_err(map_sqlite_err)?;
    Ok(id)
}

/// Alle Schnappschüsse eines Fotos, älteste zuerst.
pub(crate) fn list(conn: &Connection, photo_id: PhotoId) -> Result<Vec<Snapshot>> {
    let mut stmt = conn
        .prepare(&format!(
            "SELECT {SELECT_COLUMNS} FROM snapshots WHERE photo_id = ?1 ORDER BY created_at"
        ))
        .map_err(map_sqlite_err)?;
    let rows = stmt
        .query_map(params![photo_id.to_string()], row_to_raw)
        .map_err(map_sqlite_err)?;
    let mut result = Vec::new();
    for row in rows {
        result.push(raw_to_snapshot(row.map_err(map_sqlite_err)?)?);
    }
    Ok(result)
}

pub(crate) fn rename(conn: &Connection, snapshot_id: SnapshotId, name: &str) -> Result<()> {
    conn.execute(
        "UPDATE snapshots SET name = ?1 WHERE id = ?2",
        params![name, snapshot_id.to_string()],
    )
    .map_err(map_sqlite_err)?;
    Ok(())
}

pub(crate) fn delete(conn: &Connection, snapshot_id: SnapshotId) -> Result<()> {
    conn.execute(
        "DELETE FROM snapshots WHERE id = ?1",
        params![snapshot_id.to_string()],
    )
    .map_err(map_sqlite_err)?;
    Ok(())
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

    fn sample_edl(marker: f64) -> EdlEnvelope {
        EdlEnvelope::new(1, serde_json::json!({ "exposure_ev": marker }))
    }

    #[test]
    fn a_photo_without_snapshots_has_an_empty_list() {
        let (conn, photo_id) = setup();
        assert_eq!(list(&conn, photo_id).expect("ok"), Vec::new());
    }

    #[test]
    fn create_then_list_roundtrips_name_and_edl() {
        let (conn, photo_id) = setup();
        create(
            &conn,
            photo_id,
            "Vor Retusche",
            &sample_edl(0.5),
            OffsetDateTime::now_utc(),
        )
        .expect("anlegen");
        let snapshots = list(&conn, photo_id).expect("liste");
        assert_eq!(snapshots.len(), 1);
        assert_eq!(snapshots[0].name, "Vor Retusche");
        assert_eq!(snapshots[0].edl, sample_edl(0.5));
    }

    #[test]
    fn snapshots_survive_edit_history_being_rewritten() {
        // Der eigentliche Grund für die eigene Tabelle (siehe Moduldoku):
        // ein Schnappschuss verschwindet nicht, wenn `edit_history` durch
        // neue Bearbeitungen umgeschrieben wird.
        let (conn, photo_id) = setup();
        let snapshot_id = create(
            &conn,
            photo_id,
            "Referenz",
            &sample_edl(1.0),
            OffsetDateTime::now_utc(),
        )
        .expect("anlegen");

        crate::repository::edits::commit(
            &conn,
            photo_id,
            &sample_edl(2.0),
            None,
            OffsetDateTime::now_utc(),
        )
        .expect("commit 1");
        crate::repository::edits::undo(&conn, photo_id).expect("undo");
        crate::repository::edits::commit(
            &conn,
            photo_id,
            &sample_edl(3.0),
            None,
            OffsetDateTime::now_utc(),
        )
        .expect("commit 2 (verwirft commit 1)");

        let snapshots = list(&conn, photo_id).expect("liste");
        assert_eq!(snapshots.len(), 1);
        assert_eq!(snapshots[0].id, snapshot_id);
        assert_eq!(snapshots[0].edl, sample_edl(1.0));
    }

    #[test]
    fn rename_and_delete_work() {
        let (conn, photo_id) = setup();
        let snapshot_id = create(
            &conn,
            photo_id,
            "Alt",
            &sample_edl(0.0),
            OffsetDateTime::now_utc(),
        )
        .expect("anlegen");

        rename(&conn, snapshot_id, "Neu").expect("umbenennen");
        assert_eq!(list(&conn, photo_id).expect("liste")[0].name, "Neu");

        delete(&conn, snapshot_id).expect("löschen");
        assert_eq!(list(&conn, photo_id).expect("liste"), Vec::new());
    }

    #[test]
    fn deleting_the_photo_cascades_to_its_snapshots() {
        let (conn, photo_id) = setup();
        create(
            &conn,
            photo_id,
            "Wird mitgelöscht",
            &sample_edl(0.0),
            OffsetDateTime::now_utc(),
        )
        .expect("anlegen");
        conn.execute(
            "DELETE FROM photos WHERE id = ?1",
            params![photo_id.to_string()],
        )
        .expect("Foto löschen");
        assert_eq!(list(&conn, photo_id).expect("liste"), Vec::new());
    }
}
