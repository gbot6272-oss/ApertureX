//! Reparatur (Klonen/Reparieren) — `SPEC.md` §3.2 „Reparatur". Läuft als
//! allererster Schritt in `develop::render_rgba8`, auf den linearen
//! Kamera-RGB-Pixeln, noch vor Kalibrierung — Flecken-/Staub-Entfernung
//! soll konzeptionell auf den unveränderten Sensordaten passieren, bevor
//! Ton-/Farbwerkzeuge darauf aufbauen (dieselbe Überlegung wie
//! [`super::calibration`]s Modulplatzierung).
//!
//! Jeder [`RepairStroke`] wird als eigener, sequenzieller Durchlauf über
//! das Bild angewendet (nicht als ein gemeinsamer Fused-Pass wie die
//! übrigen linearen Werkzeuge) — Striche haben unterschiedlich lange
//! Pfade, ein gemeinsamer GPU-Uniform-Puffer für „alle Striche auf
//! einmal" bräuchte eine feste Gesamtstrichzahl. Die sequenzielle
//! Anwendung erlaubt beliebig viele Striche, nur die Punktzahl je
//! einzelnem Strich ist auf [`MAX_PATH_POINTS`] gedeckelt (das Frontend
//! dünnt gemalte Pfade entsprechend aus).
//!
//! **Bewusste Vereinfachungen** (siehe `DECISIONS.md` ADR-0028 — Auto-
//! Quellenfindung und inhaltsbasiertes Füllen sind explizit NICHT Teil
//! dieses Schritts):
//! - **Klonen:** direkter, um einen festen Versatz verschobener
//!   Lesezugriff (bilinear abgetastet), radial weichgezeichnet über
//!   `feather` am Rand von `radius` — kein Umgebungsabgleich.
//! - **Reparieren:** vereinfachtes Tiefpass/Hochpass-Überblenden
//!   (Tiefpass-Anteil von der Quelle, Hochpass-Anteil — lokale
//!   Textur/Rauschen — vom Ziel) statt eines echten Poisson-Blendings
//!   mit Gradientenfeld-Löser.
//! - **Pfad-Abstand:** der Abstand eines Pixels zum gemalten Pfad wird
//!   als minimaler Abstand zum nächsten Stützpunkt angenähert (kein
//!   echtes Punkt-zu-Liniensegment-Maß) — bei dicht abgetasteten Pfaden
//!   (Frontend sampelt bei jeder Zeigerbewegung) visuell nicht von einer
//!   echten Polylinien-Distanz zu unterscheiden.
//! - **Versatz:** ein einziger fester Versatz (`source - target_path[0]`)
//!   gilt für den gesamten Strich (wie ein klassisches Stempel-Werkzeug),
//!   kein perspektivisch/skaliert angepasster Versatz.

use bytemuck::{Pod, Zeroable};

use crate::edl::v2::{RepairMode, RepairPoint, RepairStroke};
use crate::error::Result;
use crate::gpu::{dispatch, GpuContext};

const SHADER: &str = include_str!("repair.wgsl");

/// Deckelt die Anzahl der Stützpunkte je einzelnem Strich (siehe
/// Moduldoku) — das Frontend dünnt längere gemalte Pfade entsprechend
/// aus. Beliebig viele Striche bleiben möglich, da jeder sequenziell als
/// eigener Durchlauf angewendet wird.
pub const MAX_PATH_POINTS: usize = 32;

/// Feste Referenzgröße für `radius`/`feather` — beide sind Bruchteile der
/// Bildbreite (wie `CropRect`/`GuidedLine`s normierte 0..1-Koordinaten),
/// nicht absolute Pixelwerte, damit ein Strich unabhängig von der
/// tatsächlichen Bildauflösung dieselbe relative Größe behält.
fn to_pixels(fraction: f32, width: u32) -> f32 {
    fraction * width as f32
}

