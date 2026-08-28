//! Weißabgleich, Kamera-RGB→sRGB-Transformation und Gammakurve.
//!
//! **Reihenfolge bewusst wie in `PHASE1_PROMPT.md` Abschnitt 3 festgelegt:**
//! Demosaicing kommt vor dem Weißabgleich. Das ist mathematisch
//! gleichwertig zur sonst üblichen Reihenfolge (Weißabgleich vor
//! Demosaicing), *solange* das Demosaicing — wie hier — pro Kanal nur
//! gleichfarbige Nachbarn mittelt: Mittelwertbildung und Skalierung mit
//! einer Konstante sind vertauschbar (`avg(k·x) = k·avg(x)`). Diese
//! Rechenordnung wird in Phase 2 durch die GPU-Pipeline ersetzt.

use rawler::imgop::matrix::{multiply, pseudo_inverse};
use rawler::imgop::xyz::XYZ_TO_SRGB_D65;
use rawler::RawImage;

/// Wendet auf ein einzelnes demosaicedes (aber noch nicht weißabgeglichenes)
/// RGB-Pixel Weißabgleich, Kamera→sRGB-Matrix und sRGB-Gammakurve an.
/// Eingabe und Zwischenwerte sind linear in `[0, 1]` (können durch die
/// Matrixmultiplikation kurzzeitig außerhalb liegen, werden aber vor der
/// Gammakurve geklemmt).
pub struct ColorPipeline {
    /// RGBE-Weißabgleich-Koeffizienten aus der RAW-Datei (`as shot`).
    wb_coeffs: [f32; 4],
    /// Direkte 3×3-Matrix Kamera-RGB → sRGB (linear, D65).
    cam_to_srgb: [[f32; 3]; 3],
}

impl ColorPipeline {
    /// Baut die Farbpipeline aus den in `image` enthaltenen
    /// Kamera-Metadaten (Weißabgleich-Koeffizienten, XYZ→Kamera-Matrix).
    pub fn from_raw_image(image: &RawImage) -> Self {
        Self {
            wb_coeffs: image.wb_coeffs,
            cam_to_srgb: cam_to_srgb_matrix(&image.xyz_to_cam),
        }
    }

    /// Wendet Weißabgleich + Farbmatrix auf ein demosaicedes RGB-Tripel an.
    /// Gibt lineares sRGB zurück, geklemmt auf `[0, 1]`.
    pub fn to_linear_srgb(&self, demosaiced_rgb: [f32; 3]) -> [f32; 3] {
        let balanced = [
            demosaiced_rgb[0] * self.wb_coeffs[0],
            demosaiced_rgb[1] * self.wb_coeffs[1],
            demosaiced_rgb[2] * self.wb_coeffs[2],
        ];

        let m = &self.cam_to_srgb;
        let srgb = [
            m[0][0] * balanced[0] + m[0][1] * balanced[1] + m[0][2] * balanced[2],
            m[1][0] * balanced[0] + m[1][1] * balanced[1] + m[1][2] * balanced[2],
            m[2][0] * balanced[0] + m[2][1] * balanced[1] + m[2][2] * balanced[2],
        ];

        [
            srgb[0].clamp(0.0, 1.0),
            srgb[1].clamp(0.0, 1.0),
            srgb[2].clamp(0.0, 1.0),
        ]
    }
}

/// Berechnet die direkte 3×3-Transformation Kamera-RGB → sRGB(D65) aus der
/// XYZ→Kamera-Matrix (`xyz_to_cam`, 4 Zeilen RGBE × 3 Spalten XYZ):
///
/// 1. Pseudo-Inverse von `xyz_to_cam` ergibt Kamera(4)→XYZ(3).
/// 2. Multiplikation mit `XYZ_TO_SRGB_D65` (3×3) ergibt Kamera(4)→sRGB(3).
/// 3. Nur die ersten drei Spalten (R/G/B) werden übernommen — der vierte
///    Kanal (E/Smaragd) entfällt, weil das Demosaicing bereits auf reines
///    RGB reduziert.
fn cam_to_srgb_matrix(xyz_to_cam: &[[f32; 3]; 4]) -> [[f32; 3]; 3] {
    let cam_to_xyz = pseudo_inverse::<4>(*xyz_to_cam); // [[f32; 4]; 3]
    let cam_to_srgb_4 = multiply::<3, 3, 4>(&XYZ_TO_SRGB_D65, &cam_to_xyz); // [[f32; 4]; 3]

    let mut result = [[0.0_f32; 3]; 3];
    for (row, cam_row) in result.iter_mut().zip(cam_to_srgb_4.iter()) {
        row.copy_from_slice(&cam_row[0..3]);
    }
    result
}

/// sRGB-Gammakurve (OETF), Standardformel.
pub fn srgb_gamma(linear: f32) -> f32 {
    let v = linear.clamp(0.0, 1.0);
    if v <= 0.003_130_8 {
        v * 12.92
    } else {
        1.055 * v.powf(1.0 / 2.4) - 0.055
    }
}

/// Wandelt einen `[0, 1]`-Gamma-korrigierten Wert in 16-Bit um.
pub fn to_u16(gamma_corrected: f32) -> u16 {
    (gamma_corrected.clamp(0.0, 1.0) * 65535.0).round() as u16
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_xyz_to_cam_yields_finite_matrix() {
        // xyz_to_cam = XYZ_TO_SRGB_D65 (erweitert um eine E-Zeile) sollte
        // eine Matrix nahe der Identität ergeben, weil sich die
        // Transformation dann größtenteils aufhebt.
        let mut xyz_to_cam = [[0.0_f32; 3]; 4];
        xyz_to_cam[0] = XYZ_TO_SRGB_D65[0];
        xyz_to_cam[1] = XYZ_TO_SRGB_D65[1];
        xyz_to_cam[2] = XYZ_TO_SRGB_D65[2];
        xyz_to_cam[3] = XYZ_TO_SRGB_D65[1]; // E-Zeile, hier irrelevant

        let m = cam_to_srgb_matrix(&xyz_to_cam);
        for row in &m {
            for v in row {
                assert!(v.is_finite());
            }
        }
    }

    #[test]
    fn gamma_curve_is_monotonic_and_bounded() {
        let mut previous = srgb_gamma(0.0);
        assert!((0.0..=1.0).contains(&previous));
        let mut steps = Vec::new();
        for i in 1..=20 {
            let v = srgb_gamma(i as f32 / 20.0);
            steps.push(v);
            assert!(v >= previous, "Gammakurve muss monoton steigend sein");
            assert!((0.0..=1.0).contains(&v));
            previous = v;
        }
    }

    #[test]
    fn gamma_zero_and_one_map_to_zero_and_one() {
        assert_eq!(srgb_gamma(0.0), 0.0);
        assert!((srgb_gamma(1.0) - 1.0).abs() < 1e-5);
    }

    #[test]
    fn to_u16_rounds_and_clamps() {
        assert_eq!(to_u16(0.0), 0);
        assert_eq!(to_u16(1.0), 65535);
        assert_eq!(to_u16(-1.0), 0);
        assert_eq!(to_u16(2.0), 65535);
    }
}
