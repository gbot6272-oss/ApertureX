//! Volltextsuche (`photos_fts`, FTS5) und kombinierbarer Attributfilter
//! über `photos` — siehe `migrations/0003_library.sql`, `DECISIONS.md`
//! ADR-0023 und `PLAN.md` Phase 3, Schritt 2.

use apx_core::Result;
use rusqlite::types::ToSql;
use rusqlite::Connection;

use crate::error::map_sqlite_err;
use crate::models::{Aspect, FilterCriteria};
use crate::repository::photos::{raw_to_photo, row_to_raw, NOT_TRASHED, SELECT_COLUMNS};
use crate::Photo;

/// Trifft ein Foto über ein Schlagwort oder eine Bildnotiz?
///
/// Beides liegt in eigenen Tabellen (`photo_keywords`, `photo_notes`) und
/// kann deshalb nicht in `photos_fts` stehen: das ist eine
/// External-Content-Tabelle über `photos` und indiziert ausschliesslich
/// Spalten DIESER Tabelle (siehe `migrations/0015_search_all_text.sql`
/// und `DECISIONS.md` ADR-0070). Gesucht wird hier deshalb per
/// EXISTS-Unterabfrage mit `LIKE`.
///
/// `?N` ist der Platzhalter fuer den bereits als `%wort%` aufbereiteten
/// Suchbegriff.
fn sidecar_text_clause(index: usize) -> String {
    format!(
        "(EXISTS (SELECT 1 FROM photo_keywords pk \
                  JOIN keywords k ON k.id = pk.keyword_id \
                  WHERE pk.photo_id = photos.id AND k.name LIKE ?{index}) \
          OR EXISTS (SELECT 1 FROM photo_notes pn \
                     WHERE pn.photo_id = photos.id AND pn.body LIKE ?{index}))"
    )
}

/// Baut aus der Nutzereingabe das `LIKE`-Muster fuer
/// [`sidecar_text_clause`].
///
/// FTS5-Syntax (`titel:abend*`, Anfuehrungszeichen, `AND`/`OR`) ergibt
/// fuer ein `LIKE` keinen Sinn; uebrig bleibt der Text ohne diese
/// Sonderzeichen. Ein Suchausdruck, von dem dabei nichts uebrig bleibt,
/// liefert `None` — dann greift nur der FTS5-Teil.
fn like_pattern(query: &str) -> Option<String> {
    let cleaned: String = query
        .chars()
        .map(|c| match c {
            '"' | '*' | ':' | '(' | ')' | '^' | '-' => ' ',
            // `%` und `_` sind LIKE-Platzhalter — sonst wuerde eine
            // Eingabe mit `%` alles treffen.
            '%' | '_' => ' ',
            other => other,
        })
        .collect();
    let trimmed = cleaned.trim();
    if trimmed.is_empty() {
        return None;
    }
    Some(format!("%{trimmed}%"))
}

/// Volltextsuche über alle Textfelder eines Fotos: Dateiname,
/// Kamerahersteller/-modell, Objektiv, Titel, Beschriftung, Urheber und
/// Copyright (FTS5, `photos_fts`) sowie Schlagworte und Bildnotizen
/// (EXISTS, siehe [`sidecar_text_clause`]).
///
/// `query` wird unverändert als FTS5-Match-Ausdruck durchgereicht (erlaubt
/// also z. B. `filename:sonnenuntergang*` oder mehrere Wörter per UND) —
/// Ergebnisse nach FTS5-Relevanz (`rank`) sortiert. Treffer, die NUR über
/// ein Schlagwort oder eine Notiz kommen, haben keinen FTS5-Rang und
/// stehen deshalb hinter den Volltexttreffern.
pub(crate) fn search_photos(conn: &Connection, query: &str) -> Result<Vec<Photo>> {
    let fts_sql = format!(
        "SELECT {SELECT_COLUMNS}, rank AS ordering FROM photos_fts \
         JOIN photos ON photos.rowid = photos_fts.rowid \
         WHERE photos_fts MATCH ?1 AND {NOT_TRASHED}"
    );
    let Some(pattern) = like_pattern(query) else {
        let sql = format!("{fts_sql} ORDER BY ordering");
        return run_photo_query(conn, &sql, &[&query as &dyn ToSql]);
    };

    // Die konstante 1.0 sortiert die reinen Schlagwort-/Notiz-Treffer
    // hinter die FTS5-Treffer, deren `rank` negativ ist (je kleiner,
    // desto relevanter).
    //
    // `UNION` allein entfernt hier KEINE Dubletten: die beiden Zweige
    // liefern fuer dasselbe Foto verschiedene `ordering`-Werte, die
    // Zeilen sind damit nicht identisch. Deshalb schliesst der zweite
    // Zweig ausdruecklich aus, was der Volltext ohnehin schon trifft —
    // das ist auch semantisch richtig, weil ein Foto dann seinen
    // echten Relevanzrang behaelt statt hinten einsortiert zu werden.
    let sidecar = sidecar_text_clause(2);
    let sql = format!(
        "{fts_sql} \
         UNION ALL \
         SELECT {SELECT_COLUMNS}, 1.0 AS ordering FROM photos \
         WHERE {sidecar} AND {NOT_TRASHED} \
           AND photos.rowid NOT IN (SELECT rowid FROM photos_fts WHERE photos_fts MATCH ?1) \
         ORDER BY ordering"
    );
    run_photo_query(conn, &sql, &[&query as &dyn ToSql, &pattern as &dyn ToSql])
}

