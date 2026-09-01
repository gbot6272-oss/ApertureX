//! Karte (Phase 8 Schritt 7, siehe `PLAN.md`/`DECISIONS.md` ADR-0034).
//!
//! Deckt die beiden serverseitigen Bausteine der Kartenansicht ab, die
//! kein Netzwerk brauchen: vollständig offline Reverse-Geocoding
//! ([`reverse_geocode`], gebündelter GeoNames-Auszug über den
//! `reverse_geocoder`-Crate) und GPX-Tracklog-Import ([`parse_gpx`]).
//! GPS-Koordinaten selbst liest bereits `apx_raw::metadata` aus den
//! EXIF-Daten (kein neuer Code hier), die Kartenkacheln (OpenStreetMap,
//! einzige Netzwerk-Abhängigkeit dieser Phase, siehe ADR-0034) lädt das
//! Frontend direkt über Leaflet — dafür braucht es keinen Tauri-Command.

use reverse_geocoder::ReverseGeocoder;
use std::sync::OnceLock;

use crate::error::{ExportError, Result};

/// Baut den Geocoder (lädt/indiziert den gebündelten GeoNames-Auszug,
/// ~150.000 Orte) nur beim ersten Aufruf statt bei jedem
/// `reverse_geocode`-Aufruf neu — der Aufbau des kd-Baums kostet
/// spürbar Zeit, das Nachschlagen selbst ist danach günstig.
static GEOCODER: OnceLock<ReverseGeocoder> = OnceLock::new();

fn geocoder() -> &'static ReverseGeocoder {
    GEOCODER.get_or_init(ReverseGeocoder::new)
}

/// Ergebnis eines Reverse-Geocoding-Nachschlags — der nächstgelegene
/// bekannte Ort, nicht notwendigerweise der Ort, in dem die Koordinate
/// tatsächlich liegt (der gebündelte Datensatz enthält Orte, keine
/// Verwaltungsgrenzen-Polygone).
#[derive(Debug, Clone, PartialEq)]
pub struct GeocodedLocation {
    pub name: String,
    /// Bundesland/Provinz o. Ä. (GeoNames "admin1"), leer wenn unbekannt.
    pub admin1: String,
    /// ISO-3166-1-alpha-2-Ländercode, z. B. "DE".
    pub country_code: String,
    /// Abstand zum nächstgelegenen Ort in Kilometern (Luftlinie).
    pub distance_km: f64,
}

/// Reverse-Geocoding einer WGS84-Koordinate, vollständig offline.
pub fn reverse_geocode(lat: f64, lon: f64) -> GeocodedLocation {
    let result = geocoder().search((lat, lon));
    GeocodedLocation {
        name: result.record.name.clone(),
        admin1: result.record.admin1.clone(),
        country_code: result.record.cc.clone(),
        distance_km: result.distance,
    }
}

/// Ein einzelner Trackpunkt aus einer GPX-Datei.
#[derive(Debug, Clone, PartialEq)]
pub struct GpxTrackPoint {
    pub lat: f64,
    pub lon: f64,
    /// Höhe in Metern, falls die Datei ein `<ele>`-Element trägt.
    pub elevation: Option<f64>,
    /// Roher `<time>`-Text (ISO-8601), falls vorhanden — wird hier nicht
    /// geparst, das Frontend braucht ihn nur zur Anzeige.
    pub time: Option<String>,
}

