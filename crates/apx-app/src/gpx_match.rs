//! Fotos aus einem GPX-Track verorten (Phase 34 F4, siehe
//! `DECISIONS.md` ADR-0070).
//!
//! **Wozu.** Kameras ohne GPS schreiben keine Koordinaten. Wer mit einem
//! Logger, einer Uhr oder dem Telefon einen Track mitgeschnitten hat,
//! kann die Fotos nachträglich verorten: das Aufnahmedatum sagt, wann
//! ausgelöst wurde, der Track sagt, wo man zu diesem Zeitpunkt war.
//!
//! **Warum interpoliert wird.** Ein Logger schreibt alle paar Sekunden
//! einen Punkt; der Auslöser trifft fast nie genau darauf. Den nächsten
//! Punkt zu nehmen wäre bis zu einem halben Aufzeichnungsintervall
//! daneben — auf dem Fahrrad schnell hundert Meter. Zwischen den beiden
//! umgebenden Punkten linear zu interpolieren trifft deutlich besser
//! und kostet nichts.
//!
//! **Warum ein Zeitversatz einstellbar ist.** Die Kamerauhr geht fast
//! immer falsch, oft um die volle Stunde der Zeitzone. Ohne Korrektur
//! landet jedes Foto systematisch am falschen Ort — oder gar nicht,
//! weil der Track zu dieser Zeit noch nicht lief. Der Versatz ist
//! deshalb kein Randfall, sondern der Normalfall.
//!
//! **Kein eigener GPX-Leser.** Das Einlesen macht
//! `apx_export::map::parse_gpx` (quick_xml), das es für die
//! Reiserouten-Anzeige der Kartenansicht ohnehin schon gibt. Dieses
//! Modul steuert nur bei, was wirklich neu ist: die Zuordnung eines
//! Zeitpunkts zu einer Position. Ein zweiter Leser daneben wäre genau
//! die Doppelpflege, aus der der Kalender-Fehler in ADR-0068 entstanden
//! ist.

use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

/// Ein Punkt des Tracks.
#[derive(Debug, Clone, PartialEq)]
pub struct TrackPoint {
    pub time: OffsetDateTime,
    pub lat: f64,
    pub lon: f64,
}

/// Höchstabweichung zwischen Aufnahmezeit und Track, wenn der Aufrufer
/// keine sinnvolle angibt.
///
/// Zehn Minuten: großzügig genug für eine Pause ohne Empfang, eng
/// genug, dass ein Foto vom Vortag nicht am Ort von heute landet.
pub const DEFAULT_TOLERANCE_SECONDS: i64 = 600;

/// Legt eine vom Aufrufer übergebene Toleranz aus.
///
/// Null oder negativ heißt nicht „keine Toleranz", sondern „nichts
/// Sinnvolles angegeben" — eine Toleranz von 0 träfe nur bei
/// millisekundengenauer Übereinstimmung, also praktisch nie, und das
/// Ergebnis wäre eine leere Liste ohne erkennbaren Grund.
pub fn effective_tolerance(seconds: i64) -> i64 {
    if seconds > 0 {
        seconds
    } else {
        DEFAULT_TOLERANCE_SECONDS
    }
}

/// Wandelt die Punkte aus [`apx_export::map::parse_gpx`] in eine nach
/// Zeit sortierte Liste um.
///
/// Punkte ohne oder mit unlesbarer Zeitangabe entfallen: sie lassen
/// sich keinem Auslösezeitpunkt zuordnen, und sie stillschweigend
/// mitzuführen hiesse, sie später an beliebiger Stelle einzusortieren.
pub fn timed_points(points: &[apx_export::map::GpxTrackPoint]) -> Vec<TrackPoint> {
    let mut result: Vec<TrackPoint> = points
        .iter()
        .filter_map(|p| {
            let text = p.time.as_deref()?;
            let time = OffsetDateTime::parse(text, &Rfc3339).ok()?;
            Some(TrackPoint {
                time,
                lat: p.lat,
                lon: p.lon,
            })
        })
        .collect();
    result.sort_by_key(|p| p.time);
    result
}