/// Fuehrt eine `SELECT {SELECT_COLUMNS}`-Abfrage aus und baut `Photo`s.
fn run_photo_query(conn: &Connection, sql: &str, values: &[&dyn ToSql]) -> Result<Vec<Photo>> {
    let mut stmt = conn.prepare(sql).map_err(map_sqlite_err)?;
    let rows = stmt
        .query_map(rusqlite::params_from_iter(values.iter()), row_to_raw)
        .map_err(map_sqlite_err)?;
    let mut result = Vec::new();
    for row in rows {
        result.push(raw_to_photo(row.map_err(map_sqlite_err)?)?);
    }
    Ok(result)
}

/// Baut WHERE-Klauseln und gebundene Werte aus `criteria` — gemeinsam
/// genutzt von [`filter_photos`] und [`search_and_filter_photos`] (siehe
/// `DECISIONS.md` ADR-0027), damit Attributfilter in beiden Fällen
/// identisch funktionieren. `start_index` ist die Anzahl bereits vergebener
/// `?N`-Platzhalter vor dieser Klausel (0, wenn keiner vorangeht — z. B.
/// `?1` für einen vorangestellten FTS5-Match-Ausdruck).
fn build_filter_clause(
    criteria: &FilterCriteria,
    start_index: usize,
) -> (Vec<String>, Vec<Box<dyn ToSql>>) {
    // Immer mitgeführt, nie abschaltbar: Fotos im Papierkorb (Phase 33 F1)
    // gehören in keine Filter- oder Suchtrefferliste. Hier statt in den
    // beiden Aufrufern, damit es nicht an einer Stelle fehlen kann.
    let mut clauses: Vec<String> = vec![NOT_TRASHED.to_string()];
    let mut values: Vec<Box<dyn ToSql>> = Vec::new();

    if let Some(min) = criteria.rating_at_least {
        values.push(Box::new(min as i64));
        clauses.push(format!("photos.rating >= ?{}", start_index + values.len()));
    }
    if let Some(flag) = criteria.flag {
        values.push(Box::new(flag as i64));
        clauses.push(format!("photos.flag = ?{}", start_index + values.len()));
    }
    if let Some(color) = &criteria.color_label {
        values.push(Box::new(color.clone()));
        clauses.push(format!(
            "photos.color_label = ?{}",
            start_index + values.len()
        ));
    }
    if let Some(model) = &criteria.camera_model {
        values.push(Box::new(model.clone()));
        clauses.push(format!(
            "photos.camera_model = ?{}",
            start_index + values.len()
        ));
    }

    if let Some(lens) = &criteria.lens {
        values.push(Box::new(lens.clone()));
        clauses.push(format!("photos.lens = ?{}", start_index + values.len()));
    }
    if let Some(min) = criteria.iso_min {
        values.push(Box::new(min as i64));
        clauses.push(format!(
            "photos.iso IS NOT NULL AND photos.iso >= ?{}",
            start_index + values.len()
        ));
    }
    if let Some(max) = criteria.iso_max {
        values.push(Box::new(max as i64));
        clauses.push(format!(
            "photos.iso IS NOT NULL AND photos.iso <= ?{}",
            start_index + values.len()
        ));
    }
    if let Some(from) = criteria.captured_from {
        values.push(Box::new(from));
        clauses.push(format!(
            "photos.captured_at IS NOT NULL AND photos.captured_at >= ?{}",
            start_index + values.len()
        ));
    }
    if let Some(to) = criteria.captured_to {
        values.push(Box::new(to));
        clauses.push(format!(
            "photos.captured_at IS NOT NULL AND photos.captured_at <= ?{}",
            start_index + values.len()
        ));
    }
    if let Some(aspect) = criteria.aspect {
        // Die *angezeigten* Kanten, nicht die gespeicherten: bei den
        // EXIF-Orientierungen 5..8 ist das Bild in der Datei gedreht
        // abgelegt, und wer nach „Hochformat" filtert, meint das Bild auf
        // dem Schirm. Siehe `Aspect`s Doku.
        let displayed_w = "(CASE WHEN photos.orientation IN (5,6,7,8) THEN photos.height \
                            ELSE photos.width END)";
        let displayed_h = "(CASE WHEN photos.orientation IN (5,6,7,8) THEN photos.width \
                            ELSE photos.height END)";
        let comparison = match aspect {
            Aspect::Landscape => ">",
            Aspect::Portrait => "<",
            Aspect::Square => "=",
        };
        clauses.push(format!(
            "photos.width IS NOT NULL AND photos.height IS NOT NULL \
             AND {displayed_w} {comparison} {displayed_h}"
        ));
    }
    if let Some(kind) = &criteria.media_kind {
        values.push(Box::new(kind.clone()));
        clauses.push(format!(
            "photos.media_kind = ?{}",
            start_index + values.len()
        ));
    }

    (clauses, values)
}