/// Parst alle `<trkpt>`-Elemente einer GPX-Datei (Tracklog-Import,
/// Schritt 7) — ignoriert Routen (`<rtept>`) und einzelne Wegpunkte
/// (`<wpt>`) bewusst: die Kartenansicht zeichnet nur die eigentliche
/// Trackaufzeichnung als Linie, siehe Moduldoku. Nutzt `quick-xml`s
/// Streaming-Reader statt eines vollständigen DOM-Baums — GPX-Dateien
/// einer mehrtägigen Reise können mehrere zehntausend Punkte haben.
pub fn parse_gpx(xml: &str) -> Result<Vec<GpxTrackPoint>> {
    use quick_xml::events::Event;
    use quick_xml::reader::Reader;

    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    let mut points = Vec::new();
    let mut in_trkpt = false;
    let mut current: Option<(f64, f64)> = None;
    let mut current_ele: Option<f64> = None;
    let mut current_time: Option<String> = None;
    let mut in_ele = false;
    let mut in_time = false;

    // `<trkpt lat="…" lon="…">…</trkpt>` und die selbstschließende Form
    // `<trkpt lat="…" lon="…"/>` (kein `<ele>`/`<time>`-Kind möglich) sind
    // beide gültiges GPX — `quick-xml` liefert sie als unterschiedliche
    // Ereignisse (`Start`+`End` bzw. ein einzelnes `Empty`), darum teilen
    // sich beide Fälle diese Attribut-Auswertung.
    fn parse_lat_lon(tag: &quick_xml::events::BytesStart) -> Result<(f64, f64)> {
        let mut lat = None;
        let mut lon = None;
        for attr in tag.attributes().flatten() {
            let key: &str = attr.key.into_inner();
            match key {
                "lat" => lat = attr.value.parse::<f64>().ok(),
                "lon" => lon = attr.value.parse::<f64>().ok(),
                _ => {}
            }
        }
        match (lat, lon) {
            (Some(lat), Some(lon)) => Ok((lat, lon)),
            _ => Err(ExportError::Gpx {
                message: "<trkpt> ohne gültige lat/lon-Attribute".to_string(),
            }),
        }
    }

    loop {
        match reader.read_event() {
            Ok(Event::Empty(tag)) => {
                let local: &str = tag.name().into_inner();
                if local == "trkpt" {
                    let (lat, lon) = parse_lat_lon(&tag)?;
                    points.push(GpxTrackPoint {
                        lat,
                        lon,
                        elevation: None,
                        time: None,
                    });
                }
            }
            Ok(Event::Start(tag)) => {
                let local: &str = tag.name().into_inner();
                if local == "trkpt" {
                    in_trkpt = true;
                    current_ele = None;
                    current_time = None;
                    current = Some(parse_lat_lon(&tag)?);
                } else if in_trkpt && local == "ele" {
                    in_ele = true;
                } else if in_trkpt && local == "time" {
                    in_time = true;
                }
            }
            Ok(Event::Text(text)) => {
                let raw: &str = text.as_ref();
                if in_ele {
                    current_ele = raw.parse::<f64>().ok();
                } else if in_time {
                    current_time = Some(raw.to_string());
                }
            }
            Ok(Event::End(tag)) => {
                let local: &str = tag.name().into_inner();
                if local == "ele" {
                    in_ele = false;
                } else if local == "time" {
                    in_time = false;
                } else if local == "trkpt" {
                    if let Some((lat, lon)) = current.take() {
                        points.push(GpxTrackPoint {
                            lat,
                            lon,
                            elevation: current_ele.take(),
                            time: current_time.take(),
                        });
                    }
                    in_trkpt = false;
                }
            }
            Ok(Event::Eof) => break,
            Err(err) => {
                return Err(ExportError::Gpx {
                    message: err.to_string(),
                });
            }
            _ => {}
        }
    }

    Ok(points)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reverse_geocode_finds_a_nearby_known_city() {
        // Berlin Mitte — der gebündelte Datensatz enthält Berlin garantiert.
        let location = reverse_geocode(52.5200, 13.4050);
        assert_eq!(location.country_code, "DE");
        assert!(!location.name.is_empty());
        assert!(
            location.distance_km < 50.0,
            "sollte eine nahegelegene Stadt finden, war {} km entfernt",
            location.distance_km
        );
    }

    #[test]
    fn reverse_geocode_is_cached_across_calls() {
        // Zwei Aufrufe dürfen nicht zweimal den kd-Baum aufbauen — reiner
        // Verhaltensnachweis über zwei konsistente Ergebnisse für dieselbe
        // Koordinate, der eigentliche Cache-Effekt (Performance) ist hier
        // nicht direkt messbar.
        let first = reverse_geocode(48.1351, 11.5820);
        let second = reverse_geocode(48.1351, 11.5820);
        assert_eq!(first, second);
    }

    const SAMPLE_GPX: &str = r#"<?xml version="1.0"?>
<gpx>
  <trk>
    <trkseg>
      <trkpt lat="52.520" lon="13.405">
        <ele>34.5</ele>
        <time>2026-06-01T10:00:00Z</time>
      </trkpt>
      <trkpt lat="52.521" lon="13.406">
        <ele>35.1</ele>
      </trkpt>
    </trkseg>
  </trk>
</gpx>"#;

    #[test]
    fn parse_gpx_extracts_all_trackpoints_with_optional_fields() {
        let points = parse_gpx(SAMPLE_GPX).expect("gültiges GPX");
        assert_eq!(points.len(), 2);
        assert_eq!(points[0].lat, 52.520);
        assert_eq!(points[0].lon, 13.405);
        assert_eq!(points[0].elevation, Some(34.5));
        assert_eq!(points[0].time.as_deref(), Some("2026-06-01T10:00:00Z"));
        assert_eq!(points[1].elevation, Some(35.1));
        assert!(points[1].time.is_none());
    }

    #[test]
    fn parse_gpx_ignores_routes_and_waypoints() {
        let xml = r#"<gpx>
            <wpt lat="1.0" lon="2.0"><name>Nicht relevant</name></wpt>
            <rte><rtept lat="3.0" lon="4.0"/></rte>
            <trk><trkseg><trkpt lat="5.0" lon="6.0"/></trkseg></trk>
        </gpx>"#;
        let points = parse_gpx(xml).expect("gültiges GPX");
        assert_eq!(points.len(), 1);
        assert_eq!(points[0].lat, 5.0);
    }

    #[test]
    fn parse_gpx_rejects_trkpt_without_coordinates() {
        let xml = r#"<gpx><trk><trkseg><trkpt lat="1.0"/></trkseg></trk></gpx>"#;
        assert!(parse_gpx(xml).is_err());
    }

    #[test]
    fn parse_gpx_rejects_malformed_xml() {
        // Nicht zusammenpassendes Schluss-Tag statt bloß fehlender
        // Verschachtelung — `quick-xml`s Streaming-Parser ist gegenüber
        // unvollständigem XML (offenes Tag, das nie geschlossen wird)
        // tolerant und bricht erst am echten Dateiende ohne Fehler ab,
        // meldet aber ein Schluss-Tag, das nicht zum zuletzt geöffneten
        // passt (`check_end_names`, Vorgabe an).
        assert!(parse_gpx("<gpx><trk></foo></gpx>").is_err());
    }

    #[test]
    fn parse_gpx_of_empty_track_returns_empty_list() {
        let points = parse_gpx("<gpx></gpx>").expect("leeres GPX ist gültig");
        assert!(points.is_empty());
    }
}
