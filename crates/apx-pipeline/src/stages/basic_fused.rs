//! Fusionierte Grundeinstellungen — ein GPU-Dispatch statt fünf für den
//! interaktiven Vorschau-Pfad (siehe `DECISIONS.md` ADR-0017 und
//! `basic_fused.wgsl`s Modul-Kommentar für die Begründung).

use bytemuck::{Pod, Zeroable};
use rayon::prelude::*;

use super::white_balance::WhiteBalanceParams;
use crate::edl::BasicAdjustments;
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
    pub _pad: [f32; 3],
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
            _pad: [0.0; 3],
        }
    }
}

fn formula(v0: f32, channel: usize, params: &BasicFusedParams) -> f32 {
    let gain = match channel {
        0 => params.r_gain,
        1 => params.g_gain,
        _ => params.b_gain,
    };
    let mut v = v0 * gain;
    v *= 2f32.powf(params.exposure_ev);
    let contrast_factor = 1.0 + params.contrast / 100.0;
    v = (v - 0.5) * contrast_factor + 0.5;
    let hl_weight = v * v;
    let sh_weight = (1.0 - v) * (1.0 - v);
    v += (params.highlights / 100.0) * hl_weight * 0.5 + (params.shadows / 100.0) * sh_weight * 0.5;
    let w_weight = v;
    let b_weight = 1.0 - v;
    v += (params.whites / 100.0) * w_weight * 0.3 + (params.blacks / 100.0) * b_weight * 0.3;
    v
}

/// CPU-Fallback für den fusionierten Pfad — dieselbe Formel wie
/// [`formula`], die auch der WGSL-Shader implementiert.
pub fn apply_cpu(
    pixels: &[f32],
    wb_gains: WhiteBalanceParams,
    adjustments: &BasicAdjustments,
) -> Vec<f32> {
    let params = BasicFusedParams::new(wb_gains, adjustments);
    pixels
        .par_iter()
        .enumerate()
        .map(|(i, &v)| formula(v, i % 3, &params))
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

    /// Beweist, dass der fusionierte Ein-Durchlauf-Pfad dieselbe
    /// Mathematik anwendet wie die fünf Einzel-Regler nacheinander — der
    /// eigentliche Sinn von ADR-0017 (Performance-Optimierung ohne
    /// abweichendes Ergebnis).
    #[test]
    fn fused_matches_sequential_application_of_individual_stages() {
        let pixels: Vec<f32> = (0..300).map(|i| (i as f32) / 300.0).collect();
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
        let pixels: Vec<f32> = (0..300).map(|i| (i as f32) / 300.0).collect();
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
        };

        let cpu = apply_cpu(&pixels, wb_gains, &adjustments);
        let gpu = apply_gpu(&ctx, &pixels, wb_gains, &adjustments)
            .expect("GPU-Ausführung sollte gelingen");
        for (c, g) in cpu.iter().zip(gpu.iter()) {
            assert!((c - g).abs() < 1e-4, "CPU={c} GPU={g}");
        }
    }
}
