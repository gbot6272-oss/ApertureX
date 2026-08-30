//! HSL (acht feste Farbbänder) + Farbmischer erweitert (offene Liste
//! benutzerdefinierter Farbbereiche) — `SPEC.md` §3.2 „HSL", „Farbmischer".
//! Beide verschieben Farbton/Sättigung/Luminanz gewichtet nach
//! Farbton-Abstand zum jeweiligen Band-/Regionen-Zentrum (Gauß-Gewichtung,
//! analog zu `curves.rs`s parametrischer Kurve).
//!
//! Läuft — wie `basic_fused` — im Ein-Pixel-pro-Invocation-Modell auf den
//! linearen Kamera-RGB-Werten, direkt nach `local_contrast` und vor der
//! Farbraum-Konvertierung (nicht wie `curves` auf dem fertigen sRGB-Bild):
//! HSL/Sättigung sind wie das bestehende `vibrance`/`saturation` in
//! `basic_fused` Tonwert-Operationen, die im selben linearen Arbeitsraum
//! bleiben sollen.
//!
//! **Bewusste Vereinfachung (Farbmischer):** die offene Liste
//! benutzerdefinierter Regionen wird für diesen fusionierten GPU/CPU-Pfad
//! auf [`MAX_COLOR_MIXER_REGIONS`] feste Slots gekappt (das Frontend
//! verhindert bereits das Anlegen weiterer Regionen, siehe
//! `frontend/src/components/DevelopPanel.tsx`) — mehr dürfte in der
//! Praxis kaum vorkommen, ein echter dynamischer Puffer wäre hier ein
//! nicht gerechtfertigter Mehraufwand (`Params: Pod` braucht eine zur
//! Kompilierzeit feste Größe).

use bytemuck::{Pod, Zeroable};
use rayon::prelude::*;

use crate::edl::v2::{ColorMixerAdjustment, HslAdjustment};
use crate::error::Result;
use crate::gpu::{dispatch, GpuContext};

const SHADER: &str = include_str!("hsl_color_mixer.wgsl");

const HSL_BAND_COUNT: usize = 8;
/// Siehe Moduldoku — Obergrenze für benutzerdefinierte Farbmischer-Regionen
/// im fusionierten Pfad.
pub const MAX_COLOR_MIXER_REGIONS: usize = 8;

const HSL_BAND_CENTERS_DEGREES: [f32; HSL_BAND_COUNT] =
    [0.0, 30.0, 60.0, 120.0, 180.0, 240.0, 270.0, 300.0];