fn run_filtered_query(
    conn: &Connection,
    sql: &str,
    values: &[Box<dyn ToSql>],
) -> Result<Vec<Photo>> {
    let mut stmt = conn.prepare(sql).map_err(map_sqlite_err)?;
    let param_refs: Vec<&dyn ToSql> = values.iter().map(|v| v.as_ref()).collect();
    let rows = stmt
        .query_map(param_refs.as_slice(), row_to_raw)
        .map_err(map_sqlite_err)?;
    let mut result = Vec::new();
    for row in rows {
        result.push(raw_to_photo(row.map_err(map_sqlite_err)?)?);
    }
    Ok(result)
}

/// Kombiniert alle gesetzten Felder von `criteria` per UND. Ein komplett
/// leeres `criteria` (alle Felder `None`) liefert alle Fotos, sortiert nach
/// Dateiname — konsistent mit [`crate::repository::photos::list_by_folder`].
pub(crate) fn filter_photos(conn: &Connection, criteria: &FilterCriteria) -> Result<Vec<Photo>> {
    let (clauses, values) = build_filter_clause(criteria, 0);
    let where_clause = clauses.join(" AND ");
    let sql = format!("SELECT {SELECT_COLUMNS} FROM photos WHERE {where_clause} ORDER BY filename");
    run_filtered_query(conn, &sql, &values)
}

