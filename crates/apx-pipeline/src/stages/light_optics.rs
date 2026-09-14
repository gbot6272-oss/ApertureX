//! Zwölf Werkzeuge für Licht, Tiefe und Optik (Phase 28, siehe
//! `DECISIONS.md` ADR-0058) — wie `stages::creative` bewusst EINE Stufe
//! statt zwölf einzelner: ein EDL-Feld, ein Modul, ein Flag, ein
//! Pipeline-Zweig.
//!
//! **Pipeline-Position:** nach `sky_replace`, **vor** `lut_filter`.
//! Korrekturen (Tonwert, Zonen, Detail) und optische Phänomene (Bokeh,
//! Blendenstern, Diffusion, Bewegungsunschärfe) sind Dinge, die an der
//! *Kamera* passieren und deshalb der Gradation vorausgehen. Die
//! Phase-27-Looks (`stages::creative`) liegen weiterhin danach, wie im
//! Labor.
//!
//! **Reihenfolge innerhalb der Stufe** (fest und bewusst gewählt:
//! Korrektur → Tiefe → Licht → Optik → Stil):
//! 1. Tonwert-Angleich an ein Referenzfoto (globale Tonwertkorrektur)
//! 2. Zonensystem (lokale Tonwertkorrektur)
//! 3. Detail-Pyramide (Detailkorrektur)
//! 4. Tiefenselektive Dunstentfernung
//! 5. Tiefenselektive Schärfe
//! 6. KI-Neubeleuchtung
//! 7. Himmel dramatisieren
//! 8. Bewegungsunschärfe
//! 9. Blendenstern
//! 10. Diffusionsfilter („Pro Mist")
//! 11. Kanalmatrix / Infrarot
//! 12. Poster-/Comic-Look (quantisiert alles davor, muss deshalb zuletzt)
//!
//! Alle zwölf arbeiten in sRGB `0.0..=1.0` (das Bild liegt hier bereits
//! entwickelt vor) — kein Rückweg in den linearen Arbeitsraum, dieselbe
//! Konvention wie `stages::lut_filter`/`stages::creative`.

use rayon::prelude::*;

use crate::edl::v4::{
    ChannelMatrixAdjustment, DepthDehazeAdjustment, DepthSharpenAdjustment,
    DetailPyramidAdjustment, DiffusionAdjustment, LightOpticsAdjustments, MotionBlurAdjustment,
    MotionBlurKind, PosterizeAdjustment, RelightAdjustment, SkyDramaAdjustment,
    StarFilterAdjustment, ToneMatchAdjustment, ZoneSystemAdjustment,
};
use crate::stages::frequency_separation::low_pass;
use crate::stages::pixel_util::{lerp, luminance, sample_map, smoothstep, to_rgb_f32, write_back};

// ---- Gemeinsame Helfer dieser Stufe ---------------------------------------

/// Luminanzkarte (ein Wert je Pixel) aus planarem RGB.
fn luma_map(rgb: &[f32]) -> Vec<f32> {
    rgb.chunks_exact(3)
        .map(|p| luminance(p[0], p[1], p[2]))
        .collect()
}

/// Separables Kastenmittel über eine Ein-Kanal-Karte. Gleitendes
/// Fenster statt Neuaufsummierung je Pixel: `O(w*h)` statt
/// `O(w*h*radius)` — bei den hier üblichen Radien (bis ~64 px) der
/// Unterschied zwischen flüssig und unbrauchbar.
fn box_mean(src: &[f32], w: usize, h: usize, radius: usize) -> Vec<f32> {
    if radius == 0 || w == 0 || h == 0 {
        return src.to_vec();
    }
    let mut tmp = vec![0.0f32; w * h];
    let window = (radius * 2 + 1) as f32;
    for y in 0..h {
        let row = y * w;
        let mut sum = 0.0f32;
        // Fenster für x = 0 aufbauen (Ränder werden geklemmt fortgesetzt).
        for k in 0..=radius {
            sum += src[row + k.min(w - 1)];
        }
        sum += src[row] * radius as f32;
        for x in 0..w {
            tmp[row + x] = sum / window;
            let add = src[row + (x + radius + 1).min(w - 1)];
            let sub = src[row + x.saturating_sub(radius)];
            sum += add - sub;
        }
    }
    let mut out = vec![0.0f32; w * h];
    for x in 0..w {
        let mut sum = 0.0f32;
        for k in 0..=radius {
            sum += tmp[k.min(h - 1) * w + x];
        }
        sum += tmp[x] * radius as f32;
        for y in 0..h {
            out[y * w + x] = sum / window;
            let add = tmp[(y + radius + 1).min(h - 1) * w + x];
            let sub = tmp[y.saturating_sub(radius) * w + x];
            sum += add - sub;
        }
    }
    out
}

/// Kastenminimum über eine Ein-Kanal-Karte — der „dunkle Kanal" der
/// Dunstentfernung (He et al., Dark Channel Prior).
fn box_min(src: &[f32], w: usize, h: usize, radius: usize) -> Vec<f32> {
    if radius == 0 || w == 0 || h == 0 {
        return src.to_vec();
    }
    let r = radius as isize;
    let mut tmp = vec![0.0f32; w * h];
    for y in 0..h {
        for x in 0..w {
            let mut m = f32::MAX;
            for k in -r..=r {
                let xx = (x as isize + k).clamp(0, w as isize - 1) as usize;
                m = m.min(src[y * w + xx]);
            }
            tmp[y * w + x] = m;
        }
    }
    let mut out = vec![0.0f32; w * h];
    for y in 0..h {
        for x in 0..w {
            let mut m = f32::MAX;
            for k in -r..=r {
                let yy = (y as isize + k).clamp(0, h as isize - 1) as usize;
                m = m.min(tmp[yy * w + x]);
            }
            out[y * w + x] = m;
        }
    }
    out
}

