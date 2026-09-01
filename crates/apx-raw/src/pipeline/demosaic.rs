//! Demosaicing: rekonstruiert aus dem Bayer-/CFA-Mosaik (ein Farbkanal pro
//! Pixel) ein vollständiges RGB-Bild (drei Kanäle pro Pixel).
//!
//! **Bewusst provisorisch** (siehe `PHASE1_PROMPT.md` Abschnitt 3): für
//! den häufigen Fall eines 2×2-Bayer-Musters (CR2, NEF, ARW, ORF, RW2,
//! die meisten DNGs) wird klassisches bilineares Demosaicing verwendet.
//! Für andere Muster (z. B. Fujis 6×6-X-Trans in RAF) greift ein
//! generischer Fallback, der den fehlenden Farbkanal aus dem nächstliegenden
//! Ring gleichfarbiger Nachbarn mittelt — geometrisch weniger präzise, aber
//! für kein CFA-Muster falsch. Beide Pfade werden in Phase 2 durch die
//! GPU-Pipeline ersetzt.

use rawler::CFA;

/// Farbindex, auf den ein von `CFA::color_at` gelieferter Wert reduziert
/// wird. Exotische Vier-Farb-Sensoren (Cyan/Magenta/Gelb/Weiß/E) werden auf
/// Grün abgebildet — das ist die neutralste Näherung, wenn nur R/G/B
/// verarbeitet wird.
fn channel_of(cfa_color: usize) -> usize {
    if cfa_color > 2 {
        1
    } else {
        cfa_color
    }
}

fn get(mosaic: &[f32], width: usize, height: usize, x: isize, y: isize) -> Option<f32> {
    if x < 0 || y < 0 || x >= width as isize || y >= height as isize {
        None
    } else {
        Some(mosaic[y as usize * width + x as usize])
    }
}

fn avg_of(values: &[Option<f32>]) -> f32 {
    let mut sum = 0.0;
    let mut count = 0u32;
    for value in values.iter().flatten() {
        sum += value;
        count += 1;
    }
    if count == 0 {
        0.0
    } else {
        sum / count as f32
    }
}

/// Demosaicing in voller Auflösung. `mosaic` enthält bereits normalisierte
/// (Schwarzpunkt/Weißpunkt-korrigierte) Werte in `[0, 1]`, ein Wert pro
/// Pixel. Gibt interleaved RGB-Werte zurück (`width * height * 3` Elemente,
/// weiterhin `[0, 1]`, noch **ohne** Weißabgleich — der wird
/// nachgelagert in `color.rs` angewendet, siehe Modul-Doku dort).
pub fn demosaic_full(mosaic: &[f32], width: usize, height: usize, cfa: &CFA) -> Vec<f32> {
    if cfa.width == 2 && cfa.height == 2 {
        demosaic_full_bayer(mosaic, width, height, cfa)
    } else {
        demosaic_full_generic(mosaic, width, height, cfa)
    }
}

