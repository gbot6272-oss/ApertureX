//! Weiß/Schwarz (`SPEC.md` §3.2 "Weiß", "Schwarz") — ein Rust-Modul mit
//! zwei Parametern statt zweier getrennter Module, analog zu
//! `highlights_shadows` (dieselbe lineare Clipping-Punkt-Verschiebung,
//! nur an den beiden Enden statt in den Tonwertzonen-Mitten).
//!
//! **Bewusste Vereinfachung für Phase 2:** siehe `highlights_shadows`s
//! Modul-Doku — dieselbe Einschränkung (kanalweise statt
//! luminanzbasiert) gilt hier ebenso. Regler `-100..100`, `0` = keine
//! Veränderung. Positiv = heller (Weißpunkt höher / Schwarzpunkt
//! angehoben), negativ = dunkler.

use bytemuck::{Pod, Zeroable};
use rayon::prelude::*;

use crate::error::Result;
use crate::gpu::{dispatch, GpuContext};

const SHADER: &str = include_str!("whites_blacks.wgsl");

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct WhitesBlacksParams {
    pub whites: f32,
    pub blacks: f32,
    pub _pad: [f32; 2],
}

impl WhitesBlacksParams {
    pub fn new(whites: f32, blacks: f32) -> Self {
        Self {
            whites,
            blacks,
            _pad: [0.0; 2],
        }
    }
}

fn formula(v: f32, whites: f32, blacks: f32) -> f32 {
    let w_weight = v;
    let b_weight = 1.0 - v;
    v + (whites / 100.0) * w_weight * 0.3 + (blacks / 100.0) * b_weight * 0.3
}

/// CPU-Fallback, siehe `SPEC.md` §2.2 ("GPU→CPU-Fallback muss existieren").
pub fn apply_cpu(pixels: &[f32], whites: f32, blacks: f32) -> Vec<f32> {
    pixels
        .par_iter()
        .map(|&v| formula(v, whites, blacks))
        .collect()
}

pub fn apply_gpu(ctx: &GpuContext, pixels: &[f32], whites: f32, blacks: f32) -> Result<Vec<f32>> {
    dispatch::run_compute_f32(
        ctx,
        "whites_blacks",
        SHADER,
        "main",
        WhitesBlacksParams::new(whites, blacks),
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
        assert_eq!(apply_cpu(&pixels, 0.0, 0.0), pixels);
    }

    #[test]
    fn positive_whites_raises_bright_values_more_than_dark_ones() {
        let result = apply_cpu(&[0.9, 0.1], 100.0, 0.0);
        let bright_change = result[0] - 0.9;
        let dark_change = result[1] - 0.1;
        assert!(bright_change > 0.0);
        assert!(bright_change > dark_change);
    }

    #[test]
    fn negative_blacks_lowers_dark_values_more_than_bright_ones() {
        let result = apply_cpu(&[0.9, 0.1], 0.0, -100.0);
        let bright_change = 0.9 - result[0];
        let dark_change = 0.1 - result[1];
        assert!(dark_change > 0.0);
        assert!(dark_change > bright_change);
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
        let cpu = apply_cpu(&pixels, -20.0, 25.0);
        let gpu = apply_gpu(&ctx, &pixels, -20.0, 25.0).expect("GPU-Ausführung sollte gelingen");
        for (c, g) in cpu.iter().zip(gpu.iter()) {
            assert!((c - g).abs() < 1e-4, "CPU={c} GPU={g}");
        }
    }
}
