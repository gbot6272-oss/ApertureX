//! Effekte (nachträgliche Vignettierung + Körnung) — `SPEC.md` §3.2
//! „Effekte". Läuft — wie [`super::calibration`]/[`super::lens_corrections`]
//! — im linearen Arbeitsraum, direkt nach den Objektivkorrekturen, noch
//! vor der Farbraum-Konvertierung (siehe `develop.rs`). Positions-bewusst,
//! aber ohne echten Nachbarschafts-Zugriff — beide Effekte sind reine
//! Funktionen der Pixelposition, nicht ihrer Nachbarn (siehe `PLAN.md`
//! Phase 4 Schritt 2/10).
//!
//! **Bewusste Vereinfachungen** (siehe `DECISIONS.md` ADR-0028):
//! - **Vignettierung:** `roundness` blendet nur in Richtung „runder"
//!   (Ellipse passend zum Bildseitenverhältnis bei `0` → Kreis bei
//!   `100`) statt Lightrooms vollem `-100..100`-Bereich (negative Werte
//!   wirken wie `0`). `feather`/`midpoint` steuern eine einzelne
//!   `smoothstep`-Übergangszone statt eines mehrstufigen Verlaufs;
//!   `highlights` reduziert den Effekt proportional zur Pixel-Luminanz
//!   statt einer echten tonwertabhängigen Kurve.
//! - **Körnung:** kein echtes mehrstufiges Frequenz-Rauschen — ein
//!   deterministischer Ganzzahl-Hash aus der (auf Blockgröße
//!   `grain_size` heruntergerechneten) Pixelposition liefert einen
//!   Rauschwert je „Korn"-Block; `roughness` verzerrt die
//!   Rauschverteilung über eine Potenzfunktion (höher = kontrastreichere,
//!   „gröbere" Ausschläge) statt echter Strukturvarianz. Weil der
//!   Rauschwert eine reine Funktion der (Block-)Pixelposition ist —
//!   ohne Zeit-/Aufruf-Zähler-Anteil —, ist die Körnung automatisch
//!   über beliebig viele Re-Renders desselben Fotos stabil (kein
//!   Flackern beim Regler-Ziehen an anderen Werkzeugen).

use bytemuck::{Pod, Zeroable};
use rayon::prelude::*;

use super::masks::blend_pixel;
use crate::edl::v2::EffectsAdjustment;
use crate::edl::v3::BlendMode;
use crate::error::Result;
use crate::gpu::{dispatch, GpuContext};

const SHADER: &str = include_str!("effects.wgsl");

/// Maximale Helligkeitsänderung am Bildrand bei `post_vignette_amount = ±100`.
const VIGNETTE_STRENGTH: f32 = 0.6;
/// Mindestbreite der Übergangszone, auch bei `feather = 0`.
const FEATHER_MIN: f32 = 0.05;
const FEATHER_RANGE: f32 = 0.6;
/// Maximale Helligkeitsänderung durch Körnung bei `grain_amount = 100`.
const GRAIN_STRENGTH: f32 = 0.25;
const ROUGHNESS_RANGE: f32 = 0.6;

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct EffectsParams {
    width: u32,
    height: u32,
    post_vignette_amount: f32,
    post_vignette_midpoint: f32,
    post_vignette_roundness: f32,
    post_vignette_feather: f32,
    post_vignette_highlights: f32,
    grain_amount: f32,
    grain_size: f32,
    grain_roughness: f32,
    _pad: [f32; 2],
}

impl EffectsParams {
    pub fn new(width: u32, height: u32, adjustment: &EffectsAdjustment) -> Self {
        Self {
            width,
            height,
            post_vignette_amount: adjustment.post_vignette_amount,
            post_vignette_midpoint: adjustment.post_vignette_midpoint,
            post_vignette_roundness: adjustment.post_vignette_roundness,
            post_vignette_feather: adjustment.post_vignette_feather,
            post_vignette_highlights: adjustment.post_vignette_highlights,
            grain_amount: adjustment.grain_amount,
            grain_size: adjustment.grain_size,
            grain_roughness: adjustment.grain_roughness,
            _pad: [0.0; 2],
        }
    }
}

fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn hash_u32(x: u32) -> u32 {
    let mut h = x;
    h ^= h >> 16;
    h = h.wrapping_mul(0x7feb_352d);
    h ^= h >> 15;
    h = h.wrapping_mul(0x846c_a68b);
    h ^= h >> 16;
    h
}

/// Deterministischer Rauschwert in `-1.0..=1.0` für eine Ganzzahl-
/// Blockposition — dieselbe Formel wie `effects.wgsl`s `noise_at`.
fn noise_at(x: i32, y: i32) -> f32 {
    let combined = (x as u32).wrapping_mul(374_761_393) ^ (y as u32).wrapping_mul(668_265_263);
    let h = hash_u32(combined);
    (h as f32 / u32::MAX as f32) * 2.0 - 1.0
}

fn process_pixel(
    r_in: f32,
    g_in: f32,
    b_in: f32,
    px: usize,
    py: usize,
    params: &EffectsParams,
) -> (f32, f32, f32) {
    let half_w = params.width as f32 / 2.0;
    let half_h = params.height as f32 / 2.0;
    let dx = px as f32 - half_w;
    let dy = py as f32 - half_h;

    let r2_aspect = (dx / half_w).powi(2) + (dy / half_h).powi(2);
    let max_half = half_w.max(half_h);
    let r2_circle = (dx * dx + dy * dy) / (max_half * max_half);
    let roundness_blend = (params.post_vignette_roundness / 100.0).clamp(0.0, 1.0);
    let r2 = r2_aspect + (r2_circle - r2_aspect) * roundness_blend;
    let radius = r2.max(0.0).sqrt();

    let edge0 = params.post_vignette_midpoint / 100.0;
    let edge1 = edge0 + FEATHER_MIN + (params.post_vignette_feather / 100.0) * FEATHER_RANGE;
    let weight = smoothstep(edge0, edge1, radius);

    let luminance = 0.299 * r_in + 0.587 * g_in + 0.114 * b_in;
    let protection = (1.0 - (params.post_vignette_highlights / 100.0) * luminance).clamp(0.0, 1.0);
    let vignette_delta =
        (params.post_vignette_amount / 100.0) * weight * protection * VIGNETTE_STRENGTH;

    let block = ((params.grain_size / 10.0).round().max(1.0)) as i32;
    let bx = (px as i32) / block;
    let by = (py as i32) / block;
    let raw_noise = noise_at(bx, by);
    let exponent = (1.0 - (params.grain_roughness - 50.0) / 50.0 * ROUGHNESS_RANGE).max(0.05);
    let shaped_noise = raw_noise.signum() * raw_noise.abs().powf(exponent);
    let grain_delta = shaped_noise * (params.grain_amount / 100.0) * GRAIN_STRENGTH;

    let total_delta = vignette_delta + grain_delta;
    (
        (r_in + total_delta).clamp(0.0, 1.0),
        (g_in + total_delta).clamp(0.0, 1.0),
        (b_in + total_delta).clamp(0.0, 1.0),
    )
}

/// CPU-Fallback — dieselbe Formel wie `effects.wgsl`.
pub fn apply_cpu(
    pixels: &[f32],
    width: u32,
    height: u32,
    adjustment: &EffectsAdjustment,
) -> Vec<f32> {
    let params = EffectsParams::new(width, height, adjustment);
    let w = width as usize;
    let h = height as usize;
    (0..w * h)
        .into_par_iter()
        .flat_map_iter(move |pixel_index| {
            let x = pixel_index % w;
            let y = pixel_index / w;
            let idx = pixel_index * 3;
            let (r, g, b) =
                process_pixel(pixels[idx], pixels[idx + 1], pixels[idx + 2], x, y, &params);
            [r, g, b]
        })
        .collect()
}

pub fn apply_gpu(
    ctx: &GpuContext,
    pixels: &[f32],
    width: u32,
    height: u32,
    adjustment: &EffectsAdjustment,
) -> Result<Vec<f32>> {
    let params = EffectsParams::new(width, height, adjustment);
    dispatch::run_compute_f32(ctx, "effects", SHADER, "main", params, pixels, 64)
}

