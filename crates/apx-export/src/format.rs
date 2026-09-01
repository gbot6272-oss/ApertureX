//! Ausgabeformate für die Export-Engine (Phase 8 Schritt 1, siehe
//! `DECISIONS.md` ADR-0034 Punkt 1). Kodiert einen fertig gerenderten
//! interleaved-RGBA8-Puffer (`apx_pipeline::develop::RenderedImage`) in das
//! gewählte Dateiformat — JPEG/PNG/TIFF sind dieselben Codecs, die
//! `apx-raw`/`apx-app`s Vorschau-Cache schon nutzt (`image`-Crate);
//! WebP (verlustfrei, über `image-webp`, reines Rust) und AVIF
//! (verlustbehaftet über `ravif`/`rav1e`, ebenfalls reines Rust) sind neu.
//!
//! **Bewusste Vereinfachung — 16-Bit-Ausgabetiefe (`BitDepth::Sixteen`):**
//! `apx-pipeline::develop::render_rgba8` quantisiert intern bereits auf
//! 8-Bit-RGBA (siehe dessen Moduldoku) — Kurven, Masken und Geometrie
//! laufen alle auf dem fertigen `u8`-Puffer. Eine echte durchgehende
//! 16-Bit-Präzision (zusätzliche Tonwertstufen gegen Banding) würde diese
//! Stufen alle auf einen `f32`/`u16`-Pfad umstellen — eine Kernänderung an
//! der Rendering-Pipeline, die den Rahmen der Export-Engine sprengt. Was
//! diese Datei stattdessen tut: den fertigen 8-Bit-Wert linear auf den
//! vollen 16-Bit-Bereich strecken (`v * 257`, `0..=255 → 0..=65535`,
//! exakt umkehrbar). Das ist eine reine **Dateiformat-Kompatibilität**
//! (z. B. für Druckereien, die nur 16-Bit-TIFF annehmen), keine echte
//! Präzisionssteigerung — siehe `FEATURES.md` für die Nutzer-sichtbare
//! Einschränkung. Nur für PNG/TIFF verfügbar: JPEG kennt keine 16-Bit-
//! Tiefe, WebP/AVIF unterstützen in dieser Bibliothekskombination nur
//! 8-Bit.

use std::io::Cursor;

use image::codecs::avif::AvifEncoder;
use image::codecs::jpeg::JpegEncoder;
use image::{DynamicImage, ExtendedColorType, ImageBuffer, ImageEncoder, ImageFormat, Rgba};

