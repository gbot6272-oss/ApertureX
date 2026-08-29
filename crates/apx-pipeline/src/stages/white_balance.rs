//! Weißabgleich (`SPEC.md` §3.2 "Weißabgleich").
//!
//! `WhiteBalanceAdjustment` (siehe `crate::edl::v1`) beschreibt eine
//! *Verschiebung relativ zum As-shot-Wert*, nicht absolute Kelvin/Tint-
//! Werte. [`compute_gains`] rechnet das zusammen mit den As-shot-
//! Koeffizienten aus den RAW-Metadaten (`apx_raw::LinearImage::
//! as_shot_wb_coeffs`) in die drei tatsächlichen Kanal-Gains um, die der
//! Shader dann nur noch multipliziert — Kamera-Kalibrierungsdaten bleiben
//! damit reine Rust-Vorbereitung, nicht Teil des Shaders.
//!
//! **Bewusste Vereinfachung für Phase 2:** Die Umrechnung von
//! Temperatur-/Tint-Verschiebung in Kanal-Gains ist eine einfache lineare
//! Näherung, keine physikalisch korrekte Planckscher-Strahler-Berechnung
//! (die bräuchte eine vollständige Farbmanagement-Pipeline, siehe
//! `SPEC.md` §2.2 „Farbmanagement" — kommt mit `lcms2`-Integration in
//! einer späteren Ausbaustufe). Für Phase 2s Ziel „interaktives
//! Entwickeln" reicht die Näherung: Regler bewegen sich intuitiv in die
//! richtige Richtung, exakte Farbtemperaturwerte sind nicht das Ziel.

use bytemuck::{Pod, Zeroable};
use rayon::prelude::*;

use crate::edl::WhiteBalanceAdjustment;
use crate::error::Result;
use crate::gpu::{dispatch, GpuContext};

const SHADER: &str = include_str!("white_balance.wgsl");

/// Verschiebung pro Kelvin/Tint-Einheit — siehe Modul-Doku zur
/// Vereinfachung. Bewusst klein gewählt, damit der volle praktische
/// Reglerbereich (grob ±2000 K, ±100 Tint) zu einem moderaten, nicht
/// überzogenen Farbstich führt.
const KELVIN_TO_GAIN: f32 = 0.0001;
const TINT_TO_GAIN: f32 = 0.001;

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable, PartialEq)]
pub struct WhiteBalanceParams {
    pub r_gain: f32,
    pub g_gain: f32,
    pub b_gain: f32,
    pub _pad: f32,
}

/// Rechnet As-shot-Koeffizienten (`[R, G, B, E]` — RGBE-Konvention wie
/// von `rawler`s `RawImage::wb_coeffs` geliefert, siehe
/// `apx-raw/src/pipeline/color.rs`) und die gewünschte Verschiebung in
/// die drei finalen Kanal-Gains um. Der vierte (Emerald-)Koeffizient
/// wird ignoriert — wie bei der bestehenden `ColorPipeline` entfällt er,
/// weil das Demosaicing bereits auf reines RGB reduziert (siehe dort).
pub fn compute_gains(
    as_shot_wb_coeffs: [f32; 4],
    adjustment: WhiteBalanceAdjustment,
) -> WhiteBalanceParams {
    let base_r = as_shot_wb_coeffs[0];
    let base_g = as_shot_wb_coeffs[1];
    let base_b = as_shot_wb_coeffs[2];

    let shift_r = 1.0 + adjustment.temp_shift_kelvin * KELVIN_TO_GAIN;
    let shift_b = 1.0 - adjustment.temp_shift_kelvin * KELVIN_TO_GAIN;
    let shift_g = 1.0 - adjustment.tint_shift * TINT_TO_GAIN;

    WhiteBalanceParams {
        r_gain: base_r * shift_r,
        g_gain: base_g * shift_g,
        b_gain: base_b * shift_b,
        _pad: 0.0,
    }
}

/// CPU-Fallback, siehe `SPEC.md` §2.2 ("GPU→CPU-Fallback muss existieren").
pub fn apply_cpu(pixels: &[f32], gains: WhiteBalanceParams) -> Vec<f32> {
    pixels
        .par_iter()
        .enumerate()
        .map(|(i, &v)| {
            let gain = match i % 3 {
                0 => gains.r_gain,
                1 => gains.g_gain,
                _ => gains.b_gain,
            };
            v * gain
        })
        .collect()
}

pub fn apply_gpu(ctx: &GpuContext, pixels: &[f32], gains: WhiteBalanceParams) -> Result<Vec<f32>> {
    dispatch::run_compute_f32(ctx, "white_balance", SHADER, "main", gains, pixels, 64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn neutral_shift_and_neutral_as_shot_yields_unit_gains() {
        let gains = compute_gains([1.0, 1.0, 1.0, 1.0], WhiteBalanceAdjustment::NEUTRAL);
        assert_eq!(
            gains,
            WhiteBalanceParams {
                r_gain: 1.0,
                g_gain: 1.0,
                b_gain: 1.0,
                _pad: 0.0
            }
        );
    }

    #[test]
    fn as_shot_coefficients_are_preserved_without_user_shift() {
        let gains = compute_gains([2.0, 1.0, 1.5, 1.0], WhiteBalanceAdjustment::NEUTRAL);
        assert_eq!(gains.r_gain, 2.0);
        assert_eq!(gains.g_gain, 1.0);
        assert_eq!(gains.b_gain, 1.5);
    }

    #[test]
    fn warming_shift_increases_red_and_decreases_blue_gain() {
        let neutral = compute_gains([1.0, 1.0, 1.0, 1.0], WhiteBalanceAdjustment::NEUTRAL);
        let warmer = compute_gains(
            [1.0, 1.0, 1.0, 1.0],
            WhiteBalanceAdjustment {
                temp_shift_kelvin: 1000.0,
                tint_shift: 0.0,
            },
        );
        assert!(warmer.r_gain > neutral.r_gain);
        assert!(warmer.b_gain < neutral.b_gain);
        assert_eq!(
            warmer.g_gain, neutral.g_gain,
            "Tint unverändert -> Grün-Gain unverändert"
        );
    }

    #[test]
    fn neutral_gains_are_identity_on_cpu() {
        let pixels = vec![0.1, 0.2, 0.3, 0.4, 0.5, 0.6];
        let gains = compute_gains([1.0, 1.0, 1.0, 1.0], WhiteBalanceAdjustment::NEUTRAL);
        assert_eq!(apply_cpu(&pixels, gains), pixels);
    }

    #[test]
    fn cpu_applies_gain_per_channel() {
        let pixels = vec![0.1, 0.2, 0.3];
        let gains = WhiteBalanceParams {
            r_gain: 2.0,
            g_gain: 3.0,
            b_gain: 4.0,
            _pad: 0.0,
        };
        let result = apply_cpu(&pixels, gains);
        assert!((result[0] - 0.2).abs() < 1e-6);
        assert!((result[1] - 0.6).abs() < 1e-6);
        assert!((result[2] - 1.2).abs() < 1e-6);
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
        let gains = compute_gains(
            [1.1, 1.0, 0.9, 1.0],
            WhiteBalanceAdjustment {
                temp_shift_kelvin: 300.0,
                tint_shift: -20.0,
            },
        );
        let cpu = apply_cpu(&pixels, gains);
        let gpu = apply_gpu(&ctx, &pixels, gains).expect("GPU-Ausführung sollte gelingen");
        for (c, g) in cpu.iter().zip(gpu.iter()) {
            assert!((c - g).abs() < 1e-4, "CPU={c} GPU={g}");
        }
    }
}
