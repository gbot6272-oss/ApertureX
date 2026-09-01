//! Entrauschung (Phase 9 Schritt 6, siehe `PLAN.md`/`DECISIONS.md`
//! ADR-0035) — ein echter kantenerhaltender Bilateral-Filter auf dem
//! fertigen RGBA8-Bildpuffer. `blur.rs`s Box-Filter-Näherung (Ein-Kanal-
//! `f32`-Raster für Saliency-/Kontrastkarten) ist dafür nicht geeignet:
//! ein reiner Gauß-/Box-Weichzeichner würde Kanten genauso verwischen
//! wie Rauschen, ein Bilateral-Filter gewichtet Nachbarn zusätzlich nach
//! Farbähnlichkeit und lässt Kanten deshalb weitgehend stehen.
//!
//! **Dieselbe Ehrlichkeitslinie wie ADR-0033**: kein ONNX-Modell, keine
//! echte neuronale Inferenz (das ONNX-Beschaffungsproblem ist weiterhin
//! ungelöst) — ein klassischer, deterministischer Algorithmus. Die
//! Aufrufer (Frontend/`apx-app`) beschriften dies deshalb bewusst nicht
//! als „KI"/„AI".

/// Radius `2` deckt ein 5×5-Nachbarschaftsfenster ab — ein guter
/// Kompromiss zwischen sichtbarer Rauschunterdrückung und Laufzeit
/// (O(width·height·(2r+1)²)).
pub const DEFAULT_RADIUS: i32 = 2;

fn gaussian(x: f32, sigma: f32) -> f32 {
    (-(x * x) / (2.0 * sigma * sigma)).exp()
}

