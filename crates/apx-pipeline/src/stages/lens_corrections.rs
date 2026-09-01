//! Objektivkorrekturen (`SPEC.md` §3.2 „Objektivkorrekturen") — chromatische
//! Aberration, Vignettierung, Verzeichnung, Perspektive/Upright und
//! manuelle Transformation, kombiniert zu einer einzigen inversen
//! geometrischen Abbildung mit bilinearem Sampling (siehe `PLAN.md`
//! Phase 4 Schritt 2/9: die "größenverändernde" Dispatch-Form aus dem
//! Architektur-Grundsatz ist hier NICHT nötig, siehe unten).
//!
//! Läuft — anders als die übrigen linearen Werkzeuge — nach
//! [`super::color_grading`] und vor der Farbraum-Konvertierung, da eine
//! geometrische Korrektur konzeptionell auf dem fertig farblich
//! bearbeiteten Bild sitzt (ähnlich Adobe Camera Raws interner
//! Reihenfolge: Objektivkorrekturen laufen dort nach Detail/HSL/
//! Kalibrierung, vor dem finalen Zuschnitt).
//!
//! **Bewusste Vereinfachungen** (siehe `DECISIONS.md` ADR-0028 und
//! -0030):
//! - **Ausgabegröße bleibt unverändert** (kein Zuschneiden auf den
//!   gültigen Bildbereich): außerhalb der Originaldaten liegende
//!   Quellpositionen werden randgeklemmt (`sample_at`) statt schwarz
//!   gefüllt oder automatisch zugeschnitten — echtes Zuschneiden ist
//!   Aufgabe von Schritt 11s separatem Geometrie-Werkzeug. Deshalb
//!   braucht dieser Schritt NICHT die für Schritt 11 vorgesehene
//!   größenverändernde Dispatch-Form — Ein-/Ausgabepuffer bleiben
//!   gleich groß, nur mit bilinearer statt nächstgelegener Abtastung.
//! - **Verzeichnung:** einfaches Ein-Koeffizienten-Radialmodell
//!   (`r_quelle = r_ziel · (1 + k1·r_ziel²)`, normiert auf die halbe
//!   Bildbreite/-höhe) statt eines echten mehrparametrigen
//!   Brown-Conrady-Modells.
//! - **Perspektive (vertikal/horizontal):** einfache Scherung statt
//!   einer echten projektiven Transformation (Vier-Punkt-Homografie) —
//!   für moderate Korrekturwinkel optisch ausreichend nah.
//! - **Kameraprofil-CA/-Vignette/-Verzeichnung:** additiv mit den
//!   manuellen Reglern kombiniert (Profilwert + Reglerwert), kein
//!   eigenständiges „Profil ersetzt Regler"-Modell.
//! - **Automatische CA-Erkennung** (`auto_ca`): kein echtes
//!   Kantenerkennungs-Verfahren — nutzt lediglich die CA-Werte des
//!   zugeordneten Profils (oder `0`, falls keins zugeordnet ist).
//! - **Perspektive/Upright „Auto"/„Level"/„Vertical"/„Full":** echte
//!   automatische Linien-/Kantenerkennung ist eine CV-Aufgabe
//!   vergleichbar mit der in ADR-0028 bereits zurückgestellten
//!   Auto-Ausrichtung (Schritt 11) — diese vier Modi sind daher
//!   dokumentierte No-op-Platzhalter (wählbar, ohne aktuelle Wirkung).
//!   **„Guided":** die ersten zwei markierten Hilfslinien
//!   (`guided_lines`, auf 2 statt bis zu 4 Paare vereinfacht, siehe
//!   ADR-0028) werden gemittelt und als einfache Dreh-Korrektur (nicht
//!   als echte Mehrlinien-Fluchtpunkt-Homografie) in `manual_transform`s
//!   Drehwinkel eingerechnet.

use bytemuck::{Pod, Zeroable};
use rayon::prelude::*;

use crate::edl::v2::{GuidedLine, LensCorrectionAdjustment, UprightMode};
use crate::error::Result;
use crate::gpu::{dispatch, GpuContext};
use crate::lens_profiles;

const SHADER: &str = include_str!("lens_corrections.wgsl");

