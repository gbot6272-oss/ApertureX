//! Fallback-Pfad für JPEG/PNG/TIFF — Formate, die keine echte RAW-Struktur
//! haben. Dekodierung läuft über die `image`-Crate, Metadaten (soweit
//! vorhanden) über `kamadak-exif`.

use std::fs::File;
use std::io::BufReader;
use std::path::Path;

use apx_core::{AppError, Result};
use image::ImageReader;
use time::{OffsetDateTime, UtcOffset};

use crate::orientation::Orientation;
use crate::pipeline::DecodedImage;
use crate::RawMetadata;

pub fn read_metadata(path: &Path) -> Result<RawMetadata> {
    let (width, height) = ImageReader::open(path)
        .map_err(|source| AppError::io(path, source))?
        .with_guessed_format()
        .map_err(|source| AppError::io(path, source))?
        .into_dimensions()
        .map_err(|source| AppError::decode(path, source.to_string()))?;

    let exif = read_exif(path);

    Ok(RawMetadata {
        width,
        height,
        camera_make: exif
            .as_ref()
            .and_then(|e| e.make.clone())
            .unwrap_or_default(),
        camera_model: exif
            .as_ref()
            .and_then(|e| e.model.clone())
            .unwrap_or_default(),
        lens: exif.as_ref().and_then(|e| e.lens.clone()),
        iso: exif.as_ref().and_then(|e| e.iso),
        shutter: exif.as_ref().and_then(|e| e.shutter),
        aperture: exif.as_ref().and_then(|e| e.aperture),
        focal_length: exif.as_ref().and_then(|e| e.focal_length),
        // Bis Phase 34 stand hier hart `None` mit der Begruendung, JPEG/
        // PNG/TIFF seien "nur ein Auffangnetz". Das stimmte nie: jeder
        // Import einer Kamera-JPEG-Datei landete damit ohne
        // Aufnahmedatum im Katalog, und alles, was am Datum haengt
        // (Kalenderansicht, Sortierung nach Aufnahmezeit, Serien-
        // Erkennung, Import-Umbenennung mit Datumsplatzhaltern), blieb
        // fuer diese Fotos leer. Geparst wird mit denselben beiden
        // Funktionen wie im RAW-Pfad (`metadata.rs`), damit beide Wege
        // dieselbe Zeitzonen-Auslegung haben.
        captured_at: exif.as_ref().and_then(|e| e.captured_at),
        orientation: exif
            .as_ref()
            .and_then(|e| e.orientation)
            .map(rawler::decoders::Orientation::from_u16)
            .map(Orientation::from)
            .unwrap_or(Orientation::Normal),
        gps: exif.as_ref().and_then(|e| e.gps),
    })
}

pub fn decode(path: &Path, max_edge: Option<u32>) -> Result<DecodedImage> {
    let mut image =
        image::open(path).map_err(|source| AppError::decode(path, source.to_string()))?;

    if let Some(edge) = max_edge {
        let (w, h) = (image.width(), image.height());
        if w.max(h) > edge {
            image = image.resize(edge, edge, image::imageops::FilterType::Lanczos3);
        }
    }

    let orientation = read_exif(path)
        .and_then(|e| e.orientation)
        .map(rawler::decoders::Orientation::from_u16)
        .map(Orientation::from)
        .unwrap_or(Orientation::Normal);

    let rgb16 = image.to_rgb16();
    let (width, height) = rgb16.dimensions();
    let (pixels, out_w, out_h) = orientation.apply_rgb16(rgb16.as_raw(), width, height);

    Ok(DecodedImage {
        width: out_w,
        height: out_h,
        pixels,
    })
}

/// Minimaler EXIF-Auszug für den Fallback-Pfad. Fehlt EXIF komplett (z. B.
/// bei PNG), wird `None` zurückgegeben statt eines Fehlers — das ist der
/// Normalfall für dieses Format.
struct FallbackExif {
    make: Option<String>,
    model: Option<String>,
    lens: Option<String>,
    iso: Option<u32>,
    shutter: Option<f32>,
    aperture: Option<f32>,
    focal_length: Option<f32>,
    orientation: Option<u16>,
    gps: Option<(f64, f64)>,
    captured_at: Option<OffsetDateTime>,
}

