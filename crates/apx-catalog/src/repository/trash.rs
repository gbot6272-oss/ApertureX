//! Papierkorb (Phase 33 F1) — siehe `migrations/0014_trash.sql` für die
//! Begründung, warum weggeworfene Fotos als Zeile stehen bleiben statt
//! gelöscht zu werden.
//!
//! Zwei Regeln, die hier durchgesetzt werden und nirgends sonst:
//!
//! 1. **Eine virtuelle Kopie folgt ihrem Quellfoto in den Papierkorb.**
//!    Sonst bliebe eine Bearbeitungsvariante eines Fotos im Katalog, das
//!    selbst nicht mehr da ist — sie zeigt auf dieselbe Datei und wäre
//!    ohne ihr Original weder erklärbar noch sinnvoll aufzufinden.
//! 2. **Umgekehrt holt das Wiederherstellen einer Kopie ihr Quellfoto
//!    mit zurück.** Dieselbe Begründung von der anderen Seite: eine
//!    Kopie ohne Quelle ist ein Fundstück ohne Kontext.
//!
//! Beides ist bewusst asymmetrisch zum *endgültigen* Löschen: dort
//! erledigt `ON DELETE CASCADE` aus Migration 7 die virtuellen Kopien
//! bereits, und wir lassen die Datenbank das tun.

use std::str::FromStr;

use apx_core::{AppError, PhotoId, Result};
use rusqlite::{params, Connection};
use time::OffsetDateTime;

use crate::error::map_sqlite_err;
use crate::models::{from_unix, to_unix, TrashEntry, TrashReason};
use crate::repository::photos::{raw_to_photo, row_to_raw, SELECT_COLUMNS};

/// Wirft `ids` in den Papierkorb und gibt zurück, wie viele Zeilen
/// tatsächlich gewandert sind (virtuelle Kopien mitgezählt, bereits
/// weggeworfene Fotos nicht).
///
/// Unbekannte IDs sind ein Fehler, kein stilles Übergehen — wer ein Foto
/// wegwirft, das es nicht gibt, hat einen Bug, und den soll er sehen.
pub(crate) fn trash_photos(
    conn: &Connection,
    ids: &[PhotoId],
    reason: TrashReason,
    now: OffsetDateTime,
) -> Result<u64> {
    let mut moved = 0u64;
    for id in ids {
        let exists: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM photos WHERE id = ?1",
                params![id.to_string()],
                |row| row.get(0),
            )
            .map_err(map_sqlite_err)?;
        if exists == 0 {
            return Err(AppError::not_found("Foto", id.to_string()));
        }
        moved += conn
            .execute(
                "UPDATE photos SET deleted_at = ?2, deleted_reason = ?3 \
                 WHERE (id = ?1 OR source_photo_id = ?1) AND deleted_at IS NULL",
                params![id.to_string(), to_unix(now), reason.as_str()],
            )
            .map_err(map_sqlite_err)? as u64;
    }
    Ok(moved)
}

/// Holt `ids` aus dem Papierkorb zurück; gibt die Zahl der
/// wiederhergestellten Zeilen zurück (mit zurückgeholte Quellfotos
/// mitgezählt).
pub(crate) fn restore_photos(conn: &Connection, ids: &[PhotoId]) -> Result<u64> {
    let mut restored = 0u64;
    for id in ids {
        let source: Option<String> = conn
            .query_row(
                "SELECT source_photo_id FROM photos WHERE id = ?1",
                params![id.to_string()],
                |row| row.get(0),
            )
            .map_err(|err| match err {
                rusqlite::Error::QueryReturnedNoRows => AppError::not_found("Foto", id.to_string()),
                other => map_sqlite_err(other),
            })?;
        restored += conn
            .execute(
                "UPDATE photos SET deleted_at = NULL, deleted_reason = NULL \
                 WHERE id = ?1 AND deleted_at IS NOT NULL",
                params![id.to_string()],
            )
            .map_err(map_sqlite_err)? as u64;
        if let Some(source) = source {
            restored += conn
                .execute(
                    "UPDATE photos SET deleted_at = NULL, deleted_reason = NULL \
                     WHERE id = ?1 AND deleted_at IS NOT NULL",
                    params![source],
                )
                .map_err(map_sqlite_err)? as u64;
        }
    }
    Ok(restored)
}

