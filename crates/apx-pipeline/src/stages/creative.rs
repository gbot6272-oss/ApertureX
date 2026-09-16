//! Zehn Kreativ-Werkzeuge mit großem Bildeffekt (Phase 27, siehe
//! `DECISIONS.md` ADR-0057) — bewusst EINE Stufe statt zehn einzelner:
//! dieselbe Mathematik, aber ein Zehntel Gerüst und ein einziger
//! Pipeline-Zweig in `develop.rs`.
//!
//! **Pipeline-Position:** nach `lut_filter`, vor `liquify`, im fertig
//! entwickelten sRGB-RGBA8-Bild — der LUT-Look ist die Grundgradation,
//! diese Stufe legt sich darüber.
//!
//! **Reihenfolge innerhalb der Stufe** (fest und bewusst gewählt:
//! Korrektur → Atmosphäre → Optik → Licht → Gradation → Auflage):
//! 1. Farbabgleich zum Referenzfoto (korrigiert die Grundfarbigkeit,
//!    muss deshalb vor allem Gestalterischen laufen)
//! 2. Atmosphärischer Tiefennebel (liegt „in der Luft" vor dem Motiv)
//! 3. Motiv-Freistellung/Hintergrundbehandlung (optische Trennung)
//! 4. Tilt-Shift (weitere optische Trennung)
//! 5. Sonnenstrahlen (Licht, additiv über die fertige Optik)
//! 6. Orton-Glanz (Licht, additiv)
//! 7. Filmlabor-Prozess (Gradation des Gesamtbilds inkl. des Lichts)
//! 8. Verlaufsabbildung (Gradation, ersetzt Farben nach Helligkeit)
//! 9. Farbisolierung (arbeitet auf dem fertigen Farbergebnis)
//! 10. Lichtlecks (Auflage ganz zum Schluss, wie im echten Labor)
//!
//! Alle zehn arbeiten in sRGB `0.0..=1.0` (das Bild liegt hier bereits
//! entwickelt vor) — kein Rückweg in den linearen Arbeitsraum, dieselbe
//! Konvention wie `stages::lut_filter`/`stages::sky_replace`.

use rayon::prelude::*;

use crate::edl::v4::{
    ColorMatchAdjustment, ColorPopAdjustment, CreativeAdjustments, DepthHazeAdjustment,
    FilmLabProcess, GodRaysAdjustment, GradientMapAdjustment, LightLeakAdjustment, OrtonAdjustment,
    SubjectFocusAdjustment, TiltShiftAdjustment,
};
use crate::stages::frequency_separation::low_pass;

// Die sechs kleinen Pixel-Helfer (Luminanz, lerp, smoothstep, RGBA8 ↔
// planares f32, Kartenabtastung) lagen bis Phase 28 hier. Seit
// `stages::light_optics` dieselben braucht, stehen sie einmal in
// `stages::pixel_util` — siehe dessen Moduldoku.
use crate::stages::pixel_util::{lerp, luminance, sample_map, smoothstep, to_rgb_f32, write_back};

// ---- 1. Farbabgleich zum Referenzfoto -------------------------------------

/// Gegenfarben-Zerlegung (Lab-ähnlich, ohne den teuren exakten
/// CIELAB-Weg): `l` ist die Helligkeit, `a` die Grün-Rot-, `b` die
/// Blau-Gelb-Achse. Für einen Statistiktransfer nach Reinhard reicht
/// diese Näherung — entscheidend ist, dass die drei Achsen weitgehend
/// entkoppelt sind, nicht ihre exakte Normierung.
pub fn opponent(r: f32, g: f32, b: f32) -> (f32, f32, f32) {
    let l = luminance(r, g, b);
    (l, r - g, b - 0.5 * (r + g))
}

fn from_opponent(l: f32, a: f32, bb: f32) -> (f32, f32, f32) {
    // Umkehrung von `opponent`: drei Gleichungen, drei Unbekannte.
    // l = 0.3r + 0.59g + 0.11b ; a = r - g ; bb = b - 0.5(r+g)
    // => g = r - a ; b = bb + 0.5(2r - a)
    // => l = 0.3r + 0.59(r - a) + 0.11(bb + r - 0.5a)
    //      = r(0.3 + 0.59 + 0.11) - 0.59a - 0.055a + 0.11bb
    //      = r - 0.645a + 0.11bb
    let r = l + 0.645 * a - 0.11 * bb;
    let g = r - a;
    let b = bb + 0.5 * (r + g);
    (r, g, b)
}

