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
//!
//! **Phase 28 (siehe ADR-0058):** die Stufe kennt jetzt echte
//! Bokeh-Formen. Solange `blades`, `anamorphic` und `swirl` alle auf
//! ihrer Vorgabe stehen, läuft unverändert der bisherige separierbare
//! Box-Weichzeichner — bit-für-bit dasselbe Ergebnis wie vorher, ein
//! Test hält das fest. Sobald eine Form gewählt ist, wird stattdessen
//! mit einem **geformten Kern** gefaltet: [`BOKEH_SAMPLES`] Abtastpunkte
//! auf einer Golden-Angle-Spirale, deren Radius je Winkel auf den Rand
//! eines regelmäßigen `n`-Ecks gezogen wird, anamorph gestreckt und —
//! bei `swirl` — mit wachsendem Abstand zur Bildmitte tangential
//! gedreht (Petzval-Wirbel).
//!
//! **Ehrliche Kosten:** der geformte Kern ist eine echte 2D-Faltung und
//! damit deutlich teurer als der separierbare Weg (vier Stufen ×
//! [`BOKEH_SAMPLES`] Abtastungen je Pixel statt zweier 1D-Durchläufe).
//! Genau deshalb ist er opt-in und nicht die Vorgabe.

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
/// Abtastpunkte des geformten Bokeh-Kerns (Phase 28). Bewusst knapp
/// gehalten: jeder Punkt ist eine bilineare Leseoperation je Pixel und
/// je Unschärfestufe. 16 Punkte auf einer Golden-Angle-Spirale decken
/// die Blendenfläche gleichmäßig genug ab, dass die typische
/// Polygonform sichtbar wird, ohne dass ein Regler-Tick sekundenlang
/// rechnet.
const BOKEH_SAMPLES: usize = 16;

/// Randradius eines regelmäßigen `n`-Ecks mit Inkreisradius 1 in
/// Richtung `theta` — damit liegen die Abtastpunkte innerhalb der
/// Blendenöffnung statt in einem Kreis.
fn polygon_radius(theta: f32, blades: u32, rotation_rad: f32) -> f32 {
    if blades < 3 {
        return 1.0;
    }
    let n = blades as f32;
    let sector = std::f32::consts::TAU / n;
    let a = (theta + rotation_rad).rem_euclid(sector) - sector * 0.5;
    (sector * 0.5).cos() / a.cos().max(1e-3)
}

/// Die [`BOKEH_SAMPLES`] Kern-Abtastpunkte auf dem Einheitsradius,
/// bereits an Lamellenzahl, Drehung und anamorphe Streckung angepasst.
/// Der Wirbel kommt erst pro Pixel dazu (er hängt von der Lage im Bild
/// ab) und steckt deshalb nicht hier drin.
fn bokeh_kernel(adjustment: &VirtualApertureAdjustment) -> Vec<(f32, f32)> {
    let rotation = adjustment.rotation.to_radians();
    // Streckung flächenerhaltend: die eine Achse wird gedehnt, die
    // andere um denselben Faktor gestaucht — sonst würde ein anamorphes
    // Bokeh nebenbei auch die Unschärfestärke ändern.
    let stretch = 1.0 + adjustment.anamorphic.clamp(0.0, 1.0);
    (0..BOKEH_SAMPLES)
        .map(|k| {
            let theta = k as f32 * 2.399_963_2;
            let r = ((k as f32 + 0.5) / BOKEH_SAMPLES as f32).sqrt()
                * polygon_radius(theta, adjustment.blades, rotation);
            (theta.cos() * r / stretch, theta.sin() * r * stretch)
        })
        .collect()
}

