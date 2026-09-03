//! KI-Tiefenschärfe-Simulator "Virtuelle Blende" (Phase 14 Schritt 8,
//! siehe `DECISIONS.md` ADR-0041 Nachtrag VIII, Recherche-Tabelle
//! Punkt 1): Lightroom hat keine KI-Tiefenschätzung/synthetisches Bokeh
//! — nur die vorhandene grobe Unschärfe-Heuristik in ApertureX selbst
//! (Laplace-Varianz, `stages::masks`s `BlurDepthApprox`-Maskentyp,
//! Phase 11 Schritt 7).
//!
//! **Architektur-Hinweis wie bei `BlurDepthApprox`s eigener Moduldoku:**
//! `apx-pipeline` hängt nicht von `apx-ai` ab (umgekehrt schon), die
//! echte MiDaS-Inferenz kann deshalb nicht hier laufen. Die Tiefenkarte
//! wird stattdessen einmalig vorab in `apx-app` per
//! `apx_ai::depth::DepthSession::estimate_rgb8` berechnet und als
//! [`crate::edl::v4::DepthMapPatch`] in der EDL gespeichert — dieselbe
//! „einmal berechnen, bei jedem Rendern nur noch skalieren"-Architektur
//! wie `edl::v2::AiFillPatch`/`edl::v4::CompositeLayerSource`.
//!
//! **Verfahren:** ohne Tiefenkarte oder bei `amount <= 0.0` ein reiner
//! No-Op. Sonst werden [`BLUR_LEVELS`] zunehmend weichgezeichnete
//! Fassungen des Bildes vorab berechnet (derselbe separierbare
//! Box-Weichzeichner wie `stages::effects`s Halation-Simulation, hier
//! erneut eigenständig implementiert statt geteilt — dieselbe „kleine,
//! in sich geschlossene Funktion"-Begründung wie überall in diesem
//! Projekt). Je Pixel wird aus dem Tiefenunterschied zum angeklickten
//! Fokuspunkt (multipliziert mit der "Blendenöffnung" `amount`) ein
//! Unschärfegrad `0.0..=1.0` berechnet und zwischen den beiden
//! nächstgelegenen vorab berechneten Weichzeichner-Stufen linear
//! interpoliert — eine in echten Bokeh-Simulatoren übliche, günstige
//! Näherung an eine echte, pro Pixel unterschiedlich starke
//! Weichzeichnung (die selbst mit separierbaren Filtern nicht effizient
//! direkt pro Pixel berechenbar wäre).

use rayon::prelude::*;

use crate::edl::v4::VirtualApertureAdjustment;

/// Wie viele vorab weichgezeichnete Bildstufen berechnet werden
/// (zwischen denen pro Pixel interpoliert wird) — mehr Stufen ergeben
/// eine feinere Abstufung, kosten aber je einen zusätzlichen vollen
/// zweifachen Box-Blur-Durchlauf über das ganze Bild. Fünf Stufen (0 =
/// scharf bis zur maximalen Unschärfe) sind für einen glaubwürdigen
/// Bokeh-Effekt ausreichend, ohne bei jedem Regler-Tick spürbar zu
/// bremsen.
const BLUR_LEVELS: usize = 5;
/// Unschärferadius bei voller Blendenöffnung (`amount = 100`) und
/// maximalem Tiefenabstand, als Bruchteil der Bildbreite — bewusst
/// vorsichtig gewählt (deutlich kleiner als die Halation-Obergrenze aus
/// Schritt 4), weil hier das *gesamte* außerhalb der Schärfeebene
/// liegende Bild betroffen ist, nicht nur die Lichter.
const MAX_BLUR_RADIUS_FRACTION: f32 = 0.08;

