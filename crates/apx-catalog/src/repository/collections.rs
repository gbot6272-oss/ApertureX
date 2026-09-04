//! SQL für `collections`/`collection_photos`/`collection_folders` (siehe
//! `migrations/0003_library.sql` für die ursprüngliche flache Fassung,
//! `migrations/0007_library_backlog.sql` für Sammlungssätze/intelligente
//! Sammlungen, Phase 9 Schritt 1, `DECISIONS.md` ADR-0023/ADR-0032).
//! Sammlungssätze (`collection_folders`) spiegeln `preset_folders`
//! strukturell exakt.

use apx_core::{AppError, CollectionFolderId, CollectionId, PhotoId, Result};
use rusqlite::{params, Connection};
use time::OffsetDateTime;

use crate::error::map_sqlite_err;
use crate::models::{
    from_unix, parse_filter_node, to_unix, Collection, CollectionFolder, FilterCriteria, FilterNode,
};
use crate::repository::photos::{raw_to_photo, row_to_raw, SELECT_COLUMNS};
use crate::repository::search::filter_photos;
use crate::Photo;

// ---- Sammlungssätze -------------------------------------------------------

pub(crate) fn create_folder(
    conn: &Connection,
    name: &str,
    parent_id: Option<CollectionFolderId>,
) -> Result<CollectionFolderId> {
    let next_position: i64 = conn
        .query_row(
            "SELECT COALESCE(MAX(position), -1) + 1 FROM collection_folders WHERE parent_id IS ?1",
            params![parent_id.map(|p| p.to_string())],
            |row| row.get(0),
        )
        .map_err(map_sqlite_err)?;
    let id = CollectionFolderId::new();
    conn.execute(
        "INSERT INTO collection_folders (id, name, parent_id, position) VALUES (?1, ?2, ?3, ?4)",
        params![
            id.to_string(),
            name,
            parent_id.map(|p| p.to_string()),
            next_position
        ],
    )
    .map_err(map_sqlite_err)?;
    Ok(id)
}

pub(crate) fn rename_folder(conn: &Connection, id: CollectionFolderId, name: &str) -> Result<()> {
    let changed = conn
        .execute(
            "UPDATE collection_folders SET name = ?2 WHERE id = ?1",
            params![id.to_string(), name],
        )
        .map_err(map_sqlite_err)?;
    if changed == 0 {
        return Err(AppError::not_found("Sammlungssatz", id.to_string()));
    }
    Ok(())
}

/// Löscht `id` — verschachtelte Unter-Sammlungssätze werden per
/// `ON DELETE CASCADE` mitgelöscht, direkt darin liegende Sammlungen
/// bleiben erhalten und rutschen an die Wurzel (`folder_id = NULL`).
pub(crate) fn delete_folder(conn: &Connection, id: CollectionFolderId) -> Result<()> {
    let changed = conn
        .execute(
            "DELETE FROM collection_folders WHERE id = ?1",
            params![id.to_string()],
        )
        .map_err(map_sqlite_err)?;
    if changed == 0 {
        return Err(AppError::not_found("Sammlungssatz", id.to_string()));
    }
    Ok(())
}

pub(crate) fn list_folders(conn: &Connection) -> Result<Vec<CollectionFolder>> {
    let mut stmt = conn
        .prepare("SELECT id, name, parent_id, position FROM collection_folders ORDER BY parent_id, position")
        .map_err(map_sqlite_err)?;
    let rows = stmt
        .query_map([], |row| {
            let id: String = row.get(0)?;
            let name: String = row.get(1)?;
            let parent_id: Option<String> = row.get(2)?;
            let position: i64 = row.get(3)?;
            Ok((id, name, parent_id, position))
        })
        .map_err(map_sqlite_err)?;
    let mut result = Vec::new();
    for row in rows {
        let (id, name, parent_id, position) = row.map_err(map_sqlite_err)?;
        result.push(CollectionFolder {
            id: id.parse()?,
            name,
            parent_id: parent_id.map(|p| p.parse()).transpose()?,
            position,
        });
    }
    Ok(result)
}

// ---- Sammlungen -------------------------------------------------------