/// Alles im Papierkorb, zuletzt Weggeworfenes zuerst.
pub(crate) fn list_trash(conn: &Connection) -> Result<Vec<TrashEntry>> {
    let sql = format!(
        "SELECT {SELECT_COLUMNS}, photos.deleted_at, photos.deleted_reason FROM photos \
         WHERE photos.deleted_at IS NOT NULL \
         ORDER BY photos.deleted_at DESC, photos.filename ASC"
    );
    let mut stmt = conn.prepare(&sql).map_err(map_sqlite_err)?;
    let column_count = SELECT_COLUMNS.split(',').count();
    let rows = stmt
        .query_map([], |row| {
            let raw = row_to_raw(row)?;
            let deleted_at: i64 = row.get(column_count)?;
            let reason: Option<String> = row.get(column_count + 1)?;
            Ok((raw, deleted_at, reason))
        })
        .map_err(map_sqlite_err)?;
    let mut result = Vec::new();
    for row in rows {
        let (raw, deleted_at, reason) = row.map_err(map_sqlite_err)?;
        result.push(TrashEntry {
            photo: raw_to_photo(raw)?,
            deleted_at: from_unix(deleted_at)?,
            reason: TrashReason::from_str_lossy(reason.as_deref().unwrap_or("manual")),
        });
    }
    Ok(result)
}

/// Löscht `ids` endgültig aus dem Katalog. Die Dateien auf der Platte
/// rührt diese Ebene nicht an — das entscheidet und tut `apx-app`, weil
/// nur dort der Dateipfad bekannt ist.
pub(crate) fn purge_photos(conn: &Connection, ids: &[PhotoId]) -> Result<u64> {
    let mut purged = 0u64;
    for id in ids {
        purged += conn
            .execute(
                "DELETE FROM photos WHERE id = ?1 AND deleted_at IS NOT NULL",
                params![id.to_string()],
            )
            .map_err(map_sqlite_err)? as u64;
    }
    Ok(purged)
}