/// Wendet die "Virtuelle Blende" an — die einzige Funktion, die
/// `develop::render_rgba8` dafür aufruft, immer CPU-seitig (derselbe
/// Grund wie Halation: eine mehrstufige Nachbarschaftsoperation, kein
/// per-Pixel-Shader-Fall).
pub fn apply(
    pixels: &[f32],
    width: u32,
    height: u32,
    adjustment: &VirtualApertureAdjustment,
) -> Vec<f32> {
    let Some(depth_map) = &adjustment.depth_map else {
        return pixels.to_vec();
    };
    if adjustment.amount <= 0.0 || width == 0 || height == 0 {
        return pixels.to_vec();
    }

    let w = width as usize;
    let h = height as usize;

    let depth = apx_core::raster::bilinear_resize_u8(
        &depth_map.depth,
        depth_map.bitmap_width,
        depth_map.bitmap_height,
        width,
        height,
    );

    let focus_x_px = (adjustment.focus_x.clamp(0.0, 1.0) * (width as f32 - 1.0)).round() as usize;
    let focus_y_px = (adjustment.focus_y.clamp(0.0, 1.0) * (height as f32 - 1.0)).round() as usize;
    let focus_depth = f32::from(depth[(focus_y_px.min(h - 1)) * w + focus_x_px.min(w - 1)]) / 255.0;

    let max_radius_px =
        ((adjustment.amount.clamp(0.0, 100.0) / 100.0) * MAX_BLUR_RADIUS_FRACTION * width as f32)
            .round()
            .max(1.0) as i32;

    // Stufe 0 = unverändertes Bild, jede weitere Stufe ein zunehmend
    // größerer Box-Blur — jeweils direkt vom Original aus, nicht
    // kaskadierend, damit sich Rundungsfehler nicht über die Stufen
    // aufsummieren.
    let mut levels: Vec<Vec<f32>> = Vec::with_capacity(BLUR_LEVELS);
    levels.push(pixels.to_vec());
    for level in 1..BLUR_LEVELS {
        let radius = (max_radius_px * level as i32) / (BLUR_LEVELS as i32 - 1);
        let radius = radius.max(1);
        let horizontal = box_blur_1d(pixels, w, h, radius, true);
        let blurred = box_blur_1d(&horizontal, w, h, radius, false);
        levels.push(blurred);
    }

    let amount_fraction = adjustment.amount.clamp(0.0, 100.0) / 100.0;
    (0..w * h)
        .into_par_iter()
        .flat_map_iter(|index| {
            let idx = index * 3;
            let depth_here = f32::from(depth[index]) / 255.0;
            let defocus = ((depth_here - focus_depth).abs() * amount_fraction).clamp(0.0, 1.0);
            let level_position = defocus * (BLUR_LEVELS as f32 - 1.0);
            let lo = level_position.floor() as usize;
            let hi = (lo + 1).min(BLUR_LEVELS - 1);
            let t = level_position - lo as f32;
            std::array::from_fn::<f32, 3, _>(|c| {
                let a = levels[lo][idx + c];
                let b = levels[hi][idx + c];
                a + (b - a) * t
            })
        })
        .collect()
}

