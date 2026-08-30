//! SQL für `preset_folders`/`presets`/`preset_versions` (siehe
//! `migrations/0004_presets.sql`, `DECISIONS.md` ADR-0031). Ein Preset ist
//! reine Katalogdaten — `apx-catalog` speichert `edl_subset_json`/
//! `conditions_json` als opake JSON-Strings, genau wie `edit_history`
//! (siehe `ARCHITECTURE.md` §5); nur Name/Ordner/Favorit/Tags werden
//! strukturiert gehalten, weil sie tatsächlich hier verwaltet werden
//! (Umbenennen, Verschieben, Favorisieren).
//!
//! Suche/Filter über Name oder Tags ist bewusst NICHT hier implementiert
//! (kein `search_by_name_or_tag`) — bei der für Presets erwartbaren
//! Listengröße reicht ein client-seitiger Filter über die bereits
//! geladene Liste, analog zu `lib/sortPhotos.ts`s Präzedenzfall
//! (`PLAN.md` Phase 3 Schritt 8.3: Sortierung bewusst client-seitig statt
//! eines weiteren Backend-Parameters).

use apx_core::{AppError, PresetFolderId, PresetId, PresetVersionId, Result};
use rusqlite::{params, Connection, OptionalExtension};
use time::OffsetDateTime;

use crate::error::map_sqlite_err;
use crate::models::{from_unix, to_unix, Preset, PresetFolder, PresetVersion};

fn tags_to_json(tags: &[String]) -> String {
    serde_json::to_string(tags).unwrap_or_else(|_| "[]".to_string())
}

fn tags_from_json(json: &str) -> Vec<String> {
    serde_json::from_str(json).unwrap_or_default()
}

// ---- Ordner -----------------------------------------------------------

pub(crate) fn create_folder(
    conn: &Connection,
    name: &str,
    parent_id: Option<PresetFolderId>,
    created_at: OffsetDateTime,
) -> Result<PresetFolderId> {
    let next_position: i64 = conn
        .query_row(
            "SELECT COALESCE(MAX(position), -1) + 1 FROM preset_folders WHERE parent_id IS ?1",
            params![parent_id.map(|p| p.to_string())],
            |row| row.get(0),
        )
        .map_err(map_sqlite_err)?;
    let id = PresetFolderId::new();
    conn.execute(
        "INSERT INTO preset_folders (id, name, parent_id, position, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            id.to_string(),
            name,
            parent_id.map(|p| p.to_string()),
            next_position,
            to_unix(created_at)
        ],
    )
    .map_err(map_sqlite_err)?;
    Ok(id)
}

pub(crate) fn rename_folder(conn: &Connection, id: PresetFolderId, name: &str) -> Result<()> {
    let changed = conn
        .execute(
            "UPDATE preset_folders SET name = ?2 WHERE id = ?1",
            params![id.to_string(), name],
        )
        .map_err(map_sqlite_err)?;
    if changed == 0 {
        return Err(AppError::not_found("Preset-Ordner", id.to_string()));
    }
    Ok(())
}

/// Löscht `id` — verschachtelte Unterordner werden per `ON DELETE CASCADE`
/// mitgelöscht, direkt oder verschachtelt darin liegende Presets bleiben
/// erhalten und rutschen an die Wurzel (`folder_id = NULL`, siehe
/// Migrations-Kommentar).
pub(crate) fn delete_folder(conn: &Connection, id: PresetFolderId) -> Result<()> {
    let changed = conn
        .execute(
            "DELETE FROM preset_folders WHERE id = ?1",
            params![id.to_string()],
        )
        .map_err(map_sqlite_err)?;
    if changed == 0 {
        return Err(AppError::not_found("Preset-Ordner", id.to_string()));
    }
    Ok(())
}

fn row_to_folder(
    row: &rusqlite::Row,
) -> rusqlite::Result<(String, String, Option<String>, i64, i64)> {
    Ok((
        row.get(0)?,
        row.get(1)?,
        row.get(2)?,
        row.get(3)?,
        row.get(4)?,
    ))
}