/// Faltung mit dem geformten Kern bei festem Radius.
fn shaped_blur(
    src: &[f32],
    width: usize,
    height: usize,
    radius: f32,
    kernel: &[(f32, f32)],
    swirl: f32,
) -> Vec<f32> {
    let mut out = vec![0.0f32; src.len()];
    let (cx, cy) = ((width as f32 - 1.0) * 0.5, (height as f32 - 1.0) * 0.5);
    let max_dist = (cx * cx + cy * cy).sqrt().max(1.0);
    out.par_chunks_mut(width * 3)
        .enumerate()
        .for_each(|(y, row)| {
            for x in 0..width {
                let (px, py) = (x as f32, y as f32);
                // Petzval-Wirbel: der Kern dreht sich tangential, und
                // zwar umso stärker, je weiter das Pixel von der
                // Bildmitte entfernt liegt.
                let (sin_t, cos_t) = if swirl > 0.0 {
                    let dist = ((px - cx).powi(2) + (py - cy).powi(2)).sqrt() / max_dist;
                    let angle = (py - cy).atan2(px - cx) * swirl * dist;
                    angle.sin_cos()
                } else {
                    (0.0, 1.0)
                };
                let mut acc = [0.0f32; 3];
                for (kx, ky) in kernel {
                    let (ox, oy) = (kx * radius, ky * radius);
                    let (rx, ry) = (ox * cos_t - oy * sin_t, ox * sin_t + oy * cos_t);
                    let sx = (px + rx).clamp(0.0, width as f32 - 1.0);
                    let sy = (py + ry).clamp(0.0, height as f32 - 1.0);
                    let (x0, y0) = (sx.floor() as usize, sy.floor() as usize);
                    let (x1, y1) = ((x0 + 1).min(width - 1), (y0 + 1).min(height - 1));
                    let (fx, fy) = (sx - x0 as f32, sy - y0 as f32);
                    for (c, slot) in acc.iter_mut().enumerate() {
                        let tl = src[(y0 * width + x0) * 3 + c];
                        let tr = src[(y0 * width + x1) * 3 + c];
                        let bl = src[(y1 * width + x0) * 3 + c];
                        let br = src[(y1 * width + x1) * 3 + c];
                        let top = tl + (tr - tl) * fx;
                        let bottom = bl + (br - bl) * fx;
                        *slot += top + (bottom - top) * fy;
                    }
                }
                let inv = 1.0 / kernel.len() as f32;
                for c in 0..3 {
                    row[x * 3 + c] = acc[c] * inv;
                }
            }
        });
    out
}

