//! SQL für `edit_history`/`edit_current` (siehe `migrations/0002_edits.sql`,
//! `DECISIONS.md` ADR-0014). Implementiert einen linearen Verlauf mit
//! Undo/Redo: eine neue Bearbeitung nach einem Rückgängig verwirft die
//! „Zukunft" (keine Verzweigung) — Undo/Redo selbst löschen nichts, sie
//! verschieben nur den `edit_current`-Zeiger zwischen bereits
//! existierenden Zeilen.

use apx_core::{AppError, EditHistoryId, EdlEnvelope, PhotoId, Result};
use rusqlite::{params, Connection, OptionalExtension};
use time::OffsetDateTime;

use crate::error::map_sqlite_err;
use crate::models::{from_unix, to_unix, EditHistoryEntry, HistoryPosition};

struct EditHistoryRow {
    id: String,
    photo_id: String,
    sequence: i64,
    label: Option<String>,
    edl_json: String,
    created_at: i64,
}

fn row_to_raw(row: &rusqlite::Row) -> rusqlite::Result<EditHistoryRow> {
    Ok(EditHistoryRow {
        id: row.get(0)?,
        photo_id: row.get(1)?,
        sequence: row.get(2)?,
        label: row.get(3)?,
        edl_json: row.get(4)?,
        created_at: row.get(5)?,
    })
}

fn raw_to_entry(raw: EditHistoryRow) -> Result<EditHistoryEntry> {
    Ok(EditHistoryEntry {
        id: raw.id.parse()?,
        photo_id: raw.photo_id.parse()?,
        sequence: raw.sequence,
        label: raw.label,
        edl: EdlEnvelope::from_json_str(&raw.edl_json)?,
        created_at: from_unix(raw.created_at)?,
    })
}

const SELECT_COLUMNS: &str = "id, photo_id, sequence, label, edl_json, created_at";

/// Die Sequenznummer des aktuell aktiven Verlaufs-Eintrags, oder `None`,
/// wenn kein Eintrag aktiv ist (Ausgangszustand).
fn current_sequence(conn: &Connection, photo_id: PhotoId) -> Result<Option<i64>> {
    conn.query_row(
        "SELECT h.sequence FROM edit_current c JOIN edit_history h ON h.id = c.history_id WHERE c.photo_id = ?1",
        params![photo_id.to_string()],
        |row| row.get(0),
    )
    .optional()
    .map_err(map_sqlite_err)
}

fn entry_at_sequence(
    conn: &Connection,
    photo_id: PhotoId,
    sequence: i64,
) -> Result<Option<EditHistoryEntry>> {
    let raw: Option<EditHistoryRow> = conn
        .query_row(
            &format!(
                "SELECT {SELECT_COLUMNS} FROM edit_history WHERE photo_id = ?1 AND sequence = ?2"
            ),
            params![photo_id.to_string(), sequence],
            row_to_raw,
        )
        .optional()
        .map_err(map_sqlite_err)?;
    raw.map(raw_to_entry).transpose()
}

fn point_current_to(conn: &Connection, photo_id: PhotoId, history_id: EditHistoryId) -> Result<()> {
    conn.execute(
        "INSERT INTO edit_current (photo_id, history_id) VALUES (?1, ?2)
         ON CONFLICT(photo_id) DO UPDATE SET history_id = excluded.history_id",
        params![photo_id.to_string(), history_id.to_string()],
    )
    .map_err(map_sqlite_err)?;
    Ok(())
}

