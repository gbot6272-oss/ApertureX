//! Details (Schärfung + Rauschreduzierung) — `SPEC.md` §3.2 „Details".
//! Erster Schritt, der echten Nachbarschafts-Zugriff über eine variable
//! Radius-Größe braucht (siehe `PLAN.md` Phase 4 Schritt 2:
//! `gpu/dispatch.rs::run_compute_f32` trägt das unverändert, wie schon bei
//! [`super::local_contrast`]).
//!
//! Läuft — wie [`super::local_contrast`] — im linearen Arbeitsraum, direkt
//! danach in `develop.rs` (nach Textur/Klarheit, vor HSL/Farbmischer):
//! Rauschreduzierung soll idealerweise vor nachfolgenden Farb-/Tonwert-
//! Werkzeugen laufen, die sonst Restrauschen verstärken würden, während
//! Schärfung als letzter Nachbarschafts-Schritt in derselben Kategorie
//! mitläuft — eine pragmatische Platzierung, keine strikte Nachbildung
//! einer bestimmten Lightroom-internen Rendering-Reihenfolge.
//!
//! **Bewusste Vereinfachungen** (siehe `DECISIONS.md` ADR-0028):
//! - **Ein gemeinsamer Durchlauf statt zweier sequenzieller Stufen:**
//!   echtes Lightroom wendet Rauschreduzierung *vor* Schärfung an (zwei
//!   getrennte Operationen). Hier werden beide Anteile unabhängig
//!   voneinander aus derselben Original-Nachbarschaft berechnet und
//!   anschließend addiert — spart einen zweiten vollen Bildpuffer-
//!   Durchlauf, ist für die Zwecke dieses Werkzeugs aber ausreichend nah.
//! - **Schärfung:** vereinfachtes Unsharp-Masking mit einem Box-Filter
//!   fester Ganzzahl-Radius (1–3 Pixel, aus `sharpen_radius` gerundet)
//!   statt eines echten Gauß-Kerns mit stufenlosem Radius. `sharpen_detail`
//!   skaliert die Gesamtstärke moderat statt eines echten
//!   Halo-Unterdrückungs-Verfahrens. `sharpen_masking` blendet den
//!   Hochpass-Betrag über eine `smoothstep`-Schwelle aus (kantenarme
//!   Bereiche werden weniger geschärft) statt einer echten
//!   Kantenerkennung auf Luminanzbasis.
//! - **Deconvolution-Schärfung** (`use_deconvolution_sharpen`): ein
//!   bewusst einfacher Stand-in — der Hochpass-Anteil wird über eine
//!   Potenzfunktion (Exponent < 1) statt linear verstärkt, was feine
//!   Details überproportional anhebt. Kein echtes iteratives
//!   Entfaltungsverfahren (z. B. Richardson-Lucy), das eine
//!   Punktspreizfunktions-Schätzung bräuchte.
//! - **Rauschreduzierung:** ein einfacher fester 3×3-Box-Weichzeichner
//!   (dieselbe Größenordnung wie `local_contrast.rs`s Referenz-Unschärfe)
//!   statt eines echten bilateralen oder Nicht-lokale-Mittel-Filters.
//!   Luminanz- und Farbrauschen-Reduktion teilen sich denselben
//!   Luminanz-Kantenwert für ihre jeweilige `detail`-Kantenerhaltung,
//!   statt eines eigenen Chroma-Kantenmaßes für die Farbrauschen-Stufe.

use bytemuck::{Pod, Zeroable};
use rayon::prelude::*;

use crate::edl::v2::DetailsAdjustment;
use crate::error::Result;
use crate::gpu::{dispatch, GpuContext};

const SHADER: &str = include_str!("details.wgsl");

const NR_BLUR_RADIUS: i32 = 1;
const MASKING_THRESHOLD_SCALE: f32 = 0.2;
const DETAIL_STRENGTH_BASE: f32 = 0.5;
const DETAIL_STRENGTH_RANGE: f32 = 0.5;
const DECONVOLUTION_EXPONENT: f32 = 0.6;
const EDGE_PRESERVE_SCALE: f32 = 0.15;
const CONTRAST_RESTORE_SCALE: f32 = 0.5;
const SMOOTHNESS_BOOST_RANGE: f32 = 0.5;

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct DetailsParams {
    width: u32,
    height: u32,
    sharpen_amount: f32,
    sharpen_radius: f32,
    sharpen_detail: f32,
    sharpen_masking: f32,
    use_deconvolution: f32,
    luminance_nr_amount: f32,
    luminance_nr_detail: f32,
    luminance_nr_contrast: f32,
    color_nr_amount: f32,
    color_nr_detail: f32,
    color_nr_smoothness: f32,
    _pad: [f32; 3],
}