fn read_exif(path: &Path) -> Option<FallbackExif> {
    let file = File::open(path).ok()?;
    let mut reader = BufReader::new(file);
    let exif = exif::Reader::new().read_from_container(&mut reader).ok()?;

    let ascii = |tag: exif::Tag| -> Option<String> {
        let field = exif.get_field(tag, exif::In::PRIMARY)?;
        match &field.value {
            exif::Value::Ascii(parts) => parts.first().map(|bytes| {
                String::from_utf8_lossy(bytes)
                    .trim_end_matches('\0')
                    .trim()
                    .to_string()
            }),
            _ => None,
        }
    };
    let rational = |tag: exif::Tag| -> Option<f32> {
        let field = exif.get_field(tag, exif::In::PRIMARY)?;
        match &field.value {
            exif::Value::Rational(values) => values.first().map(|r| r.to_f64() as f32),
            _ => None,
        }
    };
    let uint = |tag: exif::Tag| -> Option<u32> {
        let field = exif.get_field(tag, exif::In::PRIMARY)?;
        field.value.get_uint(0)
    };

    let gps = (|| {
        let lat = exif.get_field(exif::Tag::GPSLatitude, exif::In::PRIMARY)?;
        let lat_ref = ascii(exif::Tag::GPSLatitudeRef);
        let lon = exif.get_field(exif::Tag::GPSLongitude, exif::In::PRIMARY)?;
        let lon_ref = ascii(exif::Tag::GPSLongitudeRef);
        let exif::Value::Rational(lat_dms) = &lat.value else {
            return None;
        };
        let exif::Value::Rational(lon_dms) = &lon.value else {
            return None;
        };
        Some((
            dms_to_decimal(lat_dms, lat_ref.as_deref()),
            dms_to_decimal(lon_dms, lon_ref.as_deref()),
        ))
    })();

    // `DateTimeOriginal` ist der Aufnahmezeitpunkt; `DateTimeDigitized`
    // (EXIF-Name `CreateDate`) ist der dokumentierte Ersatz, wenn die
    // Kamera ersteren nicht schreibt — dieselbe Reihenfolge wie im
    // RAW-Pfad. Ohne Offset-Tag gilt die Kamera-Uhrzeit als UTC (siehe
    // `RawMetadata::captured_at`).
    let captured_at = ascii(exif::Tag::DateTimeOriginal)
        .or_else(|| ascii(exif::Tag::DateTimeDigitized))
        .as_deref()
        .and_then(crate::metadata::parse_exif_datetime)
        .map(|naive| {
            let offset = ascii(exif::Tag::OffsetTimeOriginal)
                .or_else(|| ascii(exif::Tag::OffsetTime))
                .as_deref()
                .and_then(crate::metadata::parse_exif_offset)
                .unwrap_or(UtcOffset::UTC);
            naive.assume_offset(offset)
        });

    Some(FallbackExif {
        make: ascii(exif::Tag::Make),
        model: ascii(exif::Tag::Model),
        lens: ascii(exif::Tag::LensModel),
        iso: uint(exif::Tag::PhotographicSensitivity),
        shutter: rational(exif::Tag::ExposureTime),
        aperture: rational(exif::Tag::FNumber),
        focal_length: rational(exif::Tag::FocalLength),
        orientation: uint(exif::Tag::Orientation).map(|v| v as u16),
        gps,
        captured_at,
    })
}

fn dms_to_decimal(dms: &[exif::Rational], reference: Option<&str>) -> f64 {
    let degrees = dms.first().map(|r| r.to_f64()).unwrap_or(0.0);
    let minutes = dms.get(1).map(|r| r.to_f64()).unwrap_or(0.0);
    let seconds = dms.get(2).map(|r| r.to_f64()).unwrap_or(0.0);
    let value = degrees + minutes / 60.0 + seconds / 3600.0;
    match reference {
        Some("S") | Some("W") => -value,
        _ => value,
    }
}
// Regressionstest für den auf dem Kopf stehenden Großansicht-Fund
// (`DECISIONS.md`-Nachtrag zu Phase 18): `orientation.rs`s eigene Tests
// prüfen nur die reine Pixel-Umordnungsmathematik mit einem von Hand
// gesetzten `Orientation`-Wert, nie den tatsächlichen Weg über
// `kamadak-exif`s Parsing eines echten JPEG-APP1-Segments — genau dieser
// Weg war bislang ungetestet (diese Datei hatte vor Phase 18 überhaupt
// kein Testmodul). Der eigentliche gemeldete Fehler lag zwar nicht hier
// (siehe `lib/webgl.ts`s `uploadRgba8`-Fix), aber die Untersuchung deckte
// diese echte Testlücke auf — hier geschlossen, statt sie wieder fallen
// zu lassen.
#[cfg(test)]
mod exif_orientation_tests {
    use super::*;
    use std::io::Cursor;