/// Sucht die Position zum Zeitpunkt `at`.
///
/// `tolerance_seconds` begrenzt, wie weit der nächste Trackpunkt
/// zeitlich entfernt sein darf. Liegt `at` zwischen zwei Punkten, wird
/// linear interpoliert; liegt es vor dem ersten oder hinter dem letzten,
/// gilt der Randpunkt — sofern er innerhalb der Toleranz liegt.
///
/// Die Interpolation rechnet auf Längen- und Breitengraden direkt. Über
/// wenige Sekunden Aufzeichnungsabstand ist die Erdkrümmung dabei
/// bedeutungslos; eine Großkreis-Interpolation wäre Genauigkeit, die
/// unterhalb der GPS-Streuung selbst liegt.
pub fn match_position(
    points: &[TrackPoint],
    at: OffsetDateTime,
    tolerance_seconds: i64,
) -> Option<(f64, f64)> {
    if points.is_empty() {
        return None;
    }
    let within =
        |point: &TrackPoint| (point.time - at).whole_seconds().abs() <= tolerance_seconds.max(0);

    match points.binary_search_by_key(&at, |p| p.time) {
        Ok(index) => Some((points[index].lat, points[index].lon)),
        Err(0) => {
            let first = &points[0];
            within(first).then_some((first.lat, first.lon))
        }
        Err(index) if index == points.len() => {
            let last = &points[points.len() - 1];
            within(last).then_some((last.lat, last.lon))
        }
        Err(index) => {
            let before = &points[index - 1];
            let after = &points[index];
            // Nur interpolieren, wenn BEIDE Nachbarn in Reichweite sind.
            // Sonst liegt `at` in einer Aufzeichnungslücke, und ein
            // interpolierter Punkt mitten darin wäre eine Erfindung.
            if !within(before) || !within(after) {
                // Der nähere Nachbar darf trotzdem gelten, wenn er
                // innerhalb der Toleranz liegt.
                let nearer = if (at - before.time) <= (after.time - at) {
                    before
                } else {
                    after
                };
                return within(nearer).then_some((nearer.lat, nearer.lon));
            }
            let span = (after.time - before.time).whole_milliseconds();
            if span <= 0 {
                return Some((before.lat, before.lon));
            }
            let offset = (at - before.time).whole_milliseconds();
            let ratio = offset as f64 / span as f64;
            Some((
                before.lat + (after.lat - before.lat) * ratio,
                before.lon + (after.lon - before.lon) * ratio,
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::Duration;

    fn t(seconds: i64) -> OffsetDateTime {
        OffsetDateTime::from_unix_timestamp(1_700_000_000 + seconds).expect("gültig")
    }

    fn track() -> Vec<TrackPoint> {
        vec![
            TrackPoint {
                time: t(0),
                lat: 50.0,
                lon: 8.0,
            },
            TrackPoint {
                time: t(10),
                lat: 50.001,
                lon: 8.002,
            },
            TrackPoint {
                time: t(20),
                lat: 50.002,
                lon: 8.004,
            },
        ]
    }

    fn raw(lat: f64, lon: f64, time: Option<&str>) -> apx_export::map::GpxTrackPoint {
        apx_export::map::GpxTrackPoint {
            lat,
            lon,
            elevation: None,
            time: time.map(str::to_string),
        }
    }

    /// Der eigentliche GPX-Leser (`apx_export::map::parse_gpx`) ist dort
    /// getestet; hier geht es um das, was dieses Modul beisteuert.
    #[test]
    fn skips_points_without_a_usable_time() {
        let points = timed_points(&[
            raw(50.0, 8.0, None),
            raw(51.0, 9.0, Some("kaputt")),
            raw(52.0, 10.0, Some("2023-11-14T22:13:20Z")),
        ]);
        assert_eq!(points.len(), 1);
        assert_eq!(points[0].lat, 52.0);
    }

    #[test]
    fn returns_points_sorted_by_time_even_if_the_file_is_not() {
        let points = timed_points(&[
            raw(2.0, 2.0, Some("2023-11-14T22:13:30Z")),
            raw(1.0, 1.0, Some("2023-11-14T22:13:20Z")),
        ]);
        assert_eq!(points[0].lat, 1.0);
        assert_eq!(points[1].lat, 2.0);
    }

    #[test]
    fn an_exact_hit_returns_that_point() {
        assert_eq!(match_position(&track(), t(10), 600), Some((50.001, 8.002)));
    }

    #[test]
    fn interpolates_between_two_points() {
        // Genau in der Mitte zwischen t(0) und t(10).
        let (lat, lon) = match_position(&track(), t(5), 600).expect("Treffer");
        assert!((lat - 50.0005).abs() < 1e-9, "lat war {lat}");
        assert!((lon - 8.001).abs() < 1e-9, "lon war {lon}");
    }

    #[test]
    fn a_photo_outside_the_tolerance_gets_no_position() {
        // Eine Stunde nach dem letzten Punkt, Toleranz zehn Minuten.
        assert_eq!(match_position(&track(), t(3600), 600), None);
        assert_eq!(match_position(&track(), t(-3600), 600), None);
    }

    #[test]
    fn a_photo_just_before_the_track_uses_the_first_point() {
        assert_eq!(match_position(&track(), t(-30), 600), Some((50.0, 8.0)));
    }

    #[test]
    fn a_gap_in_the_recording_is_not_interpolated_across() {
        // Zwischen den beiden Punkten liegen zwei Stunden; ein Foto in
        // der Mitte darf KEINEN erfundenen Punkt bekommen.
        let sparse = vec![
            TrackPoint {
                time: t(0),
                lat: 50.0,
                lon: 8.0,
            },
            TrackPoint {
                time: t(7200),
                lat: 51.0,
                lon: 9.0,
            },
        ];
        assert_eq!(match_position(&sparse, t(3600), 600), None);
        // Nah am ersten Punkt gilt dieser aber weiterhin.
        assert_eq!(match_position(&sparse, t(60), 600), Some((50.0, 8.0)));
    }

    #[test]
    fn a_nonsensical_tolerance_falls_back_to_the_default() {
        assert_eq!(effective_tolerance(120), 120);
        assert_eq!(effective_tolerance(0), DEFAULT_TOLERANCE_SECONDS);
        assert_eq!(effective_tolerance(-5), DEFAULT_TOLERANCE_SECONDS);
    }

    #[test]
    fn an_empty_track_matches_nothing() {
        assert_eq!(match_position(&[], t(0), 600), None);
        assert!(timed_points(&[]).is_empty());
    }

    #[test]
    fn a_time_offset_shifts_which_point_is_hit() {
        // So wird der Kamerauhr-Versatz angewandt: der Aufrufer
        // verschiebt die Aufnahmezeit, bevor er hier fragt.
        let camera_time = t(3600 + 10);
        assert_eq!(match_position(&track(), camera_time, 600), None);
        assert_eq!(
            match_position(&track(), camera_time - Duration::seconds(3600), 600),
            Some((50.001, 8.002))
        );
    }
}
