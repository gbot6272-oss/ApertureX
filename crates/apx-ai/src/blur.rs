//! Weichzeichnung für Ein-Kanal-`f32`-Raster (Saliency-/Kontrastkarten,
//! Sensorflecken-Erkennung) — ein dreifach wiederholter Box-Filter statt
//! eines echten Gauß-Kerns (dieselbe Näherung, die auch reale
//! Bildbearbeitungsbibliotheken für schnelle Gauß-Approximation
//! verwenden: drei Box-Filter derselben Breite konvergieren gegen eine
//! Gauß-Glocke, siehe z. B. Kovesi 2010).

/// Ein horizontaler und vertikaler Box-Filter-Durchlauf (Radius in
/// Pixeln, `0` = No-op) über ein `width * height` großes Ein-Kanal-Raster
/// — Randpixel klemmen (das nächste innerhalb des Bildes liegende Pixel
/// wird wiederholt) statt umzuwickeln oder mit Null aufzufüllen, sonst
/// würde der Bildrand künstlich abgedunkelt.
pub fn box_blur(data: &[f32], width: u32, height: u32, radius: u32) -> Vec<f32> {
    if radius == 0 || width == 0 || height == 0 {
        return data.to_vec();
    }
    let horizontal = box_blur_1d(data, width, height, radius, true);
    box_blur_1d(&horizontal, width, height, radius, false)
}

fn box_blur_1d(data: &[f32], width: u32, height: u32, radius: u32, horizontal: bool) -> Vec<f32> {
    let w = width as i64;
    let h = height as i64;
    let r = radius as i64;
    let mut out = vec![0.0f32; data.len()];

    for y in 0..h {
        for x in 0..w {
            let mut sum = 0.0f32;
            let mut count = 0.0f32;
            for offset in -r..=r {
                let (sx, sy) = if horizontal {
                    ((x + offset).clamp(0, w - 1), y)
                } else {
                    (x, (y + offset).clamp(0, h - 1))
                };
                sum += data[(sy * w + sx) as usize];
                count += 1.0;
            }
            out[(y * w + x) as usize] = sum / count;
        }
    }
    out
}

/// Dreifacher Box-Filter — die übliche Gauß-Approximation (siehe
/// Moduldoku). `radius` ist die Box-Breite je Durchlauf, nicht die
/// Gesamt-Unschärfe.
pub fn approximate_gaussian_blur(data: &[f32], width: u32, height: u32, radius: u32) -> Vec<f32> {
    let once = box_blur(data, width, height, radius);
    let twice = box_blur(&once, width, height, radius);
    box_blur(&twice, width, height, radius)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_radius_is_identity() {
        let data = vec![0.1, 0.9, 0.3, 0.7];
        let blurred = box_blur(&data, 2, 2, 0);
        assert_eq!(blurred, data);
    }

    #[test]
    fn uniform_input_stays_uniform() {
        let data = vec![0.5f32; 25];
        let blurred = box_blur(&data, 5, 5, 1);
        for v in blurred {
            assert!((v - 0.5).abs() < 1e-6);
        }
    }

    #[test]
    fn blur_smooths_a_single_bright_pixel_into_its_neighbours() {
        // 7×7 statt 3×3: bei einem 3×3-Bild mit Radius 1 erreicht die
        // Rand-Klemmung jeden Pixel gleich stark, das Ergebnis wäre
        // gleichmäßig und der Test damit aussagelos.
        let width = 7u32;
        let height = 7u32;
        let center = (height / 2 * width + width / 2) as usize;
        let mut data = vec![0.0f32; (width * height) as usize];
        data[center] = 1.0;

        let blurred = box_blur(&data, width, height, 1);
        // Energie ist auf die Nachbarn verteilt …
        assert!(blurred[center] < 1.0);
        // … der Mittelpunkt bleibt aber heller als eine weit entfernte Ecke.
        assert!(blurred[center] > blurred[0]);
    }
}