const HSL_BAND_SIGMA_DEGREES: f32 = 25.0;
const MAX_HUE_SHIFT_DEGREES: f32 = 60.0;

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
struct BandParams {
    hue: f32,
    saturation: f32,
    luminance: f32,
    _pad: f32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
struct RegionParams {
    target_hue_degrees: f32,
    bandwidth_degrees: f32,
    feather: f32,
    hue_shift: f32,
    saturation_shift: f32,
    luminance_shift: f32,
    /// `1.0` = belegter Slot, `0.0` = leer (zählt nicht mit).
    is_active: f32,
    _pad: f32,
}

const EMPTY_REGION: RegionParams = RegionParams {
    target_hue_degrees: 0.0,
    bandwidth_degrees: 0.0,
    feather: 0.0,
    hue_shift: 0.0,
    saturation_shift: 0.0,
    luminance_shift: 0.0,
    is_active: 0.0,
    _pad: 0.0,
};

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct HslColorMixerParams {
    bands: [BandParams; HSL_BAND_COUNT],
    regions: [RegionParams; MAX_COLOR_MIXER_REGIONS],
}

impl HslColorMixerParams {
    pub fn new(hsl: &HslAdjustment, color_mixer: &ColorMixerAdjustment) -> Self {
        let bands = [
            hsl.red,
            hsl.orange,
            hsl.yellow,
            hsl.green,
            hsl.aqua,
            hsl.blue,
            hsl.purple,
            hsl.magenta,
        ]
        .map(|band| BandParams {
            hue: band.hue,
            saturation: band.saturation,
            luminance: band.luminance,
            _pad: 0.0,
        });

        let mut regions = [EMPTY_REGION; MAX_COLOR_MIXER_REGIONS];
        for (slot, region) in regions.iter_mut().zip(color_mixer.regions.iter()) {
            *slot = RegionParams {
                target_hue_degrees: region.target_hue_degrees,
                bandwidth_degrees: region.bandwidth_degrees,
                feather: region.feather,
                hue_shift: region.hue_shift,
                saturation_shift: region.saturation_shift,
                luminance_shift: region.luminance_shift,
                is_active: 1.0,
                _pad: 0.0,
            };
        }

        Self { bands, regions }
    }
}

fn circular_distance_degrees(a: f32, b: f32) -> f32 {
    let diff = (a - b).abs() % 360.0;
    diff.min(360.0 - diff)
}

fn gaussian_weight(distance: f32, sigma: f32) -> f32 {
    (-(distance * distance) / (2.0 * sigma * sigma)).exp()
}

fn rgb_to_hsl(r: f32, g: f32, b: f32) -> (f32, f32, f32) {
    let max_c = r.max(g).max(b);
    let min_c = r.min(g).min(b);
    let l = (max_c + min_c) / 2.0;
    let d = max_c - min_c;
    if d < 1e-6 {
        return (0.0, 0.0, l);
    }
    let s = if l > 0.5 {
        d / (2.0 - max_c - min_c)
    } else {
        d / (max_c + min_c)
    };
    let h = if max_c == r {
        ((g - b) / d).rem_euclid(6.0)
    } else if max_c == g {
        (b - r) / d + 2.0
    } else {
        (r - g) / d + 4.0
    };
    (h * 60.0, s, l)
}

fn hue_to_rgb_component(p: f32, q: f32, mut t: f32) -> f32 {
    if t < 0.0 {
        t += 1.0;
    }
    if t > 1.0 {
        t -= 1.0;
    }
    if t < 1.0 / 6.0 {
        return p + (q - p) * 6.0 * t;
    }
    if t < 1.0 / 2.0 {
        return q;
    }
    if t < 2.0 / 3.0 {
        return p + (q - p) * (2.0 / 3.0 - t) * 6.0;
    }
    p
}

fn hsl_to_rgb(h_degrees: f32, s: f32, l: f32) -> (f32, f32, f32) {
    if s <= 0.0 {
        return (l, l, l);
    }
    let h = h_degrees.rem_euclid(360.0) / 360.0;
    let q = if l < 0.5 {
        l * (1.0 + s)
    } else {
        l + s - l * s
    };
    let p = 2.0 * l - q;
    (
        hue_to_rgb_component(p, q, h + 1.0 / 3.0),
        hue_to_rgb_component(p, q, h),
        hue_to_rgb_component(p, q, h - 1.0 / 3.0),
    )
}

fn tonal_shift(r: f32, g: f32, b: f32, params: &HslColorMixerParams) -> (f32, f32, f32) {
    let (h, s, l) = rgb_to_hsl(r, g, b);

    let mut hue_sum = 0.0;
    let mut sat_sum = 0.0;
    let mut lum_sum = 0.0;
    let mut weight_sum = 0.0;

    for (band, &center) in params.bands.iter().zip(HSL_BAND_CENTERS_DEGREES.iter()) {
        let distance = circular_distance_degrees(h, center);
        let weight = gaussian_weight(distance, HSL_BAND_SIGMA_DEGREES);
        hue_sum += weight * band.hue;
        sat_sum += weight * band.saturation;
        lum_sum += weight * band.luminance;
        weight_sum += weight;
    }

    for region in params.regions.iter() {
        if region.is_active <= 0.0 {
            continue;
        }
        let distance = circular_distance_degrees(h, region.target_hue_degrees);
        let sigma = (region.bandwidth_degrees * (0.5 + region.feather.clamp(0.0, 1.0))).max(1.0);
        let weight = gaussian_weight(distance, sigma);
        hue_sum += weight * region.hue_shift;
        sat_sum += weight * region.saturation_shift;
        lum_sum += weight * region.luminance_shift;
        weight_sum += weight;
    }

    if weight_sum < 1e-6 {
        // Unerreichbar bei den festen 8 HSL-Bändern (siehe Moduldoku-
        // Analyse in DECISIONS.md-Kommentar oben) — reiner Sicherheitsnetz-
        // Rückfall, falls doch einmal alle Gewichte kollabieren.
        return (r, g, b);
    }

    let hue_shift = (hue_sum / weight_sum) / 100.0 * MAX_HUE_SHIFT_DEGREES;
    let sat_factor = 1.0 + (sat_sum / weight_sum) / 100.0;
    let lum_shift = (lum_sum / weight_sum) / 100.0 * 0.3;

    let new_h = h + hue_shift;
    let new_s = (s * sat_factor).clamp(0.0, 1.0);
    let new_l = (l + lum_shift).clamp(0.0, 1.0);

    hsl_to_rgb(new_h, new_s, new_l)
}

/// CPU-Fallback — dieselbe Formel wie `hsl_color_mixer.wgsl`.
pub fn apply_cpu(
    pixels: &[f32],
    hsl: &HslAdjustment,
    color_mixer: &ColorMixerAdjustment,
) -> Vec<f32> {
    let params = HslColorMixerParams::new(hsl, color_mixer);
    pixels
        .par_chunks_exact(3)
        .flat_map_iter(|rgb| {
            let (r, g, b) = tonal_shift(rgb[0], rgb[1], rgb[2], &params);
            [r, g, b]
        })
        .collect()
}

pub fn apply_gpu(
    ctx: &GpuContext,
    pixels: &[f32],
    hsl: &HslAdjustment,
    color_mixer: &ColorMixerAdjustment,
) -> Result<Vec<f32>> {
    let params = HslColorMixerParams::new(hsl, color_mixer);
    dispatch::run_compute_f32(ctx, "hsl_color_mixer", SHADER, "main", params, pixels, 64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::edl::v2::{ColorMixerRegion, HslBand};

    #[test]
    fn neutral_is_identity_on_cpu() {
        let pixels = vec![0.1, 0.5, 0.9, 0.2, 0.6, 0.8];
        let result = apply_cpu(
            &pixels,
            &HslAdjustment::NEUTRAL,
            &ColorMixerAdjustment {
                regions: Vec::new(),
            },
        );
        for (input, output) in pixels.iter().zip(result.iter()) {
            assert!(
                (input - output).abs() < 1e-4,
                "input={input} output={output}"
            );
        }
    }

    #[test]
    fn red_band_hue_shift_moves_a_red_pixel_toward_orange() {
        let mut hsl = HslAdjustment::NEUTRAL;
        hsl.red = HslBand {
            hue: 80.0, // dreht in Richtung Orange (positiver Farbton-Winkel)
            saturation: 0.0,
            luminance: 0.0,
        };
        // Ein sattes Rot (Farbton 0°).
        let pixels = vec![0.8, 0.1, 0.1];
        let result = apply_cpu(
            &pixels,
            &hsl,
            &ColorMixerAdjustment {
                regions: Vec::new(),
            },
        );
        let (h_before, _, _) = rgb_to_hsl(pixels[0], pixels[1], pixels[2]);
        let (h_after, _, _) = rgb_to_hsl(result[0], result[1], result[2]);
        assert!(
            h_before < 5.0,
            "Testannahme: Ausgangspixel sollte nahe Farbton 0° liegen, war {h_before}"
        );
        assert!(h_after > h_before, "Farbton sollte sich in Richtung Orange verschieben (vorher={h_before} nachher={h_after})");
    }

    #[test]
    fn red_band_saturation_shift_does_not_affect_a_green_pixel() {
        let mut hsl = HslAdjustment::NEUTRAL;
        hsl.red = HslBand {
            hue: 0.0,
            saturation: 80.0,
            luminance: 0.0,
        };
        let green_pixel = vec![0.1, 0.8, 0.1]; // Farbton 120° — weit weg vom Rot-Band
        let result = apply_cpu(
            &green_pixel,
            &hsl,
            &ColorMixerAdjustment {
                regions: Vec::new(),
            },
        );
        for (input, output) in green_pixel.iter().zip(result.iter()) {
            assert!((input - output).abs() < 1e-3, "Grünes Pixel sollte vom Rot-Band praktisch unberührt bleiben (input={input} output={output})");
        }
    }

    #[test]
    fn color_mixer_region_shifts_only_pixels_near_its_target_hue() {
        let region = ColorMixerRegion {
            target_hue_degrees: 200.0, // Cyan-artiger Bereich
            bandwidth_degrees: 20.0,
            feather: 0.2,
            hue_shift: 0.0,
            saturation_shift: -80.0,
            luminance_shift: 0.0,
        };
        let color_mixer = ColorMixerAdjustment {
            regions: vec![region],
        };

        // Ein Pixel nahe 200° Farbton und eines nahe 0° (Rot) — nur das
        // erste sollte spürbar entsättigt werden.
        let near_pixel = vec![0.1, 0.55, 0.8]; // ungefähr im Cyan-Bereich
        let far_pixel = vec![0.8, 0.1, 0.1]; // Rot

        let near_result = apply_cpu(&near_pixel, &HslAdjustment::NEUTRAL, &color_mixer);
        let far_result = apply_cpu(&far_pixel, &HslAdjustment::NEUTRAL, &color_mixer);

        let chroma = |p: &[f32]| p[0].max(p[1]).max(p[2]) - p[0].min(p[1]).min(p[2]);
        let near_change = (chroma(&near_pixel) - chroma(&near_result)).abs();
        let far_change = (chroma(&far_pixel) - chroma(&far_result)).abs();
        assert!(
            near_change > far_change,
            "Farbmischer-Region sollte den nahen Farbton stärker entsättigen als den fernen (near_change={near_change} far_change={far_change})"
        );
    }

    #[test]
    fn regions_beyond_the_cap_are_silently_ignored_without_panicking() {
        let regions = (0..MAX_COLOR_MIXER_REGIONS + 3)
            .map(|i| ColorMixerRegion {
                target_hue_degrees: i as f32 * 10.0,
                bandwidth_degrees: 10.0,
                feather: 0.0,
                hue_shift: 10.0,
                saturation_shift: 0.0,
                luminance_shift: 0.0,
            })
            .collect();
        let pixels = vec![0.4, 0.4, 0.4];
        // Darf nicht abstürzen — mehr als MAX_COLOR_MIXER_REGIONS Einträge
        // werden einfach ignoriert (siehe Moduldoku).
        let _ = apply_cpu(
            &pixels,
            &HslAdjustment::NEUTRAL,
            &ColorMixerAdjustment { regions },
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
        let mut hsl = HslAdjustment::NEUTRAL;
        hsl.red = HslBand {
            hue: 30.0,
            saturation: -20.0,
            luminance: 10.0,
        };
        hsl.aqua = HslBand {
            hue: -15.0,
            saturation: 40.0,
            luminance: -5.0,
        };
        let color_mixer = ColorMixerAdjustment {
            regions: vec![ColorMixerRegion {
                target_hue_degrees: 90.0,
                bandwidth_degrees: 30.0,
                feather: 0.3,
                hue_shift: 20.0,
                saturation_shift: 15.0,
                luminance_shift: -10.0,
            }],
        };
        let pixels = crate::test_support::ramp(300);
        let cpu = apply_cpu(&pixels, &hsl, &color_mixer);
        let gpu =
            apply_gpu(&ctx, &pixels, &hsl, &color_mixer).expect("GPU-Ausführung sollte gelingen");
        for (c, g) in cpu.iter().zip(gpu.iter()) {
            assert!((c - g).abs() < 1e-3, "CPU={c} GPU={g}");
        }
    }
}
