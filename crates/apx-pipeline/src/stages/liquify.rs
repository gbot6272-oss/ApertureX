//! Verflüssigen (Liquify, Phase 15 Schritt 3, siehe `DECISIONS.md`
//! ADR-0042 — Photoshop-exklusives Verformungswerkzeug, das Lightroom
//! nicht bietet). Läuft im fertig entwickelten sRGB-RGBA8-Bild, nach
//! `sky_replace`, vor `geometry` (siehe `develop::render_rgba8`s fester
//! Kette) — dieselbe Pipeline-Position wie jede andere Vollbild-Stufe
//! dieser Kette.
//!
//! Reine CPU-Rückwärts-Verzerrung (kein GPU-Shader wie bei
//! `stages::repair`s Klon-/Reparaturpinsel — hier reicht die einfachere
//! Variante, dasselbe Muster wie `stages::composite`/`stages::sky_replace`):
//! für jeden Ausgabepixel innerhalb `radius` eines Strichs wird eine
//! **Quell**-Koordinate berechnet (`warp_source`), von dort bilinear
//! gesampelt. Mehrere Striche werden sequenziell angewendet, jeder auf
//! das Ergebnis des vorigen (wie `RepairStroke`s).

use crate::edl::v4::{LiquifyMode, LiquifyStroke};

/// Feste Referenzgröße für `radius` — Bruchteil der Bildbreite, dieselbe
/// Konvention wie `stages::repair`s `to_pixels` (siehe dessen
/// Begründung: unabhängig von der tatsächlichen Auflösung dieselbe
/// relative Größe).
fn to_pixels(fraction: f32, width: u32) -> f32 {
    fraction * width as f32
}

fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    if edge1 <= edge0 {
        return if x < edge0 { 0.0 } else { 1.0 };
    }
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Nächster Punkt auf dem gemalten Pfad (Pixelkoordinaten) samt Distanz
/// — dieselbe „nächster Punkt auf der Polylinie"-Idee wie
/// `stages::repair`s `distance_to_path`, hier zusätzlich mit dem
/// gefundenen Punkt selbst (als Drehzentrum für Twirl/Pucker/Bloat).
fn nearest_on_path(px: f32, py: f32, path: &[(f32, f32)]) -> ((f32, f32), f32) {
    let mut best = path.first().copied().unwrap_or((px, py));
    let mut best_dist = f32::MAX;
    for &(x, y) in path {
        let dx = px - x;
        let dy = py - y;
        let dist = (dx * dx + dy * dy).sqrt();
        if dist < best_dist {
            best_dist = dist;
            best = (x, y);
        }
    }
    (best, best_dist)
}

/// Bilinear gesampelter Einzelkanal aus einer interleaved-RGBA8-Bitmap,
/// mit Rand-Klemmung — dieselbe Interpolation wie `stages::repair`s
/// `bilinear_sample`, hier RGBA statt RGB.
fn sample_channel(
    pixels: &[u8],
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
    let at = |xi: i32, yi: i32| -> f32 {
        let cx = xi.clamp(0, width as i32 - 1) as usize;
        let cy = yi.clamp(0, height as i32 - 1) as usize;
        pixels[(cy * width + cx) * 4 + channel] as f32
    };
    let x0i = x0 as i32;
    let y0i = y0 as i32;
    let top = at(x0i, y0i) + (at(x0i + 1, y0i) - at(x0i, y0i)) * fx;
    let bottom = at(x0i, y0i + 1) + (at(x0i + 1, y0i + 1) - at(x0i, y0i + 1)) * fx;
    top + (bottom - top) * fy
}

/// Maximale Auslenkung je Modus bei `strength = 1.0`, Pixel direkt am
/// Strichzentrum (`weight = 1.0`) — von Hand austarierte Werte für einen
/// deutlich sichtbaren, aber nicht bildzerstörenden Effekt.
const PUSH_SCALE: f32 = 0.8;
const TWIRL_MAX_RADIANS: f32 = 2.4;
const PUCKER_MAX: f32 = 0.9;
const BLOAT_MAX: f32 = 0.9;