/// Guided Filter (He, Sun, Tang) — glättet `p` entlang der Kanten von
/// `guide`.
///
/// Das ist der Grund, warum das Zonensystem überhaupt brauchbar ist:
/// ein gewöhnlicher Weichzeichner über die Verstärkungskarte erzeugt an
/// harten Hell-Dunkel-Kanten genau die Lichtsäume, für die
/// Zonenwerkzeuge berüchtigt sind. Der Guided Filter berechnet je
/// Fenster eine lineare Abbildung `q = a*I + b` aus Kovarianz und
/// Varianz und folgt damit den Kanten des Führungsbilds.
fn guided_filter(
    guide: &[f32],
    p: &[f32],
    w: usize,
    h: usize,
    radius: usize,
    eps: f32,
) -> Vec<f32> {
    let mean_i = box_mean(guide, w, h, radius);
    let mean_p = box_mean(p, w, h, radius);
    let ii: Vec<f32> = guide.iter().map(|v| v * v).collect();
    let ip: Vec<f32> = guide.iter().zip(p).map(|(a, b)| a * b).collect();
    let corr_i = box_mean(&ii, w, h, radius);
    let corr_ip = box_mean(&ip, w, h, radius);

    let mut a = vec![0.0f32; w * h];
    let mut b = vec![0.0f32; w * h];
    for i in 0..w * h {
        let var_i = corr_i[i] - mean_i[i] * mean_i[i];
        let cov_ip = corr_ip[i] - mean_i[i] * mean_p[i];
        a[i] = cov_ip / (var_i + eps);
        b[i] = mean_p[i] - a[i] * mean_i[i];
    }
    let mean_a = box_mean(&a, w, h, radius);
    let mean_b = box_mean(&b, w, h, radius);
    (0..w * h)
        .map(|i| mean_a[i] * guide[i] + mean_b[i])
        .collect()
}

/// Bilineares Lesen eines RGB-Tripels aus planarem RGB.
fn sample_rgb(rgb: &[f32], w: usize, h: usize, x: f32, y: f32) -> [f32; 3] {
    let x = x.clamp(0.0, w as f32 - 1.0);
    let y = y.clamp(0.0, h as f32 - 1.0);
    let x0 = x.floor() as usize;
    let y0 = y.floor() as usize;
    let x1 = (x0 + 1).min(w - 1);
    let y1 = (y0 + 1).min(h - 1);
    let fx = x - x0 as f32;
    let fy = y - y0 as f32;
    let mut out = [0.0f32; 3];
    for (c, slot) in out.iter_mut().enumerate() {
        let tl = rgb[(y0 * w + x0) * 3 + c];
        let tr = rgb[(y0 * w + x1) * 3 + c];
        let bl = rgb[(y1 * w + x0) * 3 + c];
        let br = rgb[(y1 * w + x1) * 3 + c];
        *slot = lerp(lerp(tl, tr, fx), lerp(bl, br, fx), fy);
    }
    out
}

/// Skaliert eine Farbe so, dass ihre Luminanz `target` wird — hält also
/// den Farbton fest und ändert nur die Helligkeit. Überall dort
/// verwendet, wo ein Werkzeug eine Tonwertabbildung auf die Luminanz
/// rechnet (Tonwert-Angleich, Zonensystem) und die Farbe nicht
/// mitkippen soll.
fn retarget_luma(px: &mut [f32], target: f32) {
    let l = luminance(px[0], px[1], px[2]);
    if l <= 1e-4 {
        // Nahezu schwarz: es gibt kein Farbverhältnis, das sich halten
        // ließe — additiv aufhellen statt durch ~0 zu teilen.
        for c in px.iter_mut().take(3) {
            *c += target;
        }
        return;
    }
    let scale = (target / l).clamp(0.0, 8.0);
    for c in px.iter_mut().take(3) {
        *c *= scale;
    }
}

// ---- 1. Tonwert-Angleich an ein Referenzfoto ------------------------------

/// Die neun eigenen Luminanz-Dezile (10 %, 20 %, …, 90 %) aus einem
/// 256-Bin-Histogramm.
fn own_deciles(rgb: &[f32]) -> [f32; 9] {
    let mut hist = [0u32; 256];
    for p in rgb.chunks_exact(3) {
        let l = luminance(p[0], p[1], p[2]).clamp(0.0, 1.0);
        hist[(l * 255.0).round() as usize] += 1;
    }
    let total: u32 = hist.iter().sum();
    let mut out = [0.0f32; 9];
    if total == 0 {
        return out;
    }
    let mut cumulative = 0u32;
    let mut next = 0usize;
    for (bin, count) in hist.iter().enumerate() {
        cumulative += count;
        while next < 9 && cumulative as f32 / total as f32 >= (next + 1) as f32 * 0.1 {
            out[next] = bin as f32 / 255.0;
            next += 1;
        }
        if next >= 9 {
            break;
        }
    }
    for slot in out.iter_mut().skip(next) {
        *slot = 1.0;
    }
    out
}

/// Stückweise lineare Abbildung durch `(0,0)`, die neun Stützstellen und
/// `(1,1)`. Beide Achsen werden vorher streng monoton gemacht — sonst
/// könnte die Abbildung fallen und das Bild stellenweise invertieren.
fn tone_curve(from: &[f32; 9], to: &[f32; 9], x: f32) -> f32 {
    let mut xs = [0.0f32; 11];
    let mut ys = [0.0f32; 11];
    xs[10] = 1.0;
    ys[10] = 1.0;
    for i in 0..9 {
        xs[i + 1] = from[i].clamp(0.0, 1.0).max(xs[i] + 1e-4);
        ys[i + 1] = to[i].clamp(0.0, 1.0).max(ys[i]);
    }
    xs[10] = xs[10].max(xs[9] + 1e-4);
    ys[10] = ys[10].max(ys[9]);
    let x = x.clamp(0.0, 1.0);
    for i in 0..10 {
        if x <= xs[i + 1] {
            let t = (x - xs[i]) / (xs[i + 1] - xs[i]).max(1e-6);
            return lerp(ys[i], ys[i + 1], t.clamp(0.0, 1.0));
        }
    }
    ys[10]
}

fn apply_tone_match(rgb: &mut [f32], adj: &ToneMatchAdjustment) {
    if adj.amount <= 0.0 || !adj.has_target {
        return;
    }
    let amount = adj.amount.clamp(0.0, 1.0);
    let own = own_deciles(rgb);
    for p in rgb.chunks_exact_mut(3) {
        let l = luminance(p[0], p[1], p[2]);
        let mapped = tone_curve(&own, &adj.targets, l);
        retarget_luma(p, lerp(l, mapped, amount));
    }
}

// ---- 2. Zonensystem -------------------------------------------------------

/// Dreiecks-Zugehörigkeit einer Luminanz zu Zone `z` (von zehn) — jede
/// Zone reicht zur Hälfte in ihre Nachbarn hinein, sodass die Summe
/// aller zehn Gewichte überall 1 ergibt und keine Stufen entstehen.
fn zone_weight(l: f32, z: usize) -> f32 {
    let center = z as f32 / 9.0;
    let d = (l - center).abs() * 9.0;
    (1.0 - d).max(0.0)
}

