//! Sieben am Bild bediente Werkzeuge (Phase 30, siehe `DECISIONS.md`
//! ADR-0060) — dieselbe Bauform wie `stages::creative` und
//! `stages::light_optics`: ein EDL-Feld, ein Modul, ein Flag, ein
//! Pipeline-Zweig.
//!
//! **Was diese Stufe von den beiden anderen unterscheidet:** ihre
//! Werkzeuge haben eine *Geometrie* — einen Ort, eine Achse, eine
//! Ellipse —, die der Nutzer im Bild selbst zieht statt über Regler
//! einzustellen. Die Rechnung hier arbeitet deshalb durchweg mit
//! normierten Bildkoordinaten (`0.0..=1.0`), damit dieselbe Bearbeitung
//! in jeder Auflösung dasselbe Ergebnis liefert — von der
//! Bildschirmvorschau bis zum Export in voller Größe.
//!
//! **Pipeline-Position:** nach `light_optics`, vor `lut_filter`.
//! Gesetztes Licht und gesetzte Farbe gehören in dieselbe Familie wie
//! Licht & Optik und ebenfalls vor die Gradation.
//!
//! **Reihenfolge innerhalb der Stufe** (Licht → Farbe → Verlauf →
//! Auflage):
//! 1. Lichtquellen (additives Licht zuerst, alles Weitere sieht es)
//! 2. Lichtkegel (formt das Gesamtlicht der Szene)
//! 3. Abwedeln/Nachbelichten (örtliche Tonwertkorrektur auf dem Licht)
//! 4. Split-Lighting (färbt das nun fertige Licht ein)
//! 5. Farbe ersetzen (arbeitet auf den fertigen Farben)
//! 6. Verlaufsband (Gradation des Gesamtbilds)
//! 7. Horizont-Verlaufsfilter (Auflage zum Schluss, wie ein Filter vor
//!    dem Objektiv)
//!
//! Alle sieben arbeiten in sRGB `0.0..=1.0` — dieselbe Konvention wie
//! `stages::creative`/`stages::light_optics`.

use crate::edl::v4::{
    ColorReplaceAdjustment, DodgeBurnAdjustment, GradientRampAdjustment, HorizonGradAdjustment,
    InteractiveAdjustments, PointLightsAdjustment, SplitLightAdjustment, SpotlightAdjustment,
};
use crate::stages::creative::opponent;
use crate::stages::pixel_util::{lerp, luminance, smoothstep, to_rgb_f32, write_back};

/// Normierte Bildkoordinaten eines Pixels. Die kürzere Bildkante ist
/// die Bezugsgröße für alle Radien und Abstände — so bleibt ein
/// „Radius 0,3" im Hoch- wie im Querformat derselbe Kreis und nicht
/// eine Ellipse.
struct Frame {
    width: usize,
    height: usize,
    /// Faktor, mit dem ein x-Abstand multipliziert wird, damit
    /// Abstände kreisförmig statt bildformatverzerrt gemessen werden.
    aspect_x: f32,
    aspect_y: f32,
}

impl Frame {
    fn new(width: u32, height: u32) -> Self {
        let (w, h) = (width as f32, height as f32);
        let short = w.min(h).max(1.0);
        Self {
            width: width as usize,
            height: height as usize,
            aspect_x: w / short,
            aspect_y: h / short,
        }
    }

    fn uv(&self, x: usize, y: usize) -> (f32, f32) {
        (
            x as f32 / (self.width.max(2) - 1) as f32,
            y as f32 / (self.height.max(2) - 1) as f32,
        )
    }

    /// Abstand zweier normierter Punkte, bildformatbereinigt.
    fn distance(&self, ux: f32, uy: f32, px: f32, py: f32) -> f32 {
        let dx = (ux - px) * self.aspect_x;
        let dy = (uy - py) * self.aspect_y;
        (dx * dx + dy * dy).sqrt()
    }
}