pub(crate) fn list_folders(conn: &Connection) -> Result<Vec<PresetFolder>> {
    let mut stmt = conn
        .prepare("SELECT id, name, parent_id, position, created_at FROM preset_folders ORDER BY parent_id, position")
        .map_err(map_sqlite_err)?;
    let rows = stmt.query_map([], row_to_folder).map_err(map_sqlite_err)?;
    let mut result = Vec::new();
    for row in rows {
        let (id, name, parent_id, position, created_at) = row.map_err(map_sqlite_err)?;
        result.push(PresetFolder {
            id: id.parse()?,
            name,
            parent_id: parent_id.map(|p| p.parse()).transpose()?,
            position,
            created_at: from_unix(created_at)?,
        });
    }
    Ok(result)
}

// ---- Presets ------------------------------------------------------------

type PresetRawRow = (String, Option<String>, String, i64, String, String, i64);

fn row_to_preset_raw(row: &rusqlite::Row) -> rusqlite::Result<PresetRawRow> {
    Ok((
        row.get(0)?,
        row.get(1)?,
        row.get(2)?,
        row.get(3)?,
        row.get(4)?,
        row.get(5)?,
        row.get(6)?,
    ))
}

fn raw_to_preset(raw: PresetRawRow) -> Result<Preset> {
    let (id, folder_id, name, is_favorite, tags_json, conditions_json, created_at) = raw;
    Ok(Preset {
        id: id.parse()?,
        folder_id: folder_id.map(|f| f.parse()).transpose()?,
        name,
        is_favorite: is_favorite != 0,
        tags: tags_from_json(&tags_json),
        conditions_json,
        created_at: from_unix(created_at)?,
    })
}

const PRESET_COLUMNS: &str =
    "id, folder_id, name, is_favorite, tags_json, conditions_json, created_at";

/// Legt ein neues Preset samt seiner ersten [`PresetVersion`] (`sequence`
/// = 1) an — zwei sequenzielle Inserts auf `&Connection` statt einer
/// expliziten Transaktion, wie auch [`super::edits::commit`]s
/// Mehrfach-Schreiboperation (kein nebenläufiger Schreibzugriff auf
/// dieselbe SQLite-Datei innerhalb dieses Desktop-Prozesses).
#[allow(clippy::too_many_arguments)]
pub(crate) fn create(
    conn: &Connection,
    folder_id: Option<PresetFolderId>,
    name: &str,
    tags: &[String],
    conditions_json: &str,
    edl_subset_json: &str,
    created_at: OffsetDateTime,
) -> Result<(PresetId, PresetVersionId)> {
    let id = PresetId::new();
    conn.execute(
        &format!("INSERT INTO presets ({PRESET_COLUMNS}) VALUES (?1, ?2, ?3, 0, ?4, ?5, ?6)"),
        params![
            id.to_string(),
            folder_id.map(|f| f.to_string()),
            name,
            tags_to_json(tags),
            conditions_json,
            to_unix(created_at)
        ],
    )
    .map_err(map_sqlite_err)?;

    let version_id = PresetVersionId::new();
    conn.execute(
        "INSERT INTO preset_versions (id, preset_id, sequence, edl_subset_json, created_at) VALUES (?1, ?2, 1, ?3, ?4)",
        params![
            version_id.to_string(),
            id.to_string(),
            edl_subset_json,
            to_unix(created_at)
        ],
    )
    .map_err(map_sqlite_err)?;

    Ok((id, version_id))
}

/// Ändert Name/Ordner/Tags/Bedingungen eines bestehenden Presets, ohne
/// eine neue Version anzulegen (das übernimmt [`create_version`] separat,
/// da nicht jede Metadaten-Änderung die EDL-Teilmenge betrifft).
pub(crate) fn update_metadata(
    conn: &Connection,
    id: PresetId,
    folder_id: Option<PresetFolderId>,
    name: &str,
    tags: &[String],
    conditions_json: &str,
) -> Result<()> {
    let changed = conn
        .execute(
            "UPDATE presets SET folder_id = ?2, name = ?3, tags_json = ?4, conditions_json = ?5 WHERE id = ?1",
            params![
                id.to_string(),
                folder_id.map(|f| f.to_string()),
                name,
                tags_to_json(tags),
                conditions_json
            ],
        )
        .map_err(map_sqlite_err)?;
    if changed == 0 {
        return Err(AppError::not_found("Preset", id.to_string()));
    }
    Ok(())
}