fn apply_zone_system(rgb: &mut [f32], width: u32, height: u32, adj: &ZoneSystemAdjustment) {
    if adj.amount <= 0.0 || adj.zones.iter().all(|z| *z == 0.0) {
        return;
    }
    let (w, h) = (width as usize, height as usize);
    let amount = adj.amount.clamp(0.0, 1.0);
    let luma = luma_map(rgb);

    // Rohe EV-Karte aus den Zonengewichten …
    let ev: Vec<f32> = luma
        .iter()
        .map(|l| {
            let l = l.clamp(0.0, 1.0);
            (0..10).map(|z| zone_weight(l, z) * adj.zones[z]).sum()
        })
        .collect();

    // … und derselbe Wert kantenbewusst geglättet. Ohne diesen Schritt
    // säumt jede harte Hell-Dunkel-Kante hell auf.
    let radius = (adj.edge_radius.clamp(1.0, 128.0)) as usize;
    let smoothed = guided_filter(&luma, &ev, w, h, radius, 0.01);

    for (i, p) in rgb.chunks_exact_mut(3).enumerate() {
        let gain = (smoothed[i] * amount).exp2();
        let l = luminance(p[0], p[1], p[2]);
        retarget_luma(p, (l * gain).clamp(0.0, 1.0));
    }
}

// ---- 3. Detail-Pyramide ---------------------------------------------------

fn apply_detail_pyramid(rgb: &mut [f32], width: u32, height: u32, adj: &DetailPyramidAdjustment) {
    if adj.amount <= 0.0 || (adj.fine == 0.0 && adj.medium == 0.0 && adj.coarse == 0.0) {
        return;
    }
    let amount = adj.amount.clamp(0.0, 1.0);
    let base = width.min(height) as f32;
    let r1 = (base * 0.004).round().max(1.0) as i32;
    let r2 = (r1 * 4).max(2);
    let r3 = (r1 * 16).max(4);

    let b1 = low_pass(rgb, width, height, r1);
    let b2 = low_pass(&b1, width, height, r2);
    let b3 = low_pass(&b2, width, height, r3);

    let gf = 1.0 + adj.fine.clamp(-1.0, 1.0);
    let gm = 1.0 + adj.medium.clamp(-1.0, 1.0);
    let gc = 1.0 + adj.coarse.clamp(-1.0, 1.0);

    for i in 0..rgb.len() {
        let fine = rgb[i] - b1[i];
        let medium = b1[i] - b2[i];
        let coarse = b2[i] - b3[i];
        let rebuilt = b3[i] + coarse * gc + medium * gm + fine * gf;
        rgb[i] = lerp(rgb[i], rebuilt, amount);
    }
}

// ---- 4. Tiefenselektive Dunstentfernung -----------------------------------

fn apply_depth_dehaze(rgb: &mut [f32], width: u32, height: u32, adj: &DepthDehazeAdjustment) {
    let Some(map) = &adj.depth_map else {
        return;
    };
    if adj.amount <= 0.0 || map.depth.is_empty() {
        return;
    }
    let (w, h) = (width as usize, height as usize);
    let amount = adj.amount.clamp(0.0, 1.0);

    // Dark Channel Prior (He, Sun, Tang): in einem dunstfreien
    // Außenbild ist in fast jedem kleinen Fenster mindestens ein Kanal
    // nahe Null. Wo das nicht gilt, liegt Dunst.
    let dark_raw: Vec<f32> = rgb
        .chunks_exact(3)
        .map(|p| p[0].min(p[1]).min(p[2]))
        .collect();
    let patch = (w.min(h) / 50).clamp(2, 20);
    let dark = box_min(&dark_raw, w, h, patch);

    // Atmosphärisches Licht: der Mittelwert der hellsten 0,1 % des
    // dunklen Kanals — robuster als das einzelne hellste Pixel, das ein
    // Sensorausreißer sein kann.
    let mut sorted = dark.clone();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let cut_index = ((sorted.len() as f32 * 0.999) as usize).min(sorted.len() - 1);
    let cut = sorted[cut_index];
    let mut air = [0.0f32; 3];
    let mut count = 0u32;
    for (i, p) in rgb.chunks_exact(3).enumerate() {
        if dark[i] >= cut {
            for c in 0..3 {
                air[c] += p[c];
            }
            count += 1;
        }
    }
    if count == 0 {
        return;
    }
    for c in air.iter_mut() {
        *c = (*c / count as f32).clamp(0.3, 1.0);
    }
    let air_luma = luminance(air[0], air[1], air[2]).max(0.3);

    for y in 0..h {
        for x in 0..w {
            let i = y * w + x;
            let u = x as f32 / (w.max(2) - 1) as f32;
            let v = y as f32 / (h.max(2) - 1) as f32;
            // Tiefenkarte: 255 = am nächsten, Entfernung ist das Gegenteil.
            let distance = 1.0 - sample_map(&map.depth, map.bitmap_width, map.bitmap_height, u, v);
            let t = smoothstep(adj.start, adj.end.max(adj.start + 1e-3), distance) * amount;
            if t <= 0.0 {
                continue;
            }
            // Transmission: je mehr dunkler Kanal, desto mehr Dunst.
            let transmission = (1.0 - 0.95 * (dark[i] / air_luma)).max(0.1);
            for c in 0..3 {
                let recovered = (rgb[i * 3 + c] - air[c]) / transmission + air[c];
                rgb[i * 3 + c] = lerp(rgb[i * 3 + c], recovered.clamp(0.0, 1.0), t);
            }
        }
    }
}

// ---- 5. Tiefenselektive Schärfe -------------------------------------------

fn apply_depth_sharpen(rgb: &mut [f32], width: u32, height: u32, adj: &DepthSharpenAdjustment) {
    let Some(map) = &adj.depth_map else {
        return;
    };
    if adj.amount <= 0.0 || map.depth.is_empty() {
        return;
    }
    let (w, h) = (width as usize, height as usize);
    let radius = adj.radius.clamp(0.5, 16.0).round().max(1.0) as i32;
    let blurred = low_pass(rgb, width, height, radius);
    let amount = adj.amount.clamp(0.0, 1.0);
    let range = adj.range.clamp(0.02, 1.0);

    for y in 0..h {
        for x in 0..w {
            let i = y * w + x;
            let u = x as f32 / (w.max(2) - 1) as f32;
            let v = y as f32 / (h.max(2) - 1) as f32;
            let depth = sample_map(&map.depth, map.bitmap_width, map.bitmap_height, u, v);
            let distance = (depth - adj.focus_depth).abs() / range;
            let weight = (1.0 - smoothstep(0.0, 1.0, distance)) * amount * 1.8;
            if weight <= 0.0 {
                continue;
            }
            for c in 0..3 {
                let detail = rgb[i * 3 + c] - blurred[i * 3 + c];
                rgb[i * 3 + c] = (rgb[i * 3 + c] + detail * weight).clamp(0.0, 1.0);
            }
        }
    }
}

