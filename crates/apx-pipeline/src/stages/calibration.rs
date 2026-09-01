//! Kalibrierung — `SPEC.md` §3.2 „Kalibrierung". Passt die Interpretation
//! der drei Kamera-Primärfarben an (Farbton-/Sättigungs-Verschiebung je
//! Primärfarbe), tönt die Schatten separat vom Weißabgleich, und wählt ein
//! Kameraprofil aus einer kleinen eingebauten Liste (siehe unten).
//!
//! Läuft — wie [`super::hsl_color_mixer`] und [`super::color_grading`] — im
//! Ein-Pixel-pro-Invocation-Modell auf linearen Kamera-RGB-Werten, aber
//! *vor* diesen beiden (direkt nach dem Weißabgleich/`basic_fused`, siehe
//! `develop.rs`): Kalibrierung passt konzeptionell die Grundinterpretation
//! der Sensordaten an, bevor Ton- und Farbwerkzeuge darauf aufbauen — echte
//! Bildbearbeitungsprogramme ordnen ihr Kalibrierungs-Panel ebenso vor den
//! übrigen Farbwerkzeugen ein.
//!
//! **Bewusste Vereinfachungen** (siehe `DECISIONS.md` ADR-0028):
//! - **Primärfarben:** echte Kalibrierung verschiebt die Kamera→Arbeitsraum-
//!   Matrix direkt. Hier — konsistent mit [`super::hsl_color_mixer`]s und
//!   `curves.rs`s Gauß-gewichtetem Zonenmodell — wirkt jede Primärfarbe
//!   (Rot/Grün/Blau) als Gauß-gewichtetes Farbton-Band um 0°/120°/240°,
//!   gewichtet-gemittelt über alle drei Bänder und als Farbton-/Sättigungs-
//!   Verschiebung im HSL-Raum angewendet — optisch ein plausibler Ersatz,
//!   aber keine echte Matrixrotation.
//! - **Schattentönung:** additive Grün-/Magenta-Verschiebung (dieselbe
//!   Konvention wie [`super::white_balance`]s Tint-Regler: positiv = weniger
//!   Grün = Richtung Magenta), gewichtet mit einer festen Gauß-Schatten-Zone
//!   (Luminanz nahe 0) statt eines editierbaren Umschlagpunkts.
//! - **Kameraprofil:** kein echter DCP-/ICC-Profilwechsel (bräuchte eine
//!   Farbmanagement-Pipeline, siehe `SPEC.md` §2.2) — [`CAMERA_PROFILES`]
//!   ist eine kleine handgepflegte Liste, jedes Profil nur ein fester
//!   Sättigungs-/Kontrast-Bias (dieselbe Formel wie `basic_fused.rs`s
//!   Kontrast-Regler), global angewendet statt zonenweise.

use bytemuck::{Pod, Zeroable};
use rayon::prelude::*;

use super::color_math::{circular_distance_degrees, gaussian_weight, hsl_to_rgb, rgb_to_hsl};
use crate::edl::v2::CalibrationAdjustment;
use crate::error::Result;
use crate::gpu::{dispatch, GpuContext};

const SHADER: &str = include_str!("calibration.wgsl");

const PRIMARY_HUE_CENTERS_DEGREES: [f32; 3] = [0.0, 120.0, 240.0];
const PRIMARY_SIGMA_DEGREES: f32 = 45.0;
const MAX_PRIMARY_HUE_SHIFT_DEGREES: f32 = 30.0;

/// Feste Gauß-Schatten-Zone für die Tönung — dieselbe Größenordnung wie
/// `curves.rs`s parametrisches Zonen-Sigma.
const SHADOW_TINT_SIGMA: f32 = 0.25;
/// Verschiebung pro Tint-Einheit bei voller Schatten-Gewichtung — bewusst
/// klein, siehe `white_balance.rs`s analoge `TINT_TO_GAIN`-Konstante.
const SHADOW_TINT_STRENGTH: f32 = 0.3;