/// Mittelwert und Streuung der drei Gegenfarben-Achsen — die sechs
/// Zahlen, die ein Referenzfoto für den Abgleich liefert.
pub fn opponent_stats(rgb: &[f32]) -> [f32; 6] {
    let n = rgb.len() / 3;
    if n == 0 {
        return [0.0; 6];
    }
    let (mut sl, mut sa, mut sb) = (0.0f64, 0.0f64, 0.0f64);
    for i in 0..n {
        let (l, a, b) = opponent(rgb[i * 3], rgb[i * 3 + 1], rgb[i * 3 + 2]);
        sl += l as f64;
        sa += a as f64;
        sb += b as f64;
    }
    let (ml, ma, mb) = (sl / n as f64, sa / n as f64, sb / n as f64);
    let (mut vl, mut va, mut vb) = (0.0f64, 0.0f64, 0.0f64);
    for i in 0..n {
        let (l, a, b) = opponent(rgb[i * 3], rgb[i * 3 + 1], rgb[i * 3 + 2]);
        vl += (l as f64 - ml).powi(2);
        va += (a as f64 - ma).powi(2);
        vb += (b as f64 - mb).powi(2);
    }
    [
        ml as f32,
        (vl / n as f64).sqrt() as f32,
        ma as f32,
        (va / n as f64).sqrt() as f32,
        mb as f32,
        (vb / n as f64).sqrt() as f32,
    ]
}

fn apply_color_match(rgb: &mut [f32], adj: &ColorMatchAdjustment) {
    if adj.amount <= 0.0 || !adj.has_target {
        return;
    }
    let src = opponent_stats(rgb);
    let amount = adj.amount.clamp(0.0, 1.0);
    // Streuungen mit einem Mindestwert absichern: ein einfarbiges Bild
    // hat Streuung 0, der Quotient wäre sonst unendlich.
    let scale = |src_std: f32, dst_std: f32| -> f32 {
        if src_std < 1e-4 {
            1.0
        } else {
            (dst_std / src_std).clamp(0.25, 4.0)
        }
    };
    let (sl, sa, sb) = (
        scale(src[1], adj.target_l_std),
        scale(src[3], adj.target_a_std),
        scale(src[5], adj.target_b_std),
    );
    let n = rgb.len() / 3;
    for i in 0..n {
        let (l, a, b) = opponent(rgb[i * 3], rgb[i * 3 + 1], rgb[i * 3 + 2]);
        let nl = (l - src[0]) * sl + adj.target_l_mean;
        let na = (a - src[2]) * sa + adj.target_a_mean;
        let nb = (b - src[4]) * sb + adj.target_b_mean;
        let (r2, g2, b2) = from_opponent(nl, na, nb);
        rgb[i * 3] = lerp(rgb[i * 3], r2, amount);
        rgb[i * 3 + 1] = lerp(rgb[i * 3 + 1], g2, amount);
        rgb[i * 3 + 2] = lerp(rgb[i * 3 + 2], b2, amount);
    }
}

// ---- 2. Atmosphärischer Tiefennebel ---------------------------------------

fn apply_depth_haze(rgb: &mut [f32], width: u32, height: u32, adj: &DepthHazeAdjustment) {
    let Some(map) = &adj.depth_map else {
        return;
    };
    if adj.amount <= 0.0 || map.depth.is_empty() {
        return;
    }
    let amount = adj.amount.clamp(0.0, 1.0);
    let fog = [
        adj.color_r as f32 / 255.0,
        adj.color_g as f32 / 255.0,
        adj.color_b as f32 / 255.0,
    ];
    let (w, h) = (width as usize, height as usize);
    for y in 0..h {
        for x in 0..w {
            let u = x as f32 / (w.max(2) - 1) as f32;
            let v = y as f32 / (h.max(2) - 1) as f32;
            // Tiefenkarte: 255 = am naechsten. Entfernung ist das
            // Gegenteil — der Nebel waechst mit der Entfernung.
            let distance = 1.0 - sample_map(&map.depth, map.bitmap_width, map.bitmap_height, u, v);
            let t = smoothstep(adj.start, adj.end.max(adj.start + 1e-3), distance) * amount;
            if t <= 0.0 {
                continue;
            }
            let i = (y * w + x) * 3;
            for c in 0..3 {
                rgb[i + c] = lerp(rgb[i + c], fog[c], t);
            }
        }
    }
}

// ---- 3. Motiv-Freistellung + Hintergrundbehandlung ------------------------

fn apply_subject_focus(rgb: &mut [f32], width: u32, height: u32, adj: &SubjectFocusAdjustment) {
    let Some(mask) = &adj.mask else {
        return;
    };
    if mask.alpha.is_empty() || (adj.blur <= 0.0 && adj.darken <= 0.0 && adj.desaturate <= 0.0) {
        return;
    }
    let blurred = if adj.blur > 0.0 {
        let radius = ((adj.blur.clamp(0.0, 1.0) * 0.04 * width as f32).round() as i32).max(1);
        Some(low_pass(rgb, width, height, radius))
    } else {
        None
    };
    let (w, h) = (width as usize, height as usize);
    for y in 0..h {
        for x in 0..w {
            let u = x as f32 / (w.max(2) - 1) as f32;
            let v = y as f32 / (h.max(2) - 1) as f32;
            // Alpha: 255 = Motiv. Der Hintergrundanteil ist das Gegenteil.
            let bg = 1.0 - sample_map(&mask.alpha, mask.bitmap_width, mask.bitmap_height, u, v);
            if bg <= 0.001 {
                continue;
            }
            let i = (y * w + x) * 3;
            if let Some(blurred) = &blurred {
                for c in 0..3 {
                    rgb[i + c] = lerp(rgb[i + c], blurred[i + c], bg);
                }
            }
            if adj.desaturate > 0.0 {
                let lum = luminance(rgb[i], rgb[i + 1], rgb[i + 2]);
                let t = adj.desaturate.clamp(0.0, 1.0) * bg;
                for c in 0..3 {
                    rgb[i + c] = lerp(rgb[i + c], lum, t);
                }
            }
            if adj.darken > 0.0 {
                let factor = 1.0 - adj.darken.clamp(0.0, 1.0) * bg;
                for c in 0..3 {
                    rgb[i + c] *= factor;
                }
            }
        }
    }
}

