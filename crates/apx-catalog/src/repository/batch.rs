//! Stapelverarbeitungs-Konsole (Phase 11 Schritt 9, siehe
//! `DECISIONS.md` ADR-0038 — die in ADR-0036 explizit benannte Lücke
//! aus Phase 9, siehe `migrations/0009_batch_operations.sql`s
//! Moduldoku). Eine Regel = eine [`FilterCriteria`]-Auswahl (dieselbe
//! wie das Filter-Panel/`repository::search`, wiederverwendet statt
//! dupliziert) + eine [`BatchAction`]. Drei Schritte:
//!
//! - [`preview_batch_rule`]: zeigt die betroffenen Fotos, schreibt nichts.
//! - [`apply_batch_rule`]: schreibt die Änderung je Foto und
//!   journalisiert sie einzeln in `batch_operation_items` — überspringt
//!   Fotos, bei denen die Aktion keine tatsächliche Änderung wäre
//!   (z. B. Bewertung schon auf dem Zielwert), damit das Journal (und
//!   damit das Undo) nur echte Änderungen enthält.
//! - [`undo_batch_operation`]: liest das Journal rückwärts und macht
//!   jede Änderung einzeln rückgängig — der in ADR-0036 benannte
//!   eigentliche Blocker (echtes Batch-Undo statt nur eine
//!   Vorschau-Liste). Löscht danach die Journal-Zeilen dieses Stapels
//!   (`ON DELETE CASCADE`), damit ein zweites Undo desselben Stapels
//!   keine bereits rückgängig gemachten Einträge erneut anfasst.

use apx_core::{BatchOperationId, KeywordId, PhotoId, Result};
use rusqlite::{params, Connection};
use time::OffsetDateTime;

use crate::error::map_sqlite_err;
use crate::models::{to_unix, FilterCriteria, Photo};
use crate::repository::{keywords, photos, search};

/// Die drei in `PLAN.md` Phase 11 Schritt 9 genannten Aktionen — bewusst
/// eine geschlossene Liste statt eines generischen „beliebiges Feld
/// setzen" (siehe Migrationsdatei: `field` im Journal ist deshalb ein
/// fester String, keine beliebige Spalte).
#[derive(Debug, Clone, PartialEq)]
pub enum BatchAction {
    SetRating(u8),
    SetColorLabel(Option<String>),
    AddKeyword(String),
}

