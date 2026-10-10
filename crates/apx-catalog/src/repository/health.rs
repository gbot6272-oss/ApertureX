//! Katalog-Gesundheit: was im Bestand noch Arbeit braucht
//! (Phase 34 F5, siehe `DECISIONS.md` ADR-0070).
//!
//! **Wozu.** Ein über Jahre gewachsener Katalog hat blinde Flecken, die
//! niemand sucht, weil man sie nicht sieht: Fotos, die nie eine
//! Bewertung bekommen haben, Schlagwortlücken, Dateien, die außerhalb
//! der App verschoben wurden, Aufnahmen ohne Datum. Jede dieser Lücken
//! ist einzeln über Filter auffindbar — aber nur, wenn man auf die Idee
//! kommt, danach zu suchen. Diese Abfrage stellt sie ungefragt
//! nebeneinander.
//!
//! **Warum Zahlen und Liste getrennt sind.** Die Übersicht braucht nur
//! Zähler und muss schnell sein; die Liste holt der Nutzer erst für die
//! eine Kategorie, die ihn interessiert. Alle IDs vorab mitzuliefern
//! hieße, bei einem großen Katalog zehntausende Zeilen zu übertragen,
//! von denen er neun Zehntel nie ansieht.
//!
//! **Was NICHT als Mangel gilt.** Fotos im Papierkorb tauchen in keiner
//! Kategorie auf — sie sind bereits aussortiert, sie „fehlen" nicht.
//! Virtuelle Kopien ebenfalls nicht: sie haben absichtlich keine eigene
//! Datei und teilen sich die Metadaten ihres Originals.

use apx_core::Result;
use rusqlite::Connection;

use crate::error::map_sqlite_err;
use crate::repository::photos::{raw_to_photo, row_to_raw, NOT_TRASHED, SELECT_COLUMNS};
use crate::Photo;

/// Welche Lücke gemeint ist.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HealthKind {
    /// Kein einziges Schlagwort.
    WithoutKeywords,
    /// Bewertung 0 — also nie beurteilt.
    WithoutRating,
    /// Kein Aufnahmedatum (siehe ADR-0068: bis Phase 34 der Normalfall
    /// für jedes importierte JPEG).
    WithoutCaptureDate,
    /// Keine Koordinaten.
    WithoutPosition,
    /// Originaldatei nicht mehr am erwarteten Ort.
    Missing,
    /// Offene, noch nicht erledigte Bildnotiz.
    WithOpenNotes,
}

impl HealthKind {
    /// Der Bezeichner, unter dem das Frontend die Kategorie anspricht.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::WithoutKeywords => "without_keywords",
            Self::WithoutRating => "without_rating",
            Self::WithoutCaptureDate => "without_capture_date",
            Self::WithoutPosition => "without_position",
            Self::Missing => "missing",
            Self::WithOpenNotes => "with_open_notes",
        }
    }

    /// Umkehrung von [`as_str`](Self::as_str) — unbekannte Bezeichner
    /// ergeben `None`, statt still auf eine Kategorie zu raten.
    pub fn from_str_opt(value: &str) -> Option<Self> {
        Self::ALL
            .iter()
            .copied()
            .find(|kind| kind.as_str() == value)
    }

    pub const ALL: [HealthKind; 6] = [
        HealthKind::Missing,
        HealthKind::WithOpenNotes,
        HealthKind::WithoutCaptureDate,
        HealthKind::WithoutKeywords,
        HealthKind::WithoutRating,
        HealthKind::WithoutPosition,
    ];

    /// Die WHERE-Bedingung dieser Kategorie — eine Quelle für Zähler
    /// und Liste, damit beide nie auseinanderlaufen können.
    fn condition(self) -> &'static str {
        match self {
            Self::WithoutKeywords => {
                "NOT EXISTS (SELECT 1 FROM photo_keywords pk WHERE pk.photo_id = photos.id)"
            }
            Self::WithoutRating => "photos.rating = 0",
            Self::WithoutCaptureDate => "photos.captured_at IS NULL",
            Self::WithoutPosition => "photos.gps_lat IS NULL OR photos.gps_lon IS NULL",
            Self::Missing => "photos.missing = 1",
            Self::WithOpenNotes => {
                "EXISTS (SELECT 1 FROM photo_notes pn WHERE pn.photo_id = photos.id AND pn.done = 0)"
            }
        }
    }
}

