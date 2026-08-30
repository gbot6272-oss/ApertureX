//! Textur/Klarheit — die letzten beiden der zwölf Grundeinstellungs-Regler
//! (siehe `DECISIONS.md` ADR-0011/ADR-0028), getrennt von
//! [`super::basic_fused`], weil sie echten Nachbarschafts-Zugriff
//! brauchen (`local_contrast.wgsl`s Moduldoku erklärt, warum
//! `gpu/dispatch.rs::run_compute_f32` dafür unverändert ausreicht).
//!
//! Läuft — falls beide Regler neutral stehen — in `develop::render_rgba8`
//! gar nicht erst (kein zusätzlicher Dispatch im Regelfall).

use bytemuck::{Pod, Zeroable};
use rayon::prelude::*;

use crate::error::Result;
use crate::gpu::{dispatch, GpuContext};

const SHADER: &str = include_str!("local_contrast.wgsl");

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct LocalContrastParams {
    pub width: u32,
    pub height: u32,
    pub texture: f32,
    pub clarity: f32,
}

fn sample_at(pixels: &[f32], width: usize, height: usize, x: i32, y: i32, channel: usize) -> f32 {
    let cx = x.clamp(0, width as i32 - 1) as usize;
    let cy = y.clamp(0, height as i32 - 1) as usize;
    pixels[(cy * width + cx) * 3 + channel]
}

fn box_blur3(
    pixels: &[f32],
    width: usize,
    height: usize,
    x: usize,
    y: usize,
    channel: usize,
) -> f32 {
    let mut sum = 0.0;
    for dy in -1..=1 {
        for dx in -1..=1 {
            sum += sample_at(pixels, width, height, x as i32 + dx, y as i32 + dy, channel);
        }
    }
    sum / 9.0
}

/// CPU-Fallback — dieselbe Formel wie `local_contrast.wgsl`.
pub fn apply_cpu(pixels: &[f32], width: u32, height: u32, texture: f32, clarity: f32) -> Vec<f32> {
    let w = width as usize;
    let h = height as usize;
    (0..w * h)
        .into_par_iter()
        .flat_map_iter(move |pixel_index| {
            let x = pixel_index % w;
            let y = pixel_index / w;
            (0..3usize).map(move |channel| {
                let original = pixels[pixel_index * 3 + channel];
                let blur = box_blur3(pixels, w, h, x, y, channel);
                let high_pass = original - blur;
                let strength =
                    texture / 100.0 + (clarity / 100.0) * 4.0 * original * (1.0 - original);
                (original + high_pass * strength).clamp(0.0, 1.0)
            })
        })
        .collect()
}

pub fn apply_gpu(
    ctx: &GpuContext,
    pixels: &[f32],
    width: u32,
    height: u32,
    texture: f32,
    clarity: f32,
) -> Result<Vec<f32>> {
    let params = LocalContrastParams {
        width,
        height,
        texture,
        clarity,
    };
    dispatch::run_compute_f32(ctx, "local_contrast", SHADER, "main", params, pixels, 64)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Baut eine 1-Pixel-hohe Testreihe aus grauen (R=G=B) Pixeln.
    fn gray_row(values: &[f32]) -> Vec<f32> {
        values.iter().flat_map(|&v| [v, v, v]).collect()
    }

    #[test]
    fn neutral_is_identity_on_cpu() {
        let pixels = gray_row(&[0.1, 0.9, 0.3, 0.3]);
        let result = apply_cpu(&pixels, 2, 2, 0.0, 0.0);
        for (input, output) in pixels.iter().zip(result.iter()) {
            assert!(
                (input - output).abs() < 1e-6,
                "input={input} output={output}"
            );
        }
    }

    #[test]
    fn texture_increases_contrast_of_an_isolated_spike() {
        let pixels = gray_row(&[0.2, 0.2, 0.8, 0.2, 0.2]);
        let neutral = apply_cpu(&pixels, 5, 1, 0.0, 0.0);
        let textured = apply_cpu(&pixels, 5, 1, 100.0, 0.0);
        assert!(
            textured[6] > neutral[6],
            "Textur sollte den Spitzenwert stärker anheben (neutral={} textured={})",
            neutral[6],
            textured[6]
        );
    }

    /// Klarheit ist tonwertzonen-gewichtet (`4*v*(1-v)`) — wirkt auf ein
    /// Mitteltöne-Pixel (v nahe 0.5) deutlich stärker als auf ein Pixel
    /// nahe Weiß (v nahe 1.0), bei identischem Hochpass-Betrag.
    #[test]
    fn clarity_affects_midtones_more_than_near_white_pixels() {
        let mid = gray_row(&[0.5, 0.5, 0.6, 0.5, 0.5]);
        let extreme = gray_row(&[0.9, 0.9, 1.0, 0.9, 0.9]);

        let mid_neutral = apply_cpu(&mid, 5, 1, 0.0, 0.0);
        let mid_clarity = apply_cpu(&mid, 5, 1, 0.0, 100.0);
        let extreme_neutral = apply_cpu(&extreme, 5, 1, 0.0, 0.0);
        let extreme_clarity = apply_cpu(&extreme, 5, 1, 0.0, 100.0);

        let mid_change = (mid_clarity[6] - mid_neutral[6]).abs();
        let extreme_change = (extreme_clarity[6] - extreme_neutral[6]).abs();
        assert!(
            mid_change > extreme_change,
            "Klarheit sollte Mitteltöne stärker verändern als Pixel nahe Weiß (mid={mid_change} extreme={extreme_change})"
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
        let pixels = gray_row(&[0.1, 0.4, 0.9, 0.6, 0.2, 0.5, 0.8]);
        let cpu = apply_cpu(&pixels, 7, 1, 40.0, 60.0);
        let gpu =
            apply_gpu(&ctx, &pixels, 7, 1, 40.0, 60.0).expect("GPU-Ausführung sollte gelingen");
        for (c, g) in cpu.iter().zip(gpu.iter()) {
            assert!((c - g).abs() < 1e-4, "CPU={c} GPU={g}");
        }
    }
}
