//! SQL für `tag_rules` (siehe `migrations/0008_metadata_keywords.sql`s
//! Moduldoku) — bedingte Auto-Schlagwort-Regeln. `conditions_json` wird
//! hier nur durchgereicht, nicht ausgewertet (die Auswertung passiert im
//! Frontend über dieselbe `evaluateConditions`-Funktion wie bei
//! Import-Presets, siehe `frontend/src/lib/presets.ts`).

use apx_core::{KeywordId, Result, TagRuleId};
use rusqlite::{params, Connection};
use time::OffsetDateTime;

use crate::error::map_sqlite_err;
use crate::models::{to_unix, TagRule};

const SELECT_COLUMNS: &str = "id, name, keyword_id, conditions_json, enabled, created_at";

fn row_to_rule(
    row: &rusqlite::Row,
) -> rusqlite::Result<(String, String, String, String, i64, i64)> {
    Ok((
        row.get(0)?,
        row.get(1)?,
        row.get(2)?,
        row.get(3)?,
        row.get(4)?,
        row.get(5)?,
    ))
}

fn raw_to_rule(raw: (String, String, String, String, i64, i64)) -> Result<TagRule> {
    let (id, name, keyword_id, conditions_json, enabled, created_at) = raw;
    Ok(TagRule {
        id: id.parse()?,
        name,
        keyword_id: keyword_id.parse()?,
        conditions_json,
        enabled: enabled != 0,
        created_at: crate::models::from_unix(created_at)?,
    })
}

pub(crate) fn create(
    conn: &Connection,
    name: &str,
    keyword_id: KeywordId,
    conditions_json: &str,
    created_at: OffsetDateTime,
) -> Result<TagRuleId> {
    let id = TagRuleId::new();
    conn.execute(
        "INSERT INTO tag_rules (id, name, keyword_id, conditions_json, enabled, created_at) \
         VALUES (?1, ?2, ?3, ?4, 1, ?5)",
        params![
            id.to_string(),
            name,
            keyword_id.to_string(),
            conditions_json,
            to_unix(created_at)
        ],
    )
    .map_err(map_sqlite_err)?;
    Ok(id)
}

pub(crate) fn set_enabled(conn: &Connection, id: TagRuleId, enabled: bool) -> Result<()> {
    conn.execute(
        "UPDATE tag_rules SET enabled = ?2 WHERE id = ?1",
        params![id.to_string(), enabled as i64],
    )
    .map_err(map_sqlite_err)?;
    Ok(())
}

pub(crate) fn delete(conn: &Connection, id: TagRuleId) -> Result<()> {
    conn.execute(
        "DELETE FROM tag_rules WHERE id = ?1",
        params![id.to_string()],
    )
    .map_err(map_sqlite_err)?;
    Ok(())
}

/// Alle Regeln, auch deaktivierte — der Aufrufer (Import-Ablauf im
/// Frontend) filtert selbst auf `enabled`.
pub(crate) fn list_all(conn: &Connection) -> Result<Vec<TagRule>> {
    let sql = format!("SELECT {SELECT_COLUMNS} FROM tag_rules ORDER BY name");
    let mut stmt = conn.prepare(&sql).map_err(map_sqlite_err)?;
    let rows = stmt.query_map([], row_to_rule).map_err(map_sqlite_err)?;
    let mut result = Vec::new();
    for row in rows {
        result.push(raw_to_rule(row.map_err(map_sqlite_err)?)?);
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::migrations;
    use crate::repository::keywords;

    fn setup() -> (Connection, KeywordId) {
        let conn = Connection::open_in_memory().expect("In-Memory-DB");
        conn.execute_batch("PRAGMA foreign_keys = ON")
            .expect("FKs an");
        migrations::apply(&conn).expect("Migration");
        let keyword_id = keywords::find_or_create(&conn, "Berge").expect("Schlagwort");
        (conn, keyword_id)
    }

    #[test]
    fn create_and_list_round_trips() {
        let (conn, keyword_id) = setup();
        create(
            &conn,
            "Berge bei ISO>800",
            keyword_id,
            "[]",
            OffsetDateTime::now_utc(),
        )
        .expect("anlegen");

        let rules = list_all(&conn).expect("liste");
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].name, "Berge bei ISO>800");
        assert!(rules[0].enabled);
    }

    #[test]
    fn set_enabled_and_delete() {
        let (conn, keyword_id) = setup();
        let id =
            create(&conn, "Regel", keyword_id, "[]", OffsetDateTime::now_utc()).expect("anlegen");

        set_enabled(&conn, id, false).expect("deaktivieren");
        assert!(!list_all(&conn).expect("liste")[0].enabled);

        delete(&conn, id).expect("löschen");
        assert!(list_all(&conn).expect("liste").is_empty());
    }

    #[test]
    fn deleting_keyword_cascades_to_its_tag_rules() {
        let (conn, keyword_id) = setup();
        create(&conn, "Regel", keyword_id, "[]", OffsetDateTime::now_utc()).expect("anlegen");

        keywords::delete(&conn, keyword_id).expect("Schlagwort löschen");

        assert!(list_all(&conn).expect("liste").is_empty());
    }
}
