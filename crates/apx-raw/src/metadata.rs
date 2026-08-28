//! Metadaten-Extraktion — bewusst getrennt von der vollen Bilddekodierung,
//! damit `read_metadata()` schnell bleibt (siehe Zeitbudget < 50 ms in
//! `PHASE1_PROMPT.md` Abschnitt 3).

use std::path::Path;

use apx_core::{AppError, Result};
use rawler::decoders::RawDecodeParams;
use rawler::exif::Exif;
use rawler::rawsource::RawSource;
use time::{Date, Month, OffsetDateTime, PrimitiveDateTime, Time, UtcOffset};

use crate::orientation::Orientation;

#[derive(Debug, Clone, PartialEq)]
pub struct RawMetadata {
    pub width: u32,
    pub height: u32,
    pub camera_make: String,
    pub camera_model: String,
    pub lens: Option<String>,
    pub iso: Option<u32>,
    pub shutter: Option<f32>,
    pub aperture: Option<f32>,
    pub focal_length: Option<f32>,
    /// Aufnahmezeitpunkt, falls im EXIF vorhanden.
    ///
    /// **Zeitzonen-Annahme (siehe `PHASE1_PROMPT.md` Abschnitt 10):** EXIF
    /// speichert `DateTimeOriginal` fast immer ohne Zeitzone — das ist die
    /// Kamera-Uhrzeit, keine UTC-Zeit. Enthält die Datei zusätzlich einen
    /// `OffsetTimeOriginal`/`OffsetTime`-Tag (EXIF 2.31+), wird dieser
    /// echte Offset verwendet. Fehlt er — der Regelfall bei RAWs älterer
    /// oder vieler aktueller Kameras — wird der Zeitpunkt mit Offset
    /// UTC+0 gespeichert. Das ist **nicht** die tatsächliche UTC-Zeit,
    /// sondern die unveränderte Kamera-Uhrzeit unter einer expliziten,
    /// dokumentierten Annahme. Konsumenten dieses Felds dürfen es ohne
    /// bekannten Offset nicht für Zeitzonen-Umrechnungen verwenden.
    pub captured_at: Option<OffsetDateTime>,
    pub orientation: Orientation,
    pub gps: Option<(f64, f64)>,
}

/// Liest nur die Metadaten einer RAW- oder Fallback-Bilddatei, ohne die
/// Pixeldaten zu dekodieren. Für RAW-Formate wird `rawler` mit
/// `dummy = true` verwendet (überspringt die teure Pixel-Dekompression,
/// liefert aber korrekte Maße und kameraseitige Metadaten); für
/// Fallback-Formate (JPEG/TIFF) wird nur der Header bzw. EXIF gelesen.
pub fn read_metadata(path: &Path) -> Result<RawMetadata> {
    match crate::format::classify(path) {
        crate::format::FileKind::Raw => read_raw_metadata(path),
        crate::format::FileKind::Fallback => crate::fallback::read_metadata(path),
    }
}

fn read_raw_metadata(path: &Path) -> Result<RawMetadata> {
    let source = RawSource::new(path).map_err(|source| AppError::io(path, source))?;
    let decoder = rawler::get_decoder(&source)
        .map_err(|err| AppError::decode(path, format!("Decoder nicht gefunden: {err}")))?;
    let params = RawDecodeParams::default();

    // dummy=true: überspringt die Pixel-Dekompression, liefert aber Maße,
    // Kamera und Weißabgleich-Koeffizienten — das hält read_metadata()
    // schnell.
    let dummy_image = decoder.raw_image(&source, &params, true).map_err(|err| {
        AppError::decode(path, format!("Metadaten-Dekodierung fehlgeschlagen: {err}"))
    })?;

    let raw_meta = decoder
        .raw_metadata(&source, &params)
        .map_err(|err| AppError::decode(path, format!("EXIF-Lesen fehlgeschlagen: {err}")))?;

    let (width, height) = active_dimensions(&dummy_image);

    Ok(RawMetadata {
        width,
        height,
        camera_make: raw_meta.make,
        camera_model: raw_meta.model,
        lens: raw_meta.lens.map(|lens| lens.lens_name),
        iso: extract_iso(&raw_meta.exif),
        shutter: raw_meta.exif.exposure_time.as_ref().map(|r| r.as_f32()),
        aperture: raw_meta.exif.fnumber.as_ref().map(|r| r.as_f32()),
        focal_length: raw_meta.exif.focal_length.as_ref().map(|r| r.as_f32()),
        captured_at: extract_captured_at(&raw_meta.exif),
        orientation: dummy_image.orientation.into(),
        gps: extract_gps(&raw_meta.exif),
    })
}

/// Maße des nutzbaren Bildbereichs: `active_area`, falls vorhanden, sonst
/// die volle Sensorgröße.
fn active_dimensions(image: &rawler::RawImage) -> (u32, u32) {
    match &image.active_area {
        Some(rect) => (rect.d.w as u32, rect.d.h as u32),
        None => (image.width as u32, image.height as u32),
    }
}

fn extract_iso(exif: &Exif) -> Option<u32> {
    exif.iso_speed
        .or_else(|| exif.iso_speed_ratings.map(u32::from))
}

fn extract_gps(exif: &Exif) -> Option<(f64, f64)> {
    let gps = exif.gps.as_ref()?;
    let lat = dms_to_decimal(gps.gps_latitude.as_ref()?, gps.gps_latitude_ref.as_deref());
    let lon = dms_to_decimal(
        gps.gps_longitude.as_ref()?,
        gps.gps_longitude_ref.as_deref(),
    );
    Some((lat, lon))
}