impl DetailsParams {
    pub fn new(width: u32, height: u32, adjustment: &DetailsAdjustment) -> Self {
        Self {
            width,
            height,
            sharpen_amount: adjustment.sharpen_amount,
            sharpen_radius: adjustment.sharpen_radius,
            sharpen_detail: adjustment.sharpen_detail,
            sharpen_masking: adjustment.sharpen_masking,
            use_deconvolution: if adjustment.use_deconvolution_sharpen {
                1.0
            } else {
                0.0
            },
            luminance_nr_amount: adjustment.luminance_nr_amount,
            luminance_nr_detail: adjustment.luminance_nr_detail,
            luminance_nr_contrast: adjustment.luminance_nr_contrast,
            color_nr_amount: adjustment.color_nr_amount,
            color_nr_detail: adjustment.color_nr_detail,
            color_nr_smoothness: adjustment.color_nr_smoothness,
            _pad: [0.0; 3],
        }
    }
}

fn sample_at(pixels: &[f32], width: usize, height: usize, x: i32, y: i32, channel: usize) -> f32 {
    let cx = x.clamp(0, width as i32 - 1) as usize;
    let cy = y.clamp(0, height as i32 - 1) as usize;
    pixels[(cy * width + cx) * 3 + channel]
}

fn box_blur_radius(
    pixels: &[f32],
    width: usize,
    height: usize,
    x: usize,
    y: usize,
    channel: usize,
    radius: i32,
) -> f32 {
    let mut sum = 0.0;
    let mut count = 0.0;
    for dy in -radius..=radius {
        for dx in -radius..=radius {
            sum += sample_at(pixels, width, height, x as i32 + dx, y as i32 + dy, channel);
            count += 1.0;
        }
    }
    sum / count
}

fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Rundet den Schärfung-Radius-Regler auf einen ganzzahligen Box-Filter-
/// Radius (1–3 Pixel, siehe Moduldoku) — Lightrooms Radius-Regler bewegt
/// sich üblicherweise in `0.5..=3.0`.
fn sharpen_radius_px(radius_slider: f32) -> i32 {
    radius_slider.clamp(0.5, 3.0).round().max(1.0) as i32
}

fn process_pixel(
    pixels: &[f32],
    width: usize,
    height: usize,
    x: usize,
    y: usize,
    params: &DetailsParams,
) -> (f32, f32, f32) {
    let idx = (y * width + x) * 3;
    let r0 = pixels[idx];
    let g0 = pixels[idx + 1];
    let b0 = pixels[idx + 2];
    let luminance0 = 0.299 * r0 + 0.587 * g0 + 0.114 * b0;

    // --- Schärfung (Unsharp Masking je Kanal) ---
    let sharpen_radius = sharpen_radius_px(params.sharpen_radius);
    let mask_threshold = (params.sharpen_masking / 100.0) * MASKING_THRESHOLD_SCALE;
    let detail_factor =
        DETAIL_STRENGTH_BASE + (params.sharpen_detail / 100.0) * DETAIL_STRENGTH_RANGE;
    let sharpen_delta = |original: f32, channel: usize| -> f32 {
        let blur = box_blur_radius(pixels, width, height, x, y, channel, sharpen_radius);
        let mut high_pass = original - blur;
        if params.use_deconvolution > 0.5 {
            high_pass = high_pass.signum() * high_pass.abs().powf(DECONVOLUTION_EXPONENT);
        }
        let mask_weight = if mask_threshold < 1e-6 {
            1.0
        } else {
            smoothstep(0.0, mask_threshold, high_pass.abs())
        };
        high_pass * (params.sharpen_amount / 100.0) * detail_factor * mask_weight
    };
    let dr = sharpen_delta(r0, 0);
    let dg = sharpen_delta(g0, 1);
    let db = sharpen_delta(b0, 2);

    // --- Rauschreduzierung (fester 3×3-Box-Weichzeichner als Referenz) ---
    let nr_blur_r = box_blur_radius(pixels, width, height, x, y, 0, NR_BLUR_RADIUS);
    let nr_blur_g = box_blur_radius(pixels, width, height, x, y, 1, NR_BLUR_RADIUS);
    let nr_blur_b = box_blur_radius(pixels, width, height, x, y, 2, NR_BLUR_RADIUS);
    let luminance_blur = 0.299 * nr_blur_r + 0.587 * nr_blur_g + 0.114 * nr_blur_b;
    let luminance_edge = (luminance0 - luminance_blur).abs();

    // Luminanzrauschen: `luminance_nr_detail` bewahrt Kanten (kein
    // Verwischen dort), `luminance_nr_contrast` mischt einen Teil des
    // Originals zurück, um nicht flach zu wirken.
    let luminance_detail_threshold = (params.luminance_nr_detail / 100.0) * EDGE_PRESERVE_SCALE;
    let luminance_preserve = if luminance_detail_threshold < 1e-6 {
        0.0
    } else {
        smoothstep(0.0, luminance_detail_threshold, luminance_edge)
    };
    let luminance_blend = (params.luminance_nr_amount / 100.0) * (1.0 - luminance_preserve);
    let luminance_denoised = luminance0 + (luminance_blur - luminance0) * luminance_blend;
    let luminance_final = luminance_denoised
        + (luminance0 - luminance_denoised)
            * (params.luminance_nr_contrast / 100.0)
            * CONTRAST_RESTORE_SCALE;

    // Farbrauschen: glättet die Chroma (Kanalwert minus Luminanz) relativ
    // zu ihrer eigenen Unschärfe-Referenz — `color_nr_smoothness` boostet
    // die Mischung unabhängig von der Kantenerhaltung.
    let color_detail_threshold = (params.color_nr_detail / 100.0) * EDGE_PRESERVE_SCALE;
    let color_preserve = if color_detail_threshold < 1e-6 {
        0.0
    } else {
        smoothstep(0.0, color_detail_threshold, luminance_edge)
    };
    let smoothness_boost = (params.color_nr_smoothness / 100.0) * SMOOTHNESS_BOOST_RANGE;
    let color_blend = (params.color_nr_amount / 100.0)
        * (1.0 - color_preserve + smoothness_boost).clamp(0.0, 1.0);
    let chroma_new = |original: f32, blur: f32| -> f32 {
        let chroma0 = original - luminance0;
        let chroma_blur = blur - luminance_blur;
        chroma0 + (chroma_blur - chroma0) * color_blend
    };
    let chroma_r = chroma_new(r0, nr_blur_r);
    let chroma_g = chroma_new(g0, nr_blur_g);
    let chroma_b = chroma_new(b0, nr_blur_b);

    (
        (luminance_final + chroma_r + dr).clamp(0.0, 1.0),
        (luminance_final + chroma_g + dg).clamp(0.0, 1.0),
        (luminance_final + chroma_b + db).clamp(0.0, 1.0),
    )
}

