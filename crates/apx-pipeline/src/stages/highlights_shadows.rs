//! Lichter/Tiefen (`SPEC.md` §3.2 "Lichter", "Tiefen") — ein Rust-Modul
//! mit zwei Parametern statt zweier getrennter Module, da beide
//! mathematisch dieselbe tonwertzonen-gewichtete Operation sind (nur mit
//! entgegengesetzt gewichteter Zone), siehe `PLAN.md` Phase 2 Schritt 4.
//!
//! **Bewusste Vereinfachung für Phase 2:** die Gewichtung (`v²` für
//! Lichter, `(1-v)²` für Tiefen) arbeitet pro Kanalwert, nicht auf der
//! tatsächlichen Pixel-Luminanz über alle drei Kanäle hinweg — eine
//! echte, perzeptionsbasierte Tonwertzonen-Maskierung bräuchte einen
//! pixel- statt elementweisen Shader-Zugriff (jeder Kanal kennt aktuell
//! nur seinen eigenen Wert, siehe `crate::gpu::dispatch`). Für Phase 2s
//! Ziel „interaktives Entwickeln" reicht die Näherung; eine
//! luminanzbasierte Variante ist Teil des vollen Tonwert-Ausbaus in
//! Phase 4. Regler `-100..100`, `0` = keine Veränderung.

use bytemuck::{Pod, Zeroable};
use rayon::prelude::*;

use crate::error::Result;
use crate::gpu::{dispatch, GpuContext};

const SHADER: &str = include_str!("highlights_shadows.wgsl");

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct HighlightsShadowsParams {
    pub highlights: f32,
    pub shadows: f32,
    pub _pad: [f32; 2],
}

impl HighlightsShadowsParams {
    pub fn new(highlights: f32, shadows: f32) -> Self {
        Self {
            highlights,
            shadows,
            _pad: [0.0; 2],
        }
    }
}

fn formula(v: f32, highlights: f32, shadows: f32) -> f32 {
    let hl_weight = v * v;
    let sh_weight = (1.0 - v) * (1.0 - v);
    v + (highlights / 100.0) * hl_weight * 0.5 + (shadows / 100.0) * sh_weight * 0.5
}

/// CPU-Fallback, siehe `SPEC.md` §2.2 ("GPU→CPU-Fallback muss existieren").
pub fn apply_cpu(pixels: &[f32], highlights: f32, shadows: f32) -> Vec<f32> {
    pixels
        .par_iter()
        .map(|&v| formula(v, highlights, shadows))
        .collect()
}

pub fn apply_gpu(
    ctx: &GpuContext,
    pixels: &[f32],
    highlights: f32,
    shadows: f32,
) -> Result<Vec<f32>> {
    dispatch::run_compute_f32(
        ctx,
        "highlights_shadows",
        SHADER,
        "main",
        HighlightsShadowsParams::new(highlights, shadows),
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
    fn negative_highlights_darkens_bright_values_more_than_dark_ones() {
        let result = apply_cpu(&[0.9, 0.1], -100.0, 0.0);
        let bright_change = 0.9 - result[0];
        let dark_change = 0.1 - result[1];
        assert!(bright_change > 0.0, "helle Werte sollten abgesenkt werden");
        assert!(
            bright_change > dark_change,
            "Lichter sollten helle Werte stärker treffen als dunkle"
        );
    }

    #[test]
    fn positive_shadows_lifts_dark_values_more_than_bright_ones() {
        let result = apply_cpu(&[0.9, 0.1], 0.0, 100.0);
        let bright_change = result[0] - 0.9;
        let dark_change = result[1] - 0.1;
        assert!(dark_change > 0.0, "dunkle Werte sollten angehoben werden");
        assert!(
            dark_change > bright_change,
            "Tiefen sollten dunkle Werte stärker treffen als helle"
        );
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
        let cpu = apply_cpu(&pixels, 40.0, -30.0);
        let gpu = apply_gpu(&ctx, &pixels, 40.0, -30.0).expect("GPU-Ausführung sollte gelingen");
        for (c, g) in cpu.iter().zip(gpu.iter()) {
            assert!((c - g).abs() < 1e-4, "CPU={c} GPU={g}");
        }
    }
}
