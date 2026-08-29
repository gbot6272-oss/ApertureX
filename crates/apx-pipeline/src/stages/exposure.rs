//! Belichtungskorrektur (`SPEC.md` §3.2 "Belichtung") — Multiplikation
//! mit `2^exposure_ev` im linearen Farbraum. Das ist die physikalisch
//! exakte Bedeutung einer Blendenstufe (EV), keine Näherung wie bei den
//! übrigen Ton-Reglern in diesem Modulverzeichnis.

use bytemuck::{Pod, Zeroable};
use rayon::prelude::*;

use crate::error::Result;
use crate::gpu::{dispatch, GpuContext};

const SHADER: &str = include_str!("exposure.wgsl");

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct ExposureParams {
    pub exposure_ev: f32,
    pub _pad: [f32; 3],
}

impl From<f32> for ExposureParams {
    fn from(exposure_ev: f32) -> Self {
        Self {
            exposure_ev,
            _pad: [0.0; 3],
        }
    }
}

/// CPU-Fallback, siehe `SPEC.md` §2.2 ("GPU→CPU-Fallback muss existieren").
pub fn apply_cpu(pixels: &[f32], exposure_ev: f32) -> Vec<f32> {
    let factor = 2f32.powf(exposure_ev);
    pixels.par_iter().map(|&v| v * factor).collect()
}

pub fn apply_gpu(ctx: &GpuContext, pixels: &[f32], exposure_ev: f32) -> Result<Vec<f32>> {
    dispatch::run_compute_f32(
        ctx,
        "exposure",
        SHADER,
        "main",
        ExposureParams::from(exposure_ev),
        pixels,
        64,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn neutral_is_identity_on_cpu() {
        let pixels = vec![0.1, 0.5, 0.9];
        assert_eq!(apply_cpu(&pixels, 0.0), pixels);
    }

    #[test]
    fn plus_one_ev_doubles_value() {
        let pixels = vec![0.1, 0.25, 0.4];
        let result = apply_cpu(&pixels, 1.0);
        for (input, output) in pixels.iter().zip(result.iter()) {
            assert!((output - input * 2.0).abs() < 1e-6);
        }
    }

    #[test]
    fn minus_one_ev_halves_value() {
        let pixels = vec![0.4, 0.8];
        let result = apply_cpu(&pixels, -1.0);
        for (input, output) in pixels.iter().zip(result.iter()) {
            assert!((output - input * 0.5).abs() < 1e-6);
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
        let cpu = apply_cpu(&pixels, 0.7);
        let gpu = apply_gpu(&ctx, &pixels, 0.7).expect("GPU-Ausführung sollte gelingen");
        for (c, g) in cpu.iter().zip(gpu.iter()) {
            assert!((c - g).abs() < 1e-4, "CPU={c} GPU={g}");
        }
    }
}