    /// Baut ein minimales, gültiges JPEG mit einem echten APP1/EXIF-Block
    /// (ein einzelnes Orientation-Tag, 0x0112, little-endian TIFF-Header),
    /// direkt nach dem SOI-Marker eingefügt — wie ein reales Kamera-JPEG.
    /// 4×2 Bild, oben links rot, alles andere schwarz, damit sich jede der
    /// acht Orientierungen am Ort des roten Pixels ablesen lässt.
    fn build_jpeg_with_orientation(orientation: u16) -> Vec<u8> {
        build_jpeg_with_exif(orientation, None, None)
    }

    /// Wie [`build_jpeg_with_orientation`], schreibt aber zusaetzlich eine
    /// echte Exif-Sub-IFD (Tag 0x8769) mit `DateTimeOriginal` (0x9003) und
    /// `OffsetTimeOriginal` (0x9011). Die Datumstags leben laut
    /// EXIF-Standard NICHT in der IFD0, sondern in dieser Sub-IFD —
    /// `kamadak-exif` findet sie sonst unter `Tag::DateTimeOriginal`
    /// nicht. Genau diesen Weg deckt der Regressionstest unten ab.
    fn build_jpeg_with_exif(
        orientation: u16,
        date_time_original: Option<&str>,
        offset_time_original: Option<&str>,
    ) -> Vec<u8> {
        let mut img = image::RgbImage::from_pixel(4, 2, image::Rgb([0, 0, 0]));
        img.put_pixel(0, 0, image::Rgb([255, 0, 0]));
        let dynamic = image::DynamicImage::ImageRgb8(img);
        let mut jpeg_bytes = Vec::new();
        dynamic
            .write_to(&mut Cursor::new(&mut jpeg_bytes), image::ImageFormat::Jpeg)
            .expect("jpeg-Kodierung sollte klappen");

        // Ein ASCII-Wert wird laut TIFF nullterminiert gespeichert; `count`
        // zaehlt das Nullbyte mit.
        let ascii_bytes = |text: &str| {
            let mut bytes = text.as_bytes().to_vec();
            bytes.push(0);
            bytes
        };

        let sub_entries: Vec<(u16, Vec<u8>)> = [
            date_time_original.map(|v| (0x9003u16, ascii_bytes(v))),
            offset_time_original.map(|v| (0x9011u16, ascii_bytes(v))),
        ]
        .into_iter()
        .flatten()
        .collect();

        let mut tiff = Vec::new();
        tiff.extend_from_slice(b"II"); // little-endian
        tiff.extend_from_slice(&0x002Au16.to_le_bytes());
        tiff.extend_from_slice(&8u32.to_le_bytes()); // Offset IFD0

        let ifd0_entries = if sub_entries.is_empty() { 1u16 } else { 2u16 };
        // Alle Offsets zaehlen ab dem Anfang des TIFF-Headers.
        let ifd0_size = 2 + 12 * u32::from(ifd0_entries) + 4;
        let sub_ifd_offset = 8 + ifd0_size;
        let sub_ifd_size = 2 + 12 * sub_entries.len() as u32 + 4;
        let mut data_offset = sub_ifd_offset + sub_ifd_size;

        tiff.extend_from_slice(&ifd0_entries.to_le_bytes());
        tiff.extend_from_slice(&0x0112u16.to_le_bytes()); // Tag: Orientation
        tiff.extend_from_slice(&3u16.to_le_bytes()); // Typ: SHORT
        tiff.extend_from_slice(&1u32.to_le_bytes()); // Anzahl Werte
        let mut value_field = [0u8; 4];
        value_field[0..2].copy_from_slice(&orientation.to_le_bytes());
        tiff.extend_from_slice(&value_field);
        if !sub_entries.is_empty() {
            tiff.extend_from_slice(&0x8769u16.to_le_bytes()); // Tag: ExifIFDPointer
            tiff.extend_from_slice(&4u16.to_le_bytes()); // Typ: LONG
            tiff.extend_from_slice(&1u32.to_le_bytes());
            tiff.extend_from_slice(&sub_ifd_offset.to_le_bytes());
        }
        tiff.extend_from_slice(&0u32.to_le_bytes()); // naechstes IFD: keins

        if !sub_entries.is_empty() {
            tiff.extend_from_slice(&(sub_entries.len() as u16).to_le_bytes());
            let mut payload = Vec::new();
            for (tag, bytes) in &sub_entries {
                tiff.extend_from_slice(&tag.to_le_bytes());
                tiff.extend_from_slice(&2u16.to_le_bytes()); // Typ: ASCII
                tiff.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
                // Werte ueber vier Byte liegen ausgelagert, davor inline —
                // hier sind alle Datumsstrings laenger als vier Byte.
                assert!(bytes.len() > 4, "kurze ASCII-Werte laegen inline");
                tiff.extend_from_slice(&data_offset.to_le_bytes());
                data_offset += bytes.len() as u32;
                payload.extend_from_slice(bytes);
            }
            tiff.extend_from_slice(&0u32.to_le_bytes()); // naechstes IFD
            tiff.extend_from_slice(&payload);
        }

        let mut app1 = Vec::new();
        app1.extend_from_slice(b"Exif\0\0");
        app1.extend_from_slice(&tiff);
        let length = (app1.len() + 2) as u16;

        let mut out = Vec::new();
        out.extend_from_slice(&jpeg_bytes[0..2]); // SOI
        out.extend_from_slice(&[0xFF, 0xE1]);
        out.extend_from_slice(&length.to_be_bytes());
        out.extend_from_slice(&app1);
        out.extend_from_slice(&jpeg_bytes[2..]); // Rest ab nach SOI

        out
    }

