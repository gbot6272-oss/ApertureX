//! Fusionierte Grundeinstellungen — ein GPU-Dispatch statt vieler für den
//! interaktiven Vorschau-Pfad (siehe `DECISIONS.md` ADR-0017 und
//! `basic_fused.wgsl`s Modul-Kommentar für die Begründung).
//!
//! Deckt neun der zwölf Grundeinstellungs-Regler ab (die sieben aus
//! Phase 2 plus Dunst entfernen/Dynamik/Sättigung aus Phase 4, siehe
//! `DECISIONS.md` ADR-0011/ADR-0028) — Textur/Klarheit laufen separat in
//! [`super::local_contrast`], da sie echten Nachbarschafts-Zugriff
//! brauchen (dieser Fused-Pass bleibt bewusst ein Ein-Pixel-pro-
//! Invocation-Modell, nur mit Zugriff auf die Geschwisterkanäle
//! desselben Pixels für die Luminanz-Berechnung).

use bytemuck::{Pod, Zeroable};
use rayon::prelude::*;

use super::white_balance::WhiteBalanceParams;
use crate::edl::v2::BasicAdjustments;
use crate::error::Result;
use crate::gpu::{dispatch, GpuContext};

const SHADER: &str = include_str!("basic_fused.wgsl");

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct BasicFusedParams {
    pub r_gain: f32,
    pub g_gain: f32,
    pub b_gain: f32,
    pub exposure_ev: f32,
    pub contrast: f32,
    pub highlights: f32,
    pub shadows: f32,
    pub whites: f32,
    pub blacks: f32,
    pub dehaze: f32,
    pub vibrance: f32,
    pub saturation: f32,
}

impl BasicFusedParams {
    pub fn new(wb_gains: WhiteBalanceParams, adjustments: &BasicAdjustments) -> Self {
        Self {
            r_gain: wb_gains.r_gain,
            g_gain: wb_gains.g_gain,
            b_gain: wb_gains.b_gain,
            exposure_ev: adjustments.exposure_ev,
            contrast: adjustments.contrast,
            highlights: adjustments.highlights,
            shadows: adjustments.shadows,
            whites: adjustments.whites,
            blacks: adjustments.blacks,
            dehaze: adjustments.dehaze,
            vibrance: adjustments.vibrance,
            saturation: adjustments.saturation,
        }
    }
}

fn gain_for(channel: usize, params: &BasicFusedParams) -> f32 {
    match channel {
        0 => params.r_gain,
        1 => params.g_gain,
        _ => params.b_gain,
    }
}

/// Weißabgleich bis Weiß/Schwarz — unverändert seit Phase 2 — plus Dunst
/// entfernen (vereinfachtes Modell, kein echtes Dark-Channel-Prior-
/// Verfahren, siehe `DECISIONS.md` ADR-0028): hebt/senkt einen konstanten
/// "Schleier"-Betrag und dehnt den Kontrast wieder auf den vollen Bereich.
fn tonal(v0: f32, channel: usize, params: &BasicFusedParams) -> f32 {
    let mut v = v0 * gain_for(channel, params);
    v *= 2f32.powf(params.exposure_ev);

    let contrast_factor = 1.0 + params.contrast / 100.0;
    v = (v - 0.5) * contrast_factor + 0.5;

    let hl_weight = v * v;
    let sh_weight = (1.0 - v) * (1.0 - v);
    v += (params.highlights / 100.0) * hl_weight * 0.5 + (params.shadows / 100.0) * sh_weight * 0.5;

    let w_weight = v;
    let b_weight = 1.0 - v;
    v += (params.whites / 100.0) * w_weight * 0.3 + (params.blacks / 100.0) * b_weight * 0.3;

    let haze = params.dehaze / 100.0 * 0.2;
    v = (v - haze) / (1.0 - haze).max(0.0001);

    v
}

/// CPU-Fallback für den fusionierten Pfad — dieselbe Formel wie
/// `basic_fused.wgsl`. Verarbeitet pro Aufruf ein ganzes Pixel (nicht nur
/// einen Kanal), weil Dynamik/Sättigung die Luminanz aller drei Kanäle
/// brauchen.
pub fn apply_cpu(
    pixels: &[f32],
    wb_gains: WhiteBalanceParams,
    adjustments: &BasicAdjustments,
) -> Vec<f32> {
    let params = BasicFusedParams::new(wb_gains, adjustments);
    pixels
        .par_chunks_exact(3)
        .flat_map_iter(|rgb| {
            let v_r = tonal(rgb[0], 0, &params);
            let v_g = tonal(rgb[1], 1, &params);
            let v_b = tonal(rgb[2], 2, &params);

            let luma = 0.299 * v_r + 0.587 * v_g + 0.114 * v_b;
            let max_c = v_r.max(v_g).max(v_b);
            let min_c = v_r.min(v_g).min(v_b);
            let chroma = (max_c - min_c).clamp(0.0, 1.0);
            let vibrance_factor = 1.0 + (params.vibrance / 100.0) * (1.0 - chroma);
            let saturation_factor = 1.0 + params.saturation / 100.0;
            let total_factor = vibrance_factor * saturation_factor;

            [v_r, v_g, v_b]
                .into_iter()
                .map(move |v| luma + (v - luma) * total_factor)
        })
        .collect()
}