pub(crate) fn set_favorite(conn: &Connection, id: PresetId, is_favorite: bool) -> Result<()> {
    let changed = conn
        .execute(
            "UPDATE presets SET is_favorite = ?2 WHERE id = ?1",
            params![id.to_string(), is_favorite as i64],
        )
        .map_err(map_sqlite_err)?;
    if changed == 0 {
        return Err(AppError::not_found("Preset", id.to_string()));
    }
    Ok(())
}

pub(crate) fn delete(conn: &Connection, id: PresetId) -> Result<()> {
    let changed = conn
        .execute("DELETE FROM presets WHERE id = ?1", params![id.to_string()])
        .map_err(map_sqlite_err)?;
    if changed == 0 {
        return Err(AppError::not_found("Preset", id.to_string()));
    }
    Ok(())
}

pub(crate) fn list_all(conn: &Connection) -> Result<Vec<Preset>> {
    let mut stmt = conn
        .prepare(&format!(
            "SELECT {PRESET_COLUMNS} FROM presets ORDER BY created_at"
        ))
        .map_err(map_sqlite_err)?;
    let rows = stmt
        .query_map([], row_to_preset_raw)
        .map_err(map_sqlite_err)?;
    let mut result = Vec::new();
    for row in rows {
        result.push(raw_to_preset(row.map_err(map_sqlite_err)?)?);
    }
    Ok(result)
}

// ---- Versionen ------------------------------------------------------------

fn row_to_version_raw(row: &rusqlite::Row) -> rusqlite::Result<(String, String, i64, String, i64)> {
    Ok((
        row.get(0)?,
        row.get(1)?,
        row.get(2)?,
        row.get(3)?,
        row.get(4)?,
    ))
}

fn raw_to_version(raw: (String, String, i64, String, i64)) -> Result<PresetVersion> {
    let (id, preset_id, sequence, edl_subset_json, created_at) = raw;
    Ok(PresetVersion {
        id: id.parse()?,
        preset_id: preset_id.parse()?,
        sequence,
        edl_subset_json,
        created_at: from_unix(created_at)?,
    })
}

const VERSION_COLUMNS: &str = "id, preset_id, sequence, edl_subset_json, created_at";

/// Legt eine neue Version an (nächste `sequence` je `preset_id`) — die
/// vorherige Version bleibt unverändert erhalten (siehe Moduldoku:
/// Versionierung ohne Undo/Redo-Zeiger, immer die höchste `sequence`
/// zählt als aktuell).
pub(crate) fn create_version(
    conn: &Connection,
    preset_id: PresetId,
    edl_subset_json: &str,
    created_at: OffsetDateTime,
) -> Result<PresetVersionId> {
    let next_sequence: i64 = conn
        .query_row(
            "SELECT COALESCE(MAX(sequence), 0) + 1 FROM preset_versions WHERE preset_id = ?1",
            params![preset_id.to_string()],
            |row| row.get(0),
        )
        .map_err(map_sqlite_err)?;
    let id = PresetVersionId::new();
    conn.execute(
        "INSERT INTO preset_versions (id, preset_id, sequence, edl_subset_json, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            id.to_string(),
            preset_id.to_string(),
            next_sequence,
            edl_subset_json,
            to_unix(created_at)
        ],
    )
    .map_err(map_sqlite_err)?;
    Ok(id)
}

pub(crate) fn list_versions(conn: &Connection, preset_id: PresetId) -> Result<Vec<PresetVersion>> {
    let mut stmt = conn
        .prepare(&format!(
            "SELECT {VERSION_COLUMNS} FROM preset_versions WHERE preset_id = ?1 ORDER BY sequence"
        ))
        .map_err(map_sqlite_err)?;
    let rows = stmt
        .query_map(params![preset_id.to_string()], row_to_version_raw)
        .map_err(map_sqlite_err)?;
    let mut result = Vec::new();
    for row in rows {
        result.push(raw_to_version(row.map_err(map_sqlite_err)?)?);
    }
    Ok(result)
}