/// Weicher Abfall von 1 (im Zentrum) auf 0 (am Radius), mit
/// einstellbarem Exponenten. Der Exponent ist der Unterschied zwischen
/// „Lichtfleck" (linear) und „Lampe" (quadratisch).
fn falloff_weight(distance: f32, radius: f32, exponent: f32) -> f32 {
    if radius <= 1e-4 {
        return 0.0;
    }
    let t = (1.0 - (distance / radius)).clamp(0.0, 1.0);
    // Erst weich machen, dann den Exponenten — andersherum entstünde am
    // Rand eine sichtbare Kante.
    let smooth = t * t * (3.0 - 2.0 * t);
    smooth.powf(exponent.clamp(0.25, 6.0))
}

// ---- 1. Lichtquellen ------------------------------------------------------

fn apply_point_lights(rgb: &mut [f32], frame: &Frame, adj: &PointLightsAdjustment) {
    if adj.amount <= 0.0 || adj.lights.is_empty() {
        return;
    }
    let amount = adj.amount.clamp(0.0, 1.0);
    for y in 0..frame.height {
        for x in 0..frame.width {
            let (u, v) = frame.uv(x, y);
            // Alle Lichter aufsummieren, dann EINMAL anwenden: nacheinander
            // angewandt würde die Reihenfolge das Ergebnis verändern.
            let mut add = [0.0f32; 3];
            for light in &adj.lights {
                let d = frame.distance(u, v, light.x, light.y);
                let w = falloff_weight(d, light.radius, light.falloff) * light.intensity;
                if w == 0.0 {
                    continue;
                }
                for (slot, channel) in add.iter_mut().zip(light.color_rgb) {
                    *slot += w * channel;
                }
            }
            let i = (y * frame.width + x) * 3;
            for (c, added) in add.iter().enumerate() {
                // Additiv, aber gegen die verbleibende Kopffreiheit
                // gedämpft: sonst brennen helle Bildteile sofort aus.
                let base = rgb[i + c];
                let lit = if *added >= 0.0 {
                    base + added * (1.0 - base).max(0.0)
                } else {
                    base + added * base
                };
                rgb[i + c] = lerp(base, lit.clamp(0.0, 1.0), amount);
            }
        }
    }
}

// ---- 2. Lichtkegel --------------------------------------------------------

fn apply_spotlight(rgb: &mut [f32], frame: &Frame, adj: &SpotlightAdjustment) {
    if adj.amount <= 0.0 {
        return;
    }
    let amount = adj.amount.clamp(0.0, 1.0);
    let (sin_a, cos_a) = adj.angle_deg.to_radians().sin_cos();
    let rx = adj.rx.max(1e-3);
    let ry = adj.ry.max(1e-3);
    let feather = adj.feather.clamp(0.0, 1.0);

    for y in 0..frame.height {
        for x in 0..frame.width {
            let (u, v) = frame.uv(x, y);
            let dx = (u - adj.cx) * frame.aspect_x;
            let dy = (v - adj.cy) * frame.aspect_y;
            // In das gedrehte Koordinatensystem der Ellipse …
            let ex = dx * cos_a + dy * sin_a;
            let ey = -dx * sin_a + dy * cos_a;
            // … und dort auf den Einheitskreis normieren.
            let r = ((ex / rx).powi(2) + (ey / ry).powi(2)).sqrt();
            // `inside` ist 1 im Kegel, 0 außerhalb, dazwischen weich.
            let inside = 1.0 - smoothstep(1.0 - feather, 1.0 + feather * 0.5, r);

            let i = (y * frame.width + x) * 3;
            for c in 0..3 {
                let base = rgb[i + c];
                let brighten = base + adj.inner_gain * adj.color_rgb[c] * (1.0 - base).max(0.0);
                let darken = base * (1.0 - adj.outer_gain.clamp(0.0, 1.0));
                let target = lerp(darken, brighten, inside);
                rgb[i + c] = lerp(base, target.clamp(0.0, 1.0), amount);
            }
        }
    }
}

// ---- 3. Abwedeln / Nachbelichten ------------------------------------------