    fn write_temp_jpeg(name: &str, orientation: u16) -> std::path::PathBuf {
        let bytes = build_jpeg_with_orientation(orientation);
        let path = std::env::temp_dir().join(name);
        std::fs::write(&path, &bytes).expect("Test-JPEG sollte sich schreiben lassen");
        path
    }

    /// Regressionstest zum Kalender-Fund: `read_metadata` lieferte fuer
    /// JPEG/PNG/TIFF hart `captured_at: None`. Jedes importierte
    /// Kamera-JPEG landete damit ohne Aufnahmedatum im Katalog, und die
    /// Kalenderansicht (Phase 32 F3) blieb fuer genau diese Fotos leer —
    /// fuer den Nutzer "der Kalender funktioniert fuer kein Foto".
    /// Geprueft wird hier der volle Weg ueber einen echten APP1/EXIF-Block
    /// mit Exif-Sub-IFD, nicht nur der String-Parser.
    #[test]
    fn read_metadata_reads_the_capture_date_from_a_real_jpeg() {
        let bytes = build_jpeg_with_exif(1, Some("2024:07:15 08:30:12"), None);
        let path = std::env::temp_dir().join("apx_fallback_captured_at_test.jpg");
        std::fs::write(&path, &bytes).expect("Test-JPEG sollte sich schreiben lassen");

        let meta = read_metadata(&path).expect("read_metadata sollte klappen");
        let captured = meta
            .captured_at
            .expect("Aufnahmedatum sollte gelesen werden");
        assert_eq!(captured.year(), 2024);
        assert_eq!(u8::from(captured.month()), 7);
        assert_eq!(captured.day(), 15);
        assert_eq!(captured.hour(), 8);
        assert_eq!(captured.minute(), 30);
        // Ohne Offset-Tag gilt die Kamera-Uhrzeit als UTC — dieselbe
        // Annahme wie im RAW-Pfad.
        assert_eq!(captured.offset(), time::UtcOffset::UTC);
    }

    /// Mit `OffsetTimeOriginal` muss derselbe Wandzeit-Zeitpunkt heraus-
    /// kommen, nur mit der angegebenen Zone — sonst landete eine
    /// Abendaufnahme im Kalender am Folgetag.
    #[test]
    fn read_metadata_honours_the_exif_utc_offset() {
        let bytes = build_jpeg_with_exif(1, Some("2024:07:15 23:30:00"), Some("+02:00"));
        let path = std::env::temp_dir().join("apx_fallback_captured_at_offset_test.jpg");
        std::fs::write(&path, &bytes).expect("Test-JPEG sollte sich schreiben lassen");

        let captured = read_metadata(&path)
            .expect("read_metadata sollte klappen")
            .captured_at
            .expect("Aufnahmedatum sollte gelesen werden");
        assert_eq!(captured.hour(), 23);
        assert_eq!(captured.day(), 15);
        assert_eq!(
            captured.offset(),
            time::UtcOffset::from_hms(2, 0, 0).unwrap()
        );
    }