/// Die zuletzt angelegte Version — die tatsächlich anzuwendende EDL-
/// Teilmenge eines Presets.
pub(crate) fn latest_version(conn: &Connection, preset_id: PresetId) -> Result<PresetVersion> {
    let raw = conn
        .query_row(
            &format!(
                "SELECT {VERSION_COLUMNS} FROM preset_versions WHERE preset_id = ?1 ORDER BY sequence DESC LIMIT 1"
            ),
            params![preset_id.to_string()],
            row_to_version_raw,
        )
        .optional()
        .map_err(map_sqlite_err)?;
    match raw {
        Some(raw) => raw_to_version(raw),
        None => Err(AppError::not_found("Preset-Version", preset_id.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::migrations;

    fn setup() -> Connection {
        let conn = Connection::open_in_memory().expect("In-Memory-DB");
        conn.execute_batch("PRAGMA foreign_keys = ON")
            .expect("FKs an");
        migrations::apply(&conn).expect("Migration");
        conn
    }

    #[test]
    fn create_folder_assigns_increasing_sibling_positions() {
        let conn = setup();
        let a = create_folder(&conn, "A", None, OffsetDateTime::now_utc()).expect("ok");
        let b = create_folder(&conn, "B", None, OffsetDateTime::now_utc()).expect("ok");
        let folders = list_folders(&conn).expect("ok");
        assert_eq!(folders.len(), 2);
        assert_eq!(folders[0].id, a);
        assert_eq!(folders[0].position, 0);
        assert_eq!(folders[1].id, b);
        assert_eq!(folders[1].position, 1);
    }

    #[test]
    fn nested_folder_positions_are_independent_per_parent() {
        let conn = setup();
        let parent = create_folder(&conn, "Eltern", None, OffsetDateTime::now_utc()).expect("ok");
        let root_sibling =
            create_folder(&conn, "Wurzel-Geschwister", None, OffsetDateTime::now_utc())
                .expect("ok");
        let child =
            create_folder(&conn, "Kind", Some(parent), OffsetDateTime::now_utc()).expect("ok");

        let folders = list_folders(&conn).expect("ok");
        let child_folder = folders
            .iter()
            .find(|f| f.id == child)
            .expect("Kind vorhanden");
        assert_eq!(child_folder.position, 0, "eigener Zähler je Elternordner");
        let root_sibling_folder = folders
            .iter()
            .find(|f| f.id == root_sibling)
            .expect("vorhanden");
        assert_eq!(root_sibling_folder.position, 1);
    }

    #[test]
    fn deleting_folder_cascades_to_nested_folders_but_keeps_presets() {
        let conn = setup();
        let parent = create_folder(&conn, "Eltern", None, OffsetDateTime::now_utc()).expect("ok");
        let child =
            create_folder(&conn, "Kind", Some(parent), OffsetDateTime::now_utc()).expect("ok");
        let (preset_id, _) = create(
            &conn,
            Some(child),
            "Mein Preset",
            &["warm".to_string()],
            "[]",
            "{}",
            OffsetDateTime::now_utc(),
        )
        .expect("ok");

        delete_folder(&conn, parent).expect("ok");

        assert!(
            list_folders(&conn).expect("ok").is_empty(),
            "Eltern+Kind müssen weg sein"
        );
        let presets = list_all(&conn).expect("ok");
        assert_eq!(presets.len(), 1, "Preset bleibt erhalten");
        assert_eq!(presets[0].id, preset_id);
        assert_eq!(presets[0].folder_id, None, "rutscht an die Wurzel");
    }

    #[test]
    fn create_preset_stores_metadata_and_first_version() {
        let conn = setup();
        let (preset_id, version_id) = create(
            &conn,
            None,
            "Warmer Filmlook",
            &["warm".to_string(), "film".to_string()],
            "[]",
            r#"{"basic":{"exposure_ev":0.3}}"#,
            OffsetDateTime::now_utc(),
        )
        .expect("ok");

        let presets = list_all(&conn).expect("ok");
        assert_eq!(presets.len(), 1);
        assert_eq!(presets[0].id, preset_id);
        assert_eq!(presets[0].name, "Warmer Filmlook");
        assert_eq!(
            presets[0].tags,
            vec!["warm".to_string(), "film".to_string()]
        );
        assert!(!presets[0].is_favorite);

        let versions = list_versions(&conn, preset_id).expect("ok");
        assert_eq!(versions.len(), 1);
        assert_eq!(versions[0].id, version_id);
        assert_eq!(versions[0].sequence, 1);
        assert_eq!(
            versions[0].edl_subset_json,
            r#"{"basic":{"exposure_ev":0.3}}"#
        );
    }

    #[test]
    fn create_version_increments_sequence_and_keeps_old_versions() {
        let conn = setup();
        let (preset_id, first_version) = create(
            &conn,
            None,
            "Test",
            &[],
            "[]",
            r#"{"a":1}"#,
            OffsetDateTime::now_utc(),
        )
        .expect("ok");

        let second_version =
            create_version(&conn, preset_id, r#"{"a":2}"#, OffsetDateTime::now_utc()).expect("ok");

        let versions = list_versions(&conn, preset_id).expect("ok");
        assert_eq!(versions.len(), 2);
        assert_eq!(versions[0].id, first_version);
        assert_eq!(versions[1].id, second_version);
        assert_eq!(versions[1].sequence, 2);

        let latest = latest_version(&conn, preset_id).expect("ok");
        assert_eq!(latest.id, second_version);
        assert_eq!(latest.edl_subset_json, r#"{"a":2}"#);
    }

    #[test]
    fn update_metadata_changes_name_folder_tags_and_conditions() {
        let conn = setup();
        let (preset_id, _) = create(
            &conn,
            None,
            "Alt",
            &[],
            "[]",
            "{}",
            OffsetDateTime::now_utc(),
        )
        .expect("ok");
        let folder = create_folder(&conn, "Ordner", None, OffsetDateTime::now_utc()).expect("ok");

        update_metadata(
            &conn,
            preset_id,
            Some(folder),
            "Neu",
            &["neu".to_string()],
            r#"[{"field":"iso","op":">","value":"3200"}]"#,
        )
        .expect("ok");

        let presets = list_all(&conn).expect("ok");
        assert_eq!(presets[0].name, "Neu");
        assert_eq!(presets[0].folder_id, Some(folder));
        assert_eq!(presets[0].tags, vec!["neu".to_string()]);
        assert_eq!(
            presets[0].conditions_json,
            r#"[{"field":"iso","op":">","value":"3200"}]"#
        );
    }

    #[test]
    fn set_favorite_toggles_the_flag() {
        let conn = setup();
        let (preset_id, _) = create(
            &conn,
            None,
            "Test",
            &[],
            "[]",
            "{}",
            OffsetDateTime::now_utc(),
        )
        .expect("ok");

        set_favorite(&conn, preset_id, true).expect("ok");
        assert!(list_all(&conn).expect("ok")[0].is_favorite);

        set_favorite(&conn, preset_id, false).expect("ok");
        assert!(!list_all(&conn).expect("ok")[0].is_favorite);
    }

    #[test]
    fn deleting_preset_cascades_to_its_versions() {
        let conn = setup();
        let (preset_id, _) = create(
            &conn,
            None,
            "Test",
            &[],
            "[]",
            "{}",
            OffsetDateTime::now_utc(),
        )
        .expect("ok");

        delete(&conn, preset_id).expect("ok");

        assert!(list_all(&conn).expect("ok").is_empty());
        let version_count: i64 = conn
            .query_row("SELECT count(*) FROM preset_versions", [], |row| row.get(0))
            .expect("lesbar");
        assert_eq!(
            version_count, 0,
            "Versionen müssen per Kaskade gelöscht sein"
        );
    }

    #[test]
    fn renaming_or_deleting_unknown_folder_fails() {
        let conn = setup();
        assert!(rename_folder(&conn, PresetFolderId::new(), "x").is_err());
        assert!(delete_folder(&conn, PresetFolderId::new()).is_err());
    }

    #[test]
    fn latest_version_of_unknown_preset_fails() {
        let conn = setup();
        assert!(latest_version(&conn, PresetId::new()).is_err());
    }
}