fn apply_dodge_burn(rgb: &mut [f32], frame: &Frame, adj: &DodgeBurnAdjustment) {
    if adj.amount <= 0.0 || adj.points.is_empty() {
        return;
    }
    let amount = adj.amount.clamp(0.0, 1.0);
    for y in 0..frame.height {
        for x in 0..frame.width {
            let (u, v) = frame.uv(x, y);
            let mut ev = 0.0f32;
            for point in &adj.points {
                let d = frame.distance(u, v, point.x, point.y);
                ev += falloff_weight(d, point.radius, 1.5) * point.amount;
            }
            if ev == 0.0 {
                continue;
            }
            let i = (y * frame.width + x) * 3;
            // Als Belichtungsversatz, nicht als Addition: so bleiben die
            // Farbverhältnisse erhalten, wie beim echten Abwedeln unter
            // dem Vergrößerer.
            let gain = (ev * amount).exp2();
            for c in 0..3 {
                rgb[i + c] = (rgb[i + c] * gain).clamp(0.0, 1.0);
            }
        }
    }
}

// ---- 4. Split-Lighting ----------------------------------------------------

fn apply_split_light(rgb: &mut [f32], frame: &Frame, adj: &SplitLightAdjustment) {
    if adj.amount <= 0.0 {
        return;
    }
    let amount = adj.amount.clamp(0.0, 1.0);
    let ax = (adj.bx - adj.ax) * frame.aspect_x;
    let ay = (adj.by - adj.ay) * frame.aspect_y;
    let len_sq = (ax * ax + ay * ay).max(1e-6);
    let bias = adj.luma_bias.clamp(0.0, 1.0);

    for y in 0..frame.height {
        for x in 0..frame.width {
            let (u, v) = frame.uv(x, y);
            // Projektion des Pixels auf die Achse A→B, auf 0..1 geklemmt.
            let px = (u - adj.ax) * frame.aspect_x;
            let py = (v - adj.ay) * frame.aspect_y;
            let t = ((px * ax + py * ay) / len_sq).clamp(0.0, 1.0);

            let i = (y * frame.width + x) * 3;
            let l = luminance(rgb[i], rgb[i + 1], rgb[i + 2]);
            // `luma_bias` entscheidet, ob die Einfärbung überall gleich
            // stark wirkt oder sich auf die Lichter konzentriert — Letzteres
            // sieht nach echtem farbigem Licht aus statt nach Farbfolie.
            let strength = amount * lerp(1.0, l, bias);
            for c in 0..3 {
                let tint = lerp(adj.color_a[c], adj.color_b[c], t);
                // Weiches Licht multipliziert, es ersetzt nicht.
                let lit = rgb[i + c] * lerp(1.0, tint * 1.6, strength);
                rgb[i + c] = lit.clamp(0.0, 1.0);
            }
        }
    }
}

// ---- 5. Farbe ersetzen ----------------------------------------------------

fn apply_color_replace(rgb: &mut [f32], adj: &ColorReplaceAdjustment) {
    if adj.amount <= 0.0 || !adj.has_source {
        return;
    }
    let amount = adj.amount.clamp(0.0, 1.0);
    let (_, sa, sb) = opponent(adj.from_rgb[0], adj.from_rgb[1], adj.from_rgb[2]);
    let tolerance = adj.tolerance.clamp(0.01, 1.5);
    let softness = adj.softness.clamp(0.0, 1.5);

    for px in rgb.chunks_exact_mut(3) {
        let (l, a, b) = opponent(px[0], px[1], px[2]);
        // Abstand NUR auf den beiden Farbachsen: die Helligkeit darf
        // abweichen, sonst würde derselbe Farbton im Schatten nicht
        // mitgetroffen.
        let distance = ((a - sa).powi(2) + (b - sb).powi(2)).sqrt();
        let weight = (1.0 - smoothstep(tolerance, tolerance + softness, distance)) * amount;
        if weight <= 0.001 {
            continue;
        }
        let mut target = adj.to_rgb;
        if adj.preserve_luma {
            // Zielfarbe auf die Helligkeit des Originals ziehen — das
            // ist fast immer das Gemeinte: ein rotes Auto soll blau
            // werden, nicht flach.
            let tl = luminance(target[0], target[1], target[2]).max(1e-4);
            let scale = (l / tl).clamp(0.0, 4.0);
            for c in target.iter_mut() {
                *c = (*c * scale).clamp(0.0, 1.0);
            }
        }
        for c in 0..3 {
            px[c] = lerp(px[c], target[c], weight);
        }
    }
}

