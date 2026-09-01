//! SQL für `templates` (siehe `migrations/0006_templates.sql`s
//! Moduldoku) — eine generische Tabelle für alle Vorlagen-Arten
//! (Export/Druck/Buch/Diashow/Web/Workflow), unterschieden über `kind`.
//! Bewusst keine fünf fast identischen Tabellen: jede Vorlage ist ohnehin
//! nur ein benannter, gespeicherter Parametersatz desselben JSON, das die
//! jeweiligen Dialoge schon über den Tauri-IPC schicken.

use apx_core::{Result, TemplateId};
use rusqlite::{params, Connection};
use time::OffsetDateTime;

use crate::error::map_sqlite_err;
use crate::models::{from_unix, to_unix, Template};

struct TemplateRow {
    id: String,
    kind: String,
    name: String,
    payload_json: String,
    created_at: i64,
}

fn row_to_raw(row: &rusqlite::Row) -> rusqlite::Result<TemplateRow> {
    Ok(TemplateRow {
        id: row.get(0)?,
        kind: row.get(1)?,
        name: row.get(2)?,
        payload_json: row.get(3)?,
        created_at: row.get(4)?,
    })
}

fn raw_to_template(raw: TemplateRow) -> Result<Template> {
    Ok(Template {
        id: raw.id.parse()?,
        kind: raw.kind,
        name: raw.name,
        payload_json: raw.payload_json,
        created_at: from_unix(raw.created_at)?,
    })
}

const SELECT_COLUMNS: &str = "id, kind, name, payload_json, created_at";

/// Legt eine neue Vorlage an — kein Überschreiben bei gleichem Namen
/// innerhalb derselben `kind` (bewusst einfach, wie bei Presets: mehrere
/// Vorlagen dürfen denselben Namen tragen, unterschieden über ihre ID).
pub(crate) fn create(
    conn: &Connection,
    kind: &str,
    name: &str,
    payload_json: &str,
    created_at: OffsetDateTime,
) -> Result<TemplateId> {
    let id = TemplateId::new();
    conn.execute(
        "INSERT INTO templates (id, kind, name, payload_json, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![id.to_string(), kind, name, payload_json, to_unix(created_at)],
    )
    .map_err(map_sqlite_err)?;
    Ok(id)
}

/// Alle Vorlagen einer Art, alphabetisch nach Namen.
pub(crate) fn list_by_kind(conn: &Connection, kind: &str) -> Result<Vec<Template>> {
    let mut stmt = conn
        .prepare(&format!(
            "SELECT {SELECT_COLUMNS} FROM templates WHERE kind = ?1 ORDER BY name"
        ))
        .map_err(map_sqlite_err)?;
    let rows = stmt
        .query_map(params![kind], row_to_raw)
        .map_err(map_sqlite_err)?;
    let mut result = Vec::new();
    for row in rows {
        result.push(raw_to_template(row.map_err(map_sqlite_err)?)?);
    }
    Ok(result)
}

pub(crate) fn get(conn: &Connection, id: TemplateId) -> Result<Template> {
    let sql = format!("SELECT {SELECT_COLUMNS} FROM templates WHERE id = ?1");
    let raw = conn
        .query_row(&sql, params![id.to_string()], row_to_raw)
        .map_err(map_sqlite_err)?;
    raw_to_template(raw)
}

pub(crate) fn delete(conn: &Connection, id: TemplateId) -> Result<()> {
    conn.execute(
        "DELETE FROM templates WHERE id = ?1",
        params![id.to_string()],
    )
    .map_err(map_sqlite_err)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::migrations;

    fn setup() -> Connection {
        let conn = Connection::open_in_memory().expect("In-Memory-DB");
        migrations::apply(&conn).expect("Migration");
        conn
    }

    #[test]
    fn create_then_list_roundtrips_payload() {
        let conn = setup();
        create(
            &conn,
            "print",
            "A4 Kontaktbogen",
            r#"{"cols":4,"rows":5}"#,
            OffsetDateTime::now_utc(),
        )
        .expect("anlegen");
        let templates = list_by_kind(&conn, "print").expect("liste");
        assert_eq!(templates.len(), 1);
        assert_eq!(templates[0].name, "A4 Kontaktbogen");
        assert_eq!(templates[0].payload_json, r#"{"cols":4,"rows":5}"#);
    }

    #[test]
    fn list_by_kind_only_returns_matching_kind() {
        let conn = setup();
        create(
            &conn,
            "print",
            "Druck-Vorlage",
            "{}",
            OffsetDateTime::now_utc(),
        )
        .expect("anlegen");
        create(
            &conn,
            "book",
            "Buch-Vorlage",
            "{}",
            OffsetDateTime::now_utc(),
        )
        .expect("anlegen");

        assert_eq!(list_by_kind(&conn, "print").expect("liste").len(), 1);
        assert_eq!(list_by_kind(&conn, "book").expect("liste").len(), 1);
        assert_eq!(list_by_kind(&conn, "web").expect("liste").len(), 0);
    }

    #[test]
    fn get_and_delete_work() {
        let conn = setup();
        let id = create(
            &conn,
            "workflow",
            "Urlaub-Export",
            "{}",
            OffsetDateTime::now_utc(),
        )
        .expect("anlegen");

        assert_eq!(get(&conn, id).expect("get").name, "Urlaub-Export");

        delete(&conn, id).expect("löschen");
        assert!(get(&conn, id).is_err());
        assert_eq!(list_by_kind(&conn, "workflow").expect("liste"), Vec::new());
    }

    #[test]
    fn same_name_may_exist_more_than_once_across_different_ids() {
        let conn = setup();
        create(&conn, "export", "Standard", "{}", OffsetDateTime::now_utc()).expect("anlegen 1");
        create(&conn, "export", "Standard", "{}", OffsetDateTime::now_utc()).expect("anlegen 2");
        assert_eq!(list_by_kind(&conn, "export").expect("liste").len(), 2);
    }
}