// ---- Halation-/Bloom-Simulation (Phase 14 Schritt 4, siehe
// DECISIONS.md ADR-0041 — Lightroom Classic "cannot create true film
// halation, only a soft bloom approximation") -------------------------------
//
// Anders als Vignettierung/Körnung oben (reine Funktionen der
// Pixelposition, passen in einen einzelnen Compute-Dispatch ohne
// Nachbarschafts-Zugriff) braucht Halation einen echten, potenziell
// großen Weichzeichnungsradius — läuft deshalb bewusst CPU-only, immer
// nach dem GPU-/CPU-Dispatch von `apply_gpu`/`apply_cpu` oben angewendet
// (derselbe Kompromiss wie `stages::repair`s inhaltsbasiertes Füllen: ein
// iterativer/nachbarschaftsbasierter Sonderfall bleibt CPU-seitig, statt
// den gesamten sonst rein-positionsbasierten Shader umzubauen). Derselbe
// separierbare Box-Weichzeichner (keine echte Gauß-Unschärfe) wie
// `stages::frequency_separation`/`stages::masks::feather_alpha`.
//
// Vier Schritte: (1) Lichter-Maske per `smoothstep`-Schwelle nahe Weiß,
// (2) warme Einfärbung der Maske über HSV→RGB (`halation_hue`), (3)
// separierbarer Box-Weichzeichner (Bloom-Ausbreitung, Radius aus
// `halation_radius`), (4) `BlendMode::Screen`-Rückmischung aufs Original,
// gewichtet mit `halation_amount`.

/// Bruchteil der Bildbreite, den `halation_radius = 100` als
/// Weichzeichnungsradius ergibt — bewusst kleiner als
/// `frequency_separation`s Radius-Bereich (dort ein fester, kleiner
/// Vorgabewert für Textur-Trennung), hier ein vom Nutzer bis zu einem
/// spürbaren, aber noch performanten Bloom hochziehbarer Regler.
const HALATION_MAX_RADIUS_FRACTION: f32 = 0.15;
/// Luminanz-Schwelle, ab der ein Pixel als "Lichter" gilt.
const HALATION_THRESHOLD: f32 = 0.85;
const HALATION_THRESHOLD_SOFTNESS: f32 = 0.12;
/// Sättigung der Einfärbung — nicht `1.0`, damit auch reines Weiß in der
/// Lichter-Maske noch als "warmes Weiß" statt einer reinen Buntfarbe
/// erscheint (näher an echter Filmhalation als eine satte Vollfarbe).
const HALATION_SATURATION: f32 = 0.85;

/// HSV → RGB, `h_deg` in Grad (beliebiger Wert, wird auf `0..360`
/// reduziert), `s`/`v` in `0.0..=1.0`.
fn hsv_to_rgb(h_deg: f32, s: f32, v: f32) -> [f32; 3] {
    let h = h_deg.rem_euclid(360.0) / 60.0;
    let c = v * s;
    let x = c * (1.0 - (h % 2.0 - 1.0).abs());
    let m = v - c;
    let (r1, g1, b1) = match h as i32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    [r1 + m, g1 + m, b1 + m]
}

fn halation_sample_at(
    pixels: &[f32],
    width: usize,
    height: usize,
    x: i32,
    y: i32,
    channel: usize,
) -> f32 {
    let cx = x.clamp(0, width as i32 - 1) as usize;
    let cy = y.clamp(0, height as i32 - 1) as usize;
    pixels[(cy * width + cx) * 3 + channel]
}

/// Separierbarer Box-Weichzeichner über alle drei Kanäle — dieselbe
/// Technik wie `stages::frequency_separation::box_blur_1d_rgb`, hier
/// separat gehalten statt geteilt (kleine, in sich geschlossene
/// Funktion, dieselbe Begründung wie bei den anderen Split-Patch-
/// Hilfsfunktionen dieses Projekts).
fn halation_box_blur_1d(
    src: &[f32],
    width: usize,
    height: usize,
    radius: i32,
    horizontal: bool,
) -> Vec<f32> {
    (0..width * height)
        .into_par_iter()
        .flat_map_iter(move |index| {
            let x = (index % width) as i32;
            let y = (index / width) as i32;
            let mut sum = [0.0f32; 3];
            let mut count = 0.0f32;
            for offset in -radius..=radius {
                let (sx, sy) = if horizontal {
                    (x + offset, y)
                } else {
                    (x, y + offset)
                };
                if sx < 0 || sy < 0 || sx as usize >= width || sy as usize >= height {
                    continue;
                }
                for (c, slot) in sum.iter_mut().enumerate() {
                    *slot += halation_sample_at(src, width, height, sx, sy, c);
                }
                count += 1.0;
            }
            sum.map(|v| if count > 0.0 { v / count } else { 0.0 })
        })
        .collect()
}