/// Kombiniert Volltextsuche (optional) mit Attributfiltern (UND-verknüpft)
/// — additiv zu [`search_photos`]/[`filter_photos`], die unverändert
/// weiter bestehen (siehe `DECISIONS.md` ADR-0027). Ein leerer/`None`-`query`
/// verhält sich identisch zu [`filter_photos`]; mit `query` werden die
/// FTS5-Treffer nach Relevanz sortiert (`rank`), die Kriterien schränken die
/// Trefferliste zusätzlich per UND ein.
pub(crate) fn search_and_filter_photos(
    conn: &Connection,
    query: Option<&str>,
    criteria: &FilterCriteria,
) -> Result<Vec<Photo>> {
    let Some(query) = query.filter(|q| !q.trim().is_empty()) else {
        return filter_photos(conn, criteria);
    };

    let pattern = like_pattern(query);

    // Die Attributfilter zaehlen ab ?2 (ohne Schlagwort-/Notiz-Zweig)
    // bzw. ab ?3 (mit ihm, weil ?2 dann das LIKE-Muster ist). Beide
    // Zweige der UNION benutzen DIESELBEN `?N` — positionsgebundene
    // Platzhalter duerfen mehrfach vorkommen, die Werte werden also nur
    // einmal gebunden.
    let filter_offset = if pattern.is_some() { 2 } else { 1 };
    let (filter_clauses, filter_values) = build_filter_clause(criteria, filter_offset);
    let filter_sql = if filter_clauses.is_empty() {
        String::new()
    } else {
        format!(" AND {}", filter_clauses.join(" AND "))
    };

    let mut values: Vec<Box<dyn ToSql>> = vec![Box::new(query.to_string())];
    if let Some(pattern) = &pattern {
        values.push(Box::new(pattern.clone()));
    }
    values.extend(filter_values);

    let fts_sql = format!(
        "SELECT {SELECT_COLUMNS}, rank AS ordering FROM photos_fts \
         JOIN photos ON photos.rowid = photos_fts.rowid \
         WHERE photos_fts MATCH ?1{filter_sql}"
    );
    let sql = match &pattern {
        None => format!("{fts_sql} ORDER BY ordering"),
        Some(_) => {
            // Siehe `search_photos`: der zweite Zweig schliesst die
            // FTS5-Treffer aus, sonst erschiene ein Foto, das ueber
            // beide Wege trifft, doppelt.
            let sidecar = sidecar_text_clause(2);
            format!(
                "{fts_sql} \
                 UNION ALL \
                 SELECT {SELECT_COLUMNS}, 1.0 AS ordering FROM photos \
                 WHERE {sidecar} AND {NOT_TRASHED}{filter_sql} \
                   AND photos.rowid NOT IN \
                       (SELECT rowid FROM photos_fts WHERE photos_fts MATCH ?1) \
                 ORDER BY ordering"
            )
        }
    };
    run_filtered_query(conn, &sql, &values)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::migrations;
    use crate::models::NewPhoto;
    use crate::repository::{folders, photos};
    use std::path::Path;
    use time::OffsetDateTime;

    fn setup() -> Connection {
        let conn = Connection::open_in_memory().expect("In-Memory-DB");
        migrations::apply(&conn).expect("Migration");
        conn
    }

    fn insert_photo(
        conn: &Connection,
        filename: &str,
        camera_model: Option<&str>,
    ) -> apx_core::PhotoId {
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
            camera_model: camera_model.map(|s| s.to_string()),
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
    fn search_finds_photo_by_filename_substring_token() {
        let conn = setup();
        let id = insert_photo(&conn, "Sonnenuntergang_Strand.CR2", None);
        insert_photo(&conn, "Bergwanderung.CR2", None);

        let results = search_photos(&conn, "Sonnenuntergang*").expect("ok");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, id);
    }

    #[test]
    fn search_finds_photo_by_camera_model() {
        let conn = setup();
        let id = insert_photo(&conn, "a.cr2", Some("EOS R5"));
        insert_photo(&conn, "b.cr2", Some("Z9"));

        let results = search_photos(&conn, "R5").expect("ok");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, id);
    }

    #[test]
    fn search_reflects_updates_via_sync_trigger() {
        let conn = setup();
        let id = insert_photo(&conn, "vorher.cr2", None);

        assert!(search_photos(&conn, "nachher").expect("ok").is_empty());

        conn.execute(
            "UPDATE photos SET filename = 'nachher.cr2' WHERE id = ?1",
            rusqlite::params![id.to_string()],
        )
        .expect("Update");

        let results = search_photos(&conn, "nachher").expect("ok");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, id);
        assert!(
            search_photos(&conn, "vorher").expect("ok").is_empty(),
            "alter Dateiname darf nach Umbenennung nicht mehr treffen"
        );
    }

    #[test]
    fn empty_filter_returns_all_photos() {
        let conn = setup();
        insert_photo(&conn, "a.cr2", None);
        insert_photo(&conn, "b.cr2", None);

        let results = filter_photos(&conn, &FilterCriteria::default()).expect("ok");
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn filter_combines_rating_and_camera_model_with_and() {
        let conn = setup();
        let matching = insert_photo(&conn, "a.cr2", Some("EOS R5"));
        let wrong_camera = insert_photo(&conn, "b.cr2", Some("Z9"));
        photos::set_rating(&conn, matching, 4).expect("ok");
        photos::set_rating(&conn, wrong_camera, 4).expect("ok");

        let criteria = FilterCriteria {
            rating_at_least: Some(3),
            camera_model: Some("EOS R5".to_string()),
            ..Default::default()
        };
        let results = filter_photos(&conn, &criteria).expect("ok");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, matching);
    }

    #[test]
    fn search_and_filter_combines_query_and_criteria_with_and() {
        let conn = setup();
        let matching = insert_photo(&conn, "Sonnenuntergang_Strand.CR2", Some("EOS R5"));
        let wrong_rating = insert_photo(&conn, "Sonnenuntergang_Berg.CR2", Some("EOS R5"));
        insert_photo(&conn, "Bergwanderung.CR2", Some("EOS R5"));
        photos::set_rating(&conn, matching, 4).expect("ok");
        photos::set_rating(&conn, wrong_rating, 1).expect("ok");

        let criteria = FilterCriteria {
            rating_at_least: Some(3),
            ..Default::default()
        };
        let results =
            search_and_filter_photos(&conn, Some("Sonnenuntergang*"), &criteria).expect("ok");
        assert_eq!(
            results.len(),
            1,
            "nur der Treffer, der sowohl den Suchtext als auch die Bewertung erfüllt"
        );
        assert_eq!(results[0].id, matching);
    }

    #[test]
    fn search_and_filter_without_query_behaves_like_filter_photos() {
        let conn = setup();
        let matching = insert_photo(&conn, "a.cr2", Some("EOS R5"));
        insert_photo(&conn, "b.cr2", Some("Z9"));

        let criteria = FilterCriteria {
            camera_model: Some("EOS R5".to_string()),
            ..Default::default()
        };
        let results = search_and_filter_photos(&conn, None, &criteria).expect("ok");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, matching);
    }

    #[test]
    fn search_and_filter_without_criteria_behaves_like_search_photos() {
        let conn = setup();
        let matching = insert_photo(&conn, "Sonnenuntergang.CR2", None);
        insert_photo(&conn, "Bergwanderung.CR2", None);

        let results =
            search_and_filter_photos(&conn, Some("Sonnenuntergang*"), &FilterCriteria::default())
                .expect("ok");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, matching);
    }

    #[test]
    fn search_and_filter_treats_blank_query_as_no_query() {
        let conn = setup();
        insert_photo(&conn, "a.cr2", Some("EOS R5"));
        insert_photo(&conn, "b.cr2", Some("Z9"));

        let results =
            search_and_filter_photos(&conn, Some("   "), &FilterCriteria::default()).expect("ok");
        assert_eq!(results.len(), 2, "leerer Suchtext zählt wie kein Suchtext");
    }

    // ---- Erweiterte Katalogfilter (Phase 33 F7) --------------------------

    /// Legt ein Foto mit den Feldern an, um die es in diesem Abschnitt
    /// geht — `insert_photo` oben deckt nur Dateiname und Kameramodell ab.
    #[allow(clippy::too_many_arguments)]
    fn insert_detailed(
        conn: &Connection,
        filename: &str,
        lens: Option<&str>,
        iso: Option<u32>,
        captured_at: Option<i64>,
        width: Option<u32>,
        height: Option<u32>,
        orientation: u16,
        media_kind: &str,
    ) -> apx_core::PhotoId {
        let folder_id =
            folders::find_or_create(conn, Path::new("/fotos"), None, OffsetDateTime::now_utc())
                .expect("Ordner");
        let photo = NewPhoto {
            media_kind: media_kind.to_string(),
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
            width,
            height,
            orientation,
            camera_make: None,
            camera_model: None,
            lens: lens.map(|s| s.to_string()),
            iso,
            shutter: None,
            aperture: None,
            focal_length: None,
            captured_at: captured_at.map(|t| OffsetDateTime::from_unix_timestamp(t).unwrap()),
            gps_lat: None,
            gps_lon: None,
        };
        photos::upsert(conn, &photo, OffsetDateTime::now_utc())
            .expect("Foto anlegen")
            .0
    }

    #[test]
    fn der_objektivfilter_trennt_zwei_objektive() {
        let conn = setup();
        let fifty = insert_detailed(
            &conn,
            "a.cr2",
            Some("RF 50mm"),
            None,
            None,
            None,
            None,
            1,
            "photo",
        );
        insert_detailed(
            &conn,
            "b.cr2",
            Some("RF 85mm"),
            None,
            None,
            None,
            None,
            1,
            "photo",
        );

        let criteria = FilterCriteria {
            lens: Some("RF 50mm".to_string()),
            ..Default::default()
        };
        let results = filter_photos(&conn, &criteria).expect("ok");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, fifty);
    }

    #[test]
    fn der_iso_bereich_schliesst_beide_grenzen_ein() {
        let conn = setup();
        insert_detailed(
            &conn,
            "a.cr2",
            None,
            Some(100),
            None,
            None,
            None,
            1,
            "photo",
        );
        insert_detailed(
            &conn,
            "b.cr2",
            None,
            Some(800),
            None,
            None,
            None,
            1,
            "photo",
        );
        insert_detailed(
            &conn,
            "c.cr2",
            None,
            Some(6400),
            None,
            None,
            None,
            1,
            "photo",
        );

        let criteria = FilterCriteria {
            iso_min: Some(100),
            iso_max: Some(800),
            ..Default::default()
        };
        assert_eq!(filter_photos(&conn, &criteria).expect("ok").len(), 2);
    }

    #[test]
    fn ein_foto_ohne_iso_faellt_aus_einem_iso_filter_heraus() {
        let conn = setup();
        insert_detailed(&conn, "ohne.cr2", None, None, None, None, None, 1, "photo");
        let criteria = FilterCriteria {
            iso_min: Some(100),
            ..Default::default()
        };
        assert!(filter_photos(&conn, &criteria).expect("ok").is_empty());
    }

    #[test]
    fn der_zeitraum_filtert_beide_grenzen_einschliesslich() {
        let conn = setup();
        insert_detailed(
            &conn,
            "frueh.cr2",
            None,
            None,
            Some(1_000),
            None,
            None,
            1,
            "photo",
        );
        insert_detailed(
            &conn,
            "mitte.cr2",
            None,
            None,
            Some(2_000),
            None,
            None,
            1,
            "photo",
        );
        insert_detailed(
            &conn,
            "spaet.cr2",
            None,
            None,
            Some(3_000),
            None,
            None,
            1,
            "photo",
        );

        let criteria = FilterCriteria {
            captured_from: Some(1_000),
            captured_to: Some(2_000),
            ..Default::default()
        };
        assert_eq!(filter_photos(&conn, &criteria).expect("ok").len(), 2);
    }

    #[test]
    fn ein_foto_ohne_datum_faellt_aus_einem_zeitraumfilter_heraus() {
        let conn = setup();
        insert_detailed(&conn, "ohne.cr2", None, None, None, None, None, 1, "photo");
        let criteria = FilterCriteria {
            captured_from: Some(0),
            ..Default::default()
        };
        assert!(filter_photos(&conn, &criteria).expect("ok").is_empty());
    }

    #[test]
    fn das_seitenverhaeltnis_richtet_sich_nach_der_exif_drehung() {
        let conn = setup();
        // Beide Fotos liegen mit 6000x4000 in der Datei. Das zweite trägt
        // EXIF-Orientierung 6 — es steht auf dem Schirm hochkant.
        let quer = insert_detailed(
            &conn,
            "quer.cr2",
            None,
            None,
            None,
            Some(6000),
            Some(4000),
            1,
            "photo",
        );
        let hoch = insert_detailed(
            &conn,
            "hoch.cr2",
            None,
            None,
            None,
            Some(6000),
            Some(4000),
            6,
            "photo",
        );

        let landscape = FilterCriteria {
            aspect: Some(Aspect::Landscape),
            ..Default::default()
        };
        let portrait = FilterCriteria {
            aspect: Some(Aspect::Portrait),
            ..Default::default()
        };
        assert_eq!(filter_photos(&conn, &landscape).expect("ok")[0].id, quer);
        assert_eq!(filter_photos(&conn, &portrait).expect("ok")[0].id, hoch);
    }

    #[test]
    fn ein_quadrat_ist_weder_quer_noch_hoch() {
        let conn = setup();
        let quadrat = insert_detailed(
            &conn,
            "q.cr2",
            None,
            None,
            None,
            Some(4000),
            Some(4000),
            1,
            "photo",
        );
        let square = FilterCriteria {
            aspect: Some(Aspect::Square),
            ..Default::default()
        };
        assert_eq!(filter_photos(&conn, &square).expect("ok")[0].id, quadrat);
        for aspect in [Aspect::Landscape, Aspect::Portrait] {
            let criteria = FilterCriteria {
                aspect: Some(aspect),
                ..Default::default()
            };
            assert!(filter_photos(&conn, &criteria).expect("ok").is_empty());
        }
    }

    #[test]
    fn ein_foto_ohne_abmessungen_faellt_aus_jedem_seitenverhaeltnis_heraus() {
        let conn = setup();
        insert_detailed(&conn, "ohne.cr2", None, None, None, None, None, 1, "photo");
        let criteria = FilterCriteria {
            aspect: Some(Aspect::Landscape),
            ..Default::default()
        };
        assert!(filter_photos(&conn, &criteria).expect("ok").is_empty());
    }

    #[test]
    fn die_medienart_trennt_foto_und_video() {
        let conn = setup();
        insert_detailed(&conn, "a.cr2", None, None, None, None, None, 1, "photo");
        let video = insert_detailed(&conn, "b.mp4", None, None, None, None, None, 1, "video");
        let criteria = FilterCriteria {
            media_kind: Some("video".to_string()),
            ..Default::default()
        };
        let results = filter_photos(&conn, &criteria).expect("ok");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, video);
    }

    #[test]
    fn mehrere_neue_kriterien_werden_per_und_verknuepft() {
        let conn = setup();
        let treffer = insert_detailed(
            &conn,
            "a.cr2",
            Some("RF 50mm"),
            Some(400),
            None,
            None,
            None,
            1,
            "photo",
        );
        // Richtiges Objektiv, falsches ISO.
        insert_detailed(
            &conn,
            "b.cr2",
            Some("RF 50mm"),
            Some(6400),
            None,
            None,
            None,
            1,
            "photo",
        );
        // Richtiges ISO, falsches Objektiv.
        insert_detailed(
            &conn,
            "c.cr2",
            Some("RF 85mm"),
            Some(400),
            None,
            None,
            None,
            1,
            "photo",
        );

        let criteria = FilterCriteria {
            lens: Some("RF 50mm".to_string()),
            iso_max: Some(800),
            ..Default::default()
        };
        let results = filter_photos(&conn, &criteria).expect("ok");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, treffer);
    }

    #[test]
    fn die_neuen_kriterien_wirken_auch_zusammen_mit_der_volltextsuche() {
        let conn = setup();
        insert_detailed(
            &conn,
            "sonnenuntergang_a.cr2",
            Some("RF 50mm"),
            None,
            None,
            None,
            None,
            1,
            "photo",
        );
        insert_detailed(
            &conn,
            "sonnenuntergang_b.cr2",
            Some("RF 85mm"),
            None,
            None,
            None,
            None,
            1,
            "photo",
        );

        let criteria = FilterCriteria {
            lens: Some("RF 50mm".to_string()),
            ..Default::default()
        };
        let results =
            search_and_filter_photos(&conn, Some("sonnenuntergang*"), &criteria).expect("ok");
        assert_eq!(results.len(), 1);
    }
}