/// Die vollständige WHERE-Klausel einer Kategorie, inklusive der
/// Ausschlüsse, die für ALLE gelten.
fn where_clause(kind: HealthKind) -> String {
    format!(
        "({}) AND {NOT_TRASHED} AND photos.source_photo_id IS NULL",
        kind.condition()
    )
}

/// Wie viele Fotos fallen in diese Kategorie?
pub(crate) fn count(conn: &Connection, kind: HealthKind) -> Result<u64> {
    let sql = format!("SELECT COUNT(*) FROM photos WHERE {}", where_clause(kind));
    let value: i64 = conn
        .query_row(&sql, [], |row| row.get(0))
        .map_err(map_sqlite_err)?;
    Ok(value as u64)
}

/// Zähler für alle Kategorien, in der Reihenfolge von
/// [`HealthKind::ALL`] — diese Reihenfolge ist die Anzeigereihenfolge:
/// fehlende Dateien zuerst, weil sie echten Datenverlust bedeuten
/// können, Bewertungs- und Ortslücken zuletzt, weil sie bloß unbequem
/// sind.
pub(crate) fn summary(conn: &Connection) -> Result<Vec<(HealthKind, u64)>> {
    HealthKind::ALL
        .iter()
        .map(|&kind| Ok((kind, count(conn, kind)?)))
        .collect()
}