/// Maximale relative Verschiebung bei `offset_x`/`offset_y = 100`.
const OFFSET_STRENGTH: f32 = 0.4;
/// Maximale relative Streckung bei `aspect = 100`.
const ASPECT_STRENGTH: f32 = 0.3;
/// Scherungsstärke bei `vertical`/`horizontal = 100`.
const SHEAR_STRENGTH: f32 = 0.5;
/// Skaliert `distortion_amount` (−100..100) auf den Verzeichnungs-
/// Koeffizienten `k1`.
const MANUAL_K1_SCALE: f32 = 0.3;
/// Maximale relative Radiusverschiebung je Farbkanal bei CA-Reglern von
/// `±100`.
const CA_STRENGTH: f32 = 0.02;
/// Vignette-Korrekturstärke: bei `vignette_amount = 100` wird der
/// Bildrand (`r_ziel² ≈ 1`) um `VIGNETTE_STRENGTH · 100 %` aufgehellt.
const VIGNETTE_STRENGTH: f32 = 0.01;

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct LensCorrectionParams {
    width: u32,
    height: u32,
    distortion_k1: f32,
    vignette_amount: f32,
    ca_red_cyan: f32,
    ca_blue_yellow: f32,
    rotate_degrees: f32,
    vertical: f32,
    horizontal: f32,
    aspect: f32,
    scale: f32,
    offset_x: f32,
    offset_y: f32,
    _pad: [f32; 3],
}

/// Mittelt die Neigung der ersten zwei Hilfslinien (siehe Moduldoku) zu
/// einer einzigen Dreh-Korrektur in Grad. Weniger als zwei Linien → `0.0`
/// (keine Korrektur möglich).
fn guided_rotation_degrees(lines: &[GuidedLine]) -> f32 {
    if lines.len() < 2 {
        return 0.0;
    }
    let line_angle_degrees =
        |line: &GuidedLine| -> f32 { (line.y2 - line.y1).atan2(line.x2 - line.x1).to_degrees() };
    let average = (line_angle_degrees(&lines[0]) + line_angle_degrees(&lines[1])) / 2.0;
    -average
}

impl LensCorrectionParams {
    pub fn new(width: u32, height: u32, adjustment: &LensCorrectionAdjustment) -> Self {
        let profile = adjustment
            .profile_id
            .as_deref()
            .and_then(lens_profiles::find_profile);

        let profile_k1 = profile.as_ref().map_or(0.0, |p| p.distortion_k1);
        let profile_vignette = profile.as_ref().map_or(0.0, |p| p.vignette_amount);
        let (ca_red_cyan, ca_blue_yellow) = if adjustment.auto_ca {
            profile
                .as_ref()
                .map_or((0.0, 0.0), |p| (p.ca_red_cyan, p.ca_blue_yellow))
        } else {
            (adjustment.ca_red_cyan, adjustment.ca_blue_yellow)
        };

        let guided_contribution = if adjustment.upright_mode == UprightMode::Guided {
            guided_rotation_degrees(&adjustment.guided_lines)
        } else {
            // Off/Auto/Level/Vertical/Full: dokumentierte No-op-Platzhalter,
            // siehe Moduldoku.
            0.0
        };

        Self {
            width,
            height,
            distortion_k1: profile_k1 + (adjustment.distortion_amount / 100.0) * MANUAL_K1_SCALE,
            vignette_amount: profile_vignette + adjustment.vignette_amount,
            ca_red_cyan,
            ca_blue_yellow,
            rotate_degrees: adjustment.manual_transform.rotate_degrees + guided_contribution,
            vertical: adjustment.manual_transform.vertical,
            horizontal: adjustment.manual_transform.horizontal,
            aspect: adjustment.manual_transform.aspect,
            scale: adjustment.manual_transform.scale,
            offset_x: adjustment.manual_transform.offset_x,
            offset_y: adjustment.manual_transform.offset_y,
            _pad: [0.0; 3],
        }
    }