/// CPU-Fallback — dieselbe Formel wie `details.wgsl`.
pub fn apply_cpu(
    pixels: &[f32],
    width: u32,
    height: u32,
    adjustment: &DetailsAdjustment,
) -> Vec<f32> {
    let params = DetailsParams::new(width, height, adjustment);
    let w = width as usize;
    let h = height as usize;
    (0..w * h)
        .into_par_iter()
        .flat_map_iter(move |pixel_index| {
            let x = pixel_index % w;
            let y = pixel_index / w;
            let (r, g, b) = process_pixel(pixels, w, h, x, y, &params);
            [r, g, b]
        })
        .collect()
}

pub fn apply_gpu(
    ctx: &GpuContext,
    pixels: &[f32],
    width: u32,
    height: u32,
    adjustment: &DetailsAdjustment,
) -> Result<Vec<f32>> {
    let params = DetailsParams::new(width, height, adjustment);
    dispatch::run_compute_f32(ctx, "details", SHADER, "main", params, pixels, 64)
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
        let result = apply_cpu(&pixels, 4, 1, &DetailsAdjustment::NEUTRAL);
        for (input, output) in pixels.iter().zip(result.iter()) {
            assert!(
                (input - output).abs() < 1e-6,
                "input={input} output={output}"
            );
        }
    }

    #[test]
    fn sharpening_increases_contrast_of_an_isolated_spike() {
        let pixels = gray_row(&[0.2, 0.2, 0.8, 0.2, 0.2]);
        let neutral = apply_cpu(&pixels, 5, 1, &DetailsAdjustment::NEUTRAL);
        let sharpened = apply_cpu(
            &pixels,
            5,
            1,
            &DetailsAdjustment {
                sharpen_amount: 100.0,
                ..DetailsAdjustment::NEUTRAL
            },
        );
        assert!(
            sharpened[6] > neutral[6],
            "Schärfung sollte den Spitzenwert stärker anheben (neutral={} sharpened={})",
            neutral[6],
            sharpened[6]
        );
    }

    #[test]
    fn masking_suppresses_weak_edges_more_than_strong_ones() {
        let weak_edge = gray_row(&[0.5, 0.5, 0.55, 0.5, 0.5]);
        let strong_edge = gray_row(&[0.1, 0.1, 0.9, 0.1, 0.1]);
        let base = DetailsAdjustment {
            sharpen_amount: 100.0,
            sharpen_masking: 0.0,
            ..DetailsAdjustment::NEUTRAL
        };
        let masked = DetailsAdjustment {
            sharpen_masking: 100.0,
            ..base
        };

        let weak_base = apply_cpu(&weak_edge, 5, 1, &base);
        let weak_masked = apply_cpu(&weak_edge, 5, 1, &masked);
        let strong_base = apply_cpu(&strong_edge, 5, 1, &base);
        let strong_masked = apply_cpu(&strong_edge, 5, 1, &masked);

        let weak_delta_base = (weak_base[6] - weak_edge[6]).abs();
        let weak_delta_masked = (weak_masked[6] - weak_edge[6]).abs();
        let strong_delta_base = (strong_base[6] - strong_edge[6]).abs();
        let strong_delta_masked = (strong_masked[6] - strong_edge[6]).abs();

        let weak_ratio = weak_delta_masked / weak_delta_base.max(1e-6);
        let strong_ratio = strong_delta_masked / strong_delta_base.max(1e-6);
        assert!(
            weak_ratio < strong_ratio,
            "Maskierung sollte schwache Kanten stärker unterdrücken als starke (weak_ratio={weak_ratio} strong_ratio={strong_ratio})"
        );
    }

    #[test]
    fn deconvolution_mode_differs_from_standard_sharpening() {
        let pixels = gray_row(&[0.2, 0.2, 0.8, 0.2, 0.2]);
        let standard = DetailsAdjustment {
            sharpen_amount: 50.0,
            ..DetailsAdjustment::NEUTRAL
        };
        let deconv = DetailsAdjustment {
            use_deconvolution_sharpen: true,
            ..standard
        };
        let standard_result = apply_cpu(&pixels, 5, 1, &standard);
        let deconv_result = apply_cpu(&pixels, 5, 1, &deconv);
        assert!(
            (standard_result[6] - deconv_result[6]).abs() > 1e-4,
            "Deconvolution-Modus sollte ein anderes Ergebnis liefern als Standard-Schärfung"
        );
    }

    #[test]
    fn luminance_nr_reduces_an_isolated_luminance_spike() {
        let pixels = gray_row(&[0.2, 0.2, 0.8, 0.2, 0.2]);
        let adjustment = DetailsAdjustment {
            luminance_nr_amount: 100.0,
            luminance_nr_detail: 0.0,
            ..DetailsAdjustment::NEUTRAL
        };
        let result = apply_cpu(&pixels, 5, 1, &adjustment);
        assert!(
            result[6] < pixels[6],
            "Luminanzrauschen-Reduktion sollte den Spitzenwert absenken (vorher={} nachher={})",
            pixels[6],
            result[6]
        );
    }

    #[test]
    fn color_nr_reduces_a_pure_color_spike() {
        #[rustfmt::skip]
        let pixels: Vec<f32> = vec![
            0.5, 0.5, 0.5,
            0.5, 0.5, 0.5,
            0.9, 0.3, 0.3,
            0.5, 0.5, 0.5,
            0.5, 0.5, 0.5,
        ];
        let adjustment = DetailsAdjustment {
            color_nr_amount: 100.0,
            color_nr_detail: 0.0,
            ..DetailsAdjustment::NEUTRAL
        };
        let result = apply_cpu(&pixels, 5, 1, &adjustment);
        let diff_before = pixels[6] - pixels[7];
        let diff_after = result[6] - result[7];
        assert!(
            diff_after.abs() < diff_before.abs(),
            "Farbrauschen-Reduktion sollte die Farbabweichung verringern (vorher={diff_before} nachher={diff_after})"
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
        let adjustment = DetailsAdjustment {
            sharpen_amount: 60.0,
            sharpen_radius: 2.0,
            sharpen_detail: 40.0,
            sharpen_masking: 30.0,
            use_deconvolution_sharpen: true,
            luminance_nr_amount: 50.0,
            luminance_nr_detail: 30.0,
            luminance_nr_contrast: 20.0,
            color_nr_amount: 70.0,
            color_nr_detail: 40.0,
            color_nr_smoothness: 60.0,
        };
        let pixels = crate::test_support::gray_gradient(20);
        let cpu = apply_cpu(&pixels, 20, 1, &adjustment);
        let gpu =
            apply_gpu(&ctx, &pixels, 20, 1, &adjustment).expect("GPU-Ausführung sollte gelingen");
        for (c, g) in cpu.iter().zip(gpu.iter()) {
            assert!((c - g).abs() < 1e-3, "CPU={c} GPU={g}");
        }
    }
}