#[allow(clippy::type_complexity)]
fn row_to_collection(
    row: &rusqlite::Row,
) -> rusqlite::Result<(String, String, i64, Option<String>, i64, Option<String>)> {
    Ok((
        row.get(0)?,
        row.get(1)?,
        row.get(2)?,
        row.get(3)?,
        row.get(4)?,
        row.get(5)?,
    ))
}

fn raw_to_collection(
    raw: (String, String, i64, Option<String>, i64, Option<String>),
) -> Result<Collection> {
    let (id, name, created_at, folder_id, is_smart, smart_criteria_json) = raw;
    Ok(Collection {
        id: id.parse()?,
        name,
        created_at: from_unix(created_at)?,
        folder_id: folder_id.map(|f| f.parse()).transpose()?,
        is_smart: is_smart != 0,
        smart_criteria_json,
    })
}

const COLLECTION_COLUMNS: &str = "id, name, created_at, folder_id, is_smart, smart_criteria_json";

pub(crate) fn create(
    conn: &Connection,
    name: &str,
    folder_id: Option<CollectionFolderId>,
    created_at: OffsetDateTime,
) -> Result<CollectionId> {
    let id = CollectionId::new();
    conn.execute(
        "INSERT INTO collections (id, name, created_at, folder_id, is_smart, smart_criteria_json) \
         VALUES (?1, ?2, ?3, ?4, 0, NULL)",
        params![
            id.to_string(),
            name,
            to_unix(created_at),
            folder_id.map(|f| f.to_string())
        ],
    )
    .map_err(map_sqlite_err)?;
    Ok(id)
}

/// Legt eine intelligente Sammlung an — Mitgliedschaft wird bei jedem
/// Zugriff live über `criteria` berechnet (siehe [`list_photos`]).
/// `criteria` ist seit Phase 13 Schritt 7 ein echter, beliebig
/// verschachtelbarer UND/ODER-Regelbaum ([`FilterNode`]) — vorher eine
/// flache UND-Verknüpfung der `FilterCriteria`-Felder (siehe
/// `migrations/0007_library_backlog.sql`s Moduldoku und `DECISIONS.md`
/// ADR-0040-Nachtrag V).
pub(crate) fn create_smart(
    conn: &Connection,
    name: &str,
    folder_id: Option<CollectionFolderId>,
    criteria: &FilterNode,
    created_at: OffsetDateTime,
) -> Result<CollectionId> {
    let id = CollectionId::new();
    let criteria_json = serde_json::to_string(criteria)
        .map_err(|err| AppError::validation(format!("Kriterien nicht serialisierbar: {err}")))?;
    conn.execute(
        "INSERT INTO collections (id, name, created_at, folder_id, is_smart, smart_criteria_json) \
         VALUES (?1, ?2, ?3, ?4, 1, ?5)",
        params![
            id.to_string(),
            name,
            to_unix(created_at),
            folder_id.map(|f| f.to_string()),
            criteria_json,
        ],
    )
    .map_err(map_sqlite_err)?;
    Ok(id)
}

pub(crate) fn rename(conn: &Connection, id: CollectionId, name: &str) -> Result<()> {
    let changed = conn
        .execute(
            "UPDATE collections SET name = ?2 WHERE id = ?1",
            params![id.to_string(), name],
        )
        .map_err(map_sqlite_err)?;
    if changed == 0 {
        return Err(AppError::not_found("Sammlung", id.to_string()));
    }
    Ok(())
}

/// Verschiebt eine Sammlung in einen anderen Sammlungssatz (`None` = an
/// die Wurzel).
pub(crate) fn move_to_folder(
    conn: &Connection,
    id: CollectionId,
    folder_id: Option<CollectionFolderId>,
) -> Result<()> {
    let changed = conn
        .execute(
            "UPDATE collections SET folder_id = ?2 WHERE id = ?1",
            params![id.to_string(), folder_id.map(|f| f.to_string())],
        )
        .map_err(map_sqlite_err)?;
    if changed == 0 {
        return Err(AppError::not_found("Sammlung", id.to_string()));
    }
    Ok(())
}

pub(crate) fn delete(conn: &Connection, id: CollectionId) -> Result<()> {
    let changed = conn
        .execute(
            "DELETE FROM collections WHERE id = ?1",
            params![id.to_string()],
        )
        .map_err(map_sqlite_err)?;
    if changed == 0 {
        return Err(AppError::not_found("Sammlung", id.to_string()));
    }
    Ok(())
}