/// Die Fotos einer Kategorie, höchstens `limit` Stück.
pub(crate) fn list(conn: &Connection, kind: HealthKind, limit: usize) -> Result<Vec<Photo>> {
    let sql = format!(
        "SELECT {SELECT_COLUMNS} FROM photos WHERE {} ORDER BY photos.filename LIMIT ?1",
        where_clause(kind)
    );
    let mut stmt = conn.prepare(&sql).map_err(map_sqlite_err)?;
    let rows = stmt
        .query_map(rusqlite::params![limit as i64], row_to_raw)
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
    use crate::models::{NewPhoto, TrashReason};
    use crate::repository::{folders, keywords, notes, photos, trash};
    use std::path::Path;
    use time::OffsetDateTime;

    fn setup() -> Connection {
        let conn = Connection::open_in_memory().expect("In-Memory-DB");
        migrations::apply(&conn).expect("Migration");
        conn
    }

    fn add(conn: &Connection, filename: &str) -> apx_core::PhotoId {
        let folder_id =
            folders::find_or_create(conn, Path::new("/fotos"), None, OffsetDateTime::now_utc())
                .expect("Ordner");
        let photo = NewPhoto {
            media_kind: "photo".to_string(),
            duration_ms: None,
            video_codec: None,
            has_audio: None,
            frame_rate: None,
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
        photos::upsert(conn, &photo, OffsetDateTime::now_utc())
            .expect("Foto")
            .0
    }

    #[test]
    fn a_fresh_photo_shows_up_in_every_lack_category() {
        let conn = setup();
        add(&conn, "IMG_1.CR3");
        assert_eq!(count(&conn, HealthKind::WithoutKeywords).unwrap(), 1);
        assert_eq!(count(&conn, HealthKind::WithoutRating).unwrap(), 1);
        assert_eq!(count(&conn, HealthKind::WithoutCaptureDate).unwrap(), 1);
        assert_eq!(count(&conn, HealthKind::WithoutPosition).unwrap(), 1);
        // Aber nicht in den beiden, die einen echten Vorgang brauchen.
        assert_eq!(count(&conn, HealthKind::Missing).unwrap(), 0);
        assert_eq!(count(&conn, HealthKind::WithOpenNotes).unwrap(), 0);
    }

    #[test]
    fn filling_a_gap_removes_the_photo_from_that_category() {
        let conn = setup();
        let id = add(&conn, "IMG_1.CR3");
        keywords::add(&conn, id, "Hafen").expect("Schlagwort");
        photos::set_rating(&conn, id, 3).expect("Bewertung");
        photos::set_gps(&conn, id, Some((50.0, 8.0))).expect("Ort");

        assert_eq!(count(&conn, HealthKind::WithoutKeywords).unwrap(), 0);
        assert_eq!(count(&conn, HealthKind::WithoutRating).unwrap(), 0);
        assert_eq!(count(&conn, HealthKind::WithoutPosition).unwrap(), 0);
        // Das Datum wurde nicht gesetzt — die Kategorie bleibt.
        assert_eq!(count(&conn, HealthKind::WithoutCaptureDate).unwrap(), 1);
    }

    #[test]
    fn a_trashed_photo_counts_in_no_category() {
        let conn = setup();
        let id = add(&conn, "IMG_1.CR3");
        trash::trash_photos(&conn, &[id], TrashReason::Manual, OffsetDateTime::now_utc())
            .expect("Papierkorb");
        for kind in HealthKind::ALL {
            assert_eq!(count(&conn, kind).unwrap(), 0, "{}", kind.as_str());
        }
    }

    #[test]
    fn a_virtual_copy_is_not_a_gap() {
        // Sie hat absichtlich keine eigene Datei und teilt die
        // Metadaten des Originals — als "Luecke" zu zaehlen waere eine
        // Falschmeldung, die sich nicht schliessen liesse.
        let conn = setup();
        let id = add(&conn, "IMG_1.CR3");
        photos::create_virtual_copy(&conn, id, OffsetDateTime::now_utc()).expect("Kopie");
        assert_eq!(count(&conn, HealthKind::WithoutRating).unwrap(), 1);
    }

    #[test]
    fn an_open_note_shows_up_and_a_done_one_does_not() {
        let conn = setup();
        let id = add(&conn, "IMG_1.CR3");
        let note =
            notes::create(&conn, id, 0.5, 0.5, "Staub", OffsetDateTime::now_utc()).expect("Notiz");
        assert_eq!(count(&conn, HealthKind::WithOpenNotes).unwrap(), 1);

        notes::update(
            &conn,
            &note.id.to_string(),
            None,
            Some(true),
            None,
            OffsetDateTime::now_utc(),
        )
        .expect("erledigt");
        assert_eq!(count(&conn, HealthKind::WithOpenNotes).unwrap(), 0);
    }

    #[test]
    fn the_list_matches_the_count_of_the_same_category() {
        let conn = setup();
        for i in 0..3 {
            let id = add(&conn, &format!("IMG_{i}.CR3"));
            if i == 0 {
                photos::set_rating(&conn, id, 4).expect("Bewertung");
            }
        }
        let kind = HealthKind::WithoutRating;
        assert_eq!(count(&conn, kind).unwrap(), 2);
        assert_eq!(list(&conn, kind, 100).unwrap().len(), 2);
    }

    #[test]
    fn the_list_honours_its_limit() {
        let conn = setup();
        for i in 0..5 {
            add(&conn, &format!("IMG_{i}.CR3"));
        }
        assert_eq!(list(&conn, HealthKind::WithoutRating, 2).unwrap().len(), 2);
    }

    #[test]
    fn the_summary_reports_every_category_once() {
        let conn = setup();
        let rows = summary(&conn).expect("Uebersicht");
        assert_eq!(rows.len(), HealthKind::ALL.len());
    }

    #[test]
    fn an_unknown_kind_is_rejected_instead_of_guessed() {
        assert_eq!(
            HealthKind::from_str_opt("without_rating"),
            Some(HealthKind::WithoutRating)
        );
        assert_eq!(HealthKind::from_str_opt("erfunden"), None);
    }
}