use crate::error::{ExportError, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ExportFormat {
    Jpeg,
    Png,
    Tiff,
    /// Verlustfrei (`image-webp`, reines Rust) — siehe Moduldoku.
    WebP,
    /// Verlustbehaftet (`ravif`/`rav1e`, reines Rust).
    Avif,
}

impl ExportFormat {
    pub fn extension(self) -> &'static str {
        match self {
            Self::Jpeg => "jpg",
            Self::Png => "png",
            Self::Tiff => "tiff",
            Self::WebP => "webp",
            Self::Avif => "avif",
        }
    }

    pub fn supports_alpha(self) -> bool {
        !matches!(self, Self::Jpeg)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BitDepth {
    Eight,
    Sixteen,
}

#[derive(Debug, Clone, Copy)]
pub struct EncodeOptions {
    /// `1..=100`, nur für JPEG/AVIF relevant (ignoriert bei verlustfreien
    /// Formaten). Wird bei der Kodierung auf den gültigen Bereich geklemmt.
    pub quality: u8,
    pub bit_depth: BitDepth,
}

impl Default for EncodeOptions {
    fn default() -> Self {
        Self {
            quality: 90,
            bit_depth: BitDepth::Eight,
        }
    }
}

/// Kodiert einen interleaved-RGBA8-Puffer (`4 * width * height` Bytes) in
/// `format`. Gibt [`ExportError::Unsupported`] bei einer nicht
/// unterstützten Format-/Bit-Tiefen-Kombination zurück (siehe Moduldoku).
pub fn encode_rgba8(
    width: u32,
    height: u32,
    pixels: &[u8],
    format: ExportFormat,
    options: &EncodeOptions,
) -> Result<Vec<u8>> {
    let expected_len = width as usize * height as usize * 4;
    if pixels.len() != expected_len {
        return Err(ExportError::Unsupported(format!(
            "Pufferlänge {} passt nicht zu {width}x{height} RGBA8 (erwartet {expected_len})",
            pixels.len()
        )));
    }

    if options.bit_depth == BitDepth::Sixteen
        && !matches!(format, ExportFormat::Png | ExportFormat::Tiff)
    {
        return Err(ExportError::Unsupported(format!(
            "16-Bit-Ausgabe wird für {:?} nicht unterstützt (nur PNG/TIFF)",
            format
        )));
    }

    match format {
        ExportFormat::Jpeg => encode_jpeg(width, height, pixels, options.quality),
        ExportFormat::Avif => encode_avif(width, height, pixels, options.quality),
        ExportFormat::Png => {
            encode_via_dynamic(width, height, pixels, options.bit_depth, ImageFormat::Png)
        }
        ExportFormat::Tiff => {
            encode_via_dynamic(width, height, pixels, options.bit_depth, ImageFormat::Tiff)
        }
        ExportFormat::WebP => {
            encode_via_dynamic(width, height, pixels, options.bit_depth, ImageFormat::WebP)
        }
    }
}

fn encode_jpeg(width: u32, height: u32, pixels: &[u8], quality: u8) -> Result<Vec<u8>> {
    let rgb: Vec<u8> = pixels
        .chunks_exact(4)
        .flat_map(|p| [p[0], p[1], p[2]])
        .collect();
    let mut out = Vec::new();
    JpegEncoder::new_with_quality(&mut out, quality.clamp(1, 100))
        .write_image(&rgb, width, height, ExtendedColorType::Rgb8)
        .map_err(|err| ExportError::Encode {
            message: err.to_string(),
        })?;
    Ok(out)
}

fn encode_avif(width: u32, height: u32, pixels: &[u8], quality: u8) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    // Geschwindigkeitsstufe 6 (von 1..=10, `ravif`-Skala): guter Mittelweg
    // zwischen Kodierzeit und Kompressionseffizienz für einen interaktiven
    // Export-Vorgang statt eines Offline-Batch-Jobs.
    AvifEncoder::new_with_speed_quality(&mut out, 6, quality.clamp(1, 100))
        .write_image(pixels, width, height, ExtendedColorType::Rgba8)
        .map_err(|err| ExportError::Encode {
            message: err.to_string(),
        })?;
    Ok(out)
}

fn encode_via_dynamic(
    width: u32,
    height: u32,
    pixels: &[u8],
    bit_depth: BitDepth,
    format: ImageFormat,
) -> Result<Vec<u8>> {
    let dynamic = to_dynamic_image(width, height, pixels, bit_depth)?;
    let mut out = Vec::new();
    let mut cursor = Cursor::new(&mut out);
    dynamic
        .write_to(&mut cursor, format)
        .map_err(|err| ExportError::Encode {
            message: err.to_string(),
        })?;
    Ok(out)
}

fn to_dynamic_image(
    width: u32,
    height: u32,
    pixels: &[u8],
    bit_depth: BitDepth,
) -> Result<DynamicImage> {
    match bit_depth {
        BitDepth::Eight => {
            let buf = ImageBuffer::<Rgba<u8>, Vec<u8>>::from_raw(width, height, pixels.to_vec())
                .ok_or_else(|| ExportError::Encode {
                    message: "Pufferlayout passt nicht zu Breite/Höhe".to_string(),
                })?;
            Ok(DynamicImage::ImageRgba8(buf))
        }
        BitDepth::Sixteen => {
            // Linear auf den vollen 16-Bit-Bereich strecken, siehe Moduldoku.
            let widened: Vec<u16> = pixels.iter().map(|&v| v as u16 * 257).collect();
            let buf = ImageBuffer::<Rgba<u16>, Vec<u16>>::from_raw(width, height, widened)
                .ok_or_else(|| ExportError::Encode {
                    message: "Pufferlayout passt nicht zu Breite/Höhe".to_string(),
                })?;
            Ok(DynamicImage::ImageRgba16(buf))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn checkerboard(width: u32, height: u32) -> Vec<u8> {
        (0..width * height)
            .flat_map(|i| {
                let v = if i % 2 == 0 { 255 } else { 0 };
                [v, v, v, 255]
            })
            .collect()
    }

    #[test]
    fn jpeg_roundtrips_through_the_image_crate() {
        let pixels = checkerboard(4, 4);
        let bytes =
            encode_rgba8(4, 4, &pixels, ExportFormat::Jpeg, &EncodeOptions::default()).unwrap();
        let decoded = image::load_from_memory(&bytes).unwrap();
        assert_eq!(decoded.width(), 4);
        assert_eq!(decoded.height(), 4);
    }

    #[test]
    fn png_16bit_roundtrips_and_widens_precision() {
        let mut pixels = checkerboard(2, 2);
        pixels[0] = 128; // ein einzelner Kanalwert zur Präzisionsprüfung
        let options = EncodeOptions {
            quality: 90,
            bit_depth: BitDepth::Sixteen,
        };
        let bytes = encode_rgba8(2, 2, &pixels, ExportFormat::Png, &options).unwrap();
        let decoded = image::load_from_memory(&bytes).unwrap();
        assert!(matches!(decoded, DynamicImage::ImageRgba16(_)));
        let rgba16 = decoded.into_rgba16();
        assert_eq!(rgba16.get_pixel(0, 0).0[0], 128u16 * 257);
    }

    #[test]
    fn webp_lossless_roundtrips() {
        let pixels = checkerboard(4, 4);
        let bytes =
            encode_rgba8(4, 4, &pixels, ExportFormat::WebP, &EncodeOptions::default()).unwrap();
        let decoded = image::load_from_memory_with_format(&bytes, ImageFormat::WebP).unwrap();
        assert_eq!(decoded.to_rgba8().into_raw(), pixels);
    }

    #[test]
    fn avif_produces_a_well_formed_isobmff_container() {
        // `image`s "avif"-Feature liefert nur den Encoder (`ravif`), keinen
        // Decoder (der bräuchte das separate "avif-native"-Feature mit
        // einer C-Systembibliothek, siehe `EncodeOptions`s Moduldoku) — ein
        // echter Dekodier-Roundtrip ist mit dieser Feature-Kombination
        // nicht möglich. Stattdessen wird die ISOBMFF-Signatur geprüft
        // (`ftyp`-Box mit `avif`-Brand, siehe AVIF-Spezifikation) — genug,
        // um eine echte, nicht leere AVIF-Datei von einem stillen Encoder-
        // Fehlschlag zu unterscheiden.
        let pixels = checkerboard(8, 8);
        let bytes =
            encode_rgba8(8, 8, &pixels, ExportFormat::Avif, &EncodeOptions::default()).unwrap();
        assert!(bytes.len() > 32);
        assert_eq!(&bytes[4..8], b"ftyp");
        assert_eq!(&bytes[8..12], b"avif");
    }

    #[test]
    fn sixteen_bit_jpeg_is_rejected() {
        let pixels = checkerboard(2, 2);
        let options = EncodeOptions {
            quality: 90,
            bit_depth: BitDepth::Sixteen,
        };
        let err = encode_rgba8(2, 2, &pixels, ExportFormat::Jpeg, &options).unwrap_err();
        assert!(matches!(err, ExportError::Unsupported(_)));
    }

    #[test]
    fn mismatched_buffer_length_is_rejected() {
        let pixels = vec![0u8; 3]; // zu kurz für 2x2 RGBA8
        let err =
            encode_rgba8(2, 2, &pixels, ExportFormat::Png, &EncodeOptions::default()).unwrap_err();
        assert!(matches!(err, ExportError::Unsupported(_)));
    }
}