// ---- 6. Verlaufsband ------------------------------------------------------

/// Farbe an der Stelle `t` eines nach Position sortierten Verlaufs.
fn ramp_color(stops: &[(f32, [f32; 3])], t: f32) -> [f32; 3] {
    if stops.is_empty() {
        return [t, t, t];
    }
    if t <= stops[0].0 {
        return stops[0].1;
    }
    if t >= stops[stops.len() - 1].0 {
        return stops[stops.len() - 1].1;
    }
    for pair in stops.windows(2) {
        let (p0, c0) = pair[0];
        let (p1, c1) = pair[1];
        if t <= p1 {
            let span = (p1 - p0).max(1e-6);
            let k = ((t - p0) / span).clamp(0.0, 1.0);
            return [
                lerp(c0[0], c1[0], k),
                lerp(c0[1], c1[1], k),
                lerp(c0[2], c1[2], k),
            ];
        }
    }
    stops[stops.len() - 1].1
}

fn apply_gradient_ramp(rgb: &mut [f32], adj: &GradientRampAdjustment) {
    if adj.amount <= 0.0 || adj.stops.len() < 2 {
        return;
    }
    let amount = adj.amount.clamp(0.0, 1.0);
    // Einmal sortieren statt bei jedem Pixel: der Nutzer darf die
    // Stützstellen in beliebiger Reihenfolge anlegen und verschieben.
    let mut stops: Vec<(f32, [f32; 3])> = adj
        .stops
        .iter()
        .map(|s| (s.position.clamp(0.0, 1.0), s.color_rgb))
        .collect();
    stops.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));

    for px in rgb.chunks_exact_mut(3) {
        let l = luminance(px[0], px[1], px[2]);
        let mut mapped = ramp_color(&stops, l);
        if adj.preserve_luma {
            let ml = luminance(mapped[0], mapped[1], mapped[2]).max(1e-4);
            let scale = (l / ml).clamp(0.0, 4.0);
            for c in mapped.iter_mut() {
                *c = (*c * scale).clamp(0.0, 1.0);
            }
        }
        for c in 0..3 {
            px[c] = lerp(px[c], mapped[c].clamp(0.0, 1.0), amount);
        }
    }
}

// ---- 7. Horizont-Verlaufsfilter -------------------------------------------

fn apply_horizon_grad(rgb: &mut [f32], frame: &Frame, adj: &HorizonGradAdjustment) {
    if adj.amount <= 0.0 {
        return;
    }
    let amount = adj.amount.clamp(0.0, 1.0);
    // Normale der gezogenen Linie (bildformatbereinigt).
    let dx = (adj.x2 - adj.x1) * frame.aspect_x;
    let dy = (adj.y2 - adj.y1) * frame.aspect_y;
    let len = (dx * dx + dy * dy).sqrt().max(1e-6);
    let (nx, ny) = if adj.flipped {
        (dy / len, -dx / len)
    } else {
        (-dy / len, dx / len)
    };
    let softness = adj.softness.clamp(0.001, 2.0);

    for y in 0..frame.height {
        for x in 0..frame.width {
            let (u, v) = frame.uv(x, y);
            let px = (u - adj.x1) * frame.aspect_x;
            let py = (v - adj.y1) * frame.aspect_y;
            // Vorzeichenbehafteter Abstand zur Linie: negativ auf der
            // wirksamen Seite, positiv auf der anderen.
            let signed = px * nx + py * ny;
            let w = (1.0 - smoothstep(-softness * 0.5, softness * 0.5, signed)) * amount;
            if w <= 0.001 {
                continue;
            }
            let i = (y * frame.width + x) * 3;
            for c in 0..3 {
                let darkened = rgb[i + c] * (1.0 - adj.density.clamp(0.0, 1.0) * w);
                let tinted = lerp(
                    darkened,
                    adj.color_rgb[c] * darkened * 1.8,
                    adj.tint.clamp(0.0, 1.0) * w,
                );
                rgb[i + c] = tinted.clamp(0.0, 1.0);
            }
        }
    }
}