/// Wandelt Grad/Minuten/Sekunden (EXIF-GPS-Format) in Dezimalgrad um.
/// Ein `ref` von "S" (Süd) oder "W" (West) macht den Wert negativ.
fn dms_to_decimal(dms: &[rawler::formats::tiff::Rational; 3], reference: Option<&str>) -> f64 {
    let degrees = dms[0].as_f32() as f64;
    let minutes = dms[1].as_f32() as f64;
    let seconds = dms[2].as_f32() as f64;
    let value = degrees + minutes / 60.0 + seconds / 3600.0;
    match reference {
        Some("S") | Some("W") => -value,
        _ => value,
    }
}

/// Baut `captured_at` aus `DateTimeOriginal` (Fallback: `CreateDate`) plus
/// optionalem Offset-Tag. Siehe Doc-Kommentar an `RawMetadata::captured_at`
/// für die Zeitzonen-Annahme.
fn extract_captured_at(exif: &Exif) -> Option<OffsetDateTime> {
    let raw_datetime = exif
        .date_time_original
        .as_deref()
        .or(exif.create_date.as_deref())?;
    let naive = parse_exif_datetime(raw_datetime)?;

    let offset_str = exif
        .offset_time_original
        .as_deref()
        .or(exif.offset_time.as_deref());
    let offset = offset_str
        .and_then(parse_exif_offset)
        .unwrap_or(UtcOffset::UTC);

    Some(naive.assume_offset(offset))
}

/// Parst EXIF-Datumsangaben im Format `"YYYY:MM:DD HH:MM:SS"`.
fn parse_exif_datetime(text: &str) -> Option<PrimitiveDateTime> {
    let (date_part, time_part) = text.split_once(' ')?;
    let mut date_fields = date_part.splitn(3, ':');
    let year: i32 = date_fields.next()?.parse().ok()?;
    let month: u8 = date_fields.next()?.parse().ok()?;
    let day: u8 = date_fields.next()?.parse().ok()?;

    let mut time_fields = time_part.splitn(3, ':');
    let hour: u8 = time_fields.next()?.parse().ok()?;
    let minute: u8 = time_fields.next()?.parse().ok()?;
    let second: u8 = time_fields.next()?.parse().ok()?;

    let month = Month::try_from(month).ok()?;
    let date = Date::from_calendar_date(year, month, day).ok()?;
    let time = Time::from_hms(hour, minute, second).ok()?;
    Some(PrimitiveDateTime::new(date, time))
}

/// Parst EXIF-Offset-Angaben im Format `"+HH:MM"` / `"-HH:MM"`.
fn parse_exif_offset(text: &str) -> Option<UtcOffset> {
    let text = text.trim();
    let (sign, rest) = match text.chars().next()? {
        '+' => (1_i8, &text[1..]),
        '-' => (-1_i8, &text[1..]),
        _ => return None,
    };
    let (hours_str, minutes_str) = rest.split_once(':')?;
    let hours: i8 = hours_str.parse().ok()?;
    let minutes: i8 = minutes_str.parse().ok()?;
    UtcOffset::from_hms(sign * hours, sign * minutes, 0).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_valid_exif_datetime() {
        let parsed = parse_exif_datetime("2024:07:15 08:30:12").expect("sollte parsen");
        assert_eq!(parsed.year(), 2024);
        assert_eq!(u8::from(parsed.month()), 7);
        assert_eq!(parsed.day(), 15);
        assert_eq!(parsed.hour(), 8);
        assert_eq!(parsed.minute(), 30);
        assert_eq!(parsed.second(), 12);
    }

    #[test]
    fn rejects_malformed_exif_datetime() {
        assert!(parse_exif_datetime("nicht ein datum").is_none());
        assert!(parse_exif_datetime("2024-07-15T08:30:12").is_none());
    }

    #[test]
    fn parses_positive_and_negative_offsets() {
        assert_eq!(
            parse_exif_offset("+02:00"),
            UtcOffset::from_hms(2, 0, 0).ok()
        );
        assert_eq!(
            parse_exif_offset("-05:30"),
            UtcOffset::from_hms(-5, -30, 0).ok()
        );
    }

    #[test]
    fn missing_offset_falls_back_to_utc() {
        let exif = Exif {
            date_time_original: Some("2024:01:01 12:00:00".to_string()),
            ..Exif::default()
        };
        let captured = extract_captured_at(&exif).expect("sollte einen Zeitpunkt liefern");
        assert_eq!(captured.offset(), UtcOffset::UTC);
    }

    #[test]
    fn explicit_offset_is_used_when_present() {
        let exif = Exif {
            date_time_original: Some("2024:01:01 12:00:00".to_string()),
            offset_time_original: Some("+09:00".to_string()),
            ..Exif::default()
        };
        let captured = extract_captured_at(&exif).expect("sollte einen Zeitpunkt liefern");
        assert_eq!(
            captured.offset(),
            UtcOffset::from_hms(9, 0, 0).expect("gültiger Offset")
        );
    }

    #[test]
    fn gps_south_and_west_are_negative() {
        use rawler::formats::tiff::Rational;
        let dms = [
            Rational::new(52, 1),
            Rational::new(30, 1),
            Rational::new(0, 1),
        ];
        let north = dms_to_decimal(&dms, Some("N"));
        let south = dms_to_decimal(&dms, Some("S"));
        assert!(north > 0.0);
        assert_eq!(south, -north);
    }
}
