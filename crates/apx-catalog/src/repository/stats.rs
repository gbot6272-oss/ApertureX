//! Katalog-Statistik-Dashboard (Phase 9 Schritt 3, siehe `PLAN.md`/
//! `DECISIONS.md` ADR-0035) — reine Aggregatabfragen über `photos`, keine
//! neue Tabelle. Absichtlich in einem eigenen Modul statt in `photos.rs`,
//! weil es rein lesend/aggregierend ist statt einzelne Zeilen zu
//! manipulieren.

use apx_core::Result;
use rusqlite::Connection;

use crate::error::map_sqlite_err;
use crate::models::{CatalogStatistics, DistributionBucket, ExposureRow, GearStatistics};

const TOP_N: usize = 8;

fn top_value_counts(conn: &Connection, column: &str) -> Result<Vec<(String, u64)>> {
    // `column` kommt ausschließlich aus dieser Datei fest verdrahtet
    // (nie aus Nutzereingabe) — String-Interpolation hier ist deshalb
    // sicher, ein gebundener Parameter wäre für einen Spaltennamen ohnehin
    // nicht möglich (SQLite bindet nur Werte, keine Bezeichner).
    let sql = format!(
        "SELECT {column}, COUNT(*) as cnt FROM photos \
         WHERE {column} IS NOT NULL AND source_photo_id IS NULL \
         GROUP BY {column} ORDER BY cnt DESC, {column} ASC LIMIT {TOP_N}"
    );
    let mut stmt = conn.prepare(&sql).map_err(map_sqlite_err)?;
    let rows = stmt
        .query_map([], |row| {
            let value: String = row.get(0)?;
            let count: i64 = row.get(1)?;
            Ok((value, count as u64))
        })
        .map_err(map_sqlite_err)?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(map_sqlite_err)
}

fn rating_distribution(conn: &Connection) -> Result<Vec<(u8, u64)>> {
    let mut stmt = conn
        .prepare(
            "SELECT rating, COUNT(*) FROM photos WHERE source_photo_id IS NULL \
             GROUP BY rating ORDER BY rating ASC",
        )
        .map_err(map_sqlite_err)?;
    let rows = stmt
        .query_map([], |row| {
            let rating: i64 = row.get(0)?;
            let count: i64 = row.get(1)?;
            Ok((rating as u8, count as u64))
        })
        .map_err(map_sqlite_err)?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(map_sqlite_err)
}