// ---- 6. KI-Neubeleuchtung -------------------------------------------------

fn apply_relight(rgb: &mut [f32], width: u32, height: u32, adj: &RelightAdjustment) {
    let Some(map) = &adj.depth_map else {
        return;
    };
    if adj.amount <= 0.0 || map.depth.is_empty() {
        return;
    }
    let (w, h) = (width as usize, height as usize);
    let amount = adj.amount.clamp(0.0, 1.0);

    // Tiefenkarte auf Bildauflösung bringen und leicht glätten — auf der
    // rohen Karte wäre der Gradient reines Rauschen.
    let mut depth = vec![0.0f32; w * h];
    for y in 0..h {
        for x in 0..w {
            let u = x as f32 / (w.max(2) - 1) as f32;
            let v = y as f32 / (h.max(2) - 1) as f32;
            depth[y * w + x] = sample_map(&map.depth, map.bitmap_width, map.bitmap_height, u, v);
        }
    }
    let depth = box_mean(&depth, w, h, 2);

    // Reliefstärke: der Gradient benachbarter Pixel liegt bei typischen
    // Motiven im Bereich 0,001–0,01. Damit die Normale überhaupt kippt,
    // wird er mit der Bildgröße skaliert (bei 1000 px Kantenlänge also
    // Faktor 80) — empirisch der Bereich, in dem die Beleuchtung
    // plastisch wirkt, ohne dass jede Tiefenstufe als Kante aufblitzt.
    let relief = (w.min(h) as f32 * 0.08).max(8.0);

    for y in 0..h {
        for x in 0..w {
            let i = y * w + x;
            let xm = x.saturating_sub(1);
            let xp = (x + 1).min(w - 1);
            let ym = y.saturating_sub(1);
            let yp = (y + 1).min(h - 1);
            let dzdx = (depth[y * w + xp] - depth[y * w + xm]) * 0.5 * relief;
            let dzdy = (depth[yp * w + x] - depth[ym * w + x]) * 0.5 * relief;
            let nl = (dzdx * dzdx + dzdy * dzdy + 1.0).sqrt();
            let n = [-dzdx / nl, -dzdy / nl, 1.0 / nl];

            let u = x as f32 / (w.max(2) - 1) as f32;
            let v = y as f32 / (h.max(2) - 1) as f32;
            let mut lv = [adj.light_x - u, adj.light_y - v, adj.light_z.max(0.05)];
            let ll = (lv[0] * lv[0] + lv[1] * lv[1] + lv[2] * lv[2])
                .sqrt()
                .max(1e-6);
            for c in lv.iter_mut() {
                *c /= ll;
            }

            let diffuse = (n[0] * lv[0] + n[1] * lv[1] + n[2] * lv[2]).max(0.0);
            // `diffuse == 0.5` ist bewusst der neutrale Punkt: die Stufe
            // kann damit sowohl aufhellen als auch abdunkeln, statt das
            // Bild grundsätzlich dunkler zu machen.
            let shade = (adj.ambient + (1.0 - adj.ambient) * 2.0 * diffuse).clamp(0.0, 2.5);

            let mut hv = [lv[0], lv[1], lv[2] + 1.0];
            let hl = (hv[0] * hv[0] + hv[1] * hv[1] + hv[2] * hv[2])
                .sqrt()
                .max(1e-6);
            for c in hv.iter_mut() {
                *c /= hl;
            }
            let spec = (n[0] * hv[0] + n[1] * hv[1] + n[2] * hv[2])
                .max(0.0)
                .powf(40.0)
                * adj.specular.clamp(0.0, 1.0);

            for c in 0..3 {
                let lit = rgb[i * 3 + c] * shade * adj.color_rgb[c] + spec * adj.color_rgb[c];
                rgb[i * 3 + c] = lerp(rgb[i * 3 + c], lit.clamp(0.0, 1.0), amount);
            }
        }
    }
}

// ---- 7. Himmel dramatisieren ----------------------------------------------

fn apply_sky_drama(rgb: &mut [f32], width: u32, height: u32, adj: &SkyDramaAdjustment) {
    let Some(mask) = &adj.mask else {
        return;
    };
    if adj.amount <= 0.0 || mask.alpha.is_empty() {
        return;
    }
    let (w, h) = (width as usize, height as usize);
    let amount = adj.amount.clamp(0.0, 1.0);

    for y in 0..h {
        for x in 0..w {
            let u = x as f32 / (w.max(2) - 1) as f32;
            let v = y as f32 / (h.max(2) - 1) as f32;
            let a = sample_map(&mask.alpha, mask.bitmap_width, mask.bitmap_height, u, v) * amount;
            if a <= 0.001 {
                continue;
            }
            let i = (y * w + x) * 3;
            let px = [rgb[i], rgb[i + 1], rgb[i + 2]];
            let l = luminance(px[0], px[1], px[2]);

            // Kontrast um 0,55 statt um 0,5: ein Himmel liegt im Mittel
            // heller als das Gesamtbild, ein Drehpunkt bei 0,5 würde ihn
            // pauschal aufhellen statt ihn zu zeichnen.
            let mut nl = (l - 0.55) * (1.0 + adj.contrast.clamp(0.0, 1.0) * 1.2) + 0.55;
            nl *= 1.0 - adj.darken.clamp(0.0, 1.0) * 0.45;

            let mut out = px;
            retarget_luma(&mut out, lerp(l, nl.clamp(0.0, 1.0), 1.0));

            let mean = (out[0] + out[1] + out[2]) / 3.0;
            let sat = 1.0 + adj.saturation.clamp(-1.0, 1.0) * 1.2;
            for c in out.iter_mut() {
                *c = mean + (*c - mean) * sat;
            }
            let warm = adj.warmth.clamp(-1.0, 1.0) * 0.15;
            out[0] *= 1.0 + warm;
            out[2] *= 1.0 - warm;

            for c in 0..3 {
                rgb[i + c] = lerp(px[c], out[c].clamp(0.0, 1.0), a);
            }
        }
    }
}

// ---- 8. Bewegungsunschärfe ------------------------------------------------