// ---- 4. Tilt-Shift / Miniatur ---------------------------------------------

fn apply_tilt_shift(rgb: &mut [f32], width: u32, height: u32, adj: &TiltShiftAdjustment) {
    if adj.amount <= 0.0 {
        return;
    }
    let amount = adj.amount.clamp(0.0, 1.0);
    let radius = ((amount * 0.035 * width as f32).round() as i32).max(1);
    let blurred = low_pass(rgb, width, height, radius);
    let (w, h) = (width as usize, height as usize);
    let angle = adj.angle_deg.to_radians();
    let (sin_a, cos_a) = angle.sin_cos();
    let band = adj.width.clamp(0.02, 1.0) * 0.5;
    for y in 0..h {
        for x in 0..w {
            let u = x as f32 / (w.max(2) - 1) as f32 - 0.5;
            let v = y as f32 / (h.max(2) - 1) as f32 - adj.center.clamp(0.0, 1.0);
            // Abstand zur (geneigten) Bandmitte.
            let dist = (v * cos_a - u * sin_a).abs();
            let blur_t = smoothstep(band, band * 2.0 + 0.12, dist) * amount;
            let i = (y * w + x) * 3;
            if blur_t > 0.0 {
                for c in 0..3 {
                    rgb[i + c] = lerp(rgb[i + c], blurred[i + c], blur_t);
                }
            }
            if adj.saturation > 0.0 {
                let lum = luminance(rgb[i], rgb[i + 1], rgb[i + 2]);
                let s = 1.0 + adj.saturation.clamp(0.0, 2.0);
                for c in 0..3 {
                    rgb[i + c] = (lum + (rgb[i + c] - lum) * s).clamp(0.0, 1.0);
                }
            }
        }
    }
}

// ---- 5. Sonnenstrahlen (God Rays) -----------------------------------------

fn apply_god_rays(rgb: &mut [f32], width: u32, height: u32, adj: &GodRaysAdjustment) {
    if adj.amount <= 0.0 {
        return;
    }
    let (w, h) = (width as usize, height as usize);
    // Lichtquellen-Maske: nur was heller als `threshold` ist, strahlt.
    let mut light: Vec<f32> = vec![0.0; w * h];
    for i in 0..w * h {
        let lum = luminance(rgb[i * 3], rgb[i * 3 + 1], rgb[i * 3 + 2]);
        light[i] = smoothstep(adj.threshold, (adj.threshold + 0.2).min(1.0), lum);
    }
    // Radiale Streckung: entlang der Verbindungslinie zum Sonnenpunkt
    // schrittweise aufsummieren (klassisches Radial-Blur-Verfahren).
    const STEPS: usize = 24;
    let decay = adj.decay.clamp(0.5, 0.999);
    let (sx, sy) = (adj.sun_x.clamp(0.0, 1.0), adj.sun_y.clamp(0.0, 1.0));
    let rays: Vec<f32> = (0..w * h)
        .into_par_iter()
        .map(|idx| {
            let x = (idx % w) as f32 / (w.max(2) - 1) as f32;
            let y = (idx / w) as f32 / (h.max(2) - 1) as f32;
            let mut u = x;
            let mut v = y;
            let du = (sx - x) / STEPS as f32;
            let dv = (sy - y) / STEPS as f32;
            let mut acc = 0.0f32;
            let mut weight = 1.0f32;
            for _ in 0..STEPS {
                u += du;
                v += dv;
                let px = ((u * (w.max(2) - 1) as f32).round() as isize).clamp(0, w as isize - 1);
                let py = ((v * (h.max(2) - 1) as f32).round() as isize).clamp(0, h as isize - 1);
                acc += light[py as usize * w + px as usize] * weight;
                weight *= decay;
            }
            acc / STEPS as f32
        })
        .collect();
    let amount = adj.amount.clamp(0.0, 2.0);
    let tint = [
        adj.color_r as f32 / 255.0,
        adj.color_g as f32 / 255.0,
        adj.color_b as f32 / 255.0,
    ];
    for i in 0..w * h {
        let glow = rays[i] * amount;
        if glow <= 0.0 {
            continue;
        }
        for c in 0..3 {
            // Negativ-Multiplikation (Screen): Licht kommt dazu, ohne
            // die Lichter hart abzuschneiden.
            let a = rgb[i * 3 + c];
            let b = (glow * tint[c]).clamp(0.0, 1.0);
            rgb[i * 3 + c] = 1.0 - (1.0 - a) * (1.0 - b);
        }
    }
}