/// Speichert `edl` als neuen, aktiven Bearbeitungsschritt für `photo_id`.
/// Verwirft dabei jede „Zukunft" (Zeilen mit höherer Sequenznummer als der
/// aktuellen), falls zuvor per [`undo`] zurückgegangen wurde — siehe
/// `DECISIONS.md` ADR-0014.
pub(crate) fn commit(
    conn: &Connection,
    photo_id: PhotoId,
    edl: &EdlEnvelope,
    label: Option<&str>,
    created_at: OffsetDateTime,
) -> Result<EditHistoryId> {
    let current = current_sequence(conn, photo_id)?;
    let base_sequence = current.unwrap_or(-1);

    conn.execute(
        "DELETE FROM edit_history WHERE photo_id = ?1 AND sequence > ?2",
        params![photo_id.to_string(), base_sequence],
    )
    .map_err(map_sqlite_err)?;

    let new_id = EditHistoryId::new();
    let new_sequence = base_sequence + 1;
    let edl_json = edl.to_json_string()?;

    conn.execute(
        "INSERT INTO edit_history (id, photo_id, sequence, label, edl_json, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            new_id.to_string(),
            photo_id.to_string(),
            new_sequence,
            label,
            edl_json,
            to_unix(created_at)
        ],
    )
    .map_err(map_sqlite_err)?;

    point_current_to(conn, photo_id, new_id)?;
    Ok(new_id)
}

/// Der aktuell aktive Stand für `photo_id` — `Neutral`, wenn noch nie
/// bearbeitet wurde oder bis zum Ausgangszustand zurückgegangen wurde.
pub(crate) fn current(conn: &Connection, photo_id: PhotoId) -> Result<HistoryPosition> {
    match current_sequence(conn, photo_id)? {
        None => Ok(HistoryPosition::Neutral),
        Some(sequence) => {
            let entry = entry_at_sequence(conn, photo_id, sequence)?.ok_or_else(|| {
                AppError::Database {
                    message: format!(
                        "edit_current zeigt auf eine nicht existierende Sequenz {sequence} für Foto {photo_id} — Katalog inkonsistent"
                    ),
                }
            })?;
            Ok(HistoryPosition::At(entry))
        }
    }
}

/// Geht einen Schritt zurück. `Ok(None)`, wenn schon am Ausgangszustand
/// (nichts zu tun) — löscht dabei nie eine Zeile, nur der Zeiger bewegt
/// sich.
pub(crate) fn undo(conn: &Connection, photo_id: PhotoId) -> Result<Option<HistoryPosition>> {
    let Some(sequence) = current_sequence(conn, photo_id)? else {
        return Ok(None);
    };

    if sequence == 0 {
        conn.execute(
            "DELETE FROM edit_current WHERE photo_id = ?1",
            params![photo_id.to_string()],
        )
        .map_err(map_sqlite_err)?;
        return Ok(Some(HistoryPosition::Neutral));
    }

    let previous =
        entry_at_sequence(conn, photo_id, sequence - 1)?.ok_or_else(|| AppError::Database {
            message: format!(
                "Vorheriger Verlaufs-Schritt (Sequenz {}) für Foto {photo_id} fehlt",
                sequence - 1
            ),
        })?;
    point_current_to(conn, photo_id, previous.id)?;
    Ok(Some(HistoryPosition::At(previous)))
}

/// Geht einen Schritt vor. `Ok(None)`, wenn nichts zu wiederholen ist
/// (entweder weil noch nie zurückgegangen wurde, oder weil eine neue
/// Bearbeitung die „Zukunft" bereits verworfen hat).
pub(crate) fn redo(conn: &Connection, photo_id: PhotoId) -> Result<Option<HistoryPosition>> {
    let next_sequence = current_sequence(conn, photo_id)?.map_or(0, |s| s + 1);
    match entry_at_sequence(conn, photo_id, next_sequence)? {
        None => Ok(None),
        Some(entry) => {
            point_current_to(conn, photo_id, entry.id)?;
            Ok(Some(HistoryPosition::At(entry)))
        }
    }
}

/// Springt direkt zu einer bestimmten Sequenznummer (Phase 9 Schritt 7,
/// „Zeitleisten-Ansicht"/„Verlaufs-Vergleich" — ein Klick auf einen
/// beliebigen Verlaufspunkt statt nur Einzelschritt-Undo/Redo). Löscht
/// dabei nie eine Zeile, bewegt nur den Zeiger wie [`undo`]/[`redo`].
/// `Ok(None)`, wenn `sequence` nicht existiert.
pub(crate) fn goto(
    conn: &Connection,
    photo_id: PhotoId,
    sequence: i64,
) -> Result<Option<HistoryPosition>> {
    let Some(entry) = entry_at_sequence(conn, photo_id, sequence)? else {
        return Ok(None);
    };
    point_current_to(conn, photo_id, entry.id)?;
    Ok(Some(HistoryPosition::At(entry)))
}