fn apply_motion_blur(rgb: &mut [f32], width: u32, height: u32, adj: &MotionBlurAdjustment) {
    if adj.amount <= 0.0 || adj.length <= 0.0 {
        return;
    }
    let (w, h) = (width as usize, height as usize);
    if w < 2 || h < 2 {
        return;
    }
    let amount = adj.amount.clamp(0.0, 1.0);
    let min_edge = w.min(h) as f32;
    let len_px = (adj.length.clamp(0.0, 1.0) * min_edge).max(1.0);
    let taps = (len_px.round() as usize).clamp(3, 48);
    let src = rgb.to_vec();
    let angle = adj.angle.to_radians();
    let (dir_x, dir_y) = (angle.cos(), angle.sin());
    let cx = adj.center_x * (w - 1) as f32;
    let cy = adj.center_y * (h - 1) as f32;
    let mask = adj.mask.as_ref();

    rgb.par_chunks_mut(w * 3).enumerate().for_each(|(y, row)| {
        for x in 0..w {
            let px = x as f32;
            let py = y as f32;
            let mut acc = [0.0f32; 3];
            for k in 0..taps {
                let t = k as f32 / (taps - 1) as f32 - 0.5;
                let (sx, sy) = match adj.kind {
                    MotionBlurKind::Directional => {
                        (px + dir_x * len_px * t, py + dir_y * len_px * t)
                    }
                    MotionBlurKind::Zoom => {
                        let scale = 1.0 + t * adj.length.clamp(0.0, 1.0) * 2.0;
                        (cx + (px - cx) * scale, cy + (py - cy) * scale)
                    }
                    MotionBlurKind::Radial => {
                        let theta = t * adj.length.clamp(0.0, 1.0) * 2.0;
                        let (dx, dy) = (px - cx, py - cy);
                        (
                            cx + dx * theta.cos() - dy * theta.sin(),
                            cy + dx * theta.sin() + dy * theta.cos(),
                        )
                    }
                };
                let s = sample_rgb(&src, w, h, sx, sy);
                for c in 0..3 {
                    acc[c] += s[c];
                }
            }
            let inv = 1.0 / taps as f32;
            // Motivmaske: `255` = Motiv, das scharf bleiben soll —
            // ohne diesen Schritt verwischt der Mitzieher auch das
            // Motiv, das ja gerade scharf sein soll.
            let keep = match mask {
                Some(m) if !m.alpha.is_empty() => sample_map(
                    &m.alpha,
                    m.bitmap_width,
                    m.bitmap_height,
                    x as f32 / (w - 1) as f32,
                    y as f32 / (h - 1) as f32,
                ),
                _ => 0.0,
            };
            for c in 0..3 {
                let blurred = lerp(acc[c] * inv, src[(y * w + x) * 3 + c], keep);
                row[x * 3 + c] = lerp(src[(y * w + x) * 3 + c], blurred, amount);
            }
        }
    });
}

// ---- 9. Blendenstern ------------------------------------------------------

fn apply_star_filter(rgb: &mut [f32], width: u32, height: u32, adj: &StarFilterAdjustment) {
    if adj.amount <= 0.0 || adj.length <= 0.0 {
        return;
    }
    let (w, h) = (width as usize, height as usize);
    if w < 2 || h < 2 {
        return;
    }
    let points = adj.points.clamp(2, 12) as usize;
    let threshold = adj.threshold.clamp(0.0, 0.99);

    // Nur die Spitzlichter speisen die Strahlen.
    let highlight: Vec<f32> = rgb
        .chunks_exact(3)
        .flat_map(|p| {
            let l = luminance(p[0], p[1], p[2]);
            let gate = ((l - threshold) / (1.0 - threshold)).clamp(0.0, 1.0);
            [p[0] * gate, p[1] * gate, p[2] * gate]
        })
        .collect();

    let min_edge = w.min(h) as f32;
    let len_px = (adj.length.clamp(0.0, 1.0) * min_edge).max(2.0);
    // Logarithmischer Schmier statt naiver Abtastung: fünf Durchgänge mit
    // verdoppeltem Versatz ergeben exakt die Summe von 32 exponentiell
    // abfallenden Abtastungen — 5 statt 32 Lesezugriffen je Strahl und
    // Pixel. (Dasselbe Verfahren nutzen Echtzeit-Bloom-Filter.)
    let passes = 5usize;
    let step = (len_px / 31.0).max(0.5);
    let decay = 0.88f32;

    let mut streak = vec![0.0f32; w * h * 3];
    for pi in 0..points {
        let angle = (adj.angle + 360.0 * pi as f32 / points as f32).to_radians();
        let (dx, dy) = (angle.cos(), angle.sin());
        let mut buf = highlight.clone();
        for k in 0..passes {
            let off = step * (1u32 << k) as f32;
            let dec = decay.powf((1u32 << k) as f32);
            let mut next = vec![0.0f32; w * h * 3];
            next.par_chunks_mut(3).enumerate().for_each(|(i, dst)| {
                let x = (i % w) as f32;
                let y = (i / w) as f32;
                let s = sample_rgb(&buf, w, h, x - dx * off, y - dy * off);
                for c in 0..3 {
                    dst[c] = buf[i * 3 + c] + dec * s[c];
                }
            });
            buf = next;
        }
        for i in 0..w * h * 3 {
            streak[i] += buf[i] - highlight[i];
        }
    }

    let amount = adj.amount.clamp(0.0, 1.0);
    let chroma = adj.chroma.clamp(0.0, 1.0);
    let norm = 1.0 / points as f32;
    for i in 0..w * h {
        for c in 0..3 {
            // Regenbogen-Anteil: die drei Kanäle bekommen leicht
            // unterschiedliche Gewichte (Beugung an den Blendenlamellen).
            let tint = 1.0 + chroma * (c as f32 - 1.0) * 0.5;
            rgb[i * 3 + c] =
                (rgb[i * 3 + c] + streak[i * 3 + c] * norm * amount * tint).clamp(0.0, 1.0);
        }
    }
}

// ---- 10. Diffusionsfilter („Pro Mist") ------------------------------------

