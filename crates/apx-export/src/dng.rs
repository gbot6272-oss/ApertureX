//! DNG-Schreibpfad (Phase 11 Schritt 1, siehe `DECISIONS.md` ADR-0038).
//! `FEATURES.md`s „Import mit DNG-Konvertierung" — zum Zeitpunkt von
//! ADR-0034 (Phase 8) gab es keine schreibfähige reine-Rust-DNG-Bibliothek,
//! `gamut-dng` existiert jetzt und wurde real per Testbau verifiziert.
//!
//! **Bewusste Vereinfachung — „Linear DNG" statt Bayer-Mosaik-DNG:**
//! Ein „echter" RAW→DNG-Konverter (wie Adobes eigener DNG Converter)
//! schreibt die unveränderte Bayer-Mosaik-Rohdaten des Sensors in den
//! DNG-Container, sodass ein Raw-Verarbeitungsprogramm später mit voller
//! Freiheit selbst demosaicen/entwickeln kann. `apx-raw` demosaict bereits
//! beim Dekodieren (`decode_linear`) — die Mosaik-Rohdaten selbst liegen in
//! dieser Codebasis nicht mehr isoliert vor. Diese Datei schreibt deshalb
//! bewusst eine **„Linear DNG"** (`gamut_dng::RawPhotometry::LinearRaw`,
//! `RawImage::new_linear_raw`) — ein vom DNG-1.7.1-Standard selbst
//! vorgesehenes, spec-konformes Format für bereits demosaicte
//! Kamera-native RGB-Daten (z. B. von Apple ProRAW/HDR-Zusammenführungen
//! bekannt), kein Hack. Ein Raw-Verarbeitungsprogramm kann die Datei
//! öffnen und Weißabgleich/Belichtung/Farbmatrix erneut anwenden, aber
//! nicht mehr neu demosaicen — dieselbe Einschränkung, mit der jedes
//! „Linear DNG" grundsätzlich lebt, nicht spezifisch für diese
//! Implementierung.
//!
//! **`ColorMatrix1`/`AsShotNeutral` best-effort:** `gamut-dng` erwartet
//! `ColorMatrix1` als XYZ→Kamera-nativ; `apx_raw::LinearImage` liefert nur
//! `cam_to_srgb` (Kamera-nativ→linear-sRGB). Diese Datei kombiniert
//! `cam_to_srgb` mit der festen sRGB(D65)→XYZ-Matrix und invertiert das
//! Ergebnis — mathematisch korrekt für die Werte, die wir haben, aber
//! `cam_to_srgb` selbst ist bereits (siehe `apx-raw`s Moduldoku) eine
//! kamera-generische Näherung, keine individuell kalibrierte
//! DNG-Farbmatrix. `AsShotNeutral` wird aus `as_shot_wb_coeffs[0..3]`
//! reziprok abgeleitet (DNG-Konvention: Division durch `AsShotNeutral`
//! weißabgleicht) — nur gegen den eigenen Rundreise-Test verifiziert
//! (schreiben → mit `DngDecoder` zurücklesen), nicht gegen einen echten
//! Adobe-Referenzleser.

use apx_raw::LinearImage;
use gamut_dng::{
    CalibrationIlluminant, CameraProfile, Dimensions, DngEncoder, RawImage as DngRawImage,
};

use crate::error::{ExportError, Result};

/// Feste sRGB(D65)→XYZ-Matrix (IEC 61966-2-1), row-major — dieselbe
/// Standard-Matrix, mit der auch `apx-export::icc` Profile aus
/// Chromatizitätswerten aufbaut.
const SRGB_TO_XYZ_D65: [[f64; 3]; 3] = [
    [0.4124564, 0.3575761, 0.1804375],
    [0.2126729, 0.7151522, 0.0721750],
    [0.0193339, 0.1191920, 0.9503041],
];