pub(crate) fn list_all(conn: &Connection) -> Result<Vec<Collection>> {
    let mut stmt = conn
        .prepare(&format!(
            "SELECT {COLLECTION_COLUMNS} FROM collections ORDER BY created_at"
        ))
        .map_err(map_sqlite_err)?;
    let rows = stmt
        .query_map([], row_to_collection)
        .map_err(map_sqlite_err)?;
    let mut result = Vec::new();
    for row in rows {
        result.push(raw_to_collection(row.map_err(map_sqlite_err)?)?);
    }
    Ok(result)
}

fn get(conn: &Connection, id: CollectionId) -> Result<Collection> {
    let sql = format!("SELECT {COLLECTION_COLUMNS} FROM collections WHERE id = ?1");
    let raw = conn
        .query_row(&sql, params![id.to_string()], row_to_collection)
        .map_err(|err| match err {
            rusqlite::Error::QueryReturnedNoRows => AppError::not_found("Sammlung", id.to_string()),
            other => map_sqlite_err(other),
        })?;
    raw_to_collection(raw)
}

/// Fügt `photo_id` ans Ende von `collection_id` an (höchste `position` + 1).
/// Erneutes Hinzufügen desselben Fotos ist ein No-Op (Primärschlüssel
/// `(collection_id, photo_id)`), verschiebt es also nicht ans Ende. Auf
/// eine intelligente Sammlung angewendet ein klarer Fehler statt eines
/// stillen No-Ops — ihre Mitgliedschaft kommt ausschließlich aus den
/// Kriterien.
pub(crate) fn add_photo(
    conn: &Connection,
    collection_id: CollectionId,
    photo_id: PhotoId,
) -> Result<()> {
    if get(conn, collection_id)?.is_smart {
        return Err(AppError::validation(
            "Fotos können nicht manuell zu einer intelligenten Sammlung hinzugefügt werden"
                .to_string(),
        ));
    }
    let next_position: i64 = conn
        .query_row(
            "SELECT COALESCE(MAX(position), -1) + 1 FROM collection_photos WHERE collection_id = ?1",
            params![collection_id.to_string()],
            |row| row.get(0),
        )
        .map_err(map_sqlite_err)?;
    conn.execute(
        "INSERT OR IGNORE INTO collection_photos (collection_id, photo_id, position) VALUES (?1, ?2, ?3)",
        params![collection_id.to_string(), photo_id.to_string(), next_position],
    )
    .map_err(map_sqlite_err)?;
    Ok(())
}

pub(crate) fn remove_photo(
    conn: &Connection,
    collection_id: CollectionId,
    photo_id: PhotoId,
) -> Result<()> {
    conn.execute(
        "DELETE FROM collection_photos WHERE collection_id = ?1 AND photo_id = ?2",
        params![collection_id.to_string(), photo_id.to_string()],
    )
    .map_err(map_sqlite_err)?;
    Ok(())
}