/// Bilateral-Filter über `pixels` (RGBA8, `width * height * 4` Bytes).
/// `spatial_sigma` steuert, wie stark weiter entfernte Nachbarn zählen;
/// `range_sigma` (auf der 0..255-Byteskala), wie stark farblich
/// unähnliche Nachbarn ausgeschlossen werden — größer heißt stärkere
/// Glättung, `0` würde (rein rechnerisch) nur exakt gleichfarbige
/// Nachbarn einbeziehen. Alpha bleibt unverändert.
pub fn bilateral_filter_rgba8(
    pixels: &[u8],
    width: u32,
    height: u32,
    radius: i32,
    spatial_sigma: f32,
    range_sigma: f32,
) -> Vec<u8> {
    if width == 0 || height == 0 || radius <= 0 {
        return pixels.to_vec();
    }
    let w = width as i32;
    let h = height as i32;
    let mut out = vec![0u8; pixels.len()];

    let at = |x: i32, y: i32, channel: usize| -> f32 {
        let cx = x.clamp(0, w - 1);
        let cy = y.clamp(0, h - 1);
        pixels[((cy * w + cx) * 4 + channel as i32) as usize] as f32
    };

    for y in 0..h {
        for x in 0..w {
            let center = [at(x, y, 0), at(x, y, 1), at(x, y, 2)];
            let mut sum = [0.0f32; 3];
            let mut weight_sum = 0.0f32;

            for dy in -radius..=radius {
                for dx in -radius..=radius {
                    let spatial_dist = ((dx * dx + dy * dy) as f32).sqrt();
                    let spatial_weight = gaussian(spatial_dist, spatial_sigma);
                    let sample = [
                        at(x + dx, y + dy, 0),
                        at(x + dx, y + dy, 1),
                        at(x + dx, y + dy, 2),
                    ];
                    let range_dist = ((sample[0] - center[0]).powi(2)
                        + (sample[1] - center[1]).powi(2)
                        + (sample[2] - center[2]).powi(2))
                    .sqrt();
                    let weight = spatial_weight * gaussian(range_dist, range_sigma);
                    sum[0] += weight * sample[0];
                    sum[1] += weight * sample[1];
                    sum[2] += weight * sample[2];
                    weight_sum += weight;
                }
            }

            let index = ((y * w + x) * 4) as usize;
            if weight_sum > 1e-6 {
                out[index] = (sum[0] / weight_sum).round().clamp(0.0, 255.0) as u8;
                out[index + 1] = (sum[1] / weight_sum).round().clamp(0.0, 255.0) as u8;
                out[index + 2] = (sum[2] / weight_sum).round().clamp(0.0, 255.0) as u8;
            } else {
                out[index] = center[0] as u8;
                out[index + 1] = center[1] as u8;
                out[index + 2] = center[2] as u8;
            }
            out[index + 3] = pixels[index + 3];
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn solid(width: u32, height: u32, r: u8, g: u8, b: u8) -> Vec<u8> {
        let mut pixels = vec![0u8; (width * height * 4) as usize];
        for chunk in pixels.chunks_exact_mut(4) {
            chunk[0] = r;
            chunk[1] = g;
            chunk[2] = b;
            chunk[3] = 255;
        }
        pixels
    }

    #[test]
    fn zero_radius_is_identity() {
        let pixels = solid(4, 4, 10, 20, 30);
        let out = bilateral_filter_rgba8(&pixels, 4, 4, 0, 3.0, 30.0);
        assert_eq!(out, pixels);
    }

    #[test]
    fn uniform_image_stays_uniform() {
        let pixels = solid(9, 9, 128, 64, 200);
        let out = bilateral_filter_rgba8(&pixels, 9, 9, DEFAULT_RADIUS, 3.0, 30.0);
        assert_eq!(out, pixels);
    }

    #[test]
    fn smooths_a_mild_noisy_pixel_among_uniform_neighbours() {
        // Ein moderater Ausreißer (Differenz 50 bei `range_sigma = 30`,
        // also innerhalb der Gauß-Glocke des Range-Terms) — ein starker
        // Ausreißer (z. B. Differenz 150) würde der Filter absichtlich
        // *nicht* glätten (er behandelt ihn dann wie eine echte Kante,
        // die kantenerhaltende Eigenschaft dieses Algorithmus greift
        // gerade deshalb).
        let mut pixels = solid(9, 9, 100, 100, 100);
        let center = ((4 * 9 + 4) * 4) as usize;
        pixels[center] = 150;
        let out = bilateral_filter_rgba8(&pixels, 9, 9, DEFAULT_RADIUS, 3.0, 30.0);
        assert!(
            out[center] < 150,
            "der Ausreißer muss zur Nachbarschaft hin geglättet werden"
        );
        assert!(out[center] > 100, "aber nicht vollständig verschwinden");
    }

    #[test]
    fn preserves_a_sharp_edge_better_than_a_uniform_average_would() {
        // Linke Hälfte dunkel, rechte Hälfte hell — eine harte Kante.
        let width = 10u32;
        let height = 4u32;
        let mut pixels = vec![0u8; (width * height * 4) as usize];
        for y in 0..height {
            for x in 0..width {
                let value = if x < width / 2 { 20 } else { 220 };
                let index = ((y * width + x) * 4) as usize;
                pixels[index] = value;
                pixels[index + 1] = value;
                pixels[index + 2] = value;
                pixels[index + 3] = 255;
            }
        }
        let out = bilateral_filter_rgba8(&pixels, width, height, DEFAULT_RADIUS, 3.0, 15.0);
        // Ein Pixel direkt an der Kante (x=4, dunkle Seite) soll deutlich
        // dunkler bleiben als der einfache Mittelwert beider Seiten (120) —
        // der Range-Term unterdrückt den Beitrag der hellen Nachbarn.
        let index = ((width + 4) * 4) as usize;
        assert!(
            out[index] < 120,
            "Kante darf nicht auf den globalen Mittelwert geglättet werden"
        );
    }

    #[test]
    fn alpha_channel_is_preserved() {
        let mut pixels = solid(3, 3, 10, 20, 30);
        pixels[3] = 128; // Alpha des ersten Pixels
        let out = bilateral_filter_rgba8(&pixels, 3, 3, 1, 3.0, 30.0);
        assert_eq!(out[3], 128);
    }
}