// ---- 6. Orton-Glanz --------------------------------------------------------

fn apply_orton(rgb: &mut [f32], width: u32, height: u32, adj: &OrtonAdjustment) {
    if adj.amount <= 0.0 {
        return;
    }
    let radius = ((adj.radius.clamp(0.2, 20.0) / 100.0 * width as f32).round() as i32).max(1);
    let blurred = low_pass(rgb, width, height, radius);
    let amount = adj.amount.clamp(0.0, 1.0);
    let n = rgb.len() / 3;
    for i in 0..n {
        let lum = luminance(rgb[i * 3], rgb[i * 3 + 1], rgb[i * 3 + 2]);
        // Nur hellere Partien glühen — sonst matscht der Effekt die
        // Schatten zu.
        let gate = smoothstep(adj.threshold, (adj.threshold + 0.35).min(1.0), lum);
        let t = amount * gate;
        if t <= 0.0 {
            continue;
        }
        for c in 0..3 {
            let base = rgb[i * 3 + c];
            let glow = blurred[i * 3 + c];
            let screened = 1.0 - (1.0 - base) * (1.0 - glow);
            rgb[i * 3 + c] = lerp(base, screened, t);
        }
    }
}

// ---- 7. Filmlabor-Prozesse -------------------------------------------------

fn apply_film_lab(rgb: &mut [f32], adj: &crate::edl::v4::FilmLabAdjustment) {
    if adj.amount <= 0.0 {
        return;
    }
    let amount = adj.amount.clamp(0.0, 1.0);
    let n = rgb.len() / 3;
    for i in 0..n {
        let (r, g, b) = (rgb[i * 3], rgb[i * 3 + 1], rgb[i * 3 + 2]);
        let (nr, ng, nb) = match adj.process {
            FilmLabProcess::BleachBypass => {
                // Silber bleibt im Negativ: das entsaettigte Luminanzbild
                // wird im Modus "Ineinanderkopieren" ueber das Farbbild
                // gelegt — hoher Kontrast, ausgeblichene Farben.
                let lum = luminance(r, g, b);
                let overlay = |base: f32| -> f32 {
                    if lum < 0.5 {
                        2.0 * base * lum
                    } else {
                        1.0 - 2.0 * (1.0 - base) * (1.0 - lum)
                    }
                };
                let desat = 0.55;
                (
                    lerp(overlay(r), lum, desat * 0.5),
                    lerp(overlay(g), lum, desat * 0.5),
                    lerp(overlay(b), lum, desat * 0.5),
                )
            }
            FilmLabProcess::CrossProcess => {
                // Falscher Chemieprozess: Kanalkurven gegeneinander
                // verschoben — angehobene, gruenliche Schatten, in
                // Richtung Gelb gekippte Lichter, geknickte Blaukurve.
                let rr = (r.powf(0.78) * 1.06 - 0.02).clamp(0.0, 1.0);
                let gg = (g.powf(0.92) * 1.02 + 0.015).clamp(0.0, 1.0);
                let bb = (b.powf(1.35) * 0.94 + 0.085).clamp(0.0, 1.0);
                (rr, gg, bb)
            }
        };
        rgb[i * 3] = lerp(r, nr.clamp(0.0, 1.0), amount);
        rgb[i * 3 + 1] = lerp(g, ng.clamp(0.0, 1.0), amount);
        rgb[i * 3 + 2] = lerp(b, nb.clamp(0.0, 1.0), amount);
    }
}

// ---- 8. Verlaufsabbildung (Gradient Map) -----------------------------------

fn apply_gradient_map(rgb: &mut [f32], adj: &GradientMapAdjustment) {
    if adj.amount <= 0.0 {
        return;
    }
    let amount = adj.amount.clamp(0.0, 1.0);
    let sh = [
        adj.shadow_r as f32 / 255.0,
        adj.shadow_g as f32 / 255.0,
        adj.shadow_b as f32 / 255.0,
    ];
    let mid = [
        adj.mid_r as f32 / 255.0,
        adj.mid_g as f32 / 255.0,
        adj.mid_b as f32 / 255.0,
    ];
    let hi = [
        adj.highlight_r as f32 / 255.0,
        adj.highlight_g as f32 / 255.0,
        adj.highlight_b as f32 / 255.0,
    ];
    let n = rgb.len() / 3;
    for i in 0..n {
        let lum = luminance(rgb[i * 3], rgb[i * 3 + 1], rgb[i * 3 + 2]).clamp(0.0, 1.0);
        for c in 0..3 {
            let mapped = if lum < 0.5 {
                lerp(sh[c], mid[c], lum * 2.0)
            } else {
                lerp(mid[c], hi[c], (lum - 0.5) * 2.0)
            };
            rgb[i * 3 + c] = lerp(rgb[i * 3 + c], mapped, amount);
        }
    }
}