/// Berechnet die Quellkoordinate für einen Ausgabepixel (Rückwärts-
/// Verzerrung) — `None`, wenn der Pixel außerhalb des Wirkradius liegt
/// (dann bleibt er von diesem Strich unverändert).
#[allow(clippy::too_many_arguments)]
fn warp_source(
    px: f32,
    py: f32,
    path: &[(f32, f32)],
    drag_dir: (f32, f32),
    radius_px: f32,
    strength: f32,
    mode: LiquifyMode,
) -> Option<(f32, f32)> {
    let (center, dist) = nearest_on_path(px, py, path);
    if dist > radius_px {
        return None;
    }
    // Am Strichzentrum voll wirksam, zum Rand hin weich auf null
    // (dieselbe `smoothstep`-Rampen-Idee wie `stages::composite`s
    // Blend-If, hier radial statt tonwertbasiert).
    let weight = 1.0 - smoothstep(0.0, radius_px, dist);
    let amount = weight * strength;
    Some(match mode {
        // Verschiebt entlang der Zugrichtung (erster → letzter
        // Pfadpunkt) — Rückwärts-Verzerrung heißt: die Quelle liegt
        // *entgegen* der Zugrichtung, damit der Bildinhalt am Ziel so
        // aussieht, als wäre er dorthin geschoben worden.
        LiquifyMode::Push => (
            px - drag_dir.0 * amount * radius_px * PUSH_SCALE,
            py - drag_dir.1 * amount * radius_px * PUSH_SCALE,
        ),
        // Rotiert um das nächstgelegene Pfadzentrum.
        LiquifyMode::Twirl => {
            let angle = -amount * TWIRL_MAX_RADIANS;
            let (sin, cos) = angle.sin_cos();
            let dx = px - center.0;
            let dy = py - center.1;
            (
                center.0 + dx * cos - dy * sin,
                center.1 + dx * sin + dy * cos,
            )
        }
        // Staucht (saugt Bildinhalt zum Zentrum): die Quelle liegt
        // weiter vom Zentrum entfernt als das Ziel.
        LiquifyMode::Pucker => {
            let k = 1.0 + amount * PUCKER_MAX;
            (
                center.0 + (px - center.0) * k,
                center.1 + (py - center.1) * k,
            )
        }
        // Bläht auf (drückt Bildinhalt vom Zentrum weg): die Quelle
        // liegt näher am Zentrum als das Ziel.
        LiquifyMode::Bloat => {
            let k = 1.0 / (1.0 + amount * BLOAT_MAX);
            (
                center.0 + (px - center.0) * k,
                center.1 + (py - center.1) * k,
            )
        }
    })
}

fn apply_stroke(base: &[u8], width: u32, height: u32, stroke: &LiquifyStroke) -> Vec<u8> {
    if stroke.center_path.is_empty() || stroke.radius <= 0.0 || stroke.strength <= 0.0 {
        return base.to_vec();
    }
    let w = width as usize;
    let h = height as usize;
    let path: Vec<(f32, f32)> = stroke
        .center_path
        .iter()
        .map(|p| (to_pixels(p.x, width), to_pixels(p.y, height)))
        .collect();
    let radius_px = to_pixels(stroke.radius, width).max(1.0);
    let strength = stroke.strength.clamp(0.0, 1.0);

    let first = path[0];
    let last = path.last().copied().unwrap_or(first);
    let (mut dx, mut dy) = (last.0 - first.0, last.1 - first.1);
    let len = (dx * dx + dy * dy).sqrt();
    if len > 1e-3 {
        dx /= len;
        dy /= len;
    } else {
        dx = 0.0;
        dy = 0.0;
    }

    let mut out = base.to_vec();
    for y in 0..h {
        for x in 0..w {
            let px = x as f32 + 0.5;
            let py = y as f32 + 0.5;
            let Some((sx, sy)) =
                warp_source(px, py, &path, (dx, dy), radius_px, strength, stroke.mode)
            else {
                continue;
            };
            let dst = (y * w + x) * 4;
            for c in 0..4 {
                out[dst + c] = sample_channel(base, w, h, sx, sy, c)
                    .round()
                    .clamp(0.0, 255.0) as u8;
            }
        }
    }
    out
}