fn mat3_mul(a: &[[f64; 3]; 3], b: &[[f64; 3]; 3]) -> [[f64; 3]; 3] {
    let mut out = [[0.0f64; 3]; 3];
    for (i, row) in out.iter_mut().enumerate() {
        for (j, cell) in row.iter_mut().enumerate() {
            *cell = a[i][0] * b[0][j] + a[i][1] * b[1][j] + a[i][2] * b[2][j];
        }
    }
    out
}

/// Invertiert eine 3×3-Matrix über die Adjunkte/Determinante — Standard-
/// Textbuchformel, hier handgerollt statt einer weiteren Matrix-Crate-
/// Abhängigkeit für eine einzelne Operation.
fn mat3_invert(m: &[[f64; 3]; 3]) -> Result<[[f64; 3]; 3]> {
    let det = m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1])
        - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0])
        + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0]);
    if det.abs() < 1e-12 {
        return Err(ExportError::Dng {
            message: "Kamera→sRGB-Matrix ist singulär, keine DNG-Farbmatrix ableitbar".to_string(),
        });
    }
    let inv_det = 1.0 / det;
    Ok([
        [
            (m[1][1] * m[2][2] - m[1][2] * m[2][1]) * inv_det,
            (m[0][2] * m[2][1] - m[0][1] * m[2][2]) * inv_det,
            (m[0][1] * m[1][2] - m[0][2] * m[1][1]) * inv_det,
        ],
        [
            (m[1][2] * m[2][0] - m[1][0] * m[2][2]) * inv_det,
            (m[0][0] * m[2][2] - m[0][2] * m[2][0]) * inv_det,
            (m[0][2] * m[1][0] - m[0][0] * m[1][2]) * inv_det,
        ],
        [
            (m[1][0] * m[2][1] - m[1][1] * m[2][0]) * inv_det,
            (m[0][1] * m[2][0] - m[0][0] * m[2][1]) * inv_det,
            (m[0][0] * m[1][1] - m[0][1] * m[1][0]) * inv_det,
        ],
    ])
}

fn flatten3(m: [[f64; 3]; 3]) -> [f64; 9] {
    [
        m[0][0], m[0][1], m[0][2], m[1][0], m[1][1], m[1][2], m[2][0], m[2][1], m[2][2],
    ]
}

/// Wandelt `linear.pixels` (`f32`, ungeklemmt, siehe `apx-raw`s Moduldoku)
/// in interleaved `u16`-Samples für [`gamut_dng::RawImage::new_linear_raw`]
/// — lineare Streckung `[0, 1] → [0, 65535]` mit Klemmung an beiden
/// Enden, dasselbe Muster wie `apx_export::format`s 16-Bit-PNG/TIFF-Pfad.
fn pixels_to_u16(pixels: &[f32]) -> Vec<u16> {
    pixels
        .iter()
        .map(|&v| (v.clamp(0.0, 1.0) * 65535.0).round() as u16)
        .collect()
}