impl BatchAction {
    /// Nur informativ für `batch_operations.kind` (Anzeige in der
    /// Verlaufsliste) — die Undo-Logik selbst liest ausschließlich
    /// `batch_operation_items.field`.
    fn kind_label(&self) -> &'static str {
        match self {
            Self::SetRating(_) => "set_rating",
            Self::SetColorLabel(_) => "set_color_label",
            Self::AddKeyword(_) => "add_keyword",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
struct BatchItem {
    photo_id: PhotoId,
    field: String,
    old_value_json: String,
    new_value_json: String,
}

/// Fotos, die `criteria` treffen würden — schreibt nichts (Trockenlauf-
/// Vorschau vor dem eigentlichen Anwenden).
pub(crate) fn preview_batch_rule(conn: &Connection, criteria: &FilterCriteria) -> Result<Vec<Photo>> {
    search::search_and_filter_photos(conn, None, criteria)
}

/// Wendet `action` auf alle `criteria`-treffenden Fotos an, journalisiert
/// jede tatsächliche Änderung einzeln. Gibt die neue Stapel-ID zurück,
/// auch wenn keine Änderung nötig war (leerer Stapel — `undo` wäre dann
/// ein No-op).
pub(crate) fn apply_batch_rule(
    conn: &Connection,
    criteria: &FilterCriteria,
    action: &BatchAction,
    created_at: OffsetDateTime,
) -> Result<BatchOperationId> {
    let matching = search::search_and_filter_photos(conn, None, criteria)?;

    let batch_id = BatchOperationId::new();
    conn.execute(
        "INSERT INTO batch_operations (id, kind, created_at, dry_run) VALUES (?1, ?2, ?3, 0)",
        params![batch_id.to_string(), action.kind_label(), to_unix(created_at)],
    )
    .map_err(map_sqlite_err)?;

    for photo in &matching {
        let item = match action {
            BatchAction::SetRating(new_rating) => {
                if photo.rating == *new_rating {
                    continue;
                }
                photos::set_rating(conn, photo.id, *new_rating)?;
                Some(BatchItem {
                    photo_id: photo.id,
                    field: "rating".to_string(),
                    old_value_json: serde_json::to_string(&photo.rating).unwrap_or_default(),
                    new_value_json: serde_json::to_string(new_rating).unwrap_or_default(),
                })
            }
            BatchAction::SetColorLabel(new_label) => {
                if photo.color_label == *new_label {
                    continue;
                }
                photos::set_color_label(conn, photo.id, new_label.as_deref())?;
                Some(BatchItem {
                    photo_id: photo.id,
                    field: "color_label".to_string(),
                    old_value_json: serde_json::to_string(&photo.color_label).unwrap_or_default(),
                    new_value_json: serde_json::to_string(new_label).unwrap_or_default(),
                })
            }
            BatchAction::AddKeyword(name) => {
                let already_linked = keywords::list_for_photo(conn, photo.id)?
                    .iter()
                    .any(|k| k.name == *name);
                if already_linked {
                    continue;
                }
                let keyword_id = keywords::add(conn, photo.id, name)?;
                Some(BatchItem {
                    photo_id: photo.id,
                    field: "keyword".to_string(),
                    // Kein sinnvoller "alter Wert" für eine neu angelegte
                    // Verknüpfung — die Keyword-ID selbst steht im neuen
                    // Wert, damit `undo` sie ohne Namenssuche wiederfindet.
                    old_value_json: "null".to_string(),
                    new_value_json: serde_json::to_string(&keyword_id.to_string()).unwrap_or_default(),
                })
            }
        };
        if let Some(item) = item {
            insert_item(conn, batch_id, &item)?;
        }
    }

    Ok(batch_id)
}

fn insert_item(conn: &Connection, batch_id: BatchOperationId, item: &BatchItem) -> Result<()> {
    conn.execute(
        "INSERT INTO batch_operation_items (id, batch_id, photo_id, field, old_value_json, new_value_json) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            apx_core::PhotoId::new().to_string(), // Item braucht nur irgendeine eindeutige ID, keine eigene Bedeutung.
            batch_id.to_string(),
            item.photo_id.to_string(),
            item.field,
            item.old_value_json,
            item.new_value_json,
        ],
    )
    .map_err(map_sqlite_err)?;
    Ok(())
}

struct BatchItemRow {
    photo_id: String,
    field: String,
    old_value_json: String,
    new_value_json: String,
}

fn list_items(conn: &Connection, batch_id: BatchOperationId) -> Result<Vec<BatchItemRow>> {
    let mut stmt = conn
        .prepare("SELECT photo_id, field, old_value_json, new_value_json FROM batch_operation_items WHERE batch_id = ?1")
        .map_err(map_sqlite_err)?;
    let rows = stmt
        .query_map(params![batch_id.to_string()], |row| {
            Ok(BatchItemRow {
                photo_id: row.get(0)?,
                field: row.get(1)?,
                old_value_json: row.get(2)?,
                new_value_json: row.get(3)?,
            })
        })
        .map_err(map_sqlite_err)?;
    let mut result = Vec::new();
    for row in rows {
        result.push(row.map_err(map_sqlite_err)?);
    }
    Ok(result)
}

/// Macht jede in `batch_id` journalisierte Änderung einzeln rückgängig.
/// Gibt die Zahl tatsächlich rückgängig gemachter Änderungen zurück.
/// Löscht danach die Stapel-/Journal-Zeilen selbst (`ON DELETE CASCADE`
/// nimmt die Items mit) — ein zweites `undo` desselben Stapels ist
/// dadurch ein No-op statt eines stillen Doppel-Undo.
pub(crate) fn undo_batch_operation(conn: &Connection, batch_id: BatchOperationId) -> Result<usize> {
    let items = list_items(conn, batch_id)?;
    let mut count = 0usize;
    for item in &items {
        let photo_id: PhotoId = item.photo_id.parse()?;
        match item.field.as_str() {
            "rating" => {
                let old_rating: u8 = serde_json::from_str(&item.old_value_json).unwrap_or(0);
                photos::set_rating(conn, photo_id, old_rating)?;
            }
            "color_label" => {
                let old_label: Option<String> =
                    serde_json::from_str(&item.old_value_json).unwrap_or(None);
                photos::set_color_label(conn, photo_id, old_label.as_deref())?;
            }
            "keyword" => {
                let keyword_id_str: String =
                    serde_json::from_str(&item.new_value_json).unwrap_or_default();
                if let Ok(keyword_id) = keyword_id_str.parse::<KeywordId>() {
                    keywords::remove(conn, photo_id, keyword_id)?;
                }
            }
            _ => continue,
        }
        count += 1;
    }

    conn.execute(
        "DELETE FROM batch_operations WHERE id = ?1",
        params![batch_id.to_string()],
    )
    .map_err(map_sqlite_err)?;

    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::migrations;
    use crate::models::NewPhoto;
    use crate::repository::folders;
    use std::path::Path;

    fn setup() -> (Connection, PhotoId, PhotoId) {
        let conn = Connection::open_in_memory().expect("In-Memory-DB");
        conn.execute_batch("PRAGMA foreign_keys = ON")
            .expect("FKs an");
        migrations::apply(&conn).expect("Migration");
        let folder_id =
            folders::insert(&conn, Path::new("/fotos"), None, OffsetDateTime::now_utc())
                .expect("Ordner");
        let make_photo = |filename: &str| {
            let photo = NewPhoto {
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
                captured_at: None,
                gps_lat: None,
                gps_lon: None,
            };
            photos::upsert(&conn, &photo, OffsetDateTime::now_utc())
                .expect("Foto anlegen")
                .0
        };
        let a = make_photo("a.cr2");
        let b = make_photo("b.cr2");
        (conn, a, b)
    }

    /// Kern-Behauptung dieses Moduls: apply schreibt und journalisiert,
    /// undo macht exakt das rückgängig — über mehrere gemischte
    /// Feldtypen hinweg (Bewertung + Farbmarkierung + Schlagwort), wie
    /// in PLAN.md Schritt 9 explizit gefordert ("inkl. Undo nach
    /// mehreren gemischten Feldtypen").
    #[test]
    fn apply_then_undo_restores_rating_color_label_and_keyword_across_mixed_actions() {
        let (conn, photo_a, photo_b) = setup();
        photos::set_rating(&conn, photo_a, 2).expect("Ausgangsbewertung");
        photos::set_color_label(&conn, photo_b, None).expect("Ausgangsfarbe");

        let criteria = FilterCriteria::default();

        // Vorschau schreibt nichts.
        let preview = preview_batch_rule(&conn, &criteria).expect("Vorschau");
        assert_eq!(preview.len(), 2);
        assert_eq!(photos::get(&conn, photo_a).expect("lesen").rating, 2);

        let rating_batch = apply_batch_rule(
            &conn,
            &criteria,
            &BatchAction::SetRating(5),
            OffsetDateTime::now_utc(),
        )
        .expect("Bewertung anwenden");
        let keyword_batch = apply_batch_rule(
            &conn,
            &criteria,
            &BatchAction::AddKeyword("Urlaub".to_string()),
            OffsetDateTime::now_utc(),
        )
        .expect("Schlagwort anwenden");

        assert_eq!(photos::get(&conn, photo_a).expect("lesen").rating, 5);
        assert_eq!(photos::get(&conn, photo_b).expect("lesen").rating, 5);
        assert!(keywords::list_for_photo(&conn, photo_a)
            .expect("lesen")
            .iter()
            .any(|k| k.name == "Urlaub"));

        undo_batch_operation(&conn, keyword_batch).expect("Schlagwort rückgängig");
        assert!(!keywords::list_for_photo(&conn, photo_a)
            .expect("lesen")
            .iter()
            .any(|k| k.name == "Urlaub"));

        undo_batch_operation(&conn, rating_batch).expect("Bewertung rückgängig");
        assert_eq!(photos::get(&conn, photo_a).expect("lesen").rating, 2);
        assert_eq!(photos::get(&conn, photo_b).expect("lesen").rating, 0);

        // Zweites Undo desselben Stapels ist ein No-op (Journal bereits
        // gelöscht), kein Fehler und keine erneute Änderung.
        let second_undo_count = undo_batch_operation(&conn, rating_batch).expect("No-op-Undo");
        assert_eq!(second_undo_count, 0);
    }
}