/// Ein Stützpunkt im `path`-Array von [`RepairParams`], auf 16 Byte
/// aufgefüllt: WGSL verlangt für Arrays im `uniform`-Adressraum eine auf
/// 16 Byte ausgerichtete Element-Schrittweite (dieselbe Konvention wie
/// `hsl_color_mixer.rs`s `BandParams`/`RegionParams`) — ein rohes
/// `[f32; 2]` (8 Byte) würde von naga stillschweigend auf 16 Byte
/// aufgefüllt, was die Rust- und WGSL-Seite auseinanderlaufen ließe.
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
struct PathPoint {
    x: f32,
    y: f32,
    _pad: [f32; 2],
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct RepairParams {
    width: u32,
    height: u32,
    /// `0.0` = Klonen, `1.0` = Reparieren.
    mode: f32,
    radius: f32,
    feather: f32,
    opacity: f32,
    offset_x: f32,
    offset_y: f32,
    point_count: u32,
    _pad: [f32; 3],
    /// Pixel-Koordinaten der Stützpunkte des Zielpfads, aufgefüllt mit
    /// Nullen über `point_count` hinaus (wird im Shader nicht gelesen).
    path: [PathPoint; MAX_PATH_POINTS],
}

impl RepairParams {
    pub fn new(width: u32, height: u32, stroke: &RepairStroke) -> Self {
        let first = stroke
            .target_path
            .first()
            .copied()
            .unwrap_or(RepairPoint { x: 0.0, y: 0.0 });
        let offset_x = to_pixels(first.x - stroke.source.x, width);
        let offset_y = to_pixels(first.y - stroke.source.y, height);

        let mut path = [PathPoint {
            x: 0.0,
            y: 0.0,
            _pad: [0.0; 2],
        }; MAX_PATH_POINTS];
        let point_count = stroke.target_path.len().min(MAX_PATH_POINTS);
        for (slot, point) in path
            .iter_mut()
            .zip(stroke.target_path.iter())
            .take(point_count)
        {
            *slot = PathPoint {
                x: to_pixels(point.x, width),
                y: to_pixels(point.y, height),
                _pad: [0.0; 2],
            };
        }

        Self {
            width,
            height,
            mode: match stroke.mode {
                RepairMode::Clone => 0.0,
                RepairMode::Heal => 1.0,
            },
            radius: to_pixels(stroke.radius, width),
            feather: to_pixels(stroke.feather, width),
            opacity: stroke.opacity,
            offset_x,
            offset_y,
            point_count: point_count as u32,
            _pad: [0.0; 3],
            path,
        }
    }
}

fn sample_at(pixels: &[f32], width: usize, height: usize, x: i32, y: i32, channel: usize) -> f32 {
    let cx = x.clamp(0, width as i32 - 1) as usize;
    let cy = y.clamp(0, height as i32 - 1) as usize;
    pixels[(cy * width + cx) * 3 + channel]
}

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

fn box_blur3(pixels: &[f32], width: usize, height: usize, x: i32, y: i32, channel: usize) -> f32 {
    let mut sum = 0.0;
    for dy in -1..=1 {
        for dx in -1..=1 {
            sum += sample_at(pixels, width, height, x + dx, y + dy, channel);
        }
    }
    sum / 9.0
}

fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    if edge1 - edge0 < 1e-6 {
        return if x >= edge1 { 1.0 } else { 0.0 };
    }
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn distance_to_path(px: f32, py: f32, params: &RepairParams) -> f32 {
    let mut min_dist = f32::MAX;
    for point in params.path.iter().take(params.point_count as usize) {
        let dx = px - point.x;
        let dy = py - point.y;
        let dist = (dx * dx + dy * dy).sqrt();
        if dist < min_dist {
            min_dist = dist;
        }
    }
    min_dist
}

fn process_pixel(
    pixels: &[f32],
    width: usize,
    height: usize,
    px: usize,
    py: usize,
    params: &RepairParams,
) -> (f32, f32, f32) {
    let x = px as f32;
    let y = py as f32;
    let dist = distance_to_path(x, y, params);
    let weight =
        (1.0 - smoothstep(
            params.radius,
            params.radius + params.feather.max(1e-3),
            dist,
        )) * params.opacity;

    let idx = (py * width + px) * 3;
    let original = [pixels[idx], pixels[idx + 1], pixels[idx + 2]];
    if weight <= 0.0 {
        return (original[0], original[1], original[2]);
    }

    let src_x = x - params.offset_x;
    let src_y = y - params.offset_y;

    let mut result = [0.0f32; 3];
    for c in 0..3 {
        let cloned = bilinear_sample(pixels, width, height, src_x, src_y, c);
        let value = if params.mode > 0.5 {
            // Reparieren: Tiefpass von der Quelle, Hochpass vom Ziel.
            let source_low = box_blur3(
                pixels,
                width,
                height,
                src_x.round() as i32,
                src_y.round() as i32,
                c,
            );
            let target_high =
                original[c] - box_blur3(pixels, width, height, px as i32, py as i32, c);
            source_low + target_high
        } else {
            cloned
        };
        result[c] = (original[c] + (value - original[c]) * weight).clamp(0.0, 1.0);
    }
    (result[0], result[1], result[2])
}

/// CPU-Fallback für einen einzelnen Strich — dieselbe Formel wie
/// `repair.wgsl`.
fn apply_stroke_cpu(pixels: &[f32], width: u32, height: u32, stroke: &RepairStroke) -> Vec<f32> {
    let params = RepairParams::new(width, height, stroke);
    let w = width as usize;
    let h = height as usize;
    let mut out = vec![0.0f32; pixels.len()];
    for py in 0..h {
        for px in 0..w {
            let (r, g, b) = process_pixel(pixels, w, h, px, py, &params);
            let idx = (py * w + px) * 3;
            out[idx] = r;
            out[idx + 1] = g;
            out[idx + 2] = b;
        }
    }
    out
}

fn apply_stroke_gpu(
    ctx: &GpuContext,
    pixels: &[f32],
    width: u32,
    height: u32,
    stroke: &RepairStroke,
) -> Result<Vec<f32>> {
    let params = RepairParams::new(width, height, stroke);
    dispatch::run_compute_f32(ctx, "repair", SHADER, "main", params, pixels, 64)
}

/// Wendet alle Striche aus `strokes` nacheinander an (siehe Moduldoku) —
/// die einzige Funktion, die `develop::render_rgba8` aus diesem Modul
/// aufruft.
pub fn apply_cpu(pixels: &[f32], width: u32, height: u32, strokes: &[RepairStroke]) -> Vec<f32> {
    let mut current = pixels.to_vec();
    for stroke in strokes {
        current = apply_stroke_cpu(&current, width, height, stroke);
    }
    current
}

pub fn apply_gpu(
    ctx: &GpuContext,
    pixels: &[f32],
    width: u32,
    height: u32,
    strokes: &[RepairStroke],
) -> Result<Vec<f32>> {
    let mut current = pixels.to_vec();
    for stroke in strokes {
        current = apply_stroke_gpu(ctx, &current, width, height, stroke)?;
    }
    Ok(current)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flat_gray(width: u32, height: u32, value: f32) -> Vec<f32> {
        vec![value; (width * height * 3) as usize]
    }

    fn point(x: f32, y: f32) -> RepairPoint {
        RepairPoint { x, y }
    }

    #[test]
    fn empty_stroke_list_is_identity() {
        let pixels = flat_gray(10, 10, 0.5);
        let result = apply_cpu(&pixels, 10, 10, &[]);
        assert_eq!(result, pixels);
    }

    #[test]
    fn clone_copies_a_bright_spot_to_the_target() {
        let size = 20;
        let mut pixels = flat_gray(size, size, 0.2);
        // Helle Markierung an der Quellposition (0.25, 0.25).
        let source_px = (0.25 * size as f32) as usize;
        let idx = (source_px * size as usize + source_px) * 3;
        pixels[idx] = 0.9;
        pixels[idx + 1] = 0.9;
        pixels[idx + 2] = 0.9;

        let stroke = RepairStroke {
            mode: RepairMode::Clone,
            source: point(0.25, 0.25),
            target_path: vec![point(0.75, 0.75)],
            radius: 0.05,
            feather: 0.01,
            opacity: 1.0,
        };
        let result = apply_cpu(&pixels, size, size, std::slice::from_ref(&stroke));
        let target_px = (0.75 * size as f32) as usize;
        let target_idx = (target_px * size as usize + target_px) * 3;
        assert!(
            result[target_idx] > pixels[target_idx],
            "Klonen sollte die helle Markierung an die Zielposition übertragen (vorher={} nachher={})",
            pixels[target_idx],
            result[target_idx]
        );
    }

    #[test]
    fn zero_opacity_leaves_the_image_unchanged() {
        let size = 20;
        let mut pixels = flat_gray(size, size, 0.2);
        let idx = (5 * size as usize + 5) * 3;
        pixels[idx] = 0.9;
        let stroke = RepairStroke {
            mode: RepairMode::Clone,
            source: point(0.25, 0.25),
            target_path: vec![point(0.75, 0.75)],
            radius: 0.1,
            feather: 0.02,
            opacity: 0.0,
        };
        let result = apply_cpu(&pixels, size, size, std::slice::from_ref(&stroke));
        assert_eq!(result, pixels);
    }

    #[test]
    fn pixels_far_from_the_path_are_unaffected() {
        let size = 30;
        let mut pixels = flat_gray(size, size, 0.2);
        let idx = (5 * size as usize + 5) * 3;
        pixels[idx] = 0.9;
        let stroke = RepairStroke {
            mode: RepairMode::Clone,
            source: point(0.1, 0.1),
            target_path: vec![point(0.9, 0.9)],
            radius: 0.05,
            feather: 0.01,
            opacity: 1.0,
        };
        let result = apply_cpu(&pixels, size, size, std::slice::from_ref(&stroke));
        // Eine weit vom Pfad entfernte Ecke sollte unverändert bleiben.
        let far_idx = 0;
        assert!((result[far_idx] - pixels[far_idx]).abs() < 1e-6);
    }

    #[test]
    fn heal_mode_reduces_a_sharp_target_edge_while_keeping_it_distinct_from_pure_clone() {
        let size = 24;
        // Ziel: scharfer Hell/Dunkel-Übergang. Quelle: gleichmäßiges Grau.
        let mut pixels = flat_gray(size, size, 0.5);
        for y in 0..size as usize {
            for x in 12..size as usize {
                let idx = (y * size as usize + x) * 3;
                pixels[idx] = 0.9;
                pixels[idx + 1] = 0.9;
                pixels[idx + 2] = 0.9;
            }
        }
        let stroke = RepairStroke {
            mode: RepairMode::Heal,
            source: point(0.1, 0.5),
            target_path: vec![point(0.5, 0.5)],
            radius: 0.15,
            feather: 0.02,
            opacity: 1.0,
        };
        let clone_stroke = RepairStroke {
            mode: RepairMode::Clone,
            ..stroke.clone()
        };
        let healed = apply_cpu(&pixels, size, size, std::slice::from_ref(&stroke));
        let cloned = apply_cpu(&pixels, size, size, std::slice::from_ref(&clone_stroke));
        let target_idx = ((size as usize / 2) * size as usize + size as usize / 2) * 3;
        assert!(
            (healed[target_idx] - cloned[target_idx]).abs() > 1e-4,
            "Reparieren sollte ein anderes Ergebnis liefern als reines Klonen"
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
        let size = 20;
        let pixels = crate::test_support::gray_gradient((size * size) as usize);
        let strokes = vec![
            RepairStroke {
                mode: RepairMode::Clone,
                source: point(0.1, 0.1),
                target_path: vec![point(0.6, 0.6), point(0.65, 0.62), point(0.7, 0.64)],
                radius: 0.08,
                feather: 0.03,
                opacity: 0.8,
            },
            RepairStroke {
                mode: RepairMode::Heal,
                source: point(0.2, 0.8),
                target_path: vec![point(0.4, 0.2)],
                radius: 0.06,
                feather: 0.02,
                opacity: 1.0,
            },
        ];
        let cpu = apply_cpu(&pixels, size, size, &strokes);
        let gpu =
            apply_gpu(&ctx, &pixels, size, size, &strokes).expect("GPU-Ausführung sollte gelingen");
        for (c, g) in cpu.iter().zip(gpu.iter()) {
            assert!((c - g).abs() < 2e-3, "CPU={c} GPU={g}");
        }
    }
}