/// Schreibt `linear` als „Linear DNG" nach `dest`. `camera_model` füllt
/// `UniqueCameraModel` (darf nicht leer sein, siehe `CameraProfile::new`s
/// Vorbedingung) — bei fehlendem Kameramodell (z. B. Fallback-Formate ohne
/// EXIF) wird `"Aperture X Unbekannte Kamera"` verwendet.
pub fn encode_linear_dng(linear: &LinearImage, camera_model: &str) -> Result<Vec<u8>> {
    let model = if camera_model.trim().is_empty() {
        "Aperture X Unbekannte Kamera"
    } else {
        camera_model
    };

    let cam_to_srgb_f64: [[f64; 3]; 3] = {
        let m = linear.cam_to_srgb;
        [
            [m[0][0] as f64, m[0][1] as f64, m[0][2] as f64],
            [m[1][0] as f64, m[1][1] as f64, m[1][2] as f64],
            [m[2][0] as f64, m[2][1] as f64, m[2][2] as f64],
        ]
    };
    // Kamera-nativ → XYZ = (Kamera-nativ → linear-sRGB) × (sRGB → XYZ).
    let cam_to_xyz = mat3_mul(&cam_to_srgb_f64, &SRGB_TO_XYZ_D65);
    // DNGs ColorMatrix1 ist die Gegenrichtung: XYZ → Kamera-nativ.
    let xyz_to_cam = mat3_invert(&cam_to_xyz)?;

    let [wb_r, wb_g, wb_b, _wb_e] = linear.as_shot_wb_coeffs;
    let as_shot_neutral = [
        1.0 / f64::from(wb_r).max(1e-6),
        1.0 / f64::from(wb_g).max(1e-6),
        1.0 / f64::from(wb_b).max(1e-6),
    ];

    let profile = CameraProfile::new(
        model,
        flatten3(xyz_to_cam),
        CalibrationIlluminant::Unknown,
        as_shot_neutral,
    )
    .map_err(|err| ExportError::Dng {
        message: err.to_string(),
    })?;

    let samples = pixels_to_u16(&linear.pixels);
    let dims = Dimensions {
        width: linear.width,
        height: linear.height,
    };
    let raw =
        DngRawImage::new_linear_raw(dims, 16, 3, samples).map_err(|err| ExportError::Dng {
            message: err.to_string(),
        })?;

    let mut out = Vec::new();
    DngEncoder::new()
        .encode(&raw, &profile, &mut out)
        .map_err(|err| ExportError::Dng {
            message: err.to_string(),
        })?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use gamut_dng::DngDecoder;

    fn sample_linear_image() -> LinearImage {
        // 2×2, drei Kanäle, künstliche Kamera-Werte — kein echtes RAW
        // nötig, die Konvertierung ist unabhängig von der Quelle.
        LinearImage {
            width: 2,
            height: 2,
            pixels: vec![
                0.9, 0.1, 0.05, // oben links: rötlich
                0.1, 0.9, 0.1, // oben rechts: grünlich
                0.05, 0.1, 0.9, // unten links: bläulich
                0.5, 0.5, 0.5, // unten rechts: neutral
            ],
            as_shot_wb_coeffs: [1.1, 1.0, 1.3, 1.0],
            cam_to_srgb: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
        }
    }

    #[test]
    fn encodes_a_valid_dng_container() {
        let bytes = encode_linear_dng(&sample_linear_image(), "Aperture X Testkamera")
            .expect("Kodierung darf nicht fehlschlagen");
        // TIFF/DNG-Container beginnen mit einem Byte-Order-Marker.
        assert!(bytes.starts_with(b"II") || bytes.starts_with(b"MM"));
        assert!(bytes.len() > 100);
    }

    #[test]
    fn roundtrip_preserves_dimensions_and_samples() {
        let image = sample_linear_image();
        let bytes = encode_linear_dng(&image, "Aperture X Testkamera").expect("Kodierung");

        let decoded = DngDecoder::new().decode(&bytes).expect("Dekodierung");
        assert_eq!(decoded.raw.dimensions().width, image.width);
        assert_eq!(decoded.raw.dimensions().height, image.height);
        assert_eq!(decoded.raw.samples_per_pixel(), 3);

        let expected = pixels_to_u16(&image.pixels);
        assert_eq!(decoded.raw.samples(), expected.as_slice());
    }

    #[test]
    fn rejects_empty_camera_model_by_falling_back() {
        let bytes = encode_linear_dng(&sample_linear_image(), "")
            .expect("leerer Kameraname fällt auf einen Platzhalter zurück, statt zu scheitern");
        let decoded = DngDecoder::new().decode(&bytes).expect("Dekodierung");
        assert_eq!(
            decoded.profile.unique_camera_model(),
            "Aperture X Unbekannte Kamera"
        );
    }

    #[test]
    fn singular_matrix_is_reported_as_dng_error() {
        let mut image = sample_linear_image();
        image.cam_to_srgb = [[0.0; 3]; 3];
        let err = encode_linear_dng(&image, "Testkamera").unwrap_err();
        assert!(matches!(err, ExportError::Dng { .. }));
    }
}