/// Bei einer intelligenten Sammlung: live aus `smart_criteria_json`
/// berechnet — der gesamte Fotobestand wird per SQL geladen
/// (`repository::search::filter_photos` mit leeren Kriterien, dieselbe
/// Abfrage wie Phase 3s Filterleiste) und der Regelbaum dann **in-memory**
/// je Foto ausgewertet ([`FilterNode::matches`], siehe `DECISIONS.md`
/// ADR-0040-Nachtrag V — keine dynamische SQL-Generierung für beliebig
/// tief verschachtelte UND/ODER-Gruppen nötig, Kataloge hier sind
/// Einzelnutzer-Bibliotheken). Sonst: die manuell gepflegte
/// Mitgliederliste in `collection_photos`, nach `position` sortiert.
pub(crate) fn list_photos(conn: &Connection, collection_id: CollectionId) -> Result<Vec<Photo>> {
    let collection = get(conn, collection_id)?;
    if collection.is_smart {
        let node = match collection.smart_criteria_json.as_deref() {
            Some(json) => parse_filter_node(json)?,
            None => FilterNode::Group {
                operator: crate::models::BoolOp::And,
                children: Vec::new(),
            },
        };
        let all_photos = filter_photos(conn, &FilterCriteria::default())?;
        return Ok(all_photos
            .into_iter()
            .filter(|photo| node.matches(photo))
            .collect());
    }
    let sql = format!(
        "SELECT {SELECT_COLUMNS} FROM collection_photos cp \
         JOIN photos ON photos.id = cp.photo_id \
         WHERE cp.collection_id = ?1 ORDER BY cp.position"
    );
    let mut stmt = conn.prepare(&sql).map_err(map_sqlite_err)?;
    let rows = stmt
        .query_map(params![collection_id.to_string()], row_to_raw)
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

    fn second_photo(conn: &Connection, folder_id: apx_core::FolderId) -> PhotoId {
        let photo = NewPhoto {
            media_kind: "photo".to_string(),
            duration_ms: None,
            video_codec: None,
            has_audio: None,
            frame_rate: None,
            folder_id,
            filename: "b.cr2".to_string(),
            file_size: 200,
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
        photos::upsert(conn, &photo, OffsetDateTime::now_utc())
            .expect("Foto anlegen")
            .0
    }

    #[test]
    fn create_rename_delete_roundtrip() {
        let (conn, _) = setup();
        let id = create(&conn, "Urlaub 2026", None, OffsetDateTime::now_utc()).expect("ok");
        assert_eq!(list_all(&conn).expect("ok").len(), 1);

        rename(&conn, id, "Urlaub 2026 (Auswahl)").expect("ok");
        assert_eq!(
            list_all(&conn).expect("ok")[0].name,
            "Urlaub 2026 (Auswahl)"
        );

        delete(&conn, id).expect("ok");
        assert!(list_all(&conn).expect("ok").is_empty());
        assert!(rename(&conn, id, "x").is_err(), "gelöschte Sammlung");
    }

    #[test]
    fn add_photo_keeps_insertion_order_via_position() {
        let (conn, photo_a) = setup();
        let folder_id = photos::get(&conn, photo_a).expect("ok").folder_id;
        let photo_b = second_photo(&conn, folder_id);
        let collection_id =
            create(&conn, "Reihenfolge-Test", None, OffsetDateTime::now_utc()).expect("ok");

        add_photo(&conn, collection_id, photo_b).expect("ok");
        add_photo(&conn, collection_id, photo_a).expect("ok");

        let photos_in_order = list_photos(&conn, collection_id).expect("ok");
        assert_eq!(photos_in_order.len(), 2);
        assert_eq!(photos_in_order[0].id, photo_b, "zuerst hinzugefügt zuerst");
        assert_eq!(photos_in_order[1].id, photo_a);
    }

    #[test]
    fn adding_same_photo_twice_is_idempotent() {
        let (conn, photo_id) = setup();
        let collection_id = create(&conn, "Test", None, OffsetDateTime::now_utc()).expect("ok");

        add_photo(&conn, collection_id, photo_id).expect("ok");
        add_photo(&conn, collection_id, photo_id).expect("ok");

        assert_eq!(list_photos(&conn, collection_id).expect("ok").len(), 1);
    }

    #[test]
    fn remove_photo_detaches_without_deleting_collection_or_photo() {
        let (conn, photo_id) = setup();
        let collection_id = create(&conn, "Test", None, OffsetDateTime::now_utc()).expect("ok");
        add_photo(&conn, collection_id, photo_id).expect("ok");

        remove_photo(&conn, collection_id, photo_id).expect("ok");

        assert!(list_photos(&conn, collection_id).expect("ok").is_empty());
        assert_eq!(list_all(&conn).expect("ok").len(), 1);
    }

    #[test]
    fn deleting_collection_cascades_to_collection_photos() {
        let (conn, photo_id) = setup();
        let collection_id = create(&conn, "Test", None, OffsetDateTime::now_utc()).expect("ok");
        add_photo(&conn, collection_id, photo_id).expect("ok");

        delete(&conn, collection_id).expect("ok");

        let count: i64 = conn
            .query_row("SELECT count(*) FROM collection_photos", [], |row| {
                row.get(0)
            })
            .expect("lesbar");
        assert_eq!(count, 0, "Verknüpfung muss per Kaskade gelöscht sein");
    }

    #[test]
    fn deleting_photo_cascades_to_collection_photos_but_keeps_collection() {
        let (conn, photo_id) = setup();
        let collection_id = create(&conn, "Test", None, OffsetDateTime::now_utc()).expect("ok");
        add_photo(&conn, collection_id, photo_id).expect("ok");

        conn.execute(
            "DELETE FROM photos WHERE id = ?1",
            params![photo_id.to_string()],
        )
        .expect("Foto löschen");

        assert!(list_photos(&conn, collection_id).expect("ok").is_empty());
        assert_eq!(list_all(&conn).expect("ok").len(), 1);
    }

    #[test]
    fn collection_folder_create_rename_delete_roundtrip() {
        let (conn, _) = setup();
        let id = create_folder(&conn, "Reisen", None).expect("ok");
        assert_eq!(list_folders(&conn).expect("ok").len(), 1);

        rename_folder(&conn, id, "Reisen 2026").expect("ok");
        assert_eq!(list_folders(&conn).expect("ok")[0].name, "Reisen 2026");

        delete_folder(&conn, id).expect("ok");
        assert!(list_folders(&conn).expect("ok").is_empty());
    }

    #[test]
    fn deleting_collection_folder_moves_its_collections_to_the_root() {
        let (conn, _) = setup();
        let folder_id = create_folder(&conn, "Reisen", None).expect("ok");
        let collection_id =
            create(&conn, "Urlaub", Some(folder_id), OffsetDateTime::now_utc()).expect("ok");

        delete_folder(&conn, folder_id).expect("ok");

        let collection = list_all(&conn)
            .expect("ok")
            .into_iter()
            .find(|c| c.id == collection_id)
            .expect("noch da");
        assert_eq!(collection.folder_id, None, "sollte an die Wurzel rutschen");
    }

    #[test]
    fn smart_collection_membership_is_computed_live_from_criteria() {
        let (conn, photo_a) = setup();
        photos::set_rating(&conn, photo_a, 5).expect("ok");
        let folder_id = photos::get(&conn, photo_a).expect("ok").folder_id;
        let photo_b = second_photo(&conn, folder_id);
        photos::set_rating(&conn, photo_b, 1).expect("ok");

        let criteria = FilterNode::Condition {
            condition: crate::models::FilterCondition {
                field: crate::models::FilterField::Rating,
                op: crate::models::FilterOperator::AtLeast,
                value: "4".to_string(),
            },
        };
        let collection_id = create_smart(
            &conn,
            "Top-Bewertet",
            None,
            &criteria,
            OffsetDateTime::now_utc(),
        )
        .expect("ok");

        let members = list_photos(&conn, collection_id).expect("ok");
        assert_eq!(members.len(), 1);
        assert_eq!(members[0].id, photo_a);

        // Live: eine Nachbewertung ändert die Mitgliedschaft ohne
        // erneutes Speichern der Sammlung.
        photos::set_rating(&conn, photo_b, 5).expect("ok");
        assert_eq!(list_photos(&conn, collection_id).expect("ok").len(), 2);
    }

    #[test]
    fn manual_add_photo_to_a_smart_collection_is_rejected() {
        let (conn, photo_id) = setup();
        let collection_id = create_smart(
            &conn,
            "Alle",
            None,
            &FilterNode::Group {
                operator: crate::models::BoolOp::And,
                children: Vec::new(),
            },
            OffsetDateTime::now_utc(),
        )
        .expect("ok");
        assert!(add_photo(&conn, collection_id, photo_id).is_err());
    }

    /// Echter Regelbaum mit ODER-Verknüpfung und Verschachtelung (Phase 13
    /// Schritt 7): „Bewertung >= 4 ODER (Farbe = rot UND Kameramodell
    /// enthält 'Test')" — deckt sowohl die ODER-Gruppierung als auch das
    /// Verschachteln einer UND-Gruppe innerhalb einer ODER-Gruppe ab, was
    /// die alte flache `FilterCriteria` nicht ausdrücken konnte.
    #[test]
    fn smart_collection_evaluates_nested_and_or_groups() {
        use crate::models::{BoolOp, FilterCondition, FilterField, FilterOperator};

        let (conn, photo_a) = setup();
        let folder_id = photos::get(&conn, photo_a).expect("ok").folder_id;
        let photo_c = second_photo(&conn, folder_id);
        // camera_model kommt normalerweise aus dem EXIF-Import (kein
        // eigener Setter), daher hier direkt per `NewPhoto`/`upsert`.
        let photo_b = photos::upsert(
            &conn,
            &NewPhoto {
                media_kind: "photo".to_string(),
                duration_ms: None,
                video_codec: None,
                has_audio: None,
                frame_rate: None,
                folder_id,
                filename: "c.cr2".to_string(),
                file_size: 300,
                file_mtime: OffsetDateTime::now_utc()
                    .replace_nanosecond(0)
                    .expect("gültig"),
                content_hash: None,
                width: None,
                height: None,
                orientation: 1,
                camera_make: None,
                camera_model: Some("Meine Testkamera".to_string()),
                lens: None,
                iso: None,
                shutter: None,
                aperture: None,
                focal_length: None,
                captured_at: None,
                gps_lat: None,
                gps_lon: None,
            },
            OffsetDateTime::now_utc(),
        )
        .expect("ok")
        .0;

        photos::set_rating(&conn, photo_a, 5).expect("ok"); // trifft über die Bewertung
        photos::set_rating(&conn, photo_b, 1).expect("ok");
        photos::set_color_label(&conn, photo_b, Some("red")).expect("ok"); // trifft über die verschachtelte UND-Gruppe
        photos::set_rating(&conn, photo_c, 1).expect("ok"); // trifft gar nicht

        let node = FilterNode::Group {
            operator: BoolOp::Or,
            children: vec![
                FilterNode::Condition {
                    condition: FilterCondition {
                        field: FilterField::Rating,
                        op: FilterOperator::AtLeast,
                        value: "4".to_string(),
                    },
                },
                FilterNode::Group {
                    operator: BoolOp::And,
                    children: vec![
                        FilterNode::Condition {
                            condition: FilterCondition {
                                field: FilterField::ColorLabel,
                                op: FilterOperator::Equals,
                                value: "red".to_string(),
                            },
                        },
                        FilterNode::Condition {
                            condition: FilterCondition {
                                field: FilterField::CameraModel,
                                op: FilterOperator::Contains,
                                value: "Test".to_string(),
                            },
                        },
                    ],
                },
            ],
        };
        let collection_id =
            create_smart(&conn, "Auswahl", None, &node, OffsetDateTime::now_utc()).expect("ok");

        let members = list_photos(&conn, collection_id).expect("ok");
        let member_ids: std::collections::HashSet<_> = members.iter().map(|p| p.id).collect();
        assert_eq!(member_ids.len(), 2);
        assert!(member_ids.contains(&photo_a));
        assert!(member_ids.contains(&photo_b));
        assert!(!member_ids.contains(&photo_c));
    }

    /// Vor Phase 13 Schritt 7 angelegte intelligente Sammlungen speichern
    /// `smart_criteria_json` in der alten flachen `FilterCriteria`-Form
    /// (kein `"type"`-Tag) — [`list_photos`] muss diese unverändert lesen
    /// können (siehe [`crate::models::parse_filter_node`]s Migration).
    #[test]
    fn smart_collection_reads_legacy_flat_criteria_json() {
        let (conn, photo_a) = setup();
        photos::set_rating(&conn, photo_a, 5).expect("ok");
        let folder_id = photos::get(&conn, photo_a).expect("ok").folder_id;
        let photo_b = second_photo(&conn, folder_id);
        photos::set_rating(&conn, photo_b, 1).expect("ok");

        let id = CollectionId::new();
        conn.execute(
            "INSERT INTO collections (id, name, created_at, folder_id, is_smart, smart_criteria_json) \
             VALUES (?1, 'Legacy', ?2, NULL, 1, ?3)",
            params![
                id.to_string(),
                to_unix(OffsetDateTime::now_utc()),
                r#"{"rating_at_least":4}"#,
            ],
        )
        .expect("ok");

        let members = list_photos(&conn, id).expect("ok");
        assert_eq!(members.len(), 1);
        assert_eq!(members[0].id, photo_a);
    }
}