/// Klassisches bilineares Bayer-Demosaicing.
fn demosaic_full_bayer(mosaic: &[f32], width: usize, height: usize, cfa: &CFA) -> Vec<f32> {
    let mut out = vec![0.0_f32; width * height * 3];

    for y in 0..height {
        for x in 0..width {
            let (xi, yi) = (x as isize, y as isize);
            let own = channel_of(cfa.color_at(y, x));
            let raw = mosaic[y * width + x];

            let mut rgb = [0.0_f32; 3];
            rgb[own] = raw;

            match own {
                0 | 2 => {
                    // An einer R- oder B-Position: Grün liegt an den vier
                    // orthogonalen Nachbarn, die jeweils fehlende
                    // Gegenfarbe (B bei R, R bei B) an den vier
                    // diagonalen Nachbarn.
                    let other = if own == 0 { 2 } else { 0 };
                    rgb[1] = avg_of(&[
                        get(mosaic, width, height, xi, yi - 1),
                        get(mosaic, width, height, xi, yi + 1),
                        get(mosaic, width, height, xi - 1, yi),
                        get(mosaic, width, height, xi + 1, yi),
                    ]);
                    rgb[other] = avg_of(&[
                        get(mosaic, width, height, xi - 1, yi - 1),
                        get(mosaic, width, height, xi + 1, yi - 1),
                        get(mosaic, width, height, xi - 1, yi + 1),
                        get(mosaic, width, height, xi + 1, yi + 1),
                    ]);
                }
                _ => {
                    // An einer G-Position: ob die horizontalen oder die
                    // vertikalen Nachbarn Rot bzw. Blau sind, hängt davon
                    // ab, in welcher "Zeilenart" dieses Grün liegt. Bevorzugt
                    // wird der linke Nachbar befragt, an der linken
                    // Bildkante stattdessen der rechte.
                    let neighbor_color = match x.checked_sub(1) {
                        Some(left_x) => channel_of(cfa.color_at(y, left_x)),
                        None => channel_of(cfa.color_at(y, x + 1)),
                    };
                    let (red_horizontal, blue_horizontal) =
                        (neighbor_color == 0, neighbor_color == 2);

                    let horizontal = avg_of(&[
                        get(mosaic, width, height, xi - 1, yi),
                        get(mosaic, width, height, xi + 1, yi),
                    ]);
                    let vertical = avg_of(&[
                        get(mosaic, width, height, xi, yi - 1),
                        get(mosaic, width, height, xi, yi + 1),
                    ]);

                    if red_horizontal {
                        rgb[0] = horizontal;
                        rgb[2] = vertical;
                    } else if blue_horizontal {
                        rgb[2] = horizontal;
                        rgb[0] = vertical;
                    } else {
                        // Sollte bei einem echten Bayer-Muster nicht
                        // vorkommen; sicherer Fallback, falls doch.
                        rgb[0] = horizontal;
                        rgb[2] = vertical;
                    }
                }
            }

            let idx = (y * width + x) * 3;
            out[idx..idx + 3].copy_from_slice(&rgb);
        }
    }

    out
}

/// Generischer Fallback für nicht-2×2-CFA-Muster (z. B. X-Trans): fehlende
/// Kanäle werden aus dem kleinsten Fenster gemittelt, das mindestens einen
/// gleichfarbigen Nachbarn enthält.
fn demosaic_full_generic(mosaic: &[f32], width: usize, height: usize, cfa: &CFA) -> Vec<f32> {
    let mut out = vec![0.0_f32; width * height * 3];

    for y in 0..height {
        for x in 0..width {
            let own = channel_of(cfa.color_at(y, x));
            let raw = mosaic[y * width + x];
            let mut rgb = [0.0_f32; 3];
            rgb[own] = raw;

            for (channel, value) in rgb.iter_mut().enumerate() {
                if channel == own {
                    continue;
                }
                *value = nearest_same_channel_average(mosaic, width, height, cfa, x, y, channel);
            }

            let idx = (y * width + x) * 3;
            out[idx..idx + 3].copy_from_slice(&rgb);
        }
    }

    out
}

fn nearest_same_channel_average(
    mosaic: &[f32],
    width: usize,
    height: usize,
    cfa: &CFA,
    x: usize,
    y: usize,
    channel: usize,
) -> f32 {
    // Fenster ab Radius 1 vergrößern, bis mindestens ein passender Nachbar
    // gefunden wurde. Für reale CFA-Muster (max. Kachelgröße 6×6 bei
    // X-Trans) reicht Radius 6 in jedem Fall.
    for radius in 1..=6isize {
        let mut sum = 0.0_f32;
        let mut count = 0u32;
        for dy in -radius..=radius {
            let ny = y as isize + dy;
            if ny < 0 || ny >= height as isize {
                continue;
            }
            for dx in -radius..=radius {
                let nx = x as isize + dx;
                if nx < 0 || nx >= width as isize {
                    continue;
                }
                if channel_of(cfa.color_at(ny as usize, nx as usize)) == channel {
                    sum += mosaic[ny as usize * width + nx as usize];
                    count += 1;
                }
            }
        }
        if count > 0 {
            return sum / count as f32;
        }
    }
    0.0
}