/// Wendet die Halation-/Bloom-Simulation an (siehe Moduldoku oben) — die
/// einzige Funktion, die `develop::render_rgba8` dafür aufruft, immer
/// CPU-seitig, unabhängig davon, ob `apply_cpu`/`apply_gpu` für
/// Vignettierung/Körnung lief. No-op bei `halation_amount <= 0.0`
/// (Regelfall).
pub fn apply_halation(
    pixels: &[f32],
    width: u32,
    height: u32,
    adjustment: &EffectsAdjustment,
) -> Vec<f32> {
    if adjustment.halation_amount <= 0.0 {
        return pixels.to_vec();
    }
    let w = width as usize;
    let h = height as usize;
    let tint = hsv_to_rgb(adjustment.halation_hue, HALATION_SATURATION, 1.0);

    // 1+2: Lichter-Maske, sofort eingefärbt.
    let mut glow = vec![0.0f32; pixels.len()];
    for i in 0..w * h {
        let idx = i * 3;
        let luminance = 0.299 * pixels[idx] + 0.587 * pixels[idx + 1] + 0.114 * pixels[idx + 2];
        let mask = smoothstep(
            HALATION_THRESHOLD - HALATION_THRESHOLD_SOFTNESS,
            HALATION_THRESHOLD,
            luminance,
        );
        for (c, value) in tint.iter().enumerate() {
            glow[idx + c] = value * mask;
        }
    }

    // 3: Bloom-Ausbreitung.
    let radius_px = ((adjustment.halation_radius.clamp(0.0, 100.0) / 100.0)
        * HALATION_MAX_RADIUS_FRACTION
        * width as f32)
        .round()
        .max(1.0) as i32;
    let horizontal = halation_box_blur_1d(&glow, w, h, radius_px, true);
    let blurred = halation_box_blur_1d(&horizontal, w, h, radius_px, false);

    // 4: Screen-Rückmischung, gewichtet mit `halation_amount`.
    let amount = (adjustment.halation_amount / 100.0).clamp(0.0, 1.0);
    (0..w * h)
        .into_par_iter()
        .flat_map_iter(move |i| {
            let idx = i * 3;
            let base = [pixels[idx], pixels[idx + 1], pixels[idx + 2]];
            let glow_px = [blurred[idx], blurred[idx + 1], blurred[idx + 2]];
            let screened = blend_pixel(base, glow_px, BlendMode::Screen);
            std::array::from_fn::<f32, 3, _>(|c| {
                (base[c] + (screened[c] - base[c]) * amount).clamp(0.0, 1.0)
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flat_gray(width: u32, height: u32, value: f32) -> Vec<f32> {
        vec![value; (width * height * 3) as usize]
    }

    #[test]
    fn neutral_is_identity_on_cpu() {
        let pixels = flat_gray(21, 21, 0.5);
        let result = apply_cpu(&pixels, 21, 21, &EffectsAdjustment::NEUTRAL);
        for (input, output) in pixels.iter().zip(result.iter()) {
            assert!(
                (input - output).abs() < 1e-4,
                "input={input} output={output}"
            );
        }
    }

    #[test]
    fn negative_vignette_amount_darkens_a_corner_pixel_more_than_the_center() {
        let pixels = flat_gray(21, 21, 0.5);
        let adjustment = EffectsAdjustment {
            post_vignette_amount: -100.0,
            ..EffectsAdjustment::NEUTRAL
        };
        let result = apply_cpu(&pixels, 21, 21, &adjustment);
        let corner_idx = 0;
        let center_idx = (10 * 21 + 10) * 3;
        let corner_drop = pixels[corner_idx] - result[corner_idx];
        let center_drop = pixels[center_idx] - result[center_idx];
        assert!(
            corner_drop > center_drop,
            "Negative Vignettierung sollte die Ecke stärker abdunkeln als die Mitte (corner_drop={corner_drop} center_drop={center_drop})"
        );
    }

    #[test]
    fn positive_vignette_amount_lightens_a_corner_pixel() {
        let pixels = flat_gray(21, 21, 0.5);
        let adjustment = EffectsAdjustment {
            post_vignette_amount: 100.0,
            ..EffectsAdjustment::NEUTRAL
        };
        let result = apply_cpu(&pixels, 21, 21, &adjustment);
        assert!(
            result[0] > pixels[0],
            "Positive Vignettierung sollte die Ecke aufhellen"
        );
    }

    #[test]
    fn higher_midpoint_delays_the_vignette_effect_toward_the_edge() {
        let pixels = flat_gray(41, 41, 0.5);
        let near_edge = EffectsAdjustment {
            post_vignette_amount: -100.0,
            post_vignette_midpoint: 0.0,
            ..EffectsAdjustment::NEUTRAL
        };
        let far_edge = EffectsAdjustment {
            post_vignette_midpoint: 90.0,
            ..near_edge
        };
        let result_near = apply_cpu(&pixels, 41, 41, &near_edge);
        let result_far = apply_cpu(&pixels, 41, 41, &far_edge);
        // Ein Pixel auf halbem Weg zwischen Mitte und Rand sollte beim
        // hohen Umschlagpunkt (Effekt beginnt erst später) weniger
        // abgedunkelt werden.
        let mid_idx = (20 * 41 + 30) * 3;
        let drop_near = pixels[mid_idx] - result_near[mid_idx];
        let drop_far = pixels[mid_idx] - result_far[mid_idx];
        assert!(
            drop_far < drop_near,
            "Höherer Umschlagpunkt sollte den Effekt auf halbem Weg abschwächen (drop_near={drop_near} drop_far={drop_far})"
        );
    }

    #[test]
    fn post_vignette_highlights_reduces_the_effect_on_bright_pixels() {
        let size = 21;
        let mut pixels = flat_gray(size, size, 0.1);
        // Ecke aufhellen, um den Effekt der Lichter-Schutz-Option zu testen.
        pixels[0] = 0.95;
        pixels[1] = 0.95;
        pixels[2] = 0.95;
        let without_protection = EffectsAdjustment {
            post_vignette_amount: -100.0,
            post_vignette_highlights: 0.0,
            ..EffectsAdjustment::NEUTRAL
        };
        let with_protection = EffectsAdjustment {
            post_vignette_highlights: 100.0,
            ..without_protection
        };
        let result_without = apply_cpu(&pixels, size, size, &without_protection);
        let result_with = apply_cpu(&pixels, size, size, &with_protection);
        let drop_without = pixels[0] - result_without[0];
        let drop_with = pixels[0] - result_with[0];
        assert!(
            drop_with < drop_without,
            "Lichter-Schutz sollte den Effekt auf einem hellen Eckpixel abschwächen (ohne={drop_without} mit={drop_with})"
        );
    }

    #[test]
    fn grain_amount_adds_pixel_dependent_noise() {
        let pixels = flat_gray(21, 21, 0.5);
        let adjustment = EffectsAdjustment {
            grain_amount: 100.0,
            grain_size: 1.0,
            ..EffectsAdjustment::NEUTRAL
        };
        let result = apply_cpu(&pixels, 21, 21, &adjustment);
        let distinct_values: std::collections::HashSet<_> =
            result.iter().map(|v| (v * 10000.0) as i64).collect();
        assert!(
            distinct_values.len() > 1,
            "Körnung sollte nicht überall denselben Wert liefern"
        );
    }

    #[test]
    fn grain_is_deterministic_across_repeated_calls() {
        let pixels = flat_gray(15, 15, 0.5);
        let adjustment = EffectsAdjustment {
            grain_amount: 100.0,
            ..EffectsAdjustment::NEUTRAL
        };
        let result1 = apply_cpu(&pixels, 15, 15, &adjustment);
        let result2 = apply_cpu(&pixels, 15, 15, &adjustment);
        assert_eq!(
            result1, result2,
            "Körnung muss über wiederholte Aufrufe stabil bleiben (kein Flackern)"
        );
    }

    #[test]
    fn grain_size_groups_nearby_pixels_into_the_same_block() {
        let pixels = flat_gray(21, 1, 0.5);
        let adjustment = EffectsAdjustment {
            grain_amount: 100.0,
            grain_size: 100.0, // großer Block (10px)
            ..EffectsAdjustment::NEUTRAL
        };
        let result = apply_cpu(&pixels, 21, 1, &adjustment);
        // Pixel 0 und 1 liegen im selben 10px-Block, sollten also
        // denselben Rauschwert bekommen.
        assert!(
            (result[0] - result[3]).abs() < 1e-6,
            "Benachbarte Pixel im selben Korn-Block sollten denselben Wert bekommen"
        );
    }

    #[test]
    fn higher_grain_roughness_increases_average_noise_magnitude() {
        let pixels = flat_gray(41, 41, 0.5);
        let smooth = EffectsAdjustment {
            grain_amount: 100.0,
            grain_size: 1.0,
            grain_roughness: 0.0,
            ..EffectsAdjustment::NEUTRAL
        };
        let rough = EffectsAdjustment {
            grain_roughness: 100.0,
            ..smooth
        };
        let result_smooth = apply_cpu(&pixels, 41, 41, &smooth);
        let result_rough = apply_cpu(&pixels, 41, 41, &rough);
        let mean_abs_delta = |result: &[f32]| -> f32 {
            let sum: f32 = result
                .iter()
                .zip(pixels.iter())
                .map(|(o, i)| (o - i).abs())
                .sum();
            sum / result.len() as f32
        };
        assert!(
            mean_abs_delta(&result_rough) > mean_abs_delta(&result_smooth),
            "Höhere Rauheit sollte im Mittel stärkere Ausschläge erzeugen"
        );
    }

    /// Baut ein Testbild: dunkler Grund (unter der Lichter-Schwelle) mit
    /// einem hellen weißen Block in der Mitte — der typische Fall, den
    /// Halation sichtbar machen soll (helle Lichtquelle wie eine kleine
    /// Sonnenscheibe/ein Fenster blutet warm in die dunkle Umgebung aus).
    /// Bewusst ein 7×7-Block statt eines einzelnen Pixels: ein einzelner
    /// Lichtspitzenwert würde vom Box-Weichzeichner so stark verdünnt,
    /// dass selbst ein plausibler Radius kaum eine messbare Wirkung auf
    /// benachbarte Pixel hätte — reale Lichtquellen sind ohnehin nie nur
    /// ein Pixel groß.
    fn dark_image_with_a_bright_center(size: u32) -> Vec<f32> {
        let mut pixels = flat_gray(size, size, 0.2);
        let c = (size / 2) as i32;
        for dy in -3..=3 {
            for dx in -3..=3 {
                let x = (c + dx).clamp(0, size as i32 - 1) as usize;
                let y = (c + dy).clamp(0, size as i32 - 1) as usize;
                let idx = (y * size as usize + x) * 3;
                pixels[idx] = 1.0;
                pixels[idx + 1] = 1.0;
                pixels[idx + 2] = 1.0;
            }
        }
        pixels
    }

    #[test]
    fn halation_amount_zero_is_identity() {
        let pixels = dark_image_with_a_bright_center(31);
        let result = apply_halation(&pixels, 31, 31, &EffectsAdjustment::NEUTRAL);
        assert_eq!(
            result, pixels,
            "halation_amount = 0 darf das Bild nicht verändern"
        );
    }

    #[test]
    fn halation_bleeds_warm_color_into_a_neighboring_dark_pixel() {
        let size = 121u32;
        let pixels = dark_image_with_a_bright_center(size);
        let adjustment = EffectsAdjustment {
            halation_amount: 100.0,
            halation_radius: 60.0,
            halation_hue: 15.0, // warmes Rot-Orange
            ..EffectsAdjustment::NEUTRAL
        };
        let result = apply_halation(&pixels, size, size, &adjustment);
        // Ein paar Pixel neben dem hellen Fleck, außerhalb der Lichter-
        // Maske selbst — sollte nach der Bloom-Ausbreitung merklich röter/
        // heller sein als im Original.
        let c = (size / 2) as usize;
        let near_idx = (c * size as usize + (c + 6)) * 3;
        assert!(
            result[near_idx] > pixels[near_idx] + 0.02,
            "Rotkanal sollte durch den Bloom deutlich angehoben sein (original={} ergebnis={})",
            pixels[near_idx],
            result[near_idx]
        );
        // Warmer Farbton: der Rotkanal-Zuwachs sollte größer sein als der
        // Blaukanal-Zuwachs an derselben Stelle.
        let red_gain = result[near_idx] - pixels[near_idx];
        let blue_gain = result[near_idx + 2] - pixels[near_idx + 2];
        assert!(
            red_gain > blue_gain,
            "Warme Einfärbung (Hue 15°) sollte den Rotkanal stärker anheben als den Blaukanal (rot={red_gain} blau={blue_gain})"
        );
    }

    #[test]
    fn halation_hue_shifts_the_bled_color_toward_blue() {
        let size = 121u32;
        let pixels = dark_image_with_a_bright_center(size);
        let warm = EffectsAdjustment {
            halation_amount: 100.0,
            halation_radius: 60.0,
            halation_hue: 15.0,
            ..EffectsAdjustment::NEUTRAL
        };
        let cool = EffectsAdjustment {
            halation_hue: 220.0, // Blau
            ..warm
        };
        let result_warm = apply_halation(&pixels, size, size, &warm);
        let result_cool = apply_halation(&pixels, size, size, &cool);
        let c = (size / 2) as usize;
        let near_idx = (c * size as usize + (c + 6)) * 3;
        assert!(
            result_cool[near_idx + 2] > result_warm[near_idx + 2],
            "Blauer Farbton sollte den Blaukanal am selben Nachbarpixel stärker anheben"
        );
        assert!(
            result_warm[near_idx] > result_cool[near_idx],
            "Roter Farbton sollte den Rotkanal am selben Nachbarpixel stärker anheben"
        );
    }

    #[test]
    fn larger_halation_radius_spreads_the_bloom_further() {
        let size = 161u32;
        let pixels = dark_image_with_a_bright_center(size);
        let narrow = EffectsAdjustment {
            halation_amount: 100.0,
            halation_radius: 10.0,
            halation_hue: 15.0,
            ..EffectsAdjustment::NEUTRAL
        };
        let wide = EffectsAdjustment {
            halation_radius: 100.0,
            ..narrow
        };
        let result_narrow = apply_halation(&pixels, size, size, &narrow);
        let result_wide = apply_halation(&pixels, size, size, &wide);
        // Ein Pixel deutlich vom hellen Fleck entfernt — beim schmalen
        // Radius sollte der Bloom dort kaum ankommen, beim breiten
        // spürbar mehr.
        let c = (size / 2) as usize;
        let far_idx = (c * size as usize + (c + 12)) * 3;
        let gain_narrow = result_narrow[far_idx] - pixels[far_idx];
        let gain_wide = result_wide[far_idx] - pixels[far_idx];
        assert!(
            gain_wide > gain_narrow,
            "Größerer Radius sollte den Bloom weiter ausbreiten (schmal={gain_narrow} breit={gain_wide})"
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
        let adjustment = EffectsAdjustment {
            post_vignette_amount: -60.0,
            post_vignette_midpoint: 30.0,
            post_vignette_roundness: 50.0,
            post_vignette_feather: 70.0,
            post_vignette_highlights: 40.0,
            grain_amount: 50.0,
            grain_size: 40.0,
            grain_roughness: 80.0,
            // Halation läuft ohnehin CPU-only (siehe `apply_halation`s
            // Moduldoku) — für `apply_gpu`/`apply_cpu`s Vignette-/Korn-
            // Vergleich hier bewusst aus, sonst würde dieser Test
            // fälschlich Halation statt reinen GPU/CPU-Gleichlauf prüfen.
            halation_amount: 0.0,
            halation_radius: 30.0,
            halation_hue: 15.0,
        };
        let pixels = crate::test_support::gray_gradient(20 * 15);
        let cpu = apply_cpu(&pixels, 20, 15, &adjustment);
        let gpu =
            apply_gpu(&ctx, &pixels, 20, 15, &adjustment).expect("GPU-Ausführung sollte gelingen");
        for (c, g) in cpu.iter().zip(gpu.iter()) {
            assert!((c - g).abs() < 1e-3, "CPU={c} GPU={g}");
        }
    }
}