fn apply_diffusion(rgb: &mut [f32], width: u32, height: u32, adj: &DiffusionAdjustment) {
    if adj.amount <= 0.0 {
        return;
    }
    let threshold = adj.threshold.clamp(0.0, 0.99);
    let glow_src: Vec<f32> = rgb
        .chunks_exact(3)
        .flat_map(|p| {
            let l = luminance(p[0], p[1], p[2]);
            let gate = ((l - threshold) / (1.0 - threshold)).clamp(0.0, 1.0);
            [p[0] * gate, p[1] * gate, p[2] * gate]
        })
        .collect();
    let radius = adj.radius.clamp(1.0, 128.0).round() as i32;
    let glow = low_pass(&glow_src, width, height, radius);

    let amount = adj.amount.clamp(0.0, 1.0);
    let retain = adj.black_retention.clamp(0.0, 1.0);
    let warm = adj.warmth.clamp(-1.0, 1.0) * 0.25;
    for i in 0..rgb.len() / 3 {
        let l = luminance(rgb[i * 3], rgb[i * 3 + 1], rgb[i * 3 + 2]);
        // Der Schwarzwert-Erhalt ist der Unterschied zum Orton-Glanz:
        // dort, wo das Original dunkel ist, wird der Schein
        // zurückgenommen, statt die Schatten milchig zu machen.
        let keep = 1.0 - retain * (1.0 - l).powf(2.0);
        for c in 0..3 {
            let tint = 1.0 + warm * (1.0 - c as f32) * 0.5;
            rgb[i * 3 + c] =
                (rgb[i * 3 + c] + glow[i * 3 + c] * amount * keep * tint).clamp(0.0, 1.0);
        }
    }
}

// ---- 11. Kanalmatrix ------------------------------------------------------

fn apply_channel_matrix(rgb: &mut [f32], adj: &ChannelMatrixAdjustment) {
    if adj.amount <= 0.0 {
        return;
    }
    let amount = adj.amount.clamp(0.0, 1.0);
    let m = &adj.matrix;
    for p in rgb.chunks_exact_mut(3) {
        let (r, g, b) = (p[0], p[1], p[2]);
        let out = [
            m[0] * r + m[1] * g + m[2] * b,
            m[3] * r + m[4] * g + m[5] * b,
            m[6] * r + m[7] * g + m[8] * b,
        ];
        for c in 0..3 {
            p[c] = lerp(p[c], out[c].clamp(0.0, 1.0), amount);
        }
    }
}

// ---- 12. Poster-/Comic-Look -----------------------------------------------

fn apply_posterize(rgb: &mut [f32], width: u32, height: u32, adj: &PosterizeAdjustment) {
    if adj.amount <= 0.0 {
        return;
    }
    let (w, h) = (width as usize, height as usize);
    let amount = adj.amount.clamp(0.0, 1.0);
    let levels = adj.levels.clamp(2, 32) as f32;
    let luma = luma_map(rgb);
    let edge_amount = adj.edge_amount.clamp(0.0, 1.0);
    let thickness = adj.edge_thickness.clamp(0.2, 5.0);

    for y in 0..h {
        for x in 0..w {
            let i = y * w + x;
            for c in 0..3 {
                let q = (rgb[i * 3 + c] * (levels - 1.0)).round() / (levels - 1.0);
                rgb[i * 3 + c] = lerp(rgb[i * 3 + c], q, amount);
            }
            if edge_amount <= 0.0 {
                continue;
            }
            // Sobel auf der ORIGINAL-Luminanz: auf der quantisierten
            // würde jede Stufengrenze als Kontur erscheinen, auch mitten
            // in einem glatten Verlauf.
            let xm = x.saturating_sub(1);
            let xp = (x + 1).min(w - 1);
            let ym = y.saturating_sub(1);
            let yp = (y + 1).min(h - 1);
            let at = |px: usize, py: usize| luma[py * w + px];
            let gx = at(xp, ym) + 2.0 * at(xp, y) + at(xp, yp)
                - at(xm, ym)
                - 2.0 * at(xm, y)
                - at(xm, yp);
            let gy = at(xm, yp) + 2.0 * at(x, yp) + at(xp, yp)
                - at(xm, ym)
                - 2.0 * at(x, ym)
                - at(xp, ym);
            let mag = ((gx * gx + gy * gy).sqrt() * thickness).clamp(0.0, 1.0);
            let ink = 1.0 - mag * edge_amount * amount;
            for c in 0..3 {
                rgb[i * 3 + c] *= ink;
            }
        }
    }
}

// ---- Einstiegspunkt -------------------------------------------------------