    /// Ob diese Parameter (nach Auflösung von Profil/Guided-Linien) eine
    /// reine Identitätsabbildung ergeben — Grundlage für `develop.rs`s
    /// „kein zusätzlicher Durchlauf"-Optimierung.
    pub fn is_identity(&self) -> bool {
        self.distortion_k1 == 0.0
            && self.vignette_amount == 0.0
            && self.ca_red_cyan == 0.0
            && self.ca_blue_yellow == 0.0
            && self.rotate_degrees == 0.0
            && self.vertical == 0.0
            && self.horizontal == 0.0
            && self.aspect == 0.0
            && self.scale == 100.0
            && self.offset_x == 0.0
            && self.offset_y == 0.0
    }
}

fn sample_at(pixels: &[f32], width: usize, height: usize, x: i32, y: i32, channel: usize) -> f32 {
    let cx = x.clamp(0, width as i32 - 1) as usize;
    let cy = y.clamp(0, height as i32 - 1) as usize;
    pixels[(cy * width + cx) * 3 + channel]
}

/// Bilineare Abtastung an einer gebrochenzahligen Quellposition (in
/// Pixelkoordinaten) — Randpixel werden geklemmt statt schwarz gefüllt
/// (siehe Moduldoku).
fn bilinear_sample(
    pixels: &[f32],
    width: usize,
    height: usize,
    x: f32,
    y: f32,
    channel: usize,
) -> f32 {
    let x0 = x.floor();
    let y0 = y.floor();
    let fx = x - x0;
    let fy = y - y0;
    let x0i = x0 as i32;
    let y0i = y0 as i32;
    let v00 = sample_at(pixels, width, height, x0i, y0i, channel);
    let v10 = sample_at(pixels, width, height, x0i + 1, y0i, channel);
    let v01 = sample_at(pixels, width, height, x0i, y0i + 1, channel);
    let v11 = sample_at(pixels, width, height, x0i + 1, y0i + 1, channel);
    let top = v00 + (v10 - v00) * fx;
    let bottom = v01 + (v11 - v01) * fx;
    top + (bottom - top) * fy
}

/// Rechnet eine Ziel-Normalkoordinate (zentriert, `±1` an den Rändern) in
/// die vor der Verzeichnung liegende Quell-Normalkoordinate um — die
/// Umkehrung von `manual_transform` (siehe Moduldoku für die einzelnen
/// Terme).
fn undo_manual_transform(nx: f32, ny: f32, params: &LensCorrectionParams) -> (f32, f32) {
    let x = nx - (params.offset_x / 100.0) * OFFSET_STRENGTH;
    let y = ny - (params.offset_y / 100.0) * OFFSET_STRENGTH;

    let scale_factor = (params.scale / 100.0).max(0.01);
    let x = x / scale_factor;
    let y = y / scale_factor;

    let aspect_factor_x = 1.0 + (params.aspect / 100.0) * ASPECT_STRENGTH;
    let aspect_factor_y = 1.0 - (params.aspect / 100.0) * ASPECT_STRENGTH;
    let x = x / aspect_factor_x;
    let y = y / aspect_factor_y;

    let angle = -params.rotate_degrees.to_radians();
    let (sin_a, cos_a) = angle.sin_cos();
    let rx = x * cos_a - y * sin_a;
    let ry = x * sin_a + y * cos_a;

    let sheared_x = rx - (params.horizontal / 100.0) * SHEAR_STRENGTH * ry;
    let sheared_y = ry - (params.vertical / 100.0) * SHEAR_STRENGTH * rx;

    (sheared_x, sheared_y)
}

fn apply_distortion(x: f32, y: f32, k1: f32) -> (f32, f32) {
    let factor = 1.0 + k1 * (x * x + y * y);
    (x * factor, y * factor)
}