/// Separierbarer Box-Weichzeichner — dieselbe Technik wie
/// `stages::effects`s `halation_box_blur_1d`, hier erneut eigenständig
/// implementiert (siehe Moduldoku). Randpixel werden übersprungen statt
/// gespiegelt/geklemmt, der Mittelwert läuft deshalb am Rand über
/// weniger Abtastpunkte — dasselbe Verhalten wie das Halation-Vorbild.
fn box_blur_1d(
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
                let sample_idx = (sy as usize * width + sx as usize) * 3;
                for (c, slot) in sum.iter_mut().enumerate() {
                    *slot += src[sample_idx + c];
                }
                count += 1.0;
            }
            sum.map(|v| if count > 0.0 { v / count } else { 0.0 })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::edl::v4::DepthMapPatch;

    fn flat_gray(width: u32, height: u32, value: f32) -> Vec<f32> {
        vec![value; (width as usize) * (height as usize) * 3]
    }

    fn uniform_depth(width: u32, height: u32, value: u8) -> DepthMapPatch {
        DepthMapPatch {
            bitmap_width: width,
            bitmap_height: height,
            depth: vec![value; (width as usize) * (height as usize)],
        }
    }

    #[test]
    fn without_a_depth_map_is_identity() {
        let pixels = flat_gray(8, 8, 0.4);
        let adjustment = VirtualApertureAdjustment {
            focus_x: 0.5,
            focus_y: 0.5,
            amount: 80.0,
            depth_map: None,
        };
        assert_eq!(apply(&pixels, 8, 8, &adjustment), pixels);
    }

    #[test]
    fn zero_amount_is_identity_even_with_a_depth_map() {
        let pixels = flat_gray(8, 8, 0.4);
        let adjustment = VirtualApertureAdjustment {
            focus_x: 0.5,
            focus_y: 0.5,
            amount: 0.0,
            depth_map: Some(uniform_depth(8, 8, 200)),
        };
        assert_eq!(apply(&pixels, 8, 8, &adjustment), pixels);
    }

    #[test]
    fn a_uniform_depth_map_leaves_the_image_sharp_regardless_of_amount() {
        // Jedes Pixel hat denselben Tiefenwert wie der Fokuspunkt ->
        // `defocus` ist überall 0 -> Stufe 0 (unverändert) überall,
        // selbst bei voller Blendenöffnung.
        let size = 20;
        let mut pixels = flat_gray(size, size, 0.2);
        // Ein einzelner heller Fleck, um eine tatsächliche Weichzeichnung
        // überhaupt sichtbar zu machen, falls der Test fälschlich
        // verwischt.
        let c = (size / 2) as usize;
        pixels[(c * size as usize + c) * 3] = 1.0;
        pixels[(c * size as usize + c) * 3 + 1] = 1.0;
        pixels[(c * size as usize + c) * 3 + 2] = 1.0;

        let adjustment = VirtualApertureAdjustment {
            focus_x: 0.5,
            focus_y: 0.5,
            amount: 100.0,
            depth_map: Some(uniform_depth(size, size, 128)),
        };
        let out = apply(&pixels, size, size, &adjustment);
        assert_eq!(out, pixels);
    }

    #[test]
    fn a_pixel_far_from_the_focus_depth_gets_visibly_blurred() {
        // Fokuspunkt links (Tiefe 255 = am nächsten), ein heller Fleck
        // rechts bei Tiefe 0 (am weitesten entfernt) — maximaler
        // Tiefenabstand, muss also am stärksten unscharf werden.
        let size = 80;
        let mut pixels = flat_gray(size, size, 0.1);
        let spot_x = size as usize - 4;
        let spot_y = size as usize / 2;
        // 7x7-Block statt eines Einzelpixels — dieselbe Lehre wie
        // Schritt 4s Halation-Test: ein zu kleiner heller Fleck wird vom
        // zweifachen Box-Blur-Mittelwert zu stark verdünnt, um am
        // Nachbarpixel überhaupt messbar zu sein.
        for dy in -3i32..=3 {
            for dx in -3i32..=3 {
                let x = (spot_x as i32 + dx).clamp(0, size as i32 - 1) as usize;
                let y = (spot_y as i32 + dy).clamp(0, size as i32 - 1) as usize;
                let idx = (y * size as usize + x) * 3;
                pixels[idx] = 1.0;
                pixels[idx + 1] = 1.0;
                pixels[idx + 2] = 1.0;
            }
        }

        let mut depth = vec![0u8; (size as usize) * (size as usize)];
        for x in 0..(size as usize) {
            let d = 255 - ((x * 255) / (size as usize - 1));
            for y in 0..(size as usize) {
                depth[y * size as usize + x] = d as u8;
            }
        }

        let adjustment = VirtualApertureAdjustment {
            focus_x: 0.0,
            focus_y: 0.5,
            amount: 100.0,
            depth_map: Some(DepthMapPatch {
                bitmap_width: size,
                bitmap_height: size,
                depth,
            }),
        };
        let out = apply(&pixels, size, size, &adjustment);

        // Der exakte Fleck-Mittelpunkt muss durch die Weichzeichnung
        // heller Nachbarwerte hinzugewinnen... nein, der Mittelpunkt ist
        // schon 1.0 (Maximum) — stattdessen prüfen wir, dass ein Pixel
        // *neben* dem Fleck (im Original dunkel) durch die Unschärfe
        // sichtbar heller geworden ist (Lichtausbreitung durch den Blur).
        let neighbor_idx = (spot_y * size as usize + (spot_x - 6)) * 3;
        assert!(
            out[neighbor_idx] > pixels[neighbor_idx] + 0.05,
            "out={} original={}",
            out[neighbor_idx],
            pixels[neighbor_idx]
        );
    }
}