pub fn apply_gpu(
    ctx: &GpuContext,
    pixels: &[f32],
    wb_gains: WhiteBalanceParams,
    adjustments: &BasicAdjustments,
) -> Result<Vec<f32>> {
    let params = BasicFusedParams::new(wb_gains, adjustments);
    dispatch::run_compute_f32(ctx, "basic_fused", SHADER, "main", params, pixels, 64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::edl::WhiteBalanceAdjustment;
    use crate::stages::{contrast, exposure, highlights_shadows, white_balance, whites_blacks};

    #[test]
    fn neutral_is_identity_on_cpu() {
        let pixels = vec![0.1, 0.5, 0.9, 0.2, 0.6, 0.8];
        let wb_gains =
            white_balance::compute_gains([1.0, 1.0, 1.0, 1.0], WhiteBalanceAdjustment::NEUTRAL);
        let result = apply_cpu(&pixels, wb_gains, &BasicAdjustments::NEUTRAL);
        for (input, output) in pixels.iter().zip(result.iter()) {
            assert!((input - output).abs() < 1e-6);
        }
    }

    /// Beweist, dass der fusionierte Ein-Durchlauf-Pfad für die sieben
    /// Phase-2-Regler dieselbe Mathematik anwendet wie die fünf
    /// Einzel-Regler nacheinander — der eigentliche Sinn von ADR-0017
    /// (Performance-Optimierung ohne abweichendes Ergebnis). Dunst
    /// entfernen/Dynamik/Sättigung bleiben hier neutral, weil die
    /// Einzel-Regler-Module sie nicht kennen.
    #[test]
    fn fused_matches_sequential_application_of_individual_stages() {
        let pixels = crate::test_support::ramp(300);
        let wb_gains = white_balance::compute_gains(
            [1.05, 1.0, 0.95, 1.0],
            WhiteBalanceAdjustment {
                temp_shift_kelvin: 400.0,
                tint_shift: 10.0,
            },
        );
        let adjustments = BasicAdjustments {
            white_balance: WhiteBalanceAdjustment::NEUTRAL, // bereits in wb_gains eingerechnet
            exposure_ev: 0.4,
            contrast: 20.0,
            highlights: -30.0,
            shadows: 15.0,
            whites: 10.0,
            blacks: -5.0,
            ..BasicAdjustments::NEUTRAL
        };

        let fused = apply_cpu(&pixels, wb_gains, &adjustments);

        let sequential = {
            let step1 = white_balance::apply_cpu(&pixels, wb_gains);
            let step2 = exposure::apply_cpu(&step1, adjustments.exposure_ev);
            let step3 = contrast::apply_cpu(&step2, adjustments.contrast);
            let step4 =
                highlights_shadows::apply_cpu(&step3, adjustments.highlights, adjustments.shadows);
            whites_blacks::apply_cpu(&step4, adjustments.whites, adjustments.blacks)
        };

        for (f, s) in fused.iter().zip(sequential.iter()) {
            assert!((f - s).abs() < 1e-5, "fused={f} sequential={s}");
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
        let pixels = crate::test_support::ramp(300);
        let wb_gains = white_balance::compute_gains(
            [1.05, 1.0, 0.95, 1.0],
            WhiteBalanceAdjustment {
                temp_shift_kelvin: 200.0,
                tint_shift: -15.0,
            },
        );
        let adjustments = BasicAdjustments {
            white_balance: WhiteBalanceAdjustment::NEUTRAL,
            exposure_ev: -0.3,
            contrast: -15.0,
            highlights: 25.0,
            shadows: -10.0,
            whites: 5.0,
            blacks: 5.0,
            dehaze: 10.0,
            vibrance: 20.0,
            saturation: -10.0,
            ..BasicAdjustments::NEUTRAL
        };

        let cpu = apply_cpu(&pixels, wb_gains, &adjustments);
        let gpu = apply_gpu(&ctx, &pixels, wb_gains, &adjustments)
            .expect("GPU-Ausführung sollte gelingen");
        for (c, g) in cpu.iter().zip(gpu.iter()) {
            assert!((c - g).abs() < 1e-4, "CPU={c} GPU={g}");
        }
    }

    #[test]
    fn saturation_increases_chroma_distance_from_luma() {
        let wb_gains =
            white_balance::compute_gains([1.0, 1.0, 1.0, 1.0], WhiteBalanceAdjustment::NEUTRAL);
        // Ein deutlich buntes Pixel (viel Rot, wenig Grün/Blau).
        let pixels = vec![0.8, 0.3, 0.2];
        let neutral = apply_cpu(&pixels, wb_gains, &BasicAdjustments::NEUTRAL);
        let saturated = apply_cpu(
            &pixels,
            wb_gains,
            &BasicAdjustments {
                saturation: 50.0,
                ..BasicAdjustments::NEUTRAL
            },
        );
        let luma = |p: &[f32]| 0.299 * p[0] + 0.587 * p[1] + 0.114 * p[2];
        let distance = |p: &[f32], l: f32| (p[0] - l).abs() + (p[1] - l).abs() + (p[2] - l).abs();
        let neutral_luma = luma(&neutral);
        let saturated_luma = luma(&saturated);
        assert!(
            distance(&saturated, saturated_luma) > distance(&neutral, neutral_luma),
            "positive Sättigung sollte den Abstand zur Luminanz vergrößern"
        );
    }

    /// Der fusionierte Durchlauf skaliert für jedes Pixel den Abstand
    /// jedes Kanals zur eigenen Luminanz um denselben `vibrance_factor` —
    /// die Luminanz kürzt sich beim Differenzbilden zweier Kanäle
    /// (`out_i - out_j = (v_i - v_j) * factor`) heraus, also gilt exakt
    /// `chroma_nachher = chroma_vorher * factor`. Ein Test auf die
    /// absolute Kanal-Differenz-Summe (wie eine frühere Fassung dieses
    /// Tests) misst deshalb überwiegend die Ausgangs-Chroma selbst, nicht
    /// die eigentliche Eigenschaft — hier daher die *relative*
    /// Chroma-Zunahme, die genau `factor - 1` ist und für ein fast
    /// graues Pixel (kleine Ausgangs-Chroma) größer sein muss als für ein
    /// bereits stark gesättigtes Pixel (siehe `basic_fused.wgsl`s
    /// Kommentar zur Chroma-Gewichtung).
    #[test]
    fn vibrance_affects_low_chroma_pixels_more_than_high_chroma_ones() {
        let wb_gains =
            white_balance::compute_gains([1.0, 1.0, 1.0, 1.0], WhiteBalanceAdjustment::NEUTRAL);
        let adjustments = BasicAdjustments {
            vibrance: 80.0,
            ..BasicAdjustments::NEUTRAL
        };

        let low_chroma = vec![0.55, 0.5, 0.45]; // fast grau
        let high_chroma = vec![0.9, 0.1, 0.1]; // stark gesättigt

        let low_vibrant = apply_cpu(&low_chroma, wb_gains, &adjustments);
        let high_vibrant = apply_cpu(&high_chroma, wb_gains, &adjustments);

        let chroma = |p: &[f32]| {
            let max_c = p[0].max(p[1]).max(p[2]);
            let min_c = p[0].min(p[1]).min(p[2]);
            max_c - min_c
        };
        let relative_increase =
            |before: &[f32], after: &[f32]| chroma(after) / chroma(before) - 1.0;

        let low_relative = relative_increase(&low_chroma, &low_vibrant);
        let high_relative = relative_increase(&high_chroma, &high_vibrant);
        assert!(
            low_relative > high_relative,
            "Dynamik sollte die Chroma eines fast grauen Pixels relativ stärker anheben als die eines bereits gesättigten (low={low_relative}, high={high_relative})"
        );
    }

    #[test]
    fn dehaze_increases_contrast() {
        let wb_gains =
            white_balance::compute_gains([1.0, 1.0, 1.0, 1.0], WhiteBalanceAdjustment::NEUTRAL);
        let dark = vec![0.3, 0.3, 0.3];
        let light = vec![0.7, 0.7, 0.7];
        let adjustments = BasicAdjustments {
            dehaze: 50.0,
            ..BasicAdjustments::NEUTRAL
        };

        let dark_out = apply_cpu(&dark, wb_gains, &adjustments)[0];
        let light_out = apply_cpu(&light, wb_gains, &adjustments)[0];
        assert!(
            light_out - dark_out > light[0] - dark[0],
            "Dunst entfernen sollte den Kontrast zwischen dunkel und hell spreizen"
        );
    }
}