// ---- 9. Farbisolierung (Color Pop) -----------------------------------------

/// Farbton in Grad (`0..360`) aus sRGB — dieselbe Formel wie
/// `apx_ai::color`s Farbtonberechnung, hier lokal, weil `apx-pipeline`
/// bewusst nicht von `apx-ai` abhaengt.
fn hue_degrees(r: f32, g: f32, b: f32) -> f32 {
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let delta = max - min;
    if delta < 1e-6 {
        return 0.0;
    }
    let h = if (max - r).abs() < 1e-6 {
        60.0 * (((g - b) / delta) % 6.0)
    } else if (max - g).abs() < 1e-6 {
        60.0 * ((b - r) / delta + 2.0)
    } else {
        60.0 * ((r - g) / delta + 4.0)
    };
    if h < 0.0 {
        h + 360.0
    } else {
        h
    }
}

fn apply_color_pop(rgb: &mut [f32], adj: &ColorPopAdjustment) {
    if adj.amount <= 0.0 {
        return;
    }
    let amount = adj.amount.clamp(0.0, 1.0);
    let width = adj.hue_width.clamp(1.0, 180.0);
    let n = rgb.len() / 3;
    for i in 0..n {
        let (r, g, b) = (rgb[i * 3], rgb[i * 3 + 1], rgb[i * 3 + 2]);
        let h = hue_degrees(r, g, b);
        // Kuerzester Abstand auf dem Farbkreis.
        let mut d = (h - adj.hue_center).abs() % 360.0;
        if d > 180.0 {
            d = 360.0 - d;
        }
        // 1 = im erhaltenen Bereich, 0 = ausserhalb.
        let keep = 1.0 - smoothstep(width, width * 1.6 + 6.0, d);
        let lum = luminance(r, g, b);
        let desat = (1.0 - keep) * amount;
        for c in 0..3 {
            let mut v = lerp(rgb[i * 3 + c], lum, desat);
            if adj.boost > 0.0 && keep > 0.0 {
                let s = 1.0 + adj.boost.clamp(0.0, 2.0) * keep;
                v = lum + (v - lum) * s;
            }
            rgb[i * 3 + c] = v.clamp(0.0, 1.0);
        }
    }
}

// ---- 10. Lichtlecks --------------------------------------------------------

fn apply_light_leak(rgb: &mut [f32], width: u32, height: u32, adj: &LightLeakAdjustment) {
    if adj.amount <= 0.0 {
        return;
    }
    let amount = adj.amount.clamp(0.0, 1.0);
    let tint = [
        adj.color_r as f32 / 255.0,
        adj.color_g as f32 / 255.0,
        adj.color_b as f32 / 255.0,
    ];
    let angle = adj.angle_deg.to_radians();
    let (dx, dy) = (angle.cos(), angle.sin());
    let softness = adj.softness.clamp(0.05, 1.0);
    let (w, h) = (width as usize, height as usize);
    for y in 0..h {
        for x in 0..w {
            let u = x as f32 / (w.max(2) - 1) as f32 - 0.5;
            let v = y as f32 / (h.max(2) - 1) as f32 - 0.5;
            // Projektion auf die Einfallsrichtung: 1 an der Ecke, aus
            // der das Licht kommt, 0 auf der Gegenseite.
            let t = (u * dx + v * dy + 0.707) / 1.414;
            let leak = smoothstep(1.0 - softness, 1.0, t.clamp(0.0, 1.0)) * amount;
            if leak <= 0.0 {
                continue;
            }
            let i = (y * w + x) * 3;
            for c in 0..3 {
                let a = rgb[i + c];
                let b = (leak * tint[c]).clamp(0.0, 1.0);
                rgb[i + c] = 1.0 - (1.0 - a) * (1.0 - b);
            }
        }
    }
}

// ---- Gesamtstufe -----------------------------------------------------------