/// Kleine, handgepflegte Kameraprofil-Liste (`(Name, Sättigungs-Bias,
/// Kontrast-Bias)`, beide in Prozentpunkten wie die übrigen Regler) — kein
/// DCP-Import, siehe Moduldoku. `"Monochrome"` nutzt bewusst einen
/// Sättigungs-Bias von `-100.0`, der über [`tonal_shift`]s Sättigungsformel
/// zu vollständiger Entsättigung führt (kein Sonderfall im Code nötig).
pub const CAMERA_PROFILES: &[(&str, f32, f32)] = &[
    ("Standard", 0.0, 0.0),
    ("Neutral", -15.0, -10.0),
    ("Vivid", 20.0, 10.0),
    ("Landscape", 10.0, 5.0),
    ("Portrait", -10.0, -5.0),
    ("Monochrome", -100.0, 5.0),
];

/// Löst einen Kameraprofil-Namen zu `(Sättigungs-Bias, Kontrast-Bias)` auf.
/// `None` sowie ein unbekannter Name (z. B. veraltete gespeicherte Daten)
/// fallen beide auf „kein Effekt" zurück statt zu einem Absturz zu führen.
fn camera_profile_preset(name: Option<&str>) -> (f32, f32) {
    match name {
        None => (0.0, 0.0),
        Some(name) => CAMERA_PROFILES
            .iter()
            .find(|(profile_name, _, _)| *profile_name == name)
            .map(|&(_, saturation, contrast)| (saturation, contrast))
            .unwrap_or((0.0, 0.0)),
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
struct PrimaryParams {
    hue: f32,
    saturation: f32,
    _pad: [f32; 2],
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct CalibrationParams {
    red_primary: PrimaryParams,
    green_primary: PrimaryParams,
    blue_primary: PrimaryParams,
    shadow_tint: f32,
    camera_profile_saturation: f32,
    camera_profile_contrast: f32,
    _pad: f32,
}

impl CalibrationParams {
    pub fn new(adjustment: &CalibrationAdjustment) -> Self {
        let (camera_profile_saturation, camera_profile_contrast) =
            camera_profile_preset(adjustment.camera_profile.as_deref());
        let to_primary = |p: crate::edl::v2::PrimaryColorAdjustment| PrimaryParams {
            hue: p.hue,
            saturation: p.saturation,
            _pad: [0.0; 2],
        };
        Self {
            red_primary: to_primary(adjustment.red_primary),
            green_primary: to_primary(adjustment.green_primary),
            blue_primary: to_primary(adjustment.blue_primary),
            shadow_tint: adjustment.shadow_tint,
            camera_profile_saturation,
            camera_profile_contrast,
            _pad: 0.0,
        }
    }
}

fn tonal_shift(r: f32, g: f32, b: f32, params: &CalibrationParams) -> (f32, f32, f32) {
    let (h, s, l) = rgb_to_hsl(r, g, b);

    let mut hue_sum = 0.0;
    let mut sat_sum = 0.0;
    let mut weight_sum = 0.0;
    for (primary, &center) in [
        params.red_primary,
        params.green_primary,
        params.blue_primary,
    ]
    .iter()
    .zip(PRIMARY_HUE_CENTERS_DEGREES.iter())
    {
        let distance = circular_distance_degrees(h, center);
        let weight = gaussian_weight(distance, PRIMARY_SIGMA_DEGREES);
        hue_sum += weight * primary.hue;
        sat_sum += weight * primary.saturation;
        weight_sum += weight;
    }
    if weight_sum < 1e-6 {
        // Unerreichbar bei drei gleichmäßig verteilten Bändern — reines
        // Sicherheitsnetz, falls die Gewichtung doch einmal kollabiert.
        return (r, g, b);
    }

    let hue_shift = (hue_sum / weight_sum) / 100.0 * MAX_PRIMARY_HUE_SHIFT_DEGREES;
    let sat_factor = 1.0 + (sat_sum / weight_sum) / 100.0;
    let (nr, mut ng, nb) = hsl_to_rgb(h + hue_shift, (s * sat_factor).clamp(0.0, 1.0), l);

    // Schattentönung — additive Grün-/Magenta-Verschiebung, gewichtet nach
    // Schatten-Nähe (siehe Moduldoku).
    let shadow_weight = gaussian_weight(l, SHADOW_TINT_SIGMA);
    ng = (ng - (params.shadow_tint / 100.0) * SHADOW_TINT_STRENGTH * shadow_weight).clamp(0.0, 1.0);

    // Kameraprofil — globaler Sättigungs-/Kontrast-Bias, unabhängig von
    // Tonwertzonen (siehe Moduldoku).
    let (h2, s2, l2) = rgb_to_hsl(nr, ng, nb);
    let profile_sat_factor = 1.0 + params.camera_profile_saturation / 100.0;
    let (pr, pg, pb) = hsl_to_rgb(h2, (s2 * profile_sat_factor).clamp(0.0, 1.0), l2);

    let contrast_factor = 1.0 + params.camera_profile_contrast / 100.0;
    let apply_contrast = |v: f32| ((v - 0.5) * contrast_factor + 0.5).clamp(0.0, 1.0);
    (apply_contrast(pr), apply_contrast(pg), apply_contrast(pb))
}

/// CPU-Fallback — dieselbe Formel wie `calibration.wgsl`.
pub fn apply_cpu(pixels: &[f32], adjustment: &CalibrationAdjustment) -> Vec<f32> {
    let params = CalibrationParams::new(adjustment);
    pixels
        .par_chunks_exact(3)
        .flat_map_iter(|rgb| {
            let (r, g, b) = tonal_shift(rgb[0], rgb[1], rgb[2], &params);
            [r, g, b]
        })
        .collect()
}

pub fn apply_gpu(
    ctx: &GpuContext,
    pixels: &[f32],
    adjustment: &CalibrationAdjustment,
) -> Result<Vec<f32>> {
    let params = CalibrationParams::new(adjustment);
    dispatch::run_compute_f32(ctx, "calibration", SHADER, "main", params, pixels, 64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::edl::v2::PrimaryColorAdjustment;

    #[test]
    fn neutral_is_identity_on_cpu() {
        let pixels = vec![0.1, 0.5, 0.9, 0.2, 0.6, 0.8];
        let result = apply_cpu(&pixels, &CalibrationAdjustment::NEUTRAL);
        for (input, output) in pixels.iter().zip(result.iter()) {
            assert!(
                (input - output).abs() < 1e-4,
                "input={input} output={output}"
            );
        }
    }

    #[test]
    fn red_primary_hue_shift_moves_a_red_pixel_hue() {
        let adjustment = CalibrationAdjustment {
            red_primary: PrimaryColorAdjustment {
                hue: 100.0,
                saturation: 0.0,
            },
            ..CalibrationAdjustment::NEUTRAL
        };
        let pixels = vec![1.0, 0.0, 0.0];
        let result = apply_cpu(&pixels, &adjustment);
        let (h_before, _, _) = rgb_to_hsl(pixels[0], pixels[1], pixels[2]);
        let (h_after, _, _) = rgb_to_hsl(result[0], result[1], result[2]);
        assert!(
            circular_distance_degrees(h_before, h_after) > 1.0,
            "h_before={h_before} h_after={h_after}"
        );
    }

    #[test]
    fn red_primary_saturation_shift_leaves_a_green_pixel_mostly_unaffected() {
        let adjustment = CalibrationAdjustment {
            red_primary: PrimaryColorAdjustment {
                hue: 0.0,
                saturation: -100.0,
            },
            ..CalibrationAdjustment::NEUTRAL
        };
        let red = vec![1.0, 0.0, 0.0];
        let green = vec![0.0, 1.0, 0.0];
        let red_result = apply_cpu(&red, &adjustment);
        let green_result = apply_cpu(&green, &adjustment);
        let (_, s_red_before, _) = rgb_to_hsl(red[0], red[1], red[2]);
        let (_, s_red_after, _) = rgb_to_hsl(red_result[0], red_result[1], red_result[2]);
        let (_, s_green_before, _) = rgb_to_hsl(green[0], green[1], green[2]);
        let (_, s_green_after, _) = rgb_to_hsl(green_result[0], green_result[1], green_result[2]);
        assert!(
            s_red_before - s_red_after > s_green_before - s_green_after,
            "Rot-Primärfarbe sollte roten Pixel stärker entsättigen als grünen"
        );
    }

    #[test]
    fn positive_shadow_tint_shifts_dark_pixels_toward_magenta_more_than_bright_ones() {
        let adjustment = CalibrationAdjustment {
            shadow_tint: 80.0,
            ..CalibrationAdjustment::NEUTRAL
        };
        let dark = vec![0.1, 0.1, 0.1];
        let bright = vec![0.9, 0.9, 0.9];
        let dark_result = apply_cpu(&dark, &adjustment);
        let bright_result = apply_cpu(&bright, &adjustment);
        // Magenta-Tönung senkt Grün relativ zu Rot/Blau — beim dunklen
        // Pixel deutlich stärker als beim hellen.
        let green_drop = |before: &[f32], after: &[f32]| before[1] - after[1];
        assert!(
            green_drop(&dark, &dark_result) > green_drop(&bright, &bright_result),
            "Schattentönung sollte dunkle Pixel stärker beeinflussen als helle"
        );
    }

    #[test]
    fn monochrome_profile_fully_desaturates() {
        let adjustment = CalibrationAdjustment {
            camera_profile: Some("Monochrome".to_string()),
            ..CalibrationAdjustment::NEUTRAL
        };
        let pixels = vec![0.8, 0.2, 0.2];
        let result = apply_cpu(&pixels, &adjustment);
        let (_, s, _) = rgb_to_hsl(result[0], result[1], result[2]);
        assert!(
            s < 1e-3,
            "Monochrome-Profil sollte vollständig entsättigen, s={s}"
        );
    }

    #[test]
    fn unknown_camera_profile_name_falls_back_to_no_effect() {
        let adjustment = CalibrationAdjustment {
            camera_profile: Some("Nicht existierendes Profil".to_string()),
            ..CalibrationAdjustment::NEUTRAL
        };
        let pixels = vec![0.1, 0.5, 0.9, 0.2, 0.6, 0.8];
        let result = apply_cpu(&pixels, &adjustment);
        for (input, output) in pixels.iter().zip(result.iter()) {
            assert!(
                (input - output).abs() < 1e-4,
                "input={input} output={output}"
            );
        }
    }

    #[test]
    fn gpu_matches_cpu() {
        let ctx = match GpuContext::new_blocking() {
            Ok(ctx) => ctx,
            Err(_) => {
                eprintln!("übersprungen: kein GPU-Adapter in dieser Umgebung verfügbar");
                return;
            }
        };
        let adjustment = CalibrationAdjustment {
            shadow_tint: 40.0,
            red_primary: PrimaryColorAdjustment {
                hue: 30.0,
                saturation: 20.0,
            },
            green_primary: PrimaryColorAdjustment {
                hue: -20.0,
                saturation: -10.0,
            },
            blue_primary: PrimaryColorAdjustment {
                hue: 10.0,
                saturation: 15.0,
            },
            camera_profile: Some("Vivid".to_string()),
            ..CalibrationAdjustment::NEUTRAL
        };
        let pixels = crate::test_support::ramp(300);
        let cpu = apply_cpu(&pixels, &adjustment);
        let gpu = apply_gpu(&ctx, &pixels, &adjustment).expect("GPU-Ausführung sollte gelingen");
        for (c, g) in cpu.iter().zip(gpu.iter()) {
            assert!((c - g).abs() < 1e-3, "CPU={c} GPU={g}");
        }
    }
}