/// IDs aller Papierkorb-Einträge, die länger als `cutoff` dort liegen —
/// Grundlage für das automatische Leeren nach N Tagen. Liefert nur die
/// IDs, löscht selbst nichts: der Aufrufer soll erst die Dateien
/// aufräumen können und dann [`purge_photos`] rufen.
pub(crate) fn ids_older_than(conn: &Connection, cutoff: OffsetDateTime) -> Result<Vec<PhotoId>> {
    let mut stmt = conn
        .prepare(
            "SELECT id FROM photos WHERE deleted_at IS NOT NULL AND deleted_at < ?1 \
             ORDER BY deleted_at ASC",
        )
        .map_err(map_sqlite_err)?;
    let rows = stmt
        .query_map(params![to_unix(cutoff)], |row| row.get::<_, String>(0))
        .map_err(map_sqlite_err)?;
    let mut ids = Vec::new();
    for row in rows {
        ids.push(PhotoId::from_str(&row.map_err(map_sqlite_err)?)?);
    }
    Ok(ids)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::migrations;
    use crate::models::NewPhoto;
    use crate::repository::{folders, photos, search, stats};
    use apx_core::FolderId;
    use std::path::PathBuf;
    use time::Duration;

    fn setup() -> (Connection, FolderId) {
        let conn = Connection::open_in_memory().unwrap();
        migrations::apply(&conn).unwrap();
        let folder_id = folders::insert(
            &conn,
            &PathBuf::from("/fotos"),
            None,
            OffsetDateTime::now_utc(),
        )
        .unwrap();
        (conn, folder_id)
    }

    fn add(conn: &Connection, folder_id: FolderId, name: &str) -> PhotoId {
        let photo = NewPhoto {
            folder_id,
            filename: name.to_string(),
            file_size: 1000,
            file_mtime: OffsetDateTime::now_utc().replace_nanosecond(0).unwrap(),
            content_hash: Some(format!("hash-{name}")),
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
            media_kind: "photo".to_string(),
            duration_ms: None,
            video_codec: None,
            has_audio: None,
            frame_rate: None,
        };
        photos::upsert(conn, &photo, OffsetDateTime::now_utc())
            .unwrap()
            .0
    }

    #[test]
    fn ein_weggeworfenes_foto_verschwindet_aus_der_ordnerliste() {
        let (conn, folder_id) = setup();
        let a = add(&conn, folder_id, "a.jpg");
        add(&conn, folder_id, "b.jpg");

        trash_photos(&conn, &[a], TrashReason::Manual, OffsetDateTime::now_utc()).unwrap();

        let listed = photos::list_by_folder(&conn, folder_id).unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].filename, "b.jpg");
        assert_eq!(photos::count_by_folder(&conn, folder_id).unwrap(), 1);
    }

    #[test]
    fn ein_weggeworfenes_foto_verschwindet_aus_suche_filter_und_statistik() {
        let (conn, folder_id) = setup();
        let a = add(&conn, folder_id, "sonnenuntergang.jpg");

        assert_eq!(
            search::search_photos(&conn, "sonnenuntergang")
                .unwrap()
                .len(),
            1
        );
        assert_eq!(stats::compute(&conn).unwrap().total_photos, 1);

        trash_photos(&conn, &[a], TrashReason::Manual, OffsetDateTime::now_utc()).unwrap();

        assert!(search::search_photos(&conn, "sonnenuntergang")
            .unwrap()
            .is_empty());
        assert!(search::filter_photos(&conn, &Default::default())
            .unwrap()
            .is_empty());
        assert_eq!(stats::compute(&conn).unwrap().total_photos, 0);
    }

    #[test]
    fn get_liefert_ein_weggeworfenes_foto_weiterhin() {
        let (conn, folder_id) = setup();
        let a = add(&conn, folder_id, "a.jpg");
        trash_photos(&conn, &[a], TrashReason::Manual, OffsetDateTime::now_utc()).unwrap();
        assert_eq!(photos::get(&conn, a).unwrap().filename, "a.jpg");
    }

    #[test]
    fn wiederherstellen_bringt_das_foto_zurueck() {
        let (conn, folder_id) = setup();
        let a = add(&conn, folder_id, "a.jpg");
        trash_photos(&conn, &[a], TrashReason::Manual, OffsetDateTime::now_utc()).unwrap();
        assert_eq!(restore_photos(&conn, &[a]).unwrap(), 1);
        assert_eq!(photos::list_by_folder(&conn, folder_id).unwrap().len(), 1);
        assert!(list_trash(&conn).unwrap().is_empty());
    }

    #[test]
    fn zweimal_wegwerfen_zaehlt_nur_einmal() {
        let (conn, folder_id) = setup();
        let a = add(&conn, folder_id, "a.jpg");
        let now = OffsetDateTime::now_utc();
        assert_eq!(
            trash_photos(&conn, &[a], TrashReason::Manual, now).unwrap(),
            1
        );
        assert_eq!(
            trash_photos(&conn, &[a], TrashReason::Manual, now).unwrap(),
            0
        );
    }

    #[test]
    fn eine_virtuelle_kopie_folgt_ihrem_quellfoto_und_kommt_mit_zurueck() {
        let (conn, folder_id) = setup();
        let a = add(&conn, folder_id, "a.jpg");
        let copy = photos::create_virtual_copy(&conn, a, OffsetDateTime::now_utc()).unwrap();

        assert_eq!(
            trash_photos(&conn, &[a], TrashReason::Manual, OffsetDateTime::now_utc()).unwrap(),
            2
        );
        assert!(photos::list_virtual_copies(&conn, a).unwrap().is_empty());

        // Nur die Kopie wiederherstellen — das Quellfoto kommt mit.
        assert_eq!(restore_photos(&conn, &[copy]).unwrap(), 2);
        assert_eq!(photos::list_virtual_copies(&conn, a).unwrap().len(), 1);
    }

    #[test]
    fn der_papierkorb_listet_grund_und_zeitpunkt() {
        let (conn, folder_id) = setup();
        let a = add(&conn, folder_id, "a.jpg");
        let when = OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap();
        trash_photos(&conn, &[a], TrashReason::Duplicate, when).unwrap();

        let entries = list_trash(&conn).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].photo.filename, "a.jpg");
        assert_eq!(entries[0].reason, TrashReason::Duplicate);
        assert_eq!(entries[0].deleted_at, when);
    }

    #[test]
    fn endgueltig_loeschen_trifft_nur_was_im_papierkorb_liegt() {
        let (conn, folder_id) = setup();
        let a = add(&conn, folder_id, "a.jpg");
        let b = add(&conn, folder_id, "b.jpg");
        trash_photos(&conn, &[a], TrashReason::Manual, OffsetDateTime::now_utc()).unwrap();

        // `b` liegt nicht im Papierkorb und wird deshalb nicht angefasst.
        assert_eq!(purge_photos(&conn, &[a, b]).unwrap(), 1);
        assert!(photos::get(&conn, a).is_err());
        assert!(photos::get(&conn, b).is_ok());
    }

    #[test]
    fn alte_eintraege_lassen_sich_nach_zeit_finden() {
        let (conn, folder_id) = setup();
        let alt = add(&conn, folder_id, "alt.jpg");
        let neu = add(&conn, folder_id, "neu.jpg");
        let now = OffsetDateTime::now_utc();
        trash_photos(&conn, &[alt], TrashReason::Manual, now - Duration::days(40)).unwrap();
        trash_photos(&conn, &[neu], TrashReason::Manual, now).unwrap();

        let old = ids_older_than(&conn, now - Duration::days(30)).unwrap();
        assert_eq!(old, vec![alt]);
    }

    #[test]
    fn ein_unbekanntes_foto_wegzuwerfen_ist_ein_fehler() {
        let (conn, _folder_id) = setup();
        let err = trash_photos(
            &conn,
            &[PhotoId::new()],
            TrashReason::Manual,
            OffsetDateTime::now_utc(),
        )
        .unwrap_err();
        assert!(matches!(err, AppError::NotFound { .. }));
    }
}