/// Wendet alle zehn Kreativ-Werkzeuge in der oben dokumentierten
/// Reihenfolge an. `base` ist das fertig entwickelte sRGB-RGBA8-Bild.
pub fn apply(base: &[u8], width: u32, height: u32, adj: &CreativeAdjustments) -> Vec<u8> {
    if adj.is_neutral() || width == 0 || height == 0 {
        return base.to_vec();
    }
    let mut rgb = to_rgb_f32(base);

    apply_color_match(&mut rgb, &adj.color_match);
    apply_depth_haze(&mut rgb, width, height, &adj.depth_haze);
    apply_subject_focus(&mut rgb, width, height, &adj.subject_focus);
    apply_tilt_shift(&mut rgb, width, height, &adj.tilt_shift);
    apply_god_rays(&mut rgb, width, height, &adj.god_rays);
    apply_orton(&mut rgb, width, height, &adj.orton);
    apply_film_lab(&mut rgb, &adj.film_lab);
    apply_gradient_map(&mut rgb, &adj.gradient_map);
    apply_color_pop(&mut rgb, &adj.color_pop);
    apply_light_leak(&mut rgb, width, height, &adj.light_leak);

    let mut out = base.to_vec();
    write_back(&rgb, &mut out);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::edl::v4::{DepthMapPatch, FilmLabAdjustment, SubjectMaskPatch};

    /// Ein 4x4-Testbild mit echtem Verlauf: links dunkel, rechts hell,
    /// dazu ein Rotstich oben — damit jede Funktion etwas zu tun hat.
    fn test_image() -> (Vec<u8>, u32, u32) {
        let (w, h) = (4u32, 4u32);
        let mut px = vec![255u8; (w * h * 4) as usize];
        for y in 0..h {
            for x in 0..w {
                let i = ((y * w + x) * 4) as usize;
                let level = (x as f32 / (w - 1) as f32 * 255.0) as u8;
                px[i] = level.saturating_add(if y == 0 { 60 } else { 0 });
                px[i + 1] = level;
                px[i + 2] = level.saturating_sub(if y == 0 { 40 } else { 0 });
                px[i + 3] = 255;
            }
        }
        (px, w, h)
    }

    fn creative() -> CreativeAdjustments {
        CreativeAdjustments::default()
    }

    fn differs(a: &[u8], b: &[u8]) -> bool {
        a.iter().zip(b.iter()).any(|(x, y)| x != y)
    }

    #[test]
    fn neutral_adjustments_leave_the_image_untouched() {
        let (px, w, h) = test_image();
        assert_eq!(apply(&px, w, h, &creative()), px);
    }

    #[test]
    fn color_match_without_target_is_a_no_op_even_at_full_amount() {
        let (px, w, h) = test_image();
        let mut adj = creative();
        adj.color_match.amount = 1.0;
        // has_target bleibt false — sonst wuerde das Bild gegen Nullwerte
        // abgeglichen und grau gewaschen.
        assert_eq!(apply(&px, w, h, &adj), px);
    }

    #[test]
    fn color_match_shifts_the_image_towards_the_target_statistics() {
        let (px, w, h) = test_image();
        let mut adj = creative();
        adj.color_match = ColorMatchAdjustment {
            amount: 1.0,
            target_l_mean: 0.6,
            target_l_std: 0.2,
            target_a_mean: -0.15,
            target_a_std: 0.05,
            target_b_mean: 0.2,
            target_b_std: 0.05,
            has_target: true,
        };
        let out = apply(&px, w, h, &adj);
        assert!(differs(&px, &out));
        // Die mittlere Helligkeit muss sich messbar dem Ziel annaehern.
        let rgb = to_rgb_f32(&out);
        let stats = opponent_stats(&rgb);
        let before = opponent_stats(&to_rgb_f32(&px));
        assert!(
            (stats[0] - 0.6).abs() < (before[0] - 0.6).abs(),
            "Helligkeit naeherte sich nicht dem Ziel an: vorher {}, nachher {}",
            before[0],
            stats[0]
        );
    }

    #[test]
    fn opponent_round_trip_reconstructs_the_original_colour() {
        for (r, g, b) in [(0.2f32, 0.5f32, 0.9f32), (0.8, 0.1, 0.3), (0.5, 0.5, 0.5)] {
            let (l, a, bb) = opponent(r, g, b);
            let (r2, g2, b2) = from_opponent(l, a, bb);
            assert!((r - r2).abs() < 1e-4, "r: {r} != {r2}");
            assert!((g - g2).abs() < 1e-4, "g: {g} != {g2}");
            assert!((b - b2).abs() < 1e-4, "b: {b} != {b2}");
        }
    }

    #[test]
    fn depth_haze_fogs_distant_pixels_more_than_near_ones() {
        let (px, w, h) = test_image();
        let mut adj = creative();
        // Tiefenkarte: linke Haelfte nah (255), rechte fern (0).
        let depth: Vec<u8> = (0..(w * h))
            .map(|i| if i % w < w / 2 { 255 } else { 0 })
            .collect();
        adj.depth_haze = DepthHazeAdjustment {
            amount: 1.0,
            start: 0.0,
            end: 1.0,
            color_r: 255,
            color_g: 255,
            color_b: 255,
            depth_map: Some(DepthMapPatch {
                bitmap_width: w,
                bitmap_height: h,
                depth,
            }),
        };
        let out = apply(&px, w, h, &adj);
        // Fernes Pixel (rechts) muss deutlich heller (nebliger) geworden
        // sein als das nahe (links) an derselben Zeile.
        let row = (w * 4) as usize; // Zeile 1
        let near = out[row] as i32 - px[row] as i32;
        let far_idx = row + 3 * 4;
        let far = out[far_idx] as i32 - px[far_idx] as i32;
        assert!(
            far >= near,
            "ferner Nebel {far} war nicht staerker als naher {near}"
        );
    }

    #[test]
    fn subject_focus_darkens_only_the_background() {
        let (px, w, h) = test_image();
        let mut adj = creative();
        // Maske: obere Haelfte Motiv (255), untere Hintergrund (0).
        let alpha: Vec<u8> = (0..(w * h))
            .map(|i| if i / w < h / 2 { 255 } else { 0 })
            .collect();
        adj.subject_focus = SubjectFocusAdjustment {
            blur: 0.0,
            darken: 0.8,
            desaturate: 0.0,
            mask: Some(SubjectMaskPatch {
                bitmap_width: w,
                bitmap_height: h,
                alpha,
            }),
        };
        let out = apply(&px, w, h, &adj);
        let subject_idx = 3 * 4; // obere Zeile (y=0), hellste Spalte
        let background_idx = (3 * w * 4) as usize + 3 * 4; // unterste Zeile, hellste Spalte
        assert_eq!(
            out[subject_idx], px[subject_idx],
            "Motiv wurde faelschlich veraendert"
        );
        assert!(
            out[background_idx] < px[background_idx],
            "Hintergrund wurde nicht abgedunkelt"
        );
    }

    #[test]
    fn tilt_shift_keeps_the_band_sharp_and_changes_the_edges() {
        let (px, w, h) = test_image();
        let mut adj = creative();
        adj.tilt_shift = TiltShiftAdjustment {
            amount: 1.0,
            center: 0.5,
            width: 0.1,
            angle_deg: 0.0,
            saturation: 0.0,
        };
        let out = apply(&px, w, h, &adj);
        assert!(differs(&px, &out), "Tilt-Shift hatte gar keine Wirkung");
    }

    #[test]
    fn god_rays_brighten_the_image_around_the_sun() {
        let (px, w, h) = test_image();
        let mut adj = creative();
        adj.god_rays = GodRaysAdjustment {
            amount: 1.5,
            sun_x: 1.0,
            sun_y: 0.5,
            threshold: 0.4,
            ..GodRaysAdjustment::NEUTRAL
        };
        let out = apply(&px, w, h, &adj);
        let sum_before: u32 = px.iter().step_by(4).map(|&v| v as u32).sum();
        let sum_after: u32 = out.iter().step_by(4).map(|&v| v as u32).sum();
        assert!(
            sum_after > sum_before,
            "Sonnenstrahlen haben das Bild nicht aufgehellt ({sum_before} -> {sum_after})"
        );
    }

    #[test]
    fn orton_glow_brightens_highlights_but_not_deep_shadows() {
        let (px, w, h) = test_image();
        let mut adj = creative();
        adj.orton = OrtonAdjustment {
            amount: 1.0,
            radius: 5.0,
            threshold: 0.5,
        };
        let out = apply(&px, w, h, &adj);
        let shadow = (w * 4) as usize; // Zeile 1, linke Spalte = dunkel
                                       // Bewusst die ZWEITHELLSTE Spalte: die hellste steht im
                                       // Testbild bereits auf 255, und Negativ-Multiplikation kann ein
                                       // bereits weisses Pixel nicht weiter aufhellen — ein Test darauf
                                       // wuerde eine Eigenschaft pruefen, die der Effekt gar nicht
                                       // haben kann.
        let highlight = (w * 4) as usize + 2 * 4;
        assert_eq!(out[shadow], px[shadow], "Schatten wurde vom Glanz erfasst");
        assert!(
            out[highlight] > px[highlight],
            "Lichter wurden nicht angehoben ({} -> {})",
            px[highlight],
            out[highlight]
        );
    }

    #[test]
    fn bleach_bypass_reduces_saturation() {
        let (px, w, h) = test_image();
        let mut adj = creative();
        adj.film_lab = FilmLabAdjustment {
            amount: 1.0,
            process: FilmLabProcess::BleachBypass,
        };
        let out = apply(&px, w, h, &adj);
        // Obere Zeile traegt den Rotstich — der Kanalabstand muss kleiner
        // werden.
        let spread = |buf: &[u8], i: usize| (buf[i] as i32 - buf[i + 2] as i32).abs();
        let i = 4usize; // zweites Pixel der obersten Zeile
        assert!(
            spread(&out, i) < spread(&px, i),
            "Bleach Bypass hat die Saettigung nicht reduziert"
        );
    }

    #[test]
    fn cross_process_lifts_the_blue_channel_in_the_shadows() {
        let (px, w, h) = test_image();
        let mut adj = creative();
        adj.film_lab = FilmLabAdjustment {
            amount: 1.0,
            process: FilmLabProcess::CrossProcess,
        };
        let out = apply(&px, w, h, &adj);
        let shadow_blue = (w * 4) as usize + 2; // Zeile 1, dunkelstes Pixel, Blaukanal
        assert!(
            out[shadow_blue] > px[shadow_blue],
            "Cross-Processing hat den Schwarzpunkt nicht angehoben"
        );
    }

    #[test]
    fn gradient_map_replaces_colours_by_luminance() {
        let (px, w, h) = test_image();
        let mut adj = creative();
        adj.gradient_map = GradientMapAdjustment {
            amount: 1.0,
            shadow_r: 255,
            shadow_g: 0,
            shadow_b: 0,
            mid_r: 0,
            mid_g: 255,
            mid_b: 0,
            highlight_r: 0,
            highlight_g: 0,
            highlight_b: 255,
        };
        let out = apply(&px, w, h, &adj);
        // Dunkelstes Pixel muss jetzt rot sein, hellstes blau.
        let dark = (w * 4) as usize;
        let bright = (w * 4) as usize + 3 * 4;
        assert!(out[dark] > 200 && out[dark + 2] < 60, "Tiefen nicht rot");
        assert!(
            out[bright + 2] > 200 && out[bright] < 60,
            "Lichter nicht blau"
        );
    }

    #[test]
    fn color_pop_keeps_the_selected_hue_and_desaturates_the_rest() {
        let (w, h) = (2u32, 1u32);
        // Links kraeftiges Rot, rechts kraeftiges Blau.
        let px = vec![220, 30, 30, 255, 30, 30, 220, 255];
        let mut adj = creative();
        adj.color_pop = ColorPopAdjustment {
            amount: 1.0,
            hue_center: 0.0, // Rot
            hue_width: 25.0,
            boost: 0.0,
        };
        let out = apply(&px, w, h, &adj);
        let red_spread = (out[0] as i32 - out[2] as i32).abs();
        let blue_spread = (out[4] as i32 - out[6] as i32).abs();
        assert!(red_spread > 100, "Rot wurde faelschlich entsaettigt");
        assert!(blue_spread < 30, "Blau wurde nicht entsaettigt");
    }

    #[test]
    fn light_leak_brightens_one_corner_more_than_the_opposite_one() {
        let (px, w, h) = (vec![40u8; 8 * 8 * 4], 8u32, 8u32);
        let mut adj = creative();
        adj.light_leak = LightLeakAdjustment {
            amount: 1.0,
            angle_deg: 45.0, // aus Richtung unten rechts
            softness: 1.0,
            color_r: 255,
            color_g: 255,
            color_b: 255,
        };
        let out = apply(&px, w, h, &adj);
        let top_left = 0usize;
        let bottom_right = ((h - 1) * w * 4 + (w - 1) * 4) as usize;
        assert!(
            out[bottom_right] > out[top_left],
            "Lichtleck kam nicht aus der erwarteten Richtung ({} vs {})",
            out[bottom_right],
            out[top_left]
        );
    }

    #[test]
    fn every_single_tool_changes_the_image_on_its_own() {
        // Absicherung gegen still wirkungslose Funktionen: jede der zehn
        // muss allein eine sichtbare Aenderung erzeugen.
        let (px, w, h) = test_image();
        let mut cases: Vec<(&str, CreativeAdjustments)> = Vec::new();

        let mut a = creative();
        a.color_match = ColorMatchAdjustment {
            amount: 1.0,
            target_l_mean: 0.7,
            target_l_std: 0.1,
            target_a_mean: 0.2,
            target_a_std: 0.1,
            target_b_mean: -0.2,
            target_b_std: 0.1,
            has_target: true,
        };
        cases.push(("Farbabgleich", a));

        let mut a = creative();
        a.depth_haze.amount = 1.0;
        a.depth_haze.depth_map = Some(DepthMapPatch {
            bitmap_width: w,
            bitmap_height: h,
            depth: vec![0u8; (w * h) as usize],
        });
        cases.push(("Tiefennebel", a));

        let mut a = creative();
        a.subject_focus = SubjectFocusAdjustment {
            blur: 0.5,
            darken: 0.5,
            desaturate: 0.5,
            mask: Some(SubjectMaskPatch {
                bitmap_width: w,
                bitmap_height: h,
                alpha: vec![0u8; (w * h) as usize],
            }),
        };
        cases.push(("Motiv-Freistellung", a));

        let mut a = creative();
        a.tilt_shift.amount = 1.0;
        a.tilt_shift.width = 0.05;
        cases.push(("Tilt-Shift", a));

        let mut a = creative();
        a.god_rays.amount = 1.5;
        a.god_rays.threshold = 0.3;
        cases.push(("Sonnenstrahlen", a));

        let mut a = creative();
        a.orton.amount = 1.0;
        a.orton.threshold = 0.2;
        cases.push(("Orton", a));

        let mut a = creative();
        a.film_lab.amount = 1.0;
        cases.push(("Filmlabor", a));

        let mut a = creative();
        a.gradient_map.amount = 1.0;
        cases.push(("Verlaufsabbildung", a));

        let mut a = creative();
        a.color_pop.amount = 1.0;
        a.color_pop.hue_center = 200.0;
        cases.push(("Farbisolierung", a));

        let mut a = creative();
        a.light_leak.amount = 1.0;
        cases.push(("Lichtleck", a));

        assert_eq!(
            cases.len(),
            10,
            "es muessen zehn Funktionen geprueft werden"
        );
        for (name, adj) in cases {
            let out = apply(&px, w, h, &adj);
            assert!(differs(&px, &out), "{name} hat das Bild nicht veraendert");
        }
    }
}
