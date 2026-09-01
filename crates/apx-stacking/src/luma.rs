//! Gemeinsame Luminanz-Umwandlung — von mehreren Stacking-Algorithmen
//! genutzt (Fokus-Stacking-Schärfemaß, Phasenkorrelations-Registrierung).
//! Dieselben Rec.601-Gewichte wie `apx_ai::color::luminance` (dort auf
//! einzelnen normierten RGB-Tripeln, hier direkt auf einem ganzen RGBA8-
//! Puffer) — erneut definiert statt importiert, da `apx-stacking` bewusst
//! nicht von `apx-ai` abhängt (dieselbe Crate-Ebene wie `apx-export`,
//! siehe `Cargo.toml`s Moduldoku).

/// Wandelt einen RGBA8-Puffer in einen `0.0..=255.0`-Luminanzpuffer um
/// (ein `f32` je Pixel, Alpha ignoriert).
pub fn rgba8_to_luma_f32(pixels: &[u8]) -> Vec<f32> {
    pixels
        .chunks_exact(4)
        .map(|px| 0.299 * px[0] as f32 + 0.587 * px[1] as f32 + 0.114 * px[2] as f32)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pure_white_has_maximum_luma() {
        let luma = rgba8_to_luma_f32(&[255, 255, 255, 255]);
        assert!((luma[0] - 255.0).abs() < 1e-3);
    }

    #[test]
    fn pure_black_has_zero_luma() {
        let luma = rgba8_to_luma_f32(&[0, 0, 0, 255]);
        assert_eq!(luma[0], 0.0);
    }

    #[test]
    fn green_contributes_more_than_blue() {
        let green_luma = rgba8_to_luma_f32(&[0, 255, 0, 255])[0];
        let blue_luma = rgba8_to_luma_f32(&[0, 0, 255, 255])[0];
        assert!(green_luma > blue_luma);
    }
}