#[cfg(test)]
mod all_text_search_tests {
    use super::*;
    use crate::migrations;
    use crate::models::NewPhoto;
    use crate::repository::{folders, keywords, notes, photos};
    use std::path::Path;
    use time::OffsetDateTime;

    /// Phase 34 F1 (siehe `DECISIONS.md` ADR-0070): die Suche deckte bis
    /// dahin nur Dateiname, Kamera und Objektiv ab. Genau die Felder, die
    /// der Nutzer selbst pflegt — Titel, Beschriftung, Urheber,
    /// Copyright, Schlagworte, Notizen — waren nicht auffindbar.
    fn setup() -> Connection {
        let conn = Connection::open_in_memory().expect("In-Memory-DB");
        migrations::apply(&conn).expect("Migration");
        conn
    }

    fn add_photo(conn: &Connection, filename: &str) -> apx_core::PhotoId {
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
    fn finds_a_photo_by_its_caption() {
        let conn = setup();
        let id = add_photo(&conn, "IMG_1.CR3");
        add_photo(&conn, "IMG_2.CR3");
        photos::set_metadata(&conn, id, None, Some("Abendrot über dem See"), None, None)
            .expect("Metadaten");

        let hits = search_photos(&conn, "Abendrot").expect("Suche");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].id, id);
    }

    #[test]
    fn finds_a_photo_by_title_creator_and_copyright() {
        let conn = setup();
        let id = add_photo(&conn, "IMG_1.CR3");
        photos::set_metadata(
            &conn,
            id,
            Some("Hafenpanorama"),
            None,
            Some("Musterlizenz"),
            Some("Nikla"),
        )
        .expect("Metadaten");

        for term in ["Hafenpanorama", "Musterlizenz", "Nikla"] {
            let hits = search_photos(&conn, term).expect("Suche");
            assert_eq!(hits.len(), 1, "'{term}' sollte genau ein Foto finden");
        }
    }

    #[test]
    fn finds_a_photo_by_keyword_although_the_index_only_covers_photos() {
        let conn = setup();
        let id = add_photo(&conn, "IMG_1.CR3");
        add_photo(&conn, "IMG_2.CR3");
        keywords::add(&conn, id, "Leuchtturm").expect("Schlagwort");

        let hits = search_photos(&conn, "Leuchtturm").expect("Suche");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].id, id);
    }

    #[test]
    fn finds_a_photo_by_the_body_of_a_note() {
        let conn = setup();
        let id = add_photo(&conn, "IMG_1.CR3");
        notes::create(
            &conn,
            id,
            0.5,
            0.5,
            "Staubfleck oben links retuschieren",
            OffsetDateTime::now_utc(),
        )
        .expect("Notiz");

        let hits = search_photos(&conn, "Staubfleck").expect("Suche");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].id, id);
    }

    #[test]
    fn returns_a_photo_only_once_when_it_matches_both_ways() {
        // Dasselbe Wort als Beschriftung UND als Schlagwort: die UNION
        // darf das Foto nicht doppelt liefern.
        let conn = setup();
        let id = add_photo(&conn, "IMG_1.CR3");
        photos::set_metadata(&conn, id, None, Some("Leuchtturm bei Nacht"), None, None)
            .expect("Metadaten");
        keywords::add(&conn, id, "Leuchtturm").expect("Schlagwort");

        let hits = search_photos(&conn, "Leuchtturm").expect("Suche");
        assert_eq!(hits.len(), 1);
    }

    #[test]
    fn keyword_hits_still_respect_the_attribute_filter() {
        // Der Schlagwort-Zweig der UNION muss dieselben Attributfilter
        // bekommen wie der Volltext-Zweig — sonst schmuggelte ein
        // Schlagworttreffer an jedem Filter vorbei.
        let conn = setup();
        let id = add_photo(&conn, "IMG_1.CR3");
        keywords::add(&conn, id, "Leuchtturm").expect("Schlagwort");
        photos::set_rating(&conn, id, 2).expect("Bewertung");

        let mut criteria = FilterCriteria {
            rating_at_least: Some(4),
            ..Default::default()
        };
        let hits = search_and_filter_photos(&conn, Some("Leuchtturm"), &criteria).expect("Suche");
        assert!(
            hits.is_empty(),
            "Bewertung 2 darf den Filter >=4 nicht passieren"
        );

        criteria.rating_at_least = Some(2);
        let hits = search_and_filter_photos(&conn, Some("Leuchtturm"), &criteria).expect("Suche");
        assert_eq!(hits.len(), 1);
    }

    #[test]
    fn a_trashed_photo_stays_out_of_both_branches() {
        let conn = setup();
        let id = add_photo(&conn, "IMG_1.CR3");
        keywords::add(&conn, id, "Leuchtturm").expect("Schlagwort");
        crate::repository::trash::trash_photos(
            &conn,
            &[id],
            crate::models::TrashReason::Manual,
            OffsetDateTime::now_utc(),
        )
        .expect("Papierkorb");

        assert!(search_photos(&conn, "Leuchtturm")
            .expect("Suche")
            .is_empty());
    }

    #[test]
    fn like_pattern_strips_fts_syntax_and_wildcards() {
        // `%`/`_` sind LIKE-Platzhalter: bliebe `%` stehen, traefe die
        // Schlagwortsuche jedes Foto.
        assert_eq!(like_pattern("abend*"), Some("%abend%".to_string()));
        assert_eq!(
            like_pattern("titel:abend"),
            Some("%titel abend%".to_string())
        );
        assert_eq!(like_pattern("%"), None);
        assert_eq!(like_pattern("   "), None);
    }
}