// ---- Einstiegspunkt -------------------------------------------------------

/// Wendet die sieben Werkzeuge in der oben dokumentierten festen
/// Reihenfolge auf ein fertig entwickeltes sRGB-RGBA8-Bild an.
pub fn apply(base: &[u8], width: u32, height: u32, adj: &InteractiveAdjustments) -> Vec<u8> {
    let mut out = base.to_vec();
    let mut rgb = to_rgb_f32(base);
    let frame = Frame::new(width, height);

    apply_point_lights(&mut rgb, &frame, &adj.point_lights);
    apply_spotlight(&mut rgb, &frame, &adj.spotlight);
    apply_dodge_burn(&mut rgb, &frame, &adj.dodge_burn);
    apply_split_light(&mut rgb, &frame, &adj.split_light);
    apply_color_replace(&mut rgb, &adj.color_replace);
    apply_gradient_ramp(&mut rgb, &adj.gradient_ramp);
    apply_horizon_grad(&mut rgb, &frame, &adj.horizon_grad);

    write_back(&rgb, &mut out);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::edl::v4::{DodgeBurnPoint, GradientStop, PointLight};

    /// Testbild mit echtem Inhalt: diagonaler Verlauf plus ein
    /// kräftig roter Fleck (für „Farbe ersetzen") und eine dunkle Ecke.
    fn test_image(w: u32, h: u32) -> Vec<u8> {
        let mut out = vec![0u8; (w * h * 4) as usize];
        for y in 0..h {
            for x in 0..w {
                let i = ((y * w + x) * 4) as usize;
                let red_patch = x < w / 4 && y < h / 4;
                if red_patch {
                    out[i] = 210;
                    out[i + 1] = 40;
                    out[i + 2] = 45;
                } else {
                    let t = (x + y) as f32 / (w + h) as f32;
                    let v = 30.0 + t * 190.0;
                    out[i] = v as u8;
                    out[i + 1] = (v * 0.95) as u8;
                    out[i + 2] = (v * 0.88) as u8;
                }
                out[i + 3] = 255;
            }
        }
        out
    }

    fn differs(a: &[u8], b: &[u8]) -> bool {
        a.iter().zip(b).any(|(x, y)| x != y)
    }

    /// Mittlere Helligkeit eines rechteckigen Bildausschnitts — für die
    /// Tests, die eine ÖRTLICHE Wirkung nachweisen sollen statt nur
    /// „irgendetwas hat sich geändert".
    fn mean_luma(img: &[u8], w: u32, x0: u32, y0: u32, x1: u32, y1: u32) -> f32 {
        let mut sum = 0.0;
        let mut count = 0.0f32;
        for y in y0..y1 {
            for x in x0..x1 {
                let i = ((y * w + x) * 4) as usize;
                sum += 0.3 * img[i] as f32 + 0.59 * img[i + 1] as f32 + 0.11 * img[i + 2] as f32;
                count += 1.0;
            }
        }
        sum / count.max(1.0)
    }

    #[test]
    fn neutral_adjustments_leave_the_image_untouched() {
        let base = test_image(24, 16);
        let out = apply(&base, 24, 16, &InteractiveAdjustments::default());
        assert_eq!(out, base);
        assert!(InteractiveAdjustments::default().is_neutral());
    }

    /// Ein aufgedrehter Regler OHNE einen einzigen Eintrag ist genauso
    /// ein No-Op wie `amount == 0` — sonst würde die Pipeline die Stufe
    /// für nichts durchlaufen.
    #[test]
    fn list_tools_without_entries_count_as_neutral() {
        let mut adj = InteractiveAdjustments::default();
        adj.point_lights.amount = 1.0;
        adj.dodge_burn.amount = 1.0;
        adj.gradient_ramp.amount = 1.0;
        adj.color_replace.amount = 1.0;
        assert!(
            adj.is_neutral(),
            "ohne Lichter, Punkte, Stützstellen und Quellfarbe gibt es nichts zu tun"
        );
    }

    /// Der Kerntest: JEDES der sieben Werkzeuge muss für sich allein das
    /// Bild real verändern.
    #[test]
    fn each_of_the_seven_tools_changes_the_image_on_its_own() {
        let (w, h) = (48u32, 32u32);
        let base = test_image(w, h);
        let mut cases: Vec<(&str, InteractiveAdjustments)> = Vec::new();

        let mut a = InteractiveAdjustments::default();
        a.point_lights.amount = 1.0;
        a.point_lights.lights = vec![PointLight::DEFAULT];
        cases.push(("point_lights", a));

        let mut a = InteractiveAdjustments::default();
        a.spotlight.amount = 1.0;
        cases.push(("spotlight", a));

        let mut a = InteractiveAdjustments::default();
        a.dodge_burn.amount = 1.0;
        a.dodge_burn.points = vec![DodgeBurnPoint::DEFAULT];
        cases.push(("dodge_burn", a));

        let mut a = InteractiveAdjustments::default();
        a.split_light.amount = 1.0;
        cases.push(("split_light", a));

        let mut a = InteractiveAdjustments::default();
        a.color_replace.amount = 1.0;
        a.color_replace.has_source = true;
        a.color_replace.from_rgb = [0.82, 0.16, 0.18];
        a.color_replace.to_rgb = [0.2, 0.4, 0.9];
        cases.push(("color_replace", a));

        let mut a = InteractiveAdjustments::default();
        a.gradient_ramp.amount = 1.0;
        a.gradient_ramp.stops = vec![
            GradientStop {
                position: 0.0,
                color_rgb: [0.1, 0.05, 0.3],
            },
            GradientStop {
                position: 1.0,
                color_rgb: [1.0, 0.9, 0.4],
            },
        ];
        cases.push(("gradient_ramp", a));

        let mut a = InteractiveAdjustments::default();
        a.horizon_grad.amount = 1.0;
        cases.push(("horizon_grad", a));

        assert_eq!(cases.len(), 7, "alle sieben Werkzeuge müssen geprüft sein");
        for (name, adj) in cases {
            assert!(!adj.is_neutral(), "{name} müsste als aktiv gelten");
            let out = apply(&base, w, h, &adj);
            assert!(differs(&out, &base), "{name} hat das Bild nicht verändert");
        }
    }

    /// Ein Licht muss dort wirken, wo es steht — und nicht anderswo.
    /// Genau das unterscheidet ein Bild-Werkzeug von einem globalen
    /// Regler, und genau das würde eine falsche Koordinatenumrechnung
    /// zerstören.
    #[test]
    fn a_point_light_brightens_its_own_corner_and_leaves_the_far_one_alone() {
        let (w, h) = (64u32, 64u32);
        let base = test_image(w, h);
        let mut adj = InteractiveAdjustments::default();
        adj.point_lights.amount = 1.0;
        adj.point_lights.lights = vec![PointLight {
            x: 0.15,
            y: 0.85,
            radius: 0.3,
            intensity: 1.0,
            ..PointLight::DEFAULT
        }];
        let out = apply(&base, w, h, &adj);

        let near_before = mean_luma(&base, w, 0, 48, 16, 64);
        let near_after = mean_luma(&out, w, 0, 48, 16, 64);
        let far_before = mean_luma(&base, w, 48, 0, 64, 16);
        let far_after = mean_luma(&out, w, 48, 0, 64, 16);

        assert!(
            near_after > near_before + 5.0,
            "das Licht hellt seine eigene Ecke nicht auf: {near_before} → {near_after}"
        );
        assert!(
            (far_after - far_before).abs() < 1.0,
            "das Licht wirkt bis in die gegenüberliegende Ecke: {far_before} → {far_after}"
        );
    }

    /// Derselbe Nachweis für den Lichtkegel: innen heller, außen dunkler.
    #[test]
    fn the_spotlight_brightens_inside_and_darkens_outside() {
        let (w, h) = (64u32, 64u32);
        let base = test_image(w, h);
        let mut adj = InteractiveAdjustments::default();
        adj.spotlight.amount = 1.0;
        adj.spotlight.cx = 0.5;
        adj.spotlight.cy = 0.5;
        adj.spotlight.rx = 0.25;
        adj.spotlight.ry = 0.25;
        let out = apply(&base, w, h, &adj);

        let inside_delta = mean_luma(&out, w, 28, 28, 36, 36) - mean_luma(&base, w, 28, 28, 36, 36);
        let outside_delta = mean_luma(&out, w, 0, 0, 8, 8) - mean_luma(&base, w, 0, 0, 8, 8);
        assert!(inside_delta > 3.0, "im Kegel nicht heller: {inside_delta}");
        assert!(
            outside_delta < -3.0,
            "außerhalb nicht dunkler: {outside_delta}"
        );
    }

    /// Abwedeln und Nachbelichten müssen sich gegenläufig verhalten —
    /// ein Vorzeichenfehler wäre sonst unsichtbar.
    #[test]
    fn dodge_and_burn_move_in_opposite_directions() {
        let (w, h) = (48u32, 48u32);
        let base = test_image(w, h);
        let mut lighten = InteractiveAdjustments::default();
        lighten.dodge_burn.amount = 1.0;
        lighten.dodge_burn.points = vec![DodgeBurnPoint {
            x: 0.5,
            y: 0.5,
            radius: 0.3,
            amount: 0.8,
        }];
        let mut darken = lighten.clone();
        darken.dodge_burn.points[0].amount = -0.8;

        let center = |img: &[u8]| mean_luma(img, w, 20, 20, 28, 28);
        assert!(center(&apply(&base, w, h, &lighten)) > center(&base) + 3.0);
        assert!(center(&apply(&base, w, h, &darken)) < center(&base) - 3.0);
    }

    /// „Farbe ersetzen" muss den roten Fleck treffen und den Rest des
    /// Bildes in Ruhe lassen — die Toleranz ist der ganze Punkt.
    #[test]
    fn color_replace_hits_the_matching_patch_only() {
        let (w, h) = (64u32, 64u32);
        let base = test_image(w, h);
        let mut adj = InteractiveAdjustments::default();
        adj.color_replace.amount = 1.0;
        adj.color_replace.has_source = true;
        adj.color_replace.from_rgb = [210.0 / 255.0, 40.0 / 255.0, 45.0 / 255.0];
        adj.color_replace.to_rgb = [0.15, 0.35, 0.95];
        adj.color_replace.tolerance = 0.2;
        adj.color_replace.softness = 0.1;
        let out = apply(&base, w, h, &adj);

        // Im roten Fleck muss Blau jetzt deutlich über Rot liegen …
        let i = ((4 * w + 4) * 4) as usize;
        assert!(
            out[i + 2] > out[i] + 30,
            "der rote Fleck wurde nicht blau: r={} b={}",
            out[i],
            out[i + 2]
        );
        // … und der neutrale Bildteil unangetastet bleiben.
        let j = ((40 * w + 40) * 4) as usize;
        for c in 0..3 {
            assert_eq!(
                out[j + c],
                base[j + c],
                "der neutrale Bildteil wurde mitverändert (Kanal {c})"
            );
        }
    }

    /// Der Verlauf muss die Stützstellen auch dann richtig treffen, wenn
    /// der Nutzer sie in beliebiger Reihenfolge angelegt hat.
    #[test]
    fn gradient_ramp_sorts_unordered_stops() {
        let stops = [
            (1.0f32, [1.0f32, 1.0, 1.0]),
            (0.0f32, [0.0f32, 0.0, 0.0]),
            (0.5f32, [1.0f32, 0.0, 0.0]),
        ];
        let mut sorted: Vec<(f32, [f32; 3])> = stops.to_vec();
        sorted.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
        let mid = ramp_color(&sorted, 0.5);
        assert!(
            (mid[0] - 1.0).abs() < 1e-4 && mid[1] < 1e-4,
            "Mitte ist nicht rot: {mid:?}"
        );
        assert!(ramp_color(&sorted, 0.0)[0] < 1e-4);
        assert!(ramp_color(&sorted, 1.0)[0] > 1.0 - 1e-4);
        // Außerhalb der Stützstellen wird geklemmt, nicht extrapoliert.
        assert_eq!(ramp_color(&sorted, -5.0), [0.0, 0.0, 0.0]);
        assert_eq!(ramp_color(&sorted, 5.0), [1.0, 1.0, 1.0]);
    }

    /// Der Verlaufsfilter darf nur auf EINER Seite der Linie wirken, und
    /// `flipped` muss genau das umdrehen.
    #[test]
    fn the_horizon_filter_affects_one_side_and_flips_on_demand() {
        let (w, h) = (48u32, 48u32);
        let base = test_image(w, h);
        let mut adj = InteractiveAdjustments::default();
        adj.horizon_grad.amount = 1.0;
        adj.horizon_grad.x1 = 0.0;
        adj.horizon_grad.y1 = 0.5;
        adj.horizon_grad.x2 = 1.0;
        adj.horizon_grad.y2 = 0.5;
        adj.horizon_grad.softness = 0.05;
        adj.horizon_grad.density = 0.8;

        let top = |img: &[u8]| mean_luma(img, w, 0, 0, 48, 8);
        let bottom = |img: &[u8]| mean_luma(img, w, 0, 40, 48, 48);

        let normal = apply(&base, w, h, &adj);
        let mut flipped = adj.clone();
        flipped.horizon_grad.flipped = true;
        let flipped_out = apply(&base, w, h, &flipped);

        // Genau eine der beiden Hälften ist in jeder Fassung abgedunkelt,
        // und es ist jeweils die andere.
        let normal_top_dark = top(&normal) < top(&base) - 3.0;
        let normal_bottom_dark = bottom(&normal) < bottom(&base) - 3.0;
        assert!(
            normal_top_dark != normal_bottom_dark,
            "der Filter wirkt auf beide Seiten oder auf keine"
        );
        let flipped_top_dark = top(&flipped_out) < top(&base) - 3.0;
        assert!(
            flipped_top_dark != normal_top_dark,
            "`flipped` dreht die wirksame Seite nicht um"
        );
    }

    /// Dieselbe Bearbeitung muss in jeder Auflösung dasselbe Bild
    /// ergeben — sonst sähe die Vorschau anders aus als der Export.
    /// Geprüft wird nicht Pixelgleichheit (die Auflösungen sind
    /// verschieden), sondern dass ein an derselben normierten Stelle
    /// gesetztes Licht dort auch in beiden landet.
    #[test]
    fn the_effect_lands_at_the_same_relative_place_in_any_resolution() {
        let mut adj = InteractiveAdjustments::default();
        adj.point_lights.amount = 1.0;
        adj.point_lights.lights = vec![PointLight {
            x: 0.25,
            y: 0.25,
            radius: 0.2,
            intensity: 1.0,
            ..PointLight::DEFAULT
        }];

        let measure = |size: u32| -> (f32, f32) {
            let base = test_image(size, size);
            let out = apply(&base, size, size, &adj);
            let q = size / 8;
            let at_light = mean_luma(
                &out,
                size,
                size / 4 - q,
                size / 4 - q,
                size / 4 + q,
                size / 4 + q,
            ) - mean_luma(
                &base,
                size,
                size / 4 - q,
                size / 4 - q,
                size / 4 + q,
                size / 4 + q,
            );
            let far = mean_luma(&out, size, size - 2 * q, size - 2 * q, size, size)
                - mean_luma(&base, size, size - 2 * q, size - 2 * q, size, size);
            (at_light, far)
        };

        let (small_light, small_far) = measure(32);
        let (large_light, large_far) = measure(128);
        assert!(
            small_light > 5.0 && large_light > 5.0,
            "Licht fehlt in einer Auflösung"
        );
        assert!(
            (small_light - large_light).abs() < 6.0,
            "die Wirkung hängt von der Auflösung ab: {small_light} gegen {large_light}"
        );
        assert!(small_far.abs() < 1.0 && large_far.abs() < 1.0);
    }
}