/// Der vollständige Verlauf eines Fotos, älteste Sequenz zuerst.
pub(crate) fn list_history(conn: &Connection, photo_id: PhotoId) -> Result<Vec<EditHistoryEntry>> {
    let mut stmt = conn
        .prepare(&format!(
            "SELECT {SELECT_COLUMNS} FROM edit_history WHERE photo_id = ?1 ORDER BY sequence"
        ))
        .map_err(map_sqlite_err)?;
    let rows = stmt
        .query_map(params![photo_id.to_string()], row_to_raw)
        .map_err(map_sqlite_err)?;
    let mut result = Vec::new();
    for row in rows {
        result.push(raw_to_entry(row.map_err(map_sqlite_err)?)?);
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
    fn photo_without_edits_is_neutral() {
        let (conn, photo_id) = setup();
        assert_eq!(
            current(&conn, photo_id).expect("ok"),
            HistoryPosition::Neutral
        );
    }

    #[test]
    fn commit_then_current_roundtrips() {
        let (conn, photo_id) = setup();
        let edl = sample_edl(0.5);
        let id = commit(
            &conn,
            photo_id,
            &edl,
            Some("Belichtung"),
            OffsetDateTime::now_utc(),
        )
        .expect("commit");

        let position = current(&conn, photo_id).expect("ok");
        match position {
            HistoryPosition::At(entry) => {
                assert_eq!(entry.id, id);
                assert_eq!(entry.sequence, 0);
                assert_eq!(entry.label.as_deref(), Some("Belichtung"));
                assert_eq!(entry.edl, edl);
            }
            HistoryPosition::Neutral => panic!("sollte nicht neutral sein"),
        }
    }

    #[test]
    fn undo_then_redo_restores_state() {
        let (conn, photo_id) = setup();
        commit(
            &conn,
            photo_id,
            &sample_edl(0.5),
            None,
            OffsetDateTime::now_utc(),
        )
        .expect("commit 1");
        commit(
            &conn,
            photo_id,
            &sample_edl(1.0),
            None,
            OffsetDateTime::now_utc(),
        )
        .expect("commit 2");

        let after_undo = undo(&conn, photo_id)
            .expect("undo")
            .expect("sollte etwas zurückgeben");
        assert_eq!(
            after_undo,
            HistoryPosition::At(match current(&conn, photo_id).unwrap() {
                HistoryPosition::At(e) => e,
                HistoryPosition::Neutral => panic!("unerwartet neutral"),
            })
        );

        let after_redo = redo(&conn, photo_id)
            .expect("redo")
            .expect("sollte etwas zurückgeben");
        match after_redo {
            HistoryPosition::At(entry) => assert_eq!(entry.edl, sample_edl(1.0)),
            HistoryPosition::Neutral => panic!("sollte nicht neutral sein"),
        }
    }

    #[test]
    fn goto_jumps_directly_to_an_arbitrary_sequence() {
        let (conn, photo_id) = setup();
        commit(
            &conn,
            photo_id,
            &sample_edl(0.1),
            None,
            OffsetDateTime::now_utc(),
        )
        .expect("commit 0");
        commit(
            &conn,
            photo_id,
            &sample_edl(0.2),
            None,
            OffsetDateTime::now_utc(),
        )
        .expect("commit 1");
        commit(
            &conn,
            photo_id,
            &sample_edl(0.3),
            None,
            OffsetDateTime::now_utc(),
        )
        .expect("commit 2");

        // Direkt zur ersten Sequenz springen, ohne über die zweite zu
        // gehen (Einzelschritt-`undo` bräuchte hier zwei Aufrufe).
        let jumped = goto(&conn, photo_id, 0)
            .expect("goto")
            .expect("Sequenz 0 existiert");
        match &jumped {
            HistoryPosition::At(entry) => assert_eq!(entry.edl, sample_edl(0.1)),
            HistoryPosition::Neutral => panic!("sollte nicht neutral sein"),
        }
        assert_eq!(current(&conn, photo_id).expect("current"), jumped);
    }

    #[test]
    fn goto_an_unknown_sequence_returns_none_without_moving_the_pointer() {
        let (conn, photo_id) = setup();
        commit(
            &conn,
            photo_id,
            &sample_edl(0.5),
            None,
            OffsetDateTime::now_utc(),
        )
        .expect("commit");
        let before = current(&conn, photo_id).expect("current");

        assert_eq!(goto(&conn, photo_id, 99).expect("goto"), None);
        assert_eq!(current(&conn, photo_id).expect("current"), before);
    }

    #[test]
    fn undo_past_first_edit_reaches_neutral() {
        let (conn, photo_id) = setup();
        commit(
            &conn,
            photo_id,
            &sample_edl(0.5),
            None,
            OffsetDateTime::now_utc(),
        )
        .expect("commit");

        let position = undo(&conn, photo_id)
            .expect("undo")
            .expect("sollte etwas zurückgeben");
        assert_eq!(position, HistoryPosition::Neutral);

        // Ein weiteres Rückgängig ohne aktive Bearbeitung ist ein No-Op.
        assert_eq!(undo(&conn, photo_id).expect("undo"), None);
    }

    #[test]
    fn redo_from_neutral_after_undo_reaches_first_edit() {
        let (conn, photo_id) = setup();
        let edl = sample_edl(0.5);
        commit(&conn, photo_id, &edl, None, OffsetDateTime::now_utc()).expect("commit");
        undo(&conn, photo_id)
            .expect("undo")
            .expect("sollte neutral erreichen");

        let position = redo(&conn, photo_id)
            .expect("redo")
            .expect("sollte etwas zurückgeben");
        match position {
            HistoryPosition::At(entry) => assert_eq!(entry.edl, edl),
            HistoryPosition::Neutral => panic!("sollte nicht neutral sein"),
        }
    }

    #[test]
    fn new_edit_after_undo_discards_the_future() {
        let (conn, photo_id) = setup();
        commit(
            &conn,
            photo_id,
            &sample_edl(0.5),
            None,
            OffsetDateTime::now_utc(),
        )
        .expect("commit 1");
        commit(
            &conn,
            photo_id,
            &sample_edl(1.0),
            None,
            OffsetDateTime::now_utc(),
        )
        .expect("commit 2");
        undo(&conn, photo_id)
            .expect("undo")
            .expect("sollte etwas zurückgeben");

        // Neue Bearbeitung statt Redo — die verworfene "Zukunft" (Sequenz 1
        // mit exposure_ev=1.0) darf danach nicht mehr wiederherstellbar sein.
        commit(
            &conn,
            photo_id,
            &sample_edl(2.0),
            None,
            OffsetDateTime::now_utc(),
        )
        .expect("commit 3");

        assert_eq!(
            redo(&conn, photo_id).expect("redo"),
            None,
            "verworfene Zukunft darf nicht wiederhergestellt werden"
        );
        let history = list_history(&conn, photo_id).expect("history");
        assert_eq!(
            history.len(),
            2,
            "nur die zwei tatsächlich erreichbaren Schritte sollten übrig sein"
        );
        assert_eq!(history[1].edl, sample_edl(2.0));
    }

    #[test]
    fn deleting_photo_cascades_to_history_and_current() {
        let (conn, photo_id) = setup();
        commit(
            &conn,
            photo_id,
            &sample_edl(0.5),
            None,
            OffsetDateTime::now_utc(),
        )
        .expect("commit");

        conn.execute(
            "DELETE FROM photos WHERE id = ?1",
            params![photo_id.to_string()],
        )
        .expect("Foto löschen");

        let history_count: i64 = conn
            .query_row(
                "SELECT count(*) FROM edit_history WHERE photo_id = ?1",
                params![photo_id.to_string()],
                |row| row.get(0),
            )
            .expect("lesbar");
        assert_eq!(history_count, 0);
        let current_count: i64 = conn
            .query_row(
                "SELECT count(*) FROM edit_current WHERE photo_id = ?1",
                params![photo_id.to_string()],
                |row| row.get(0),
            )
            .expect("lesbar");
        assert_eq!(current_count, 0);
    }
}