/// Hebt die Spitzlichter an, BEVOR weichgezeichnet wird. Ohne diesen
/// Schritt mittelt jede Weichzeichnung helle Punkte einfach weg, statt
/// die typischen „Bokeh-Bälle" stehen zu lassen — der Grund, warum
/// naiv weichgezeichnete Hintergründe flau aussehen.
fn boost_highlights(pixels: &[f32], boost: f32, threshold: f32) -> Vec<f32> {
    let threshold = threshold.clamp(0.0, 0.99);
    pixels
        .chunks_exact(3)
        .flat_map(|p| {
            let l = 0.3 * p[0] + 0.59 * p[1] + 0.11 * p[2];
            let gate = ((l - threshold) / (1.0 - threshold)).clamp(0.0, 1.0);
            let gain = 1.0 + boost.clamp(0.0, 1.0) * 4.0 * gate * gate;
            [p[0] * gain, p[1] * gain, p[2] * gain]
        })
        .collect()
}

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
    //
    // Phase 28: Stufe 0 bleibt immer das unveränderte, SCHARFE Bild —
    // auch bei `highlight_boost`, damit die Schärfeebene nicht
    // nebenbei mit ausbrennt. Nur die weichgezeichneten Stufen werden
    // aus der angehobenen Fassung gespeist.
    let shaped = adjustment.blades >= 3 || adjustment.anamorphic > 0.0 || adjustment.swirl > 0.0;
    let blur_source = if adjustment.highlight_boost > 0.0 {
        boost_highlights(
            pixels,
            adjustment.highlight_boost,
            adjustment.highlight_threshold,
        )
    } else {
        pixels.to_vec()
    };
    let kernel = shaped.then(|| bokeh_kernel(adjustment));

    let mut levels: Vec<Vec<f32>> = Vec::with_capacity(BLUR_LEVELS);
    levels.push(pixels.to_vec());
    for level in 1..BLUR_LEVELS {
        let radius = (max_radius_px * level as i32) / (BLUR_LEVELS as i32 - 1);
        let radius = radius.max(1);
        let blurred = match &kernel {
            Some(kernel) => shaped_blur(
                &blur_source,
                w,
                h,
                radius as f32,
                kernel,
                adjustment.swirl.clamp(0.0, 1.0),
            ),
            None => {
                let horizontal = box_blur_1d(&blur_source, w, h, radius, true);
                box_blur_1d(&horizontal, w, h, radius, false)
            }
        };
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
            ..VirtualApertureAdjustment::NEUTRAL
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
            ..VirtualApertureAdjustment::NEUTRAL
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
            ..VirtualApertureAdjustment::NEUTRAL
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
            ..VirtualApertureAdjustment::NEUTRAL
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

/// Phase-28-Regressionstests: die Bokeh-Formen dürfen das bisherige
/// Verhalten nicht anfassen.
#[cfg(test)]
mod bokeh_tests {
    use super::*;
    use crate::edl::v4::DepthMapPatch;

    /// Wortgetreue Kopie des Verfahrens aus dem Stand VOR Phase 28
    /// (`git show HEAD~:…virtual_aperture.rs`). Nur so lässt sich die
    /// Zusage „ohne gesetzte Bokeh-Form bit-für-bit dasselbe Ergebnis"
    /// wirklich prüfen statt sie nur zu behaupten — ein Test, der die
    /// neue Fassung mit sich selbst vergleicht, würde nichts beweisen.
    fn legacy_apply(
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
        let focus_x_px =
            (adjustment.focus_x.clamp(0.0, 1.0) * (width as f32 - 1.0)).round() as usize;
        let focus_y_px =
            (adjustment.focus_y.clamp(0.0, 1.0) * (height as f32 - 1.0)).round() as usize;
        let focus_depth =
            f32::from(depth[(focus_y_px.min(h - 1)) * w + focus_x_px.min(w - 1)]) / 255.0;
        let max_radius_px = ((adjustment.amount.clamp(0.0, 100.0) / 100.0)
            * MAX_BLUR_RADIUS_FRACTION
            * width as f32)
            .round()
            .max(1.0) as i32;
        let mut levels: Vec<Vec<f32>> = Vec::with_capacity(BLUR_LEVELS);
        levels.push(pixels.to_vec());
        for level in 1..BLUR_LEVELS {
            let radius = ((max_radius_px * level as i32) / (BLUR_LEVELS as i32 - 1)).max(1);
            let horizontal = box_blur_1d(pixels, w, h, radius, true);
            levels.push(box_blur_1d(&horizontal, w, h, radius, false));
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

    fn scene(size: u32) -> (Vec<f32>, DepthMapPatch) {
        let n = (size * size) as usize;
        let mut pixels = vec![0.0f32; n * 3];
        let mut depth = vec![0u8; n];
        for y in 0..size {
            for x in 0..size {
                let i = (y * size + x) as usize;
                // Ein helles Spitzlicht links oben, sonst ein Verlauf —
                // genau das, woran sich Bokeh-Formen zeigen.
                let spot = x < size / 6 && y < size / 6;
                let v = if spot {
                    3.0
                } else {
                    0.05 + x as f32 / size as f32 * 0.4
                };
                pixels[i * 3] = v;
                pixels[i * 3 + 1] = v * 0.9;
                pixels[i * 3 + 2] = v * 0.7;
                depth[i] = (y * 255 / size.max(2)) as u8;
            }
        }
        (
            pixels,
            DepthMapPatch {
                bitmap_width: size,
                bitmap_height: size,
                depth,
            },
        )
    }

    #[test]
    fn default_bokeh_fields_reproduce_the_pre_phase_28_result_bit_for_bit() {
        let size = 32u32;
        let (pixels, depth_map) = scene(size);
        let adjustment = VirtualApertureAdjustment {
            focus_x: 0.5,
            focus_y: 0.9,
            amount: 80.0,
            depth_map: Some(depth_map),
            ..VirtualApertureAdjustment::NEUTRAL
        };
        let new = apply(&pixels, size, size, &adjustment);
        let old = legacy_apply(&pixels, size, size, &adjustment);
        assert_eq!(
            new, old,
            "die Phase-28-Erweiterung hat das Vorgabe-Verhalten verändert"
        );
    }

    #[test]
    fn polygonal_blades_change_the_result() {
        let size = 32u32;
        let (pixels, depth_map) = scene(size);
        let base = VirtualApertureAdjustment {
            focus_x: 0.5,
            focus_y: 0.9,
            amount: 80.0,
            depth_map: Some(depth_map),
            ..VirtualApertureAdjustment::NEUTRAL
        };
        let round = apply(&pixels, size, size, &base);
        let hexagonal = apply(
            &pixels,
            size,
            size,
            &VirtualApertureAdjustment {
                blades: 6,
                ..base.clone()
            },
        );
        assert!(
            round.iter().zip(&hexagonal).any(|(a, b)| a != b),
            "eine Sechseck-Blende muss ein anderes Bokeh erzeugen als eine runde"
        );
    }

    #[test]
    fn anamorphic_and_swirl_each_change_the_result_on_their_own() {
        let size = 32u32;
        let (pixels, depth_map) = scene(size);
        let base = VirtualApertureAdjustment {
            focus_x: 0.5,
            focus_y: 0.9,
            amount: 80.0,
            depth_map: Some(depth_map),
            ..VirtualApertureAdjustment::NEUTRAL
        };
        let plain = apply(&pixels, size, size, &base);
        for (name, adjustment) in [
            (
                "anamorph",
                VirtualApertureAdjustment {
                    anamorphic: 0.8,
                    ..base.clone()
                },
            ),
            (
                "Wirbel",
                VirtualApertureAdjustment {
                    swirl: 1.0,
                    ..base.clone()
                },
            ),
        ] {
            let out = apply(&pixels, size, size, &adjustment);
            assert!(
                plain.iter().zip(&out).any(|(a, b)| a != b),
                "{name} hat das Ergebnis nicht verändert"
            );
        }
    }

    /// Die Spitzlicht-Anhebung ist der Grund, warum synthetisches Bokeh
    /// überhaupt nach Bokeh aussieht: ohne sie mittelt die
    /// Weichzeichnung helle Punkte weg.
    #[test]
    fn highlight_boost_makes_the_out_of_focus_highlight_brighter() {
        let size = 32u32;
        let (pixels, depth_map) = scene(size);
        let base = VirtualApertureAdjustment {
            // Fokus unten, das Spitzlicht liegt oben — es ist also
            // unscharf und wird vom Bokeh-Kern erfasst.
            focus_x: 0.5,
            focus_y: 0.95,
            amount: 100.0,
            depth_map: Some(depth_map),
            ..VirtualApertureAdjustment::NEUTRAL
        };
        let plain = apply(&pixels, size, size, &base);
        let boosted = apply(
            &pixels,
            size,
            size,
            &VirtualApertureAdjustment {
                highlight_boost: 1.0,
                highlight_threshold: 0.5,
                ..base.clone()
            },
        );
        let sum = |v: &[f32]| -> f32 { v.iter().sum() };
        assert!(
            sum(&boosted) > sum(&plain) * 1.05,
            "die Spitzlicht-Anhebung wirkt nicht: {} gegenüber {}",
            sum(&boosted),
            sum(&plain)
        );
    }
}
