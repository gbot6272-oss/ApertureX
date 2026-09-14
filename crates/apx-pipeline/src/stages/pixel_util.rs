//! Kleine, stufenübergreifende Pixel-Helfer.
//!
//! Diese sechs Funktionen lagen bis Phase 28 privat in
//! `stages::creative`. Mit `stages::light_optics` kam eine zweite Stufe
//! dazu, die genau dieselben Umwandlungen braucht (RGBA8 ↔ planares
//! f32-RGB, bilineares Lesen aus einer Karte anderer Auflösung). Sie
//! ein zweites Mal zu schreiben hätte zwei Fassungen erzeugt, die
//! auseinanderlaufen können — deshalb hier einmal, von beiden Stufen
//! genutzt.
//!
//! Bewusst **kein** öffentliches Crate-API (`pub(crate)`): das sind
//! Implementierungsdetails der Stufen, keine Zusage nach außen.

/// Luminanz-Gewichtung, projektweit dieselbe (auch `builtin_luts` und
/// `stages::creative` rechnen so) — eine Konstante statt mehrerer
/// leicht abweichender.
pub(crate) fn luminance(r: f32, g: f32, b: f32) -> f32 {
    0.3 * r + 0.59 * g + 0.11 * b
}

pub(crate) fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// Weiche Stufenfunktion — überall dort verwendet, wo ein harter
/// Schwellwert sichtbare Kanten erzeugen würde.
pub(crate) fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    if (edge1 - edge0).abs() < 1e-6 {
        return if x < edge0 { 0.0 } else { 1.0 };
    }
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// RGBA8 → planares RGB `0.0..=1.0` (drei Werte je Pixel). Der
/// Alphakanal bleibt unangetastet und wird von [`write_back`] wieder
/// eingesetzt.
pub(crate) fn to_rgb_f32(rgba: &[u8]) -> Vec<f32> {
    let n = rgba.len() / 4;
    let mut out = vec![0.0f32; n * 3];
    for i in 0..n {
        out[i * 3] = rgba[i * 4] as f32 / 255.0;
        out[i * 3 + 1] = rgba[i * 4 + 1] as f32 / 255.0;
        out[i * 3 + 2] = rgba[i * 4 + 2] as f32 / 255.0;
    }
    out
}

pub(crate) fn write_back(rgb: &[f32], rgba: &mut [u8]) {
    let n = rgba.len() / 4;
    for i in 0..n {
        for c in 0..3 {
            rgba[i * 4 + c] = (rgb[i * 3 + c].clamp(0.0, 1.0) * 255.0).round() as u8;
        }
    }
}

/// Bilinear aus einer Ein-Kanal-Karte (Tiefenkarte/Alphamaske) lesen,
/// die in einer anderen Auflösung vorliegt als das Bild — dieselbe
/// „Karte einmal berechnen, beim Rendern skalieren"-Konvention wie
/// `stages::virtual_aperture`.
pub(crate) fn sample_map(map: &[u8], map_w: u32, map_h: u32, u: f32, v: f32) -> f32 {
    if map_w == 0 || map_h == 0 || map.is_empty() {
        return 0.0;
    }
    let x = (u * (map_w as f32 - 1.0)).clamp(0.0, map_w as f32 - 1.0);
    let y = (v * (map_h as f32 - 1.0)).clamp(0.0, map_h as f32 - 1.0);
    let x0 = x.floor() as usize;
    let y0 = y.floor() as usize;
    let x1 = (x0 + 1).min(map_w as usize - 1);
    let y1 = (y0 + 1).min(map_h as usize - 1);
    let fx = x - x0 as f32;
    let fy = y - y0 as f32;
    let at = |px: usize, py: usize| -> f32 {
        map.get(py * map_w as usize + px).copied().unwrap_or(0) as f32 / 255.0
    };
    let top = lerp(at(x0, y0), at(x1, y0), fx);
    let bottom = lerp(at(x0, y1), at(x1, y1), fx);
    lerp(top, bottom, fy)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rgba_roundtrip_is_lossless_for_byte_values() {
        let rgba = vec![0u8, 64, 128, 255, 255, 200, 7, 255];
        let rgb = to_rgb_f32(&rgba);
        let mut back = rgba.clone();
        write_back(&rgb, &mut back);
        assert_eq!(back, rgba, "Hin- und Rückweg muss byte-genau sein");
    }

    #[test]
    fn sample_map_interpolates_between_neighbours() {
        // 2x1-Karte: links 0, rechts 255 — die Mitte muss ~0.5 ergeben.
        let map = [0u8, 255u8];
        assert!((sample_map(&map, 2, 1, 0.5, 0.0) - 0.5).abs() < 1e-3);
        assert!((sample_map(&map, 2, 1, 0.0, 0.0) - 0.0).abs() < 1e-6);
        assert!((sample_map(&map, 2, 1, 1.0, 0.0) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn sample_map_returns_zero_for_empty_map() {
        assert_eq!(sample_map(&[], 0, 0, 0.5, 0.5), 0.0);
    }
}
