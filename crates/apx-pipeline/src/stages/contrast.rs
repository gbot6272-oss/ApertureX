//! Kontrast (`SPEC.md` §3.2 "Kontrast").
//!
//! **Bewusste Vereinfachung für Phase 2:** eine symmetrische lineare
//! Spreizung/Stauchung um den Mittelpunkt `0.5` im aktuellen (noch nicht
//! perzeptionsbasierten) Farbraum — kein S-Kurven-Kontrast, wie ihn
//! reale Fotoeditoren typischerweise verwenden. Eine perzeptionskorrekte
//! Variante (angewendet nach Farbmanagement/Output-Transform, mit
//! Luminanz statt Kanalwert) ist Teil des vollen Gradationskurven-Moduls
//! in Phase 4 (`SPEC.md` §3.2 "Gradationskurve"). Regler `-100..100`,
//! `0` = keine Veränderung.

use bytemuck::{Pod, Zeroable};
use rayon::prelude::*;

use crate::error::Result;
use crate::gpu::{dispatch, GpuContext};

const SHADER: &str = include_str!("contrast.wgsl");

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct ContrastParams {
    pub contrast: f32,
    pub _pad: [f32; 3],
}

impl From<f32> for ContrastParams {
    fn from(contrast: f32) -> Self {
        Self {
            contrast,
            _pad: [0.0; 3],
        }
    }
}

fn formula(v: f32, contrast: f32) -> f32 {
    let factor = 1.0 + contrast / 100.0;
    (v - 0.5) * factor + 0.5
}

/// CPU-Fallback, siehe `SPEC.md` §2.2 ("GPU→CPU-Fallback muss existieren").
pub fn apply_cpu(pixels: &[f32], contrast: f32) -> Vec<f32> {
    pixels.par_iter().map(|&v| formula(v, contrast)).collect()
}

pub fn apply_gpu(ctx: &GpuContext, pixels: &[f32], contrast: f32) -> Result<Vec<f32>> {
    dispatch::run_compute_f32(
        ctx,
        "contrast",
        SHADER,
        "main",
        ContrastParams::from(contrast),
        pixels,
        64,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn neutral_is_identity_on_cpu() {
        // `(v - 0.5) * 1.0 + 0.5` ist nicht bit-identisch zu `v` (f32-
        // Rundung bei der Subtraktion/Addition) — daher Toleranzvergleich
        // statt `assert_eq!`, wie bei den übrigen Reglern auch üblich.
        let pixels = vec![0.1, 0.5, 0.9];
        let result = apply_cpu(&pixels, 0.0);
        for (input, output) in pixels.iter().zip(result.iter()) {
            assert!(
                (input - output).abs() < 1e-6,
                "input={input} output={output}"
            );
        }
    }

    #[test]
    fn midpoint_is_a_fixed_point() {
        // 0.5 bleibt bei jedem Kontrastwert unverändert, da die Formel
        // um genau diesen Punkt spreizt/staucht.
        for contrast in [-100.0, -50.0, 50.0, 100.0] {
            let result = apply_cpu(&[0.5], contrast);
            assert!((result[0] - 0.5).abs() < 1e-6, "contrast={contrast}");
        }
    }

    #[test]
    fn positive_contrast_pushes_values_away_from_midpoint() {
        let result = apply_cpu(&[0.8, 0.2], 50.0);
        assert!(result[0] > 0.8);
        assert!(result[1] < 0.2);
    }

    #[test]
    fn negative_contrast_pulls_values_toward_midpoint() {
        let result = apply_cpu(&[0.8, 0.2], -50.0);
        assert!(result[0] < 0.8 && result[0] > 0.5);
        assert!(result[1] > 0.2 && result[1] < 0.5);
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
        let cpu = apply_cpu(&pixels, 35.0);
        let gpu = apply_gpu(&ctx, &pixels, 35.0).expect("GPU-Ausführung sollte gelingen");
        for (c, g) in cpu.iter().zip(gpu.iter()) {
            assert!((c - g).abs() < 1e-4, "CPU={c} GPU={g}");
        }
    }
}
