//! Fallback-Pfad für JPEG/PNG/TIFF — Formate, die keine echte RAW-Struktur
//! haben. Dekodierung läuft über die `image`-Crate, Metadaten (soweit
//! vorhanden) über `kamadak-exif`.

use std::fs::File;
use std::io::BufReader;
use std::path::Path;

use apx_core::{AppError, Result};
use image::ImageReader;

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
        captured_at: None, // Datum/Zeit-Parsing für den Fallback-Pfad ist in Phase 1 nicht
        // erforderlich (JPEG/PNG/TIFF sind hier nur ein Auffangnetz für
        // Nicht-RAW-Importe); die Timestamp-Logik lebt zentral in
        // `metadata.rs` für den RAW-Pfad.
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