/// Half-size-Demosaicing für Vorschauen: fasst jeden 2×2-Block des Mosaiks
/// zu einem Ausgabepixel zusammen (grobe, aber sehr schnelle Näherung).
/// Gibt `(pixels, width, height)` des halbierten Bildes zurück.
pub fn demosaic_half(
    mosaic: &[f32],
    width: usize,
    height: usize,
    cfa: &CFA,
) -> (Vec<f32>, usize, usize) {
    let out_w = width / 2;
    let out_h = height / 2;
    let mut out = vec![0.0_f32; out_w * out_h * 3];

    for by in 0..out_h {
        for bx in 0..out_w {
            let (y0, x0) = (by * 2, bx * 2);
            let mut sums = [0.0_f32; 3];
            let mut counts = [0u32; 3];

            for dy in 0..2 {
                for dx in 0..2 {
                    let (y, x) = (y0 + dy, x0 + dx);
                    let channel = channel_of(cfa.color_at(y, x));
                    sums[channel] += mosaic[y * width + x];
                    counts[channel] += 1;
                }
            }

            let idx = (by * out_w + bx) * 3;
            for channel in 0..3 {
                out[idx + channel] = if counts[channel] > 0 {
                    sums[channel] / counts[channel] as f32
                } else {
                    // Block ohne Sample dieses Kanals (kann bei
                    // ungewöhnlichen CFA-Mustern vorkommen): auf Grün
                    // ausweichen, falls vorhanden, sonst 0 — akzeptabel für
                    // eine reine Vorschau-Näherung.
                    if counts[1] > 0 {
                        sums[1] / counts[1] as f32
                    } else {
                        0.0
                    }
                };
            }
        }
    }

    (out, out_w, out_h)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bayer_rggb() -> CFA {
        CFA::new("RGGB")
    }

    #[test]
    fn full_demosaic_preserves_native_channel_at_each_position() {
        // 4x4 RGGB-Mosaik mit eindeutigen Werten, damit sich die
        // Positionen zurückverfolgen lassen.
        let width = 4;
        let height = 4;
        let mosaic: Vec<f32> = (0..width * height).map(|i| i as f32 / 100.0).collect();
        let cfa = bayer_rggb();

        let rgb = demosaic_full(&mosaic, width, height, &cfa);
        assert_eq!(rgb.len(), width * height * 3);

        for y in 0..height {
            for x in 0..width {
                let own = channel_of(cfa.color_at(y, x));
                let idx = (y * width + x) * 3;
                assert_eq!(rgb[idx + own], mosaic[y * width + x], "Position ({x},{y})");
            }
        }
    }

    #[test]
    fn full_demosaic_fills_missing_channels_with_finite_values() {
        let width = 6;
        let height = 6;
        let mosaic: Vec<f32> = vec![0.5; width * height];
        let cfa = bayer_rggb();
        let rgb = demosaic_full(&mosaic, width, height, &cfa);
        assert!(rgb.iter().all(|v| v.is_finite()));
        // Bei konstantem Mosaikwert müssen alle rekonstruierten Kanäle
        // ebenfalls 0.5 sein.
        assert!(rgb.iter().all(|v| (*v - 0.5).abs() < 1e-6));
    }

    #[test]
    fn half_size_output_has_expected_dimensions() {
        let width = 8;
        let height = 6;
        let mosaic: Vec<f32> = vec![0.25; width * height];
        let cfa = bayer_rggb();
        let (half, hw, hh) = demosaic_half(&mosaic, width, height, &cfa);
        assert_eq!((hw, hh), (4, 3));
        assert_eq!(half.len(), hw * hh * 3);
        assert!(half.iter().all(|v| (*v - 0.25).abs() < 1e-6));
    }

    #[test]
    fn generic_fallback_handles_non_bayer_pattern_without_panicking() {
        // 6x6-Muster (rawler erlaubt nur die Längen 4, 16, 36 und 144 für
        // CFA::new — 36 Zeichen ergeben ein 6x6-Muster wie bei Fujis
        // X-Trans-Sensoren). Kein echtes X-Trans-Layout, aber ausreichend,
        // um den generischen (Nicht-2×2-)Pfad zu testen.
        let cfa = CFA::new("RGGBRGGBRGGBRGGBRGGBRGGBRGGBRGGBRGGB");
        assert_eq!((cfa.width, cfa.height), (6, 6));
        let width = 12;
        let height = 12;
        let mosaic: Vec<f32> = (0..width * height)
            .map(|i| (i % 50) as f32 / 100.0)
            .collect();
        let rgb = demosaic_full(&mosaic, width, height, &cfa);
        assert_eq!(rgb.len(), width * height * 3);
        assert!(rgb.iter().all(|v| v.is_finite()));
    }
}