/// Wendet alle `strokes` nacheinander an (siehe Moduldoku) — die einzige
/// Funktion, die `develop::render_rgba8` aus diesem Modul aufruft.
pub fn apply(base: &[u8], width: u32, height: u32, strokes: &[LiquifyStroke]) -> Vec<u8> {
    let mut current = base.to_vec();
    for stroke in strokes {
        current = apply_stroke(&current, width, height, stroke);
    }
    current
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::edl::v4::LiquifyPoint;

    fn point(x: f32, y: f32) -> LiquifyPoint {
        LiquifyPoint { x, y }
    }

    fn flat_image(width: u32, height: u32, rgb: [u8; 3]) -> Vec<u8> {
        let mut out = vec![0u8; (width * height * 4) as usize];
        for px in out.chunks_exact_mut(4) {
            px[0] = rgb[0];
            px[1] = rgb[1];
            px[2] = rgb[2];
            px[3] = 255;
        }
        out
    }

    #[test]
    fn empty_stroke_list_is_identity() {
        let base = flat_image(6, 6, [10, 20, 30]);
        assert_eq!(apply(&base, 6, 6, &[]), base);
    }

    #[test]
    fn zero_strength_leaves_the_image_unchanged() {
        let base = flat_image(8, 8, [10, 20, 30]);
        let stroke = LiquifyStroke {
            center_path: vec![point(0.5, 0.5)],
            radius: 0.3,
            strength: 0.0,
            mode: LiquifyMode::Twirl,
        };
        assert_eq!(apply(&base, 8, 8, &[stroke]), base);
    }

    #[test]
    fn a_pixel_far_outside_the_radius_stays_unchanged() {
        // Ein Farbfleck rechts unten, weit außerhalb des Wirkradius um
        // die Bildmitte — Bloat um die Mitte darf ihn nicht erreichen.
        let width = 30u32;
        let height = 30u32;
        let mut base = vec![0u8; (width * height * 4) as usize];
        for px in base.chunks_exact_mut(4) {
            px[3] = 255;
        }
        let corner = (((height - 1) * width + (width - 1)) * 4) as usize;
        base[corner] = 200;
        base[corner + 1] = 100;
        base[corner + 2] = 50;
        let stroke = LiquifyStroke {
            center_path: vec![point(0.5, 0.5)],
            radius: 0.1,
            strength: 1.0,
            mode: LiquifyMode::Bloat,
        };
        let out = apply(&base, width, height, &[stroke]);
        assert_eq!(&out[corner..corner + 4], &[200, 100, 50, 255]);
    }

    #[test]
    fn bloat_changes_pixels_within_its_radius() {
        // Ein Farbfleck in der linken Bildhälfte, Rest schwarz — Bloat
        // um den Fleckrand herum muss die Kante sichtbar verschieben
        // (das Ergebnis unterscheidet sich vom unveränderten Bild),
        // ohne die Bildgröße zu ändern.
        let width = 20u32;
        let height = 20u32;
        let mut base = vec![0u8; (width * height * 4) as usize];
        for y in 0..height {
            for x in 0..width {
                let i = ((y * width + x) * 4) as usize;
                if x < 10 {
                    base[i] = 200;
                    base[i + 1] = 200;
                    base[i + 2] = 200;
                }
                base[i + 3] = 255;
            }
        }
        let stroke = LiquifyStroke {
            center_path: vec![point(0.5, 0.5)],
            radius: 0.4,
            strength: 1.0,
            mode: LiquifyMode::Bloat,
        };
        let out = apply(&base, width, height, &[stroke]);
        assert_eq!(out.len(), base.len());
        assert_ne!(out, base);
    }

    #[test]
    fn push_shifts_pixels_along_the_drag_direction() {
        // Ein einzelner heller Punkt, ein Push-Strich von links nach
        // rechts direkt darüber sollte den Punkt sichtbar verschieben.
        let width = 20u32;
        let height = 20u32;
        let mut base = vec![0u8; (width * height * 4) as usize];
        for px in base.chunks_exact_mut(4) {
            px[3] = 255;
        }
        let mark = ((10 * width + 5) * 4) as usize;
        base[mark] = 255;
        base[mark + 1] = 255;
        base[mark + 2] = 255;
        let stroke = LiquifyStroke {
            center_path: vec![point(0.1, 0.5), point(0.9, 0.5)],
            radius: 0.3,
            strength: 1.0,
            mode: LiquifyMode::Push,
        };
        let out = apply(&base, width, height, &[stroke]);
        assert_ne!(out, base);
    }
}
