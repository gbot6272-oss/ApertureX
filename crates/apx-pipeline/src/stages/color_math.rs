//! Gemeinsame Farbraum-Hilfsfunktionen für die Werkzeuge, die auf
//! Farbton/Sättigung/Luminanz statt auf rohen Kanalwerten rechnen —
//! aktuell [`super::hsl_color_mixer`] und [`super::color_grading`]. Kein
//! eigenes Werkzeug/Regler, daher kein eigener WGSL-Shader: die
//! GPU-Seite jedes Aufrufers bekommt ihre eigene, nach demselben Muster
//! geschriebene WGSL-Fassung (`include_str!`-Shader in dieser Codebase
//! können keine WGSL-Module importieren, siehe `gpu/dispatch.rs`).

/// Kürzester Abstand zweier Farbton-Winkel in Grad (`0..=180`), unter
/// Berücksichtigung des Umlaufs bei 360°/0°.
pub(crate) fn circular_distance_degrees(a: f32, b: f32) -> f32 {
    let diff = (a - b).abs() % 360.0;
    diff.min(360.0 - diff)
}

/// Gauß-Gewichtung nach Abstand — Grundlage der bandgewichteten
/// Verschiebungen in HSL/Farbmischer/Color-Grading.
pub(crate) fn gaussian_weight(distance: f32, sigma: f32) -> f32 {
    (-(distance * distance) / (2.0 * sigma * sigma)).exp()
}

/// `r`/`g`/`b` in `0.0..=1.0` → `(Farbton in Grad, Sättigung, Luminanz)`,
/// alle standardmäßigen HSL-Definitionen.
pub(crate) fn rgb_to_hsl(r: f32, g: f32, b: f32) -> (f32, f32, f32) {
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

/// Kehrfunktion zu [`rgb_to_hsl`].
pub(crate) fn hsl_to_rgb(h_degrees: f32, s: f32, l: f32) -> (f32, f32, f32) {
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

/// `r`/`g`/`b` in `0.0..=1.0` → `(Farbton in 0.0..6.0, Sättigung, Wert)` —
/// **HSV**, nicht HSL (siehe [`rgb_to_hsl`] für die andernorts in diesem
/// Projekt genutzte Variante). Der Farbton läuft bewusst `0.0..6.0` statt
/// in Grad — dieselbe „Hexcone"-Parametrisierung, die Adobes DNG-SDK
/// selbst für seine `ProfileHueSatMapData`-Tabellen nutzt (siehe
/// [`super::super::dcp_profile`]s Moduldoku); diese Funktion ist eine
/// direkte Portierung von `DNG_RGBtoHSV` (`dng_utils.h`, Adobe DNG SDK,
/// BSD-artig lizenziert, gegen den echten Quelltext auf GitHub verifiziert
/// — nicht aus dem Gedächtnis nachgebaut) statt der andernorts in diesem
/// Projekt üblichen HSL-Formel, weil Phase 13 Schritt 3s DCP-HueSatMap-
/// Anwendung exakt in diesem Farbraum/dieser Parametrisierung arbeiten
/// muss, um mit echten Adobe-Profilen numerisch übereinzustimmen.
pub(crate) fn rgb_to_hsv6(r: f32, g: f32, b: f32) -> (f32, f32, f32) {
    let v = r.max(g).max(b);
    let gap = v - r.min(g).min(b);
    if gap <= 0.0 {
        return (0.0, 0.0, v);
    }
    let h = if r == v {
        let h = (g - b) / gap;
        if h < 0.0 {
            h + 6.0
        } else {
            h
        }
    } else if g == v {
        2.0 + (b - r) / gap
    } else {
        4.0 + (r - g) / gap
    };
    (h, gap / v, v)
}

/// Kehrfunktion zu [`rgb_to_hsv6`] — ebenfalls eine direkte Portierung
/// von Adobes `DNG_HSVtoRGB`.
pub(crate) fn hsv6_to_rgb(h: f32, s: f32, v: f32) -> (f32, f32, f32) {
    if s <= 0.0 {
        return (v, v, v);
    }
    let h = h.rem_euclid(6.0);
    let i = h as i32;
    let f = h - i as f32;
    let p = v * (1.0 - s);
    let q = v * (1.0 - s * f);
    let t = v * (1.0 - s * (1.0 - f));
    match i {
        0 => (v, t, p),
        1 => (q, v, p),
        2 => (p, v, t),
        3 => (p, q, v),
        4 => (t, p, v),
        _ => (v, p, q),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gray_pixels_have_zero_saturation() {
        let (_, s, l) = rgb_to_hsl(0.5, 0.5, 0.5);
        assert_eq!(s, 0.0);
        assert_eq!(l, 0.5);
    }

    #[test]
    fn pure_red_is_hue_zero_full_saturation() {
        let (h, s, l) = rgb_to_hsl(1.0, 0.0, 0.0);
        assert!((h - 0.0).abs() < 1e-3);
        assert!((s - 1.0).abs() < 1e-3);
        assert!((l - 0.5).abs() < 1e-3);
    }

    #[test]
    fn hsl_to_rgb_and_back_roundtrips() {
        for hue in [0.0, 45.0, 120.0, 200.0, 300.0] {
            let (r, g, b) = hsl_to_rgb(hue, 0.6, 0.4);
            let (h2, s2, l2) = rgb_to_hsl(r, g, b);
            assert!(
                circular_distance_degrees(hue, h2) < 0.5,
                "hue={hue} h2={h2}"
            );
            assert!((s2 - 0.6).abs() < 1e-3, "s2={s2}");
            assert!((l2 - 0.4).abs() < 1e-3, "l2={l2}");
        }
    }

    #[test]
    fn gaussian_weight_is_one_at_zero_distance_and_decays() {
        assert!((gaussian_weight(0.0, 25.0) - 1.0).abs() < 1e-6);
        assert!(gaussian_weight(50.0, 25.0) < gaussian_weight(10.0, 25.0));
    }

    #[test]
    fn circular_distance_wraps_around_360() {
        assert!((circular_distance_degrees(5.0, 355.0) - 10.0).abs() < 1e-3);
    }

    #[test]
    fn hsv6_to_rgb_and_back_roundtrips() {
        for hue in [0.0, 1.5, 3.0, 4.5, 5.9] {
            let (r, g, b) = hsv6_to_rgb(hue, 0.6, 0.4);
            let (h2, s2, v2) = rgb_to_hsv6(r, g, b);
            let hue_dist = (hue - h2).abs().min(6.0 - (hue - h2).abs());
            assert!(hue_dist < 0.01, "hue={hue} h2={h2}");
            assert!((s2 - 0.6).abs() < 1e-3, "s2={s2}");
            assert!((v2 - 0.4).abs() < 1e-3, "v2={v2}");
        }
    }

    #[test]
    fn pure_red_is_hsv6_hue_zero_full_saturation_and_value() {
        let (h, s, v) = rgb_to_hsv6(1.0, 0.0, 0.0);
        assert!((h - 0.0).abs() < 1e-3);
        assert!((s - 1.0).abs() < 1e-3);
        assert!((v - 1.0).abs() < 1e-3);
    }
}