/// Aggregierte Katalog-Statistik — schließt virtuelle Kopien
/// (`source_photo_id IS NOT NULL`) konsequent aus, damit z. B. die
/// Foto-Gesamtzahl den tatsächlichen Datei-Bestand zeigt, nicht durch
/// zusätzliche Bearbeitungsstände desselben Fotos aufgebläht wird.
pub(crate) fn compute(conn: &Connection) -> Result<CatalogStatistics> {
    let (total_photos, total_file_size): (i64, i64) = conn
        .query_row(
            "SELECT COUNT(*), COALESCE(SUM(file_size), 0) FROM photos WHERE source_photo_id IS NULL",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(map_sqlite_err)?;

    let (earliest, latest): (Option<i64>, Option<i64>) = conn
        .query_row(
            "SELECT MIN(captured_at), MAX(captured_at) FROM photos \
             WHERE source_photo_id IS NULL AND captured_at IS NOT NULL",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(map_sqlite_err)?;

    Ok(CatalogStatistics {
        total_photos: total_photos as u64,
        total_file_size: total_file_size as u64,
        earliest_captured_at: earliest.map(crate::models::from_unix).transpose()?,
        latest_captured_at: latest.map(crate::models::from_unix).transpose()?,
        rating_distribution: rating_distribution(conn)?,
        top_camera_models: top_value_counts(conn, "camera_model")?,
        top_lenses: top_value_counts(conn, "lens")?,
    })
}

// ---- Ausrüstungs-/Belichtungs-Statistik (Phase 32 F5) ----------------------

/// Brennweiten-Klassen in Millimetern (Kleinbild-Äquivalent wird NICHT
/// gerechnet — die EXIF-Brennweite steht so in der Datei, und ein
/// Crop-Faktor ließe sich ohne verlässliche Sensorgröße nur raten).
///
/// Die Grenzen folgen den üblichen Objektivklassen (Ultraweitwinkel,
/// Weitwinkel, Normal, Porträt, Tele, Supertele) statt einer
/// gleichmäßigen Einteilung: 24 mm und 35 mm sind zwei verschiedene
/// Bildsprachen, 300 mm und 320 mm nicht.
const FOCAL_BUCKETS: &[(&str, f32, f32)] = &[
    ("≤ 16 mm", 0.0, 16.0),
    ("17–24 mm", 16.0, 24.0),
    ("25–35 mm", 24.0, 35.0),
    ("36–50 mm", 35.0, 50.0),
    ("51–85 mm", 50.0, 85.0),
    ("86–135 mm", 85.0, 135.0),
    ("136–200 mm", 135.0, 200.0),
    ("201–400 mm", 200.0, 400.0),
    ("> 400 mm", 400.0, f32::INFINITY),
];

/// Blenden-Klassen, an den ganzen Blendenstufen ausgerichtet.
const APERTURE_BUCKETS: &[(&str, f32, f32)] = &[
    ("f/1,4 und offener", 0.0, 1.4),
    ("f/1,4–2,0", 1.4, 2.0),
    ("f/2,0–2,8", 2.0, 2.8),
    ("f/2,8–4,0", 2.8, 4.0),
    ("f/4,0–5,6", 4.0, 5.6),
    ("f/5,6–8,0", 5.6, 8.0),
    ("f/8,0–11", 8.0, 11.0),
    ("f/11–16", 11.0, 16.0),
    ("f/16 und kleiner", 16.0, f32::INFINITY),
];

/// ISO-Klassen, stufenweise verdoppelt — so wie die Werte selbst.
const ISO_BUCKETS: &[(&str, f32, f32)] = &[
    ("≤ 100", 0.0, 100.0),
    ("101–200", 100.0, 200.0),
    ("201–400", 200.0, 400.0),
    ("401–800", 400.0, 800.0),
    ("801–1600", 800.0, 1600.0),
    ("1601–3200", 1600.0, 3200.0),
    ("3201–6400", 3200.0, 6400.0),
    ("> 6400", 6400.0, f32::INFINITY),
];

/// Belichtungszeit-Klassen in Sekunden. Die Grenzen liegen dort, wo sich
/// die Aufnahmesituation ändert: verwacklungsfrei aus der Hand, bewegte
/// Motive einfrieren, Stativ nötig.
const SHUTTER_BUCKETS: &[(&str, f32, f32)] = &[
    ("≤ 1/1000 s", 0.0, 0.001),
    ("1/1000–1/250 s", 0.001, 0.004),
    ("1/250–1/60 s", 0.004, 1.0 / 60.0),
    ("1/60–1/15 s", 1.0 / 60.0, 1.0 / 15.0),
    ("1/15–1 s", 1.0 / 15.0, 1.0),
    ("> 1 s", 1.0, f32::INFINITY),
];

/// Sortiert Werte in Klassen ein.
///
/// Die Grenzen sind **oben** einschließend (`wert <= max`) und unten
/// ausschließend: 50 mm gehört zu „36–50 mm", nicht zu „51–85 mm" —
/// genau so, wie die Beschriftung es verspricht. Ein leerer Balken
/// bleibt stehen statt zu verschwinden, damit die Lücke sichtbar ist
/// („in diesem Bereich fotografiere ich nie").
fn bucketize(values: &[Option<f32>], buckets: &[(&str, f32, f32)]) -> Vec<DistributionBucket> {
    let mut counts = vec![0u64; buckets.len()];
    let mut missing = 0u64;
    for value in values {
        match value {
            None => missing += 1,
            Some(value) => {
                let index = buckets
                    .iter()
                    .position(|(_, min, max)| *value > *min && *value <= *max)
                    // Ein Wert unterhalb der ersten Untergrenze (0.0)
                    // ist nur bei kaputtem EXIF möglich; er gehört dann
                    // in den ersten Balken, nicht ins Nichts.
                    .unwrap_or(0);
                counts[index] += 1;
            }
        }
    }
    let mut result: Vec<DistributionBucket> = buckets
        .iter()
        .zip(counts)
        .map(|((label, _, _), count)| DistributionBucket {
            label: (*label).to_string(),
            count,
            missing: false,
        })
        .collect();
    if missing > 0 {
        result.push(DistributionBucket {
            label: "keine Angabe".to_string(),
            count: missing,
            missing: true,
        });
    }
    result
}

/// Rechnet die vier Verteilungen aus den Roh-Belichtungswerten.
pub(crate) fn distributions(
    rows: &[ExposureRow],
) -> (
    Vec<DistributionBucket>,
    Vec<DistributionBucket>,
    Vec<DistributionBucket>,
    Vec<DistributionBucket>,
) {
    let focal: Vec<Option<f32>> = rows.iter().map(|r| r.focal_length).collect();
    let aperture: Vec<Option<f32>> = rows.iter().map(|r| r.aperture).collect();
    let iso: Vec<Option<f32>> = rows.iter().map(|r| r.iso.map(|v| v as f32)).collect();
    let shutter: Vec<Option<f32>> = rows.iter().map(|r| r.shutter).collect();
    (
        bucketize(&focal, FOCAL_BUCKETS),
        bucketize(&aperture, APERTURE_BUCKETS),
        bucketize(&iso, ISO_BUCKETS),
        bucketize(&shutter, SHUTTER_BUCKETS),
    )
}

/// Vollständige Kamera-/Objektivlisten (nicht auf `TOP_N` gekürzt, anders
/// als in [`compute`]) — wer wissen will, wie oft das alte 50er noch
/// drankommt, darf nicht an Platz 9 abgeschnitten werden.
fn all_value_counts(conn: &Connection, column: &str) -> Result<Vec<(String, u64)>> {
    // `column` ist wie in `top_value_counts` fest verdrahtet, nie
    // Nutzereingabe.
    let sql = format!(
        "SELECT {column}, COUNT(*) as cnt FROM photos \
         WHERE {column} IS NOT NULL AND source_photo_id IS NULL \
         GROUP BY {column} ORDER BY cnt DESC, {column} ASC"
    );
    let mut stmt = conn.prepare(&sql).map_err(map_sqlite_err)?;
    let rows = stmt
        .query_map([], |row| {
            let value: String = row.get(0)?;
            let count: i64 = row.get(1)?;
            Ok((value, count as u64))
        })
        .map_err(map_sqlite_err)?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(map_sqlite_err)
}

pub(crate) fn compute_gear(conn: &Connection) -> Result<GearStatistics> {
    let mut stmt = conn
        .prepare(
            "SELECT focal_length, aperture, iso, shutter FROM photos \
             WHERE source_photo_id IS NULL",
        )
        .map_err(map_sqlite_err)?;
    let rows = stmt
        .query_map([], |row| {
            Ok(ExposureRow {
                focal_length: row.get::<_, Option<f64>>(0)?.map(|v| v as f32),
                aperture: row.get::<_, Option<f64>>(1)?.map(|v| v as f32),
                iso: row.get::<_, Option<i64>>(2)?.map(|v| v as u32),
                shutter: row.get::<_, Option<f64>>(3)?.map(|v| v as f32),
            })
        })
        .map_err(map_sqlite_err)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(map_sqlite_err)?;

    let (focal_lengths, apertures, isos, shutters) = distributions(&rows);
    Ok(GearStatistics {
        cameras: all_value_counts(conn, "camera_model")?,
        lenses: all_value_counts(conn, "lens")?,
        focal_lengths,
        apertures,
        isos,
        shutters,
        total: rows.len() as u64,
    })
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

    fn insert_photo(conn: &Connection, filename: &str, camera_model: Option<&str>, rating: u8) {
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
            file_size: 1000,
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
        let (photo_id, _) =
            photos::upsert(conn, &photo, OffsetDateTime::now_utc()).expect("Foto anlegen");
        photos::set_rating(conn, photo_id, rating).expect("Bewertung setzen");
    }

    fn row(
        focal: Option<f32>,
        aperture: Option<f32>,
        iso: Option<u32>,
        shutter: Option<f32>,
    ) -> crate::models::ExposureRow {
        crate::models::ExposureRow {
            focal_length: focal,
            aperture,
            iso,
            shutter,
        }
    }

    fn count_of(buckets: &[DistributionBucket], label: &str) -> u64 {
        buckets
            .iter()
            .find(|bucket| bucket.label == label)
            .map(|bucket| bucket.count)
            .unwrap_or_else(|| panic!("Balken {label} fehlt"))
    }

    #[test]
    fn a_bucket_boundary_belongs_to_the_class_its_label_promises() {
        // 50 mm steht in der Beschriftung von „36–50 mm" — dort muss es
        // auch landen, nicht bei „51–85 mm".
        let (focal, _, _, _) = distributions(&[row(Some(50.0), None, None, None)]);
        assert_eq!(count_of(&focal, "36–50 mm"), 1);
        assert_eq!(count_of(&focal, "51–85 mm"), 0);
    }

    #[test]
    fn missing_values_get_their_own_bucket_instead_of_being_guessed() {
        let (focal, aperture, iso, shutter) = distributions(&[row(None, None, None, None)]);
        for buckets in [&focal, &aperture, &iso, &shutter] {
            let last = buckets.last().expect("mindestens ein Balken");
            assert!(last.missing);
            assert_eq!(last.count, 1);
            // Kein regulärer Balken hat das Foto zusätzlich gezählt.
            assert_eq!(
                buckets
                    .iter()
                    .filter(|b| !b.missing)
                    .map(|b| b.count)
                    .sum::<u64>(),
                0
            );
        }
    }

    #[test]
    fn empty_classes_stay_in_the_list_so_the_gap_is_visible() {
        let (focal, _, _, _) = distributions(&[row(Some(24.0), None, None, None)]);
        assert_eq!(
            focal.len(),
            FOCAL_BUCKETS.len(),
            "kein Balken darf wegfallen"
        );
        assert_eq!(count_of(&focal, "17–24 mm"), 1);
        assert_eq!(count_of(&focal, "> 400 mm"), 0);
    }

    #[test]
    fn every_bucket_list_covers_its_whole_range_without_gaps() {
        // Hält die vier Klassen-Tabellen zusammen: jede Obergrenze ist
        // die nächste Untergrenze. Ohne diesen Test fiele ein Tippfehler
        // in einer Grenze (z. B. 5.6 vs. 5.7) erst auf, wenn ein Foto
        // still im falschen Balken landet.
        for buckets in [
            FOCAL_BUCKETS,
            APERTURE_BUCKETS,
            ISO_BUCKETS,
            SHUTTER_BUCKETS,
        ] {
            for pair in buckets.windows(2) {
                assert_eq!(
                    pair[0].2, pair[1].1,
                    "Lücke zwischen {} und {}",
                    pair[0].0, pair[1].0
                );
            }
            assert_eq!(buckets[0].1, 0.0);
            assert!(buckets[buckets.len() - 1].2.is_infinite());
        }
    }

    #[test]
    fn shutter_classes_separate_handheld_from_tripod_speeds() {
        let (_, _, _, shutter) = distributions(&[
            row(None, None, None, Some(1.0 / 2000.0)),
            row(None, None, None, Some(1.0 / 125.0)),
            row(None, None, None, Some(4.0)),
        ]);
        assert_eq!(count_of(&shutter, "≤ 1/1000 s"), 1);
        assert_eq!(count_of(&shutter, "1/250–1/60 s"), 1);
        assert_eq!(count_of(&shutter, "> 1 s"), 1);
    }

    #[test]
    fn gear_statistics_lists_every_camera_not_just_the_top_eight() {
        let conn = setup();
        for index in 0..10 {
            insert_photo(
                &conn,
                &format!("{index}.cr2"),
                Some(&format!("Kamera {index}")),
                0,
            );
        }
        let gear = compute_gear(&conn).expect("Statistik");
        assert_eq!(gear.cameras.len(), 10);
        assert_eq!(gear.total, 10);
        // Die alte, gekürzte Liste bleibt daneben unverändert bei acht.
        assert_eq!(
            compute(&conn).expect("Statistik").top_camera_models.len(),
            8
        );
    }

    #[test]
    fn empty_catalog_has_zero_totals_and_empty_distributions() {
        let conn = setup();
        let stats = compute(&conn).expect("Statistik");
        assert_eq!(stats.total_photos, 0);
        assert_eq!(stats.total_file_size, 0);
        assert!(stats.top_camera_models.is_empty());
    }

    #[test]
    fn counts_photos_and_groups_by_camera_model() {
        let conn = setup();
        insert_photo(&conn, "a.cr2", Some("Canon EOS R5"), 5);
        insert_photo(&conn, "b.cr2", Some("Canon EOS R5"), 3);
        insert_photo(&conn, "c.cr2", Some("Nikon Z9"), 0);

        let stats = compute(&conn).expect("Statistik");
        assert_eq!(stats.total_photos, 3);
        assert_eq!(stats.total_file_size, 3000);
        assert_eq!(stats.top_camera_models[0], ("Canon EOS R5".to_string(), 2));
        assert!(stats.rating_distribution.contains(&(5, 1)));
        assert!(stats.rating_distribution.contains(&(0, 1)));
    }
}