fn process_pixel(
    pixels: &[f32],
    width: usize,
    height: usize,
    px: usize,
    py: usize,
    params: &LensCorrectionParams,
) -> (f32, f32, f32) {
    let half_w = width as f32 / 2.0;
    let half_h = height as f32 / 2.0;
    // Bewusst ohne "+0.5"-Pixelmitten-Versatz: `to_pixel` unten (und
    // `bilinear_sample`/`sample_at`) arbeiten in reinen Pixelindex-
    // Koordinaten (Index 0 = erste Spalte/Zeile), nicht in
    // Pixelmitten-Koordinaten — sonst würde selbst die Identitätsabbildung
    // (kein Regler verändert) durch einen `fx = 0.5`-Bruch am `floor()`
    // eine 50/50-Verwaschung mit dem Nachbarpixel erzeugen.
    let nx = (px as f32 - half_w) / half_w;
    let ny = (py as f32 - half_h) / half_h;

    let (ux, uy) = undo_manual_transform(nx, ny, params);
    let (dx, dy) = apply_distortion(ux, uy, params.distortion_k1);

    let ca_r = 1.0 + CA_STRENGTH * (params.ca_red_cyan / 100.0);
    let ca_b = 1.0 + CA_STRENGTH * (params.ca_blue_yellow / 100.0);
    let to_pixel = |x: f32, y: f32| -> (f32, f32) { (half_w + x * half_w, half_h + y * half_h) };

    let (src_rx, src_ry) = to_pixel(dx * ca_r, dy * ca_r);
    let (src_gx, src_gy) = to_pixel(dx, dy);
    let (src_bx, src_by) = to_pixel(dx * ca_b, dy * ca_b);

    let r = bilinear_sample(pixels, width, height, src_rx, src_ry, 0);
    let g = bilinear_sample(pixels, width, height, src_gx, src_gy, 1);
    let b = bilinear_sample(pixels, width, height, src_bx, src_by, 2);

    let vignette_factor =
        1.0 + (params.vignette_amount / 100.0) * VIGNETTE_STRENGTH * 100.0 * (nx * nx + ny * ny);

    (
        (r * vignette_factor).clamp(0.0, 1.0),
        (g * vignette_factor).clamp(0.0, 1.0),
        (b * vignette_factor).clamp(0.0, 1.0),
    )
}