/// Wendet die zwölf Werkzeuge in der oben dokumentierten festen
/// Reihenfolge auf ein fertig entwickeltes sRGB-RGBA8-Bild an.
///
/// Der Aufrufer (`develop.rs`) ruft die Stufe nur auf, wenn
/// [`LightOpticsAdjustments::is_neutral`] `false` ergibt; jedes einzelne
/// Werkzeug prüft zusätzlich seinen eigenen `amount` und ist sonst ein
/// No-Op.
pub fn apply(base: &[u8], width: u32, height: u32, adj: &LightOpticsAdjustments) -> Vec<u8> {
    let mut out = base.to_vec();
    let mut rgb = to_rgb_f32(base);

    apply_tone_match(&mut rgb, &adj.tone_match);
    apply_zone_system(&mut rgb, width, height, &adj.zone_system);
    apply_detail_pyramid(&mut rgb, width, height, &adj.detail_pyramid);
    apply_depth_dehaze(&mut rgb, width, height, &adj.depth_dehaze);
    apply_depth_sharpen(&mut rgb, width, height, &adj.depth_sharpen);
    apply_relight(&mut rgb, width, height, &adj.relight);
    apply_sky_drama(&mut rgb, width, height, &adj.sky_drama);
    apply_motion_blur(&mut rgb, width, height, &adj.motion_blur);
    apply_star_filter(&mut rgb, width, height, &adj.star_filter);
    apply_diffusion(&mut rgb, width, height, &adj.diffusion);
    apply_channel_matrix(&mut rgb, &adj.channel_matrix);
    apply_posterize(&mut rgb, width, height, &adj.posterize);

    write_back(&rgb, &mut out);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::edl::v4::{DepthMapPatch, SubjectMaskPatch};

    /// Ein Testbild mit echtem Inhalt: diagonaler Helligkeitsverlauf,
    /// ein heller Fleck (für Blendenstern/Diffusion) und ein dunkler
    /// Bereich (für Schwarzwert-Erhalt). Ein Vollton-Bild würde bei den
    /// meisten dieser Werkzeuge fälschlich „kein Effekt" melden.
    fn test_image(w: u32, h: u32) -> Vec<u8> {
        let mut out = vec![0u8; (w * h * 4) as usize];
        for y in 0..h {
            for x in 0..w {
                let i = ((y * w + x) * 4) as usize;
                let t = (x + y) as f32 / (w + h) as f32;
                let bright = x > w * 2 / 3 && y < h / 3;
                let v = if bright { 250.0 } else { 20.0 + t * 180.0 };
                out[i] = (v * 1.05).min(255.0) as u8;
                out[i + 1] = v as u8;
                out[i + 2] = (v * 0.85) as u8;
                out[i + 3] = 255;
            }
        }
        out
    }

    /// Verlaufs-Tiefenkarte: links fern (0), rechts nah (255).
    fn depth_patch(w: u32, h: u32) -> DepthMapPatch {
        let mut depth = vec![0u8; (w * h) as usize];
        for y in 0..h {
            for x in 0..w {
                depth[(y * w + x) as usize] = (x * 255 / w.max(2)) as u8;
            }
        }
        DepthMapPatch {
            bitmap_width: w,
            bitmap_height: h,
            depth,
        }
    }

    /// Maske: obere Hälfte voll, untere leer.
    fn half_mask(w: u32, h: u32) -> SubjectMaskPatch {
        let mut alpha = vec![0u8; (w * h) as usize];
        for y in 0..h / 2 {
            for x in 0..w {
                alpha[(y * w + x) as usize] = 255;
            }
        }
        SubjectMaskPatch {
            bitmap_width: w,
            bitmap_height: h,
            alpha,
        }
    }

    fn differs(a: &[u8], b: &[u8]) -> bool {
        a.iter().zip(b).any(|(x, y)| x != y)
    }

    #[test]
    fn neutral_adjustments_leave_the_image_untouched() {
        let base = test_image(24, 16);
        let out = apply(&base, 24, 16, &LightOpticsAdjustments::default());
        assert_eq!(out, base, "neutrale Einstellungen dürfen nichts ändern");
    }

    #[test]
    fn default_adjustments_report_neutral() {
        assert!(LightOpticsAdjustments::default().is_neutral());
    }

    /// Der wichtigste Test der Phase: JEDES der zwölf Werkzeuge muss für
    /// sich allein das Bild real verändern. Ein Werkzeug, das nur in
    /// der Oberfläche existiert, fällt hier durch.
    #[test]
    fn each_of_the_twelve_tools_changes_the_image_on_its_own() {
        let (w, h) = (48u32, 32u32);
        let base = test_image(w, h);

        let mut cases: Vec<(&str, LightOpticsAdjustments)> = Vec::new();

        let mut a = LightOpticsAdjustments::default();
        a.tone_match.amount = 1.0;
        a.tone_match.has_target = true;
        a.tone_match.targets = [0.05, 0.1, 0.15, 0.2, 0.3, 0.45, 0.6, 0.8, 0.95];
        cases.push(("tone_match", a));

        let mut a = LightOpticsAdjustments::default();
        a.zone_system.amount = 1.0;
        a.zone_system.zones = [-1.0, -0.8, -0.5, 0.0, 0.0, 0.0, 0.5, 0.8, 1.0, 1.0];
        cases.push(("zone_system", a));

        let mut a = LightOpticsAdjustments::default();
        a.detail_pyramid.amount = 1.0;
        a.detail_pyramid.fine = 1.0;
        a.detail_pyramid.coarse = -0.5;
        cases.push(("detail_pyramid", a));

        let mut a = LightOpticsAdjustments::default();
        a.depth_dehaze.amount = 1.0;
        a.depth_dehaze.depth_map = Some(depth_patch(w, h));
        cases.push(("depth_dehaze", a));

        let mut a = LightOpticsAdjustments::default();
        a.depth_sharpen.amount = 1.0;
        a.depth_sharpen.depth_map = Some(depth_patch(w, h));
        cases.push(("depth_sharpen", a));

        let mut a = LightOpticsAdjustments::default();
        a.relight.amount = 1.0;
        a.relight.depth_map = Some(depth_patch(w, h));
        cases.push(("relight", a));

        let mut a = LightOpticsAdjustments::default();
        a.sky_drama.amount = 1.0;
        a.sky_drama.mask = Some(half_mask(w, h));
        cases.push(("sky_drama", a));

        let mut a = LightOpticsAdjustments::default();
        a.motion_blur.amount = 1.0;
        a.motion_blur.length = 0.2;
        cases.push(("motion_blur", a));

        let mut a = LightOpticsAdjustments::default();
        a.star_filter.amount = 1.0;
        a.star_filter.length = 0.3;
        cases.push(("star_filter", a));

        let mut a = LightOpticsAdjustments::default();
        a.diffusion.amount = 1.0;
        cases.push(("diffusion", a));

        let mut a = LightOpticsAdjustments::default();
        a.channel_matrix.amount = 1.0;
        a.channel_matrix.matrix = [0.0, 0.0, 1.0, 0.0, 1.0, 0.0, 1.0, 0.0, 0.0];
        cases.push(("channel_matrix", a));

        let mut a = LightOpticsAdjustments::default();
        a.posterize.amount = 1.0;
        a.posterize.levels = 3;
        cases.push(("posterize", a));

        assert_eq!(cases.len(), 12, "alle zwölf Werkzeuge müssen geprüft sein");
        for (name, adj) in cases {
            assert!(!adj.is_neutral(), "{name} müsste als aktiv gelten");
            let out = apply(&base, w, h, &adj);
            assert!(differs(&out, &base), "{name} hat das Bild nicht verändert");
        }
    }

    #[test]
    fn tone_curve_is_monotone_and_hits_its_anchors() {
        let from = [0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9];
        let to = [0.05, 0.1, 0.2, 0.3, 0.5, 0.7, 0.8, 0.9, 0.95];
        let mut previous = -1.0;
        for step in 0..=100 {
            let x = step as f32 / 100.0;
            let y = tone_curve(&from, &to, x);
            assert!(y >= previous - 1e-4, "Kurve fällt bei x={x}");
            previous = y;
        }
        for i in 0..9 {
            assert!(
                (tone_curve(&from, &to, from[i]) - to[i]).abs() < 1e-3,
                "Stützstelle {i} wird nicht getroffen"
            );
        }
    }

    /// Eine absteigende Zielreihe darf die Abbildung nicht kippen lassen
    /// — sonst würden Bildteile invertieren.
    #[test]
    fn tone_curve_survives_non_monotone_targets() {
        let from = [0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9];
        let to = [0.9, 0.1, 0.8, 0.2, 0.7, 0.3, 0.6, 0.4, 0.5];
        let mut previous = -1.0;
        for step in 0..=100 {
            let y = tone_curve(&from, &to, step as f32 / 100.0);
            assert!(y >= previous - 1e-4, "Kurve fällt trotz Monotonisierung");
            previous = y;
        }
    }

    #[test]
    fn zone_weights_sum_to_one_everywhere() {
        for step in 0..=50 {
            let l = step as f32 / 50.0;
            let sum: f32 = (0..10).map(|z| zone_weight(l, z)).sum();
            assert!(
                (sum - 1.0).abs() < 1e-3,
                "Zonengewichte summieren bei l={l} zu {sum} statt 1"
            );
        }
    }

    /// Der Guided Filter muss die Kante halten: ein Weichzeichner würde
    /// die Stufe verschmieren, der Guided Filter folgt ihr.
    #[test]
    fn guided_filter_preserves_a_hard_edge_that_a_blur_would_smear() {
        let (w, h) = (32usize, 8usize);
        let mut guide = vec![0.0f32; w * h];
        let mut p = vec![0.0f32; w * h];
        for y in 0..h {
            for x in 0..w {
                let bright = x >= w / 2;
                guide[y * w + x] = if bright { 1.0 } else { 0.0 };
                // Die zu glättende Karte folgt derselben Kante, ist aber
                // verrauscht.
                p[y * w + x] =
                    if bright { 1.0 } else { 0.0 } + if (x + y) % 2 == 0 { 0.05 } else { -0.05 };
            }
        }
        let guided = guided_filter(&guide, &p, w, h, 4, 0.01);
        let blurred = box_mean(&p, w, h, 4);
        let y = h / 2;
        // Direkt links der Kante muss der Guided Filter nahe 0 bleiben,
        // während das Kastenmittel dort schon deutlich angestiegen ist.
        let left = w / 2 - 1;
        assert!(
            guided[y * w + left] < 0.2,
            "Guided Filter verschmiert die Kante: {}",
            guided[y * w + left]
        );
        assert!(
            blurred[y * w + left] > 0.3,
            "Testaufbau falsch: das Kastenmittel müsste hier verschmieren, ist aber {}",
            blurred[y * w + left]
        );
    }

    #[test]
    fn box_mean_of_a_constant_field_is_that_constant() {
        let src = vec![0.42f32; 20 * 12];
        let out = box_mean(&src, 20, 12, 3);
        for v in out {
            assert!((v - 0.42).abs() < 1e-4, "Kastenmittel verschiebt Konstante");
        }
    }

    #[test]
    fn box_min_picks_the_darkest_neighbour() {
        let mut src = vec![1.0f32; 9 * 9];
        src[4 * 9 + 4] = 0.0;
        let out = box_min(&src, 9, 9, 2);
        assert_eq!(out[4 * 9 + 4], 0.0);
        assert_eq!(
            out[4 * 9 + 6],
            0.0,
            "im Radius muss das Minimum durchschlagen"
        );
        assert_eq!(out[4 * 9 + 8], 1.0, "außerhalb des Radius nicht");
    }

    /// Der Schwarzwert-Erhalt ist die Kernzusage des Diffusionsfilters:
    /// die Schatten dürfen nicht milchig werden.
    #[test]
    fn diffusion_keeps_shadows_darker_than_it_lifts_highlights() {
        let (w, h) = (32u32, 32u32);
        // Linke Hälfte tiefschwarz, rechte Hälfte hell.
        let mut base = vec![255u8; (w * h * 4) as usize];
        for y in 0..h {
            for x in 0..w / 2 {
                let i = ((y * w + x) * 4) as usize;
                base[i] = 0;
                base[i + 1] = 0;
                base[i + 2] = 0;
            }
        }
        let mut adj = LightOpticsAdjustments::default();
        adj.diffusion.amount = 1.0;
        adj.diffusion.radius = 6.0;
        adj.diffusion.black_retention = 1.0;
        let out = apply(&base, w, h, &adj);

        // Ein Pixel tief im schwarzen Bereich, aber innerhalb des
        // Glanz-Radius der Kante.
        let idx = ((h / 2 * w + (w / 2 - 3)) * 4) as usize;
        assert!(
            out[idx] < 40,
            "Schwarzwert-Erhalt greift nicht, Schatten ist auf {} gestiegen",
            out[idx]
        );
    }

    #[test]
    fn posterize_reduces_the_number_of_distinct_levels() {
        let (w, h) = (64u32, 8u32);
        let mut base = vec![255u8; (w * h * 4) as usize];
        for y in 0..h {
            for x in 0..w {
                let i = ((y * w + x) * 4) as usize;
                let v = (x * 4) as u8;
                base[i] = v;
                base[i + 1] = v;
                base[i + 2] = v;
            }
        }
        let mut adj = LightOpticsAdjustments::default();
        adj.posterize.amount = 1.0;
        adj.posterize.levels = 4;
        adj.posterize.edge_amount = 0.0;
        let out = apply(&base, w, h, &adj);

        let before: std::collections::BTreeSet<u8> = base.chunks_exact(4).map(|p| p[1]).collect();
        let after: std::collections::BTreeSet<u8> = out.chunks_exact(4).map(|p| p[1]).collect();
        assert!(
            after.len() < before.len() / 4,
            "Quantisierung greift nicht: {} statt deutlich weniger als {} Stufen",
            after.len(),
            before.len()
        );
    }

    /// Ohne Motivmaske verwischt die Bewegungsunschärfe alles; mit Maske
    /// muss das maskierte Gebiet erkennbar schärfer bleiben.
    #[test]
    fn motion_blur_mask_keeps_the_subject_sharper_than_the_background() {
        let (w, h) = (48u32, 32u32);
        let base = test_image(w, h);

        let mut without = LightOpticsAdjustments::default();
        without.motion_blur.amount = 1.0;
        without.motion_blur.length = 0.25;
        let blurred_all = apply(&base, w, h, &without);

        let mut with_mask = without.clone();
        with_mask.motion_blur.mask = Some(half_mask(w, h));
        let masked = apply(&base, w, h, &with_mask);

        let delta = |img: &[u8], y0: u32, y1: u32| -> u32 {
            let mut sum = 0u32;
            for y in y0..y1 {
                for x in 0..w {
                    let i = ((y * w + x) * 4) as usize;
                    sum += (img[i] as i32 - base[i] as i32).unsigned_abs();
                }
            }
            sum
        };
        // Obere Hälfte ist maskiert (255 = Motiv), muss also näher am
        // Original liegen als dieselbe Hälfte ohne Maske.
        assert!(
            delta(&masked, 0, h / 2) < delta(&blurred_all, 0, h / 2),
            "die Maske schützt das Motiv nicht"
        );
    }
}