    /// Ein JPEG ohne Datumstags darf weiterhin `None` liefern statt zu
    /// raten — ein erfundenes Datum waere im Kalender nicht als solches
    /// erkennbar (siehe `lib/calendarGrid.ts`s `countByDay`).
    #[test]
    fn read_metadata_leaves_the_capture_date_empty_without_exif_date() {
        let path = write_temp_jpeg("apx_fallback_no_date_test.jpg", 1);
        let meta = read_metadata(&path).expect("read_metadata sollte klappen");
        assert!(meta.captured_at.is_none());
    }

    #[test]
    fn read_exif_finds_the_real_orientation_tag() {
        let path = write_temp_jpeg("apx_fallback_exif_tag_test.jpg", 6);
        let exif = read_exif(&path);
        assert_eq!(exif.and_then(|e| e.orientation), Some(6));
    }

    #[test]
    fn decode_rotates_180_for_a_real_exif_orientation_3_jpeg() {
        let path = write_temp_jpeg("apx_fallback_exif_180_test.jpg", 3);
        let decoded = decode(&path, None).expect("decode sollte klappen");
        // Bei einer 180°-Drehung landet das ursprünglich oben-linke rote
        // Pixel unten rechts.
        let (w, h) = (decoded.width as usize, decoded.height as usize);
        let last_idx = ((h - 1) * w + (w - 1)) * 3;
        assert!(
            decoded.pixels[last_idx] > decoded.pixels[0],
            "rotes Pixel sollte nach der 180°-Drehung unten rechts sitzen, nicht oben links"
        );
    }

    #[test]
    fn decode_swaps_dimensions_for_a_real_exif_orientation_6_jpeg() {
        // Orientation 6 = 90°-Drehung — vertauscht Breite/Höhe des 4×2-
        // Quellbilds zu 2×4.
        let path = write_temp_jpeg("apx_fallback_exif_90_test.jpg", 6);
        let decoded = decode(&path, None).expect("decode sollte klappen");
        assert_eq!((decoded.width, decoded.height), (2, 4));
    }

    #[test]
    fn decode_linear_matches_decode_for_a_real_exif_jpeg() {
        // `decode_linear` (der von der Entwickeln-Route genutzte
        // Einstiegspunkt, siehe `pipeline::mod::decode_linear`) muss für
        // Fallback-Formate exakt dieselbe Orientierung anwenden wie
        // `decode` (die Vorschau-/Vollbild-Route) — beide teilen sich
        // denselben `fallback::decode`-Aufruf, hier end-to-end über ein
        // echtes JPEG bestätigt statt nur durch Code-Lesen angenommen.
        //
        // Die Pixelwerte selbst sind NICHT mehr byte-identisch zu `decode`
        // (nur reskaliert) — seit Phase 20 (ADR-0048) linearisiert
        // `decode_linear` die bereits gammakodierten JPEG-Bytes echt via
        // `srgb_gamma_inverse`, sonst würde `apx-pipeline`s
        // `linear_camera_rgb_to_srgb_rgba8` die Gammakurve ein zweites Mal
        // anwenden und das Bild überbelichten. Ein linearisierter Wert
        // muss daher (außer an den Rändern 0/1) klar unter dem
        // gammakodierten Ausgangswert liegen.
        let path = write_temp_jpeg("apx_fallback_exif_linear_test.jpg", 3);
        let decoded = decode(&path, None).expect("decode sollte klappen");
        let linear = crate::decode_linear(&path, None).expect("decode_linear sollte klappen");

        assert_eq!(
            (decoded.width, decoded.height),
            (linear.width, linear.height)
        );
        assert_eq!(linear.pixels.len(), decoded.pixels.len());
        let mut saw_darkened_midtone = false;
        for (&encoded_u16, &lin) in decoded.pixels.iter().zip(linear.pixels.iter()) {
            let encoded = encoded_u16 as f32 / 65535.0;
            assert!((0.0..=1.0).contains(&lin));
            if encoded > 0.05 && encoded < 0.95 {
                assert!(
                    lin <= encoded + 1e-4,
                    "linearisierter Wert {lin} sollte nicht über dem gammakodierten {encoded} liegen"
                );
                if lin < encoded - 1e-3 {
                    saw_darkened_midtone = true;
                }
            }
        }
        assert!(
            saw_darkened_midtone,
            "mindestens ein Mittenwert sollte durch die Gamma-Umkehrung sichtbar abgedunkelt werden"
        );
    }
}
