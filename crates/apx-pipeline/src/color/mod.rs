//! Farbmanagement für den Entwickeln-Renderpfad (siehe `crate::develop`):
//! die feste, nicht nutzerseitig verstellbare 3×3-Kamera→sRGB-Matrix
//! (aus `apx_raw::LinearImage::cam_to_srgb`) plus die sRGB-Gammakurve —
//! danach Quantisierung auf RGBA8 für die neue `develop/...`-Route
//! (`DECISIONS.md` ADR-0016, ADR-0019).
//!
//! **Bewusste Vereinfachung für Phase 2:** Die ursprünglich hier geplante
//! `lcms2`-/ProPhoto-Arbeitsraum-Anbindung (echtes ICC-Farbmanagement,
//! Monitor-/Ausgabeprofile) ist zurückgestellt — siehe `PLAN.md` Phase 2
//! Schritt 4: keiner der sieben Regler braucht sie, und Schritt 5 (dieser
//! Renderpfad) kommt mit derselben festen Kamera-Matrix + sRGB-Gammakurve
//! aus, die `apx_raw::decode()` bereits für Phase-1-Vorschauen verwendet
//! (`apx_raw::srgb_gamma`, wiederverwendet statt dupliziert). Echtes
//! ICC-Farbmanagement bleibt einem eigenen, späteren Ausbau vorbehalten,
//! sobald ein konkreter Aufrufer (z. B. eine Bildschirmprofil-Anzeige)
//! existiert.

use rayon::prelude::*;

/// Wendet die feste 3×3-Kamera→sRGB-Matrix und die sRGB-Gammakurve auf
/// einen interleaved linear-Kamera-RGB-`f32`-Puffer (`3 * width * height`
/// Elemente) an und quantisiert das Ergebnis auf interleaved RGBA8
/// (`4 * width * height` Bytes, Alpha immer `255` — undurchsichtiges
/// Foto).
pub fn linear_camera_rgb_to_srgb_rgba8(pixels: &[f32], matrix: [[f32; 3]; 3]) -> Vec<u8> {
    pixels
        .par_chunks_exact(3)
        .flat_map_iter(|rgb| {
            let transformed = [
                matrix[0][0] * rgb[0] + matrix[0][1] * rgb[1] + matrix[0][2] * rgb[2],
                matrix[1][0] * rgb[0] + matrix[1][1] * rgb[1] + matrix[1][2] * rgb[2],
                matrix[2][0] * rgb[0] + matrix[2][1] * rgb[1] + matrix[2][2] * rgb[2],
            ];
            transformed
                .into_iter()
                .map(|v| (apx_raw::srgb_gamma(v) * 255.0).round() as u8)
                .chain(std::iter::once(255u8))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const IDENTITY: [[f32; 3]; 3] = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];

    #[test]
    fn output_length_is_four_bytes_per_pixel() {
        let pixels = vec![0.0f32; 3 * 4]; // 4 Pixel
        let rgba = linear_camera_rgb_to_srgb_rgba8(&pixels, IDENTITY);
        assert_eq!(rgba.len(), 4 * 4);
    }

    #[test]
    fn alpha_channel_is_always_opaque() {
        let pixels = vec![0.3f32, 0.6, 0.9];
        let rgba = linear_camera_rgb_to_srgb_rgba8(&pixels, IDENTITY);
        assert_eq!(rgba[3], 255);
    }

    #[test]
    fn black_and_white_roundtrip_through_identity_matrix() {
        let pixels = vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0];
        let rgba = linear_camera_rgb_to_srgb_rgba8(&pixels, IDENTITY);
        assert_eq!(&rgba[0..4], &[0, 0, 0, 255]);
        assert_eq!(&rgba[4..8], &[255, 255, 255, 255]);
    }

    #[test]
    fn matrix_mixes_channels_before_gamma() {
        // Matrix vertauscht R und B — ein reiner Rot-Input muss als Blau
        // ausgegeben werden.
        let swap_r_b = [[0.0, 0.0, 1.0], [0.0, 1.0, 0.0], [1.0, 0.0, 0.0]];
        let pixels = vec![1.0, 0.0, 0.0];
        let rgba = linear_camera_rgb_to_srgb_rgba8(&pixels, swap_r_b);
        assert_eq!(rgba[0], 0, "Rot-Kanal sollte nach der Matrix 0 sein");
        assert_eq!(
            rgba[2], 255,
            "Blau-Kanal sollte den ursprünglichen Rot-Wert tragen"
        );
    }
}