/// CPU-Fallback — dieselbe Formel wie `lens_corrections.wgsl`.
pub fn apply_cpu(
    pixels: &[f32],
    width: u32,
    height: u32,
    adjustment: &LensCorrectionAdjustment,
) -> Vec<f32> {
    let params = LensCorrectionParams::new(width, height, adjustment);
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
    adjustment: &LensCorrectionAdjustment,
) -> Result<Vec<f32>> {
    let params = LensCorrectionParams::new(width, height, adjustment);
    dispatch::run_compute_f32(ctx, "lens_corrections", SHADER, "main", params, pixels, 64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::edl::v2::ManualTransform;

    /// Baut ein `size`×`size`-Testbild: grau überall, mit einer klar
    /// unterscheidbaren Farbmarkierung an einer bestimmten Pixelposition
    /// — für Geometrie-Tests, die eine erkennbare Verschiebung brauchen.
    fn marked_gray_image(size: usize, mark_x: usize, mark_y: usize, mark: [f32; 3]) -> Vec<f32> {
        let mut pixels = vec![0.5; size * size * 3];
        let idx = (mark_y * size + mark_x) * 3;
        pixels[idx] = mark[0];
        pixels[idx + 1] = mark[1];
        pixels[idx + 2] = mark[2];
        pixels
    }

    #[test]
    fn neutral_is_identity_on_cpu() {
        let pixels = marked_gray_image(9, 4, 4, [0.9, 0.1, 0.1]);
        let result = apply_cpu(&pixels, 9, 9, &LensCorrectionAdjustment::NEUTRAL);
        for (input, output) in pixels.iter().zip(result.iter()) {
            assert!(
                (input - output).abs() < 1e-4,
                "input={input} output={output}"
            );
        }
    }

    #[test]
    fn is_identity_detects_neutral_resolved_params() {
        let params = LensCorrectionParams::new(9, 9, &LensCorrectionAdjustment::NEUTRAL);
        assert!(params.is_identity());
    }

    #[test]
    fn positive_scale_zooms_in_moving_edge_content_toward_center() {
        // Eine Markierung nahe der linken Kante wandert bei einem
        // Hinein-Zoom (`scale > 100`) näher an den Bildrand heran, weil
        // derselbe Bildausschnitt jetzt größer dargestellt wird — hier
        // geprüft, indem der Wert an der ursprünglichen Randposition nach
        // dem Zoom heller (näher am grauen Hintergrund) wird, weil die
        // Markierung dorthin verschoben wurde.
        let pixels = marked_gray_image(21, 2, 10, [0.9, 0.1, 0.1]);
        let adjustment = LensCorrectionAdjustment {
            manual_transform: ManualTransform {
                scale: 150.0,
                ..ManualTransform::NEUTRAL
            },
            ..LensCorrectionAdjustment::NEUTRAL
        };
        let result = apply_cpu(&pixels, 21, 21, &adjustment);
        let idx = (10 * 21 + 2) * 3;
        assert!(
            result[idx] < pixels[idx],
            "Nach dem Hinein-Zoom sollte die Markierung nicht mehr an ihrer alten Position liegen (vorher={} nachher={})",
            pixels[idx],
            result[idx]
        );
    }

    #[test]
    fn positive_offset_x_shifts_content_to_the_right() {
        let size = 21;
        let pixels = marked_gray_image(size, 10, 10, [0.9, 0.1, 0.1]);
        let adjustment = LensCorrectionAdjustment {
            manual_transform: ManualTransform {
                offset_x: 50.0,
                ..ManualTransform::NEUTRAL
            },
            ..LensCorrectionAdjustment::NEUTRAL
        };
        let result = apply_cpu(&pixels, size as u32, size as u32, &adjustment);
        // Sucht die Spalte mit dem höchsten Rot-Wert in Zeile 10 — die
        // verschobene Markierung landet dort (bei bilinearer Abtastung
        // i. A. nicht exakt auf einer ganzzahligen Spalte, siehe
        // `undo_manual_transform`s Nachbildung, deshalb kein Vergleich
        // fester Spalten).
        let row = 10;
        let mut best_col = 0;
        let mut best_value = -1.0;
        for col in 0..size {
            let value = result[(row * size + col) * 3];
            if value > best_value {
                best_value = value;
                best_col = col;
            }
        }
        assert!(
            best_col > 10,
            "Positiver X-Versatz sollte die Markierung nach rechts verschieben (war bei Spalte {best_col})"
        );
    }

    #[test]
    fn rotate_degrees_moves_a_marked_pixel_away_from_its_original_position() {
        let pixels = marked_gray_image(21, 10, 3, [0.9, 0.1, 0.1]);
        let adjustment = LensCorrectionAdjustment {
            manual_transform: ManualTransform {
                rotate_degrees: 20.0,
                ..ManualTransform::NEUTRAL
            },
            ..LensCorrectionAdjustment::NEUTRAL
        };
        let result = apply_cpu(&pixels, 21, 21, &adjustment);
        let idx = (3 * 21 + 10) * 3;
        assert!(
            (result[idx] - pixels[idx]).abs() > 1e-3,
            "Drehung sollte den Wert an der ursprünglichen Markierungsposition verändern"
        );
    }

    #[test]
    fn guided_lines_derive_a_rotation_from_their_average_tilt() {
        let lines = vec![
            GuidedLine {
                x1: 0.1,
                y1: 0.5,
                x2: 0.9,
                y2: 0.55,
            },
            GuidedLine {
                x1: 0.1,
                y1: 0.6,
                x2: 0.9,
                y2: 0.65,
            },
        ];
        let degrees = guided_rotation_degrees(&lines);
        assert!(degrees < 0.0, "leicht nach unten geneigte Linien sollten eine negative Korrektur ergeben, war {degrees}");
    }

    #[test]
    fn guided_mode_with_fewer_than_two_lines_contributes_no_rotation() {
        let adjustment = LensCorrectionAdjustment {
            upright_mode: UprightMode::Guided,
            guided_lines: vec![GuidedLine {
                x1: 0.0,
                y1: 0.0,
                x2: 1.0,
                y2: 0.2,
            }],
            ..LensCorrectionAdjustment::NEUTRAL
        };
        let params = LensCorrectionParams::new(9, 9, &adjustment);
        assert_eq!(params.rotate_degrees, 0.0);
    }

    #[test]
    fn positive_vignette_amount_brightens_a_dark_corner_pixel() {
        let mut pixels = vec![0.3; 21 * 21 * 3];
        // Ecke abdunkeln, wie ein natürliches Vignettieren es tun würde.
        let corner_idx = 0;
        pixels[corner_idx] = 0.1;
        pixels[corner_idx + 1] = 0.1;
        pixels[corner_idx + 2] = 0.1;
        let adjustment = LensCorrectionAdjustment {
            vignette_amount: 80.0,
            ..LensCorrectionAdjustment::NEUTRAL
        };
        let result = apply_cpu(&pixels, 21, 21, &adjustment);
        assert!(
            result[corner_idx] > pixels[corner_idx],
            "Vignette-Korrektur sollte die dunkle Ecke aufhellen (vorher={} nachher={})",
            pixels[corner_idx],
            result[corner_idx]
        );
    }

    #[test]
    fn manual_ca_shifts_red_and_blue_channels_differently_at_the_edge() {
        // Ein scharfer Hell/Dunkel-Übergang nahe dem Bildrand — bei
        // aktivierter CA-Korrektur sollten Rot und Blau unterschiedlich
        // stark verschoben abgetastet werden als Grün.
        let size = 21;
        let mut pixels = vec![0.2; size * size * 3];
        for y in 0..size {
            for x in 10..size {
                let idx = (y * size + x) * 3;
                pixels[idx] = 0.9;
                pixels[idx + 1] = 0.9;
                pixels[idx + 2] = 0.9;
            }
        }
        let neutral = apply_cpu(
            &pixels,
            size as u32,
            size as u32,
            &LensCorrectionAdjustment::NEUTRAL,
        );
        let adjustment = LensCorrectionAdjustment {
            ca_red_cyan: 100.0,
            ca_blue_yellow: -100.0,
            ..LensCorrectionAdjustment::NEUTRAL
        };
        let result = apply_cpu(&pixels, size as u32, size as u32, &adjustment);
        // Direkt an der Kante (x=10, mittlere Höhe) sollte sich Rot- und
        // Blau-Kanal unterschiedlich stark ändern als ohne CA-Korrektur.
        let idx = (10 * size + 10) * 3;
        let red_change = (result[idx] - neutral[idx]).abs();
        let blue_change = (result[idx + 2] - neutral[idx + 2]).abs();
        assert!(
            red_change > 1e-4 || blue_change > 1e-4,
            "CA-Korrektur sollte Rot- oder Blau-Kanal an einer Kante sichtbar verschieben (red={red_change} blue={blue_change})"
        );
    }

    #[test]
    fn distortion_moves_content_relative_to_the_edge() {
        let pixels = marked_gray_image(21, 18, 10, [0.9, 0.1, 0.1]);
        let adjustment = LensCorrectionAdjustment {
            distortion_amount: 100.0,
            ..LensCorrectionAdjustment::NEUTRAL
        };
        let result = apply_cpu(&pixels, 21, 21, &adjustment);
        let idx = (10 * 21 + 18) * 3;
        assert!(
            (result[idx] - pixels[idx]).abs() > 1e-3,
            "Verzeichnungs-Korrektur sollte randnahe Inhalte sichtbar verschieben"
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
        let adjustment = LensCorrectionAdjustment {
            ca_red_cyan: 40.0,
            ca_blue_yellow: -30.0,
            auto_ca: false,
            vignette_amount: 25.0,
            distortion_amount: -50.0,
            upright_mode: UprightMode::Guided,
            guided_lines: vec![
                GuidedLine {
                    x1: 0.1,
                    y1: 0.4,
                    x2: 0.9,
                    y2: 0.5,
                },
                GuidedLine {
                    x1: 0.1,
                    y1: 0.5,
                    x2: 0.9,
                    y2: 0.6,
                },
            ],
            manual_transform: ManualTransform {
                vertical: 10.0,
                horizontal: -10.0,
                rotate_degrees: 5.0,
                aspect: 15.0,
                scale: 110.0,
                offset_x: 5.0,
                offset_y: -5.0,
            },
            profile_id: None,
        };
        let pixels = marked_gray_image(24, 12, 12, [0.8, 0.4, 0.2]);
        let cpu = apply_cpu(&pixels, 24, 24, &adjustment);
        let gpu =
            apply_gpu(&ctx, &pixels, 24, 24, &adjustment).expect("GPU-Ausführung sollte gelingen");
        for (c, g) in cpu.iter().zip(gpu.iter()) {
            assert!((c - g).abs() < 2e-3, "CPU={c} GPU={g}");
        }
    }
}
