//! Maskensystem: Ebenenmodell statt Fused-Pass (`DECISIONS.md` ADR-0032
//! Punkt 4) — jede Maske ist ein eigener Durchlauf, der (a) ihre
//! zusammengesetzte Alpha berechnet, (b) ihre eigenen Werkzeuge
//! (Grundeinstellungen/Kurven/HSL/Farbmischer/Color Grading/Details,
//! siehe ADR-0032 Punkt 2) auf eine Kopie des aktuellen Bildzustands
//! anwendet (dieselben Stufenfunktionen wie die globale Pipeline, nur
//! mit den Masken-eigenen EDL-Werten), und (c) alpha-gewichtet mit ihrem
//! Ebenen-Mischmodus zurückmischt.
//!
//! **Pipeline-Position** (siehe `develop.rs`): direkt nach `effects`, vor
//! der Farbraum-Konvertierung — also noch im linearen Arbeitsraum, an
//! genau der Stelle, an der auch die globalen Fassungen von
//! Grundeinstellungen/HSL/Color Grading/Details bereits laufen. Das
//! bedeutet: Masken-Kurven wirken bewusst auf dem linearen Pixelwert
//! selbst statt auf dem display-referred Tonwert wie die globale Kurve
//! (die erst nach der Farbraum-Konvertierung auf dem fertigen RGBA8-
//! Puffer läuft, siehe `curves.rs`) — dieselbe LUT
//! (`curves::apply_linear_rgb`), andere Eingangsdomäne. Eine echte
//! zweite Farbraum-Konvertierung pro Maske nur für ihre Kurve wäre
//! unnötig verlustreich und würde die Ebenenmodell-Architektur
//! verkomplizieren; als bewusste Vereinfachung dokumentiert.
//!
//! **Schritt 2 (dieser Commit) ist CPU-only** — GPU-Beschleunigung der
//! Maskenalpha-Berechnung kommt erst, wenn die jeweilige Geometrie
//! tatsächlich über eine Frontend-Interaktion befüllt werden kann
//! (Schritt 3–5); ein GPU-Pfad vorab für noch unbenutzte Geometrie wäre
//! dieselbe Art Vorab-Arbeit, die ADR-0029 bereits für Phase 4 vermieden
//! hat.
//!
//! **Alle fünf `BlendMode`-Varianten sind seit Schritt 6 echt
//! implementiert** (`blend_pixel`) — Multiplizieren/Weiches Licht sind
//! pro Kanal separierbar, Farbe/Luminanz brauchen die Ganzpixel-
//! Luminanz-Formel (`luminosity`/`set_luminosity`/`clip_color`, nach dem
//! Photoshop-/W3C-Compositing-Standardverfahren „SetLum"/„ClipColor").
//! **Bewusste Vereinfachung:** diese Formeln setzen einen ungefähr
//! `0.0..=1.0`-Wertebereich voraus (Standardverfahren für
//! Ebenen-Mischmodi); im linearen Arbeitsraum können helle Lichter
//! diesen Bereich überschreiten — `clip_color` faltet solche Werte auf
//! den gültigen Bereich zurück statt sie unverändert durchzureichen,
//! was bei extremen Lichtern zu einem leicht anderen Ergebnis führen
//! kann als ein Compositing-Werkzeug, das explizit für HDR ausgelegt
//! ist. Für die in dieser Phase erreichbaren Werte ist das nicht
//! sichtbar relevant.

use rayon::prelude::*;

use super::{
    basic_fused, color_grading, curves, details, hsl_color_mixer, local_contrast, white_balance,
};
use crate::edl::v2::{ColorGradingAdjustment, CurvesAdjustment, DetailsAdjustment, HslAdjustment};
use crate::edl::v3::{BlendMode, Mask, MaskAdjustments, MaskCombine, MaskGeometry, MaskGroup};

/// Wendet alle sichtbaren Masken sequenziell an (`masks`-Reihenfolge ist
/// die Anwendungsreihenfolge, siehe `EdlV4::masks`-Moduldoku) — jede
/// Maske sieht das Ergebnis der vorangehenden.
pub fn apply_all(
    pixels: &[f32],
    width: u32,
    height: u32,
    as_shot_wb_coeffs: [f32; 4],
    masks: &[Mask],
    groups: &[MaskGroup],
) -> Vec<f32> {
    let mut current = pixels.to_vec();
    for mask in visible_masks(masks, groups) {
        current = apply_one(&current, width, height, as_shot_wb_coeffs, mask);
    }
    current
}

fn apply_one(
    pixels: &[f32],
    width: u32,
    height: u32,
    as_shot_wb_coeffs: [f32; 4],
    mask: &Mask,
) -> Vec<f32> {
    let alpha = compose_mask_alpha(mask, pixels, width, height);
    let adjusted =
        apply_mask_adjustments(pixels, width, height, as_shot_wb_coeffs, &mask.adjustments);
    pixels
        .par_chunks_exact(3)
        .zip(adjusted.par_chunks_exact(3))
        .zip(alpha.par_iter())
        .flat_map_iter(|((base, adjusted), &a)| {
            let base = [base[0], base[1], base[2]];
            let adjusted = [adjusted[0], adjusted[1], adjusted[2]];
            let blended = blend_pixel(base, adjusted, mask.blend_mode);
            (0..3).map(move |channel| base[channel] * (1.0 - a) + blended[channel] * a)
        })
        .collect()
}

/// Verrechnet den unveränderten (`base`) mit dem maskiert-bearbeiteten
/// (`adjusted`) Pixel nach dem gewählten Ebenen-Mischmodus — das
/// Ergebnis wird danach in `apply_one` alpha-gewichtet mit `base`
/// zurückgemischt (Normal-Modus liefert also `adjusted` unverändert,
/// als hätte gar kein Mischmodus stattgefunden).
fn blend_pixel(base: [f32; 3], adjusted: [f32; 3], mode: BlendMode) -> [f32; 3] {
    match mode {
        BlendMode::Normal => adjusted,
        BlendMode::Multiply => std::array::from_fn(|i| base[i] * adjusted[i]),
        BlendMode::SoftLight => std::array::from_fn(|i| soft_light_channel(base[i], adjusted[i])),
        // „Farbe": Farbton/Sättigung von `adjusted`, Luminanz von `base`.
        BlendMode::Color => set_luminosity(adjusted, luminosity(base)),
        // „Luminanz": Luminanz von `adjusted`, Farbton/Sättigung von `base`.
        BlendMode::Luminosity => set_luminosity(base, luminosity(adjusted)),
    }
}

/// Photoshop-/W3C-Compositing-„Soft Light"-Formel, pro Kanal separierbar.
fn soft_light_channel(cb: f32, cs: f32) -> f32 {
    let d = if cb <= 0.25 {
        ((16.0 * cb - 12.0) * cb + 4.0) * cb
    } else {
        cb.sqrt()
    };
    if cs <= 0.5 {
        cb - (1.0 - 2.0 * cs) * cb * (1.0 - cb)
    } else {
        cb + (2.0 * cs - 1.0) * (d - cb)
    }
}

/// Photoshop-/W3C-Compositing-„Lum"-Gewichtung (Rec.601-Luminanzgewichte,
/// dieselben wie `luminance_range_alpha` weiter unten).
fn luminosity(c: [f32; 3]) -> f32 {
    0.3 * c[0] + 0.59 * c[1] + 0.11 * c[2]
}

/// „SetLum": verschiebt `c` so, dass seine Luminanz `target_lum` wird,
/// ohne seinen Farbton/seine Sättigung zu ändern; `clip_color` faltet ein
/// dadurch außerhalb `0.0..=1.0` geratenes Ergebnis zurück (siehe
/// Moduldoku oben zur bewussten Vereinfachung im linearen Arbeitsraum).
fn set_luminosity(c: [f32; 3], target_lum: f32) -> [f32; 3] {
    let diff = target_lum - luminosity(c);
    clip_color(std::array::from_fn(|i| c[i] + diff))
}

fn clip_color(c: [f32; 3]) -> [f32; 3] {
    let lum = luminosity(c);
    let min = c[0].min(c[1]).min(c[2]);
    let max = c[0].max(c[1]).max(c[2]);
    let mut c = c;
    if min < 0.0 && lum > min {
        c = std::array::from_fn(|i| lum + (c[i] - lum) * lum / (lum - min));
    }
    if max > 1.0 && max > lum {
        c = std::array::from_fn(|i| lum + (c[i] - lum) * (1.0 - lum) / (max - lum));
    }
    c
}

/// Wendet die ton-/farb-/detailbezogenen Werkzeuge einer Maske auf eine
/// Kopie von `pixels` an — dieselben Stufenfunktionen wie die globale
/// Pipeline in `develop.rs`, nur mit `adjustments` statt dem globalen
/// EDL-Anteil.
fn apply_mask_adjustments(
    pixels: &[f32],
    width: u32,
    height: u32,
    as_shot_wb_coeffs: [f32; 4],
    adjustments: &MaskAdjustments,
) -> Vec<f32> {
    let wb_gains = white_balance::compute_gains(as_shot_wb_coeffs, adjustments.basic.white_balance);
    let toned = basic_fused::apply_cpu(pixels, wb_gains, &adjustments.basic);

    let textured = if adjustments.basic.texture == 0.0 && adjustments.basic.clarity == 0.0 {
        toned
    } else {
        local_contrast::apply_cpu(
            &toned,
            width,
            height,
            adjustments.basic.texture,
            adjustments.basic.clarity,
        )
    };

    let detailed = if adjustments.details == DetailsAdjustment::NEUTRAL {
        textured
    } else {
        details::apply_cpu(&textured, width, height, &adjustments.details)
    };

    let colored = if adjustments.hsl == HslAdjustment::NEUTRAL
        && adjustments.color_mixer.regions.is_empty()
    {
        detailed
    } else {
        hsl_color_mixer::apply_cpu(&detailed, &adjustments.hsl, &adjustments.color_mixer)
    };

    let graded = if adjustments.color_grading == ColorGradingAdjustment::NEUTRAL {
        colored
    } else {
        color_grading::apply_cpu(&colored, &adjustments.color_grading)
    };

    if adjustments.curves == CurvesAdjustment::neutral() {
        graded
    } else {
        curves::apply_linear_rgb(&graded, &adjustments.curves)
    }
}

// ---- Zusammengesetzte Maskenalpha --------------------------------------------

/// Berechnet die endgültige Alpha einer Maske: Komponenten kombinieren
/// (`SPEC.md` §5 „Maskenkombination"), globale Weichzeichnung, globale
/// Invertierung, Deckkraft — in dieser Reihenfolge.
fn compose_mask_alpha(mask: &Mask, pixels: &[f32], width: u32, height: u32) -> Vec<f32> {
    let w = width as usize;
    let h = height as usize;
    let mut composed = vec![0.0f32; w * h];

    for (index, component) in mask.components.iter().enumerate() {
        let mut alpha = geometry_alpha(&component.geometry, pixels, w, h);
        if component.invert {
            for a in alpha.iter_mut() {
                *a = 1.0 - *a;
            }
        }
        if index == 0 {
            composed = alpha;
        } else {
            for (c, a) in composed.iter_mut().zip(alpha.iter()) {
                *c = match component.combine {
                    // Vereinigung (Union) — die stärkere der beiden
                    // Abdeckungen gewinnt statt sich zu addieren (das
                    // würde bei überlappenden Komponenten sonst über
                    // 100 % Deckung hinausgehen).
                    MaskCombine::Add => c.max(*a),
                    MaskCombine::Subtract => *c * (1.0 - *a),
                    MaskCombine::Intersect => *c * *a,
                };
            }
        }
    }

    if mask.feather > 0.0 {
        composed = feather_alpha(&composed, w, h, mask.feather);
    }

    if mask.invert {
        for a in composed.iter_mut() {
            *a = 1.0 - *a;
        }
    }

    let opacity = (mask.opacity / 100.0).clamp(0.0, 1.0);
    if opacity != 1.0 {
        for a in composed.iter_mut() {
            *a *= opacity;
        }
    }

    composed
}

fn geometry_alpha(geometry: &MaskGeometry, pixels: &[f32], w: usize, h: usize) -> Vec<f32> {
    match geometry {
        MaskGeometry::Brush { strokes } => brush_alpha(strokes, pixels, w, h),
        MaskGeometry::LinearGradient { x1, y1, x2, y2 } => {
            linear_gradient_alpha(*x1, *y1, *x2, *y2, w, h)
        }
        MaskGeometry::RadialGradient {
            center_x,
            center_y,
            radius_x,
            radius_y,
            angle_degrees,
            feather,
        } => radial_gradient_alpha(
            (*center_x, *center_y),
            (*radius_x, *radius_y),
            *angle_degrees,
            *feather,
            w,
            h,
        ),
        MaskGeometry::ColorRange {
            target_r,
            target_g,
            target_b,
            tolerance,
            feather,
        } => color_range_alpha(
            (*target_r, *target_g, *target_b),
            *tolerance,
            *feather,
            pixels,
        ),
        MaskGeometry::LuminanceRange {
            range_min,
            range_max,
            feather,
        } => luminance_range_alpha(*range_min, *range_max, *feather, pixels),
        MaskGeometry::AiGenerated {
            width,
            height,
            alpha,
            ..
        } => ai_generated_alpha(alpha, *width, *height, w, h),
        MaskGeometry::BlurDepthApprox { threshold } => {
            blur_depth_approx_alpha(*threshold, pixels, w, h)
        }
    }
}

/// Skaliert eine per `apx-ai` einmalig berechnete Alpha-Bitmap (siehe
/// `MaskGeometry::AiGenerated`s Moduldoku) bilinear auf die aktuelle
/// Render-Auflösung `(w, h)` und normiert von `0..=255` auf `0.0..=1.0` —
/// dieselbe Normierung wie jede andere Alpha-Funktion in diesem Modul.
fn ai_generated_alpha(alpha: &[u8], src_w: u32, src_h: u32, w: usize, h: usize) -> Vec<f32> {
    let resized = apx_core::raster::bilinear_resize_u8(alpha, src_w, src_h, w as u32, h as u32);
    resized.into_iter().map(|v| v as f32 / 255.0).collect()
}

/// Pixelmittelpunkt in normierten Bildkoordinaten (`0.0..=1.0`) — dieselbe
/// Konvention wie die übrige Maskengeometrie (siehe `v3.rs`).
fn pixel_uv(w: usize, h: usize, x: usize, y: usize) -> (f32, f32) {
    ((x as f32 + 0.5) / w as f32, (y as f32 + 0.5) / h as f32)
}

fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    if edge1 <= edge0 {
        return if x < edge0 { 0.0 } else { 1.0 };
    }
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn linear_gradient_alpha(x1: f32, y1: f32, x2: f32, y2: f32, w: usize, h: usize) -> Vec<f32> {
    let dx = x2 - x1;
    let dy = y2 - y1;
    let len_sq = (dx * dx + dy * dy).max(1e-6);
    (0..w * h)
        .into_par_iter()
        .map(|index| {
            let x = index % w;
            let y = index / w;
            let (u, v) = pixel_uv(w, h, x, y);
            let t = ((u - x1) * dx + (v - y1) * dy) / len_sq;
            1.0 - t.clamp(0.0, 1.0)
        })
        .collect()
}

/// `center`/`radius` als Tupel statt einzelner Parameter, damit die
/// Funktion unter Clippys `too_many_arguments`-Grenze bleibt.
fn radial_gradient_alpha(
    center: (f32, f32),
    radius: (f32, f32),
    angle_degrees: f32,
    feather: f32,
    w: usize,
    h: usize,
) -> Vec<f32> {
    let (center_x, center_y) = center;
    let angle = -angle_degrees.to_radians();
    let (sin_a, cos_a) = angle.sin_cos();
    let radius_x = radius.0.max(1e-4);
    let radius_y = radius.1.max(1e-4);
    let inner = (1.0 - feather.clamp(0.0, 1.0)).max(0.0);
    (0..w * h)
        .into_par_iter()
        .map(|index| {
            let x = index % w;
            let y = index / w;
            let (u, v) = pixel_uv(w, h, x, y);
            let dx = u - center_x;
            let dy = v - center_y;
            let rotated_x = dx * cos_a - dy * sin_a;
            let rotated_y = dx * sin_a + dy * cos_a;
            let d = ((rotated_x / radius_x).powi(2) + (rotated_y / radius_y).powi(2)).sqrt();
            1.0 - smoothstep(inner, 1.0, d)
        })
        .collect()
}

fn color_range_alpha(
    target: (f32, f32, f32),
    tolerance: f32,
    feather: f32,
    pixels: &[f32],
) -> Vec<f32> {
    let (target_r, target_g, target_b) = target;
    let tolerance = tolerance.max(0.0);
    let outer = (tolerance + feather.max(0.0)).max(tolerance + 1e-4);
    pixels
        .par_chunks_exact(3)
        .map(|rgb| {
            let dr = rgb[0] - target_r;
            let dg = rgb[1] - target_g;
            let db = rgb[2] - target_b;
            let dist = (dr * dr + dg * dg + db * db).sqrt();
            1.0 - smoothstep(tolerance, outer, dist)
        })
        .collect()
}

fn luminance_range_alpha(range_min: f32, range_max: f32, feather: f32, pixels: &[f32]) -> Vec<f32> {
    let feather = feather.max(1e-4);
    pixels
        .par_chunks_exact(3)
        .map(|rgb| {
            let luminance = 0.299 * rgb[0] + 0.587 * rgb[1] + 0.114 * rgb[2];
            let rising = smoothstep(range_min - feather, range_min, luminance);
            let falling = 1.0 - smoothstep(range_max, range_max + feather, luminance);
            rising * falling
        })
        .collect()
}

/// Berechnet eine grobe, **bildrelative** Schärfe-Karte (`0.0..=1.0`,
/// höher = schärfer) über die Laplace-Varianz in einem gleitenden
/// 5×5-Fenster (klassisches „Variance of Laplacian"-Schärfemaß) — die
/// Grundlage für [`MaskGeometry::BlurDepthApprox`] (siehe dessen
/// Moduldoku). Randpixel klemmen auf den nächstgelegenen Nachbarn statt
/// Nullpolsterung, sonst gäbe es einen künstlichen Schärfesprung am
/// Bildrand.
///
/// **Architektur-Hinweis:** die im Plan genannte Heimat
/// `apx_ai::depth_estimate` ist hier bewusst *nicht* verwendet —
/// `apx-pipeline` hängt nicht von `apx-ai` ab (`apx-ai` hängt umgekehrt
/// von `apx-pipeline` ab, siehe dessen `Cargo.toml`s Beschreibung), eine
/// Abhängigkeit in diese Richtung wäre ein Zyklus. Diese Funktion ist
/// deshalb wie [`color_range_alpha`]/[`luminance_range_alpha`]
/// selbstständig direkt hier implementiert statt aus `apx-ai` importiert.
fn relative_sharpness_map(pixels: &[f32], w: usize, h: usize) -> Vec<f32> {
    if w == 0 || h == 0 {
        return Vec::new();
    }

    let luminance: Vec<f32> = pixels
        .par_chunks_exact(3)
        .map(|rgb| 0.299 * rgb[0] + 0.587 * rgb[1] + 0.114 * rgb[2])
        .collect();

    let clamped_index = |x: isize, y: isize| -> usize {
        let cx = x.clamp(0, w as isize - 1) as usize;
        let cy = y.clamp(0, h as isize - 1) as usize;
        cy * w + cx
    };

    // 3x3-Laplace-Kernel [[0,1,0],[1,-4,1],[0,1,0]] je Pixel.
    let laplacian_map: Vec<f32> = (0..h)
        .into_par_iter()
        .flat_map(|y| {
            let luminance = &luminance;
            (0..w)
                .map(move |x| {
                    let (xi, yi) = (x as isize, y as isize);
                    let center = luminance[clamped_index(xi, yi)];
                    luminance[clamped_index(xi, yi - 1)]
                        + luminance[clamped_index(xi, yi + 1)]
                        + luminance[clamped_index(xi - 1, yi)]
                        + luminance[clamped_index(xi + 1, yi)]
                        - 4.0 * center
                })
                .collect::<Vec<_>>()
        })
        .collect();

    // Lokale Varianz der Laplace-Antwort in einem Fenster mit Radius 2
    // (5x5) — das "gleitende Fenster" aus der Moduldoku.
    const RADIUS: isize = 2;
    let variance_map: Vec<f32> = (0..h)
        .into_par_iter()
        .flat_map(|y| {
            let laplacian_map = &laplacian_map;
            (0..w)
                .map(move |x| {
                    let mut sum = 0.0f32;
                    let mut sum_sq = 0.0f32;
                    let mut count = 0.0f32;
                    for dy in -RADIUS..=RADIUS {
                        for dx in -RADIUS..=RADIUS {
                            let v = laplacian_map[clamped_index(x as isize + dx, y as isize + dy)];
                            sum += v;
                            sum_sq += v * v;
                            count += 1.0;
                        }
                    }
                    let mean = sum / count;
                    (sum_sq / count - mean * mean).max(0.0)
                })
                .collect::<Vec<_>>()
        })
        .collect();

    let max_variance = variance_map
        .iter()
        .copied()
        .fold(0.0f32, f32::max)
        .max(1e-6);
    variance_map
        .into_iter()
        .map(|v| (v / max_variance).clamp(0.0, 1.0))
        .collect()
}

/// Fester weicher Übergang um `threshold` — die Laplace-Varianz-Karte
/// selbst ist schon bildrelativ normiert (siehe [`relative_sharpness_map`]),
/// ein zusätzlicher `feather`-Parameter (wie bei den übrigen Bereichs-
/// Masken) wäre für diese eine grobe Heuristik ein nicht gerechtfertigter
/// zusätzlicher Regler.
const BLUR_DEPTH_APPROX_FEATHER: f32 = 0.15;

fn blur_depth_approx_alpha(threshold: f32, pixels: &[f32], w: usize, h: usize) -> Vec<f32> {
    let sharpness = relative_sharpness_map(pixels, w, h);
    let t = threshold.clamp(0.0, 1.0);
    sharpness
        .into_iter()
        .map(|s| {
            smoothstep(
                t - BLUR_DEPTH_APPROX_FEATHER,
                t + BLUR_DEPTH_APPROX_FEATHER,
                s,
            )
        })
        .collect()
}

/// Kürzester Abstand zu irgendeinem Stützpunkt des Pinselzugs — wie
/// `stages::repair.rs`s `distance_to_path` ein vereinfachter minimaler
/// Stützpunkt-Abstand statt einer echten Punkt-zu-Liniensegment-Distanz
/// (siehe `DECISIONS.md` ADR-0028, dieselbe Vereinfachung hier
/// übernommen).
fn distance_to_stroke(points: &[crate::edl::v3::MaskPoint], u: f32, v: f32) -> f32 {
    let mut min_dist = f32::MAX;
    for point in points {
        let dx = u - point.x;
        let dy = v - point.y;
        let dist = (dx * dx + dy * dy).sqrt();
        if dist < min_dist {
            min_dist = dist;
        }
    }
    min_dist
}

/// Mehrere Striche akkumulieren ihre Deckung per Maximum (nicht Summe),
/// siehe `v3.rs`s `BrushStroke`-Moduldoku.
/// Wie stark Auto-Mask (siehe `BrushStroke::auto_mask`s Moduldoku) die
/// Deckkraft an starken lokalen Kanten dämpft — `1.0` würde eine Kante
/// vollständig blockieren (zu hart, ein Strich mittendrin auf einer
/// Kante verschwände dann ganz), `0.85` lässt einen Rest Deckkraft, wie
/// Lightrooms eigenes Auto-Mask ebenfalls keine perfekte Blockade ist.
const AUTO_MASK_EDGE_DAMPING: f32 = 0.85;

fn brush_alpha(
    strokes: &[crate::edl::v3::BrushStroke],
    pixels: &[f32],
    w: usize,
    h: usize,
) -> Vec<f32> {
    if strokes.is_empty() {
        return vec![0.0; w * h];
    }
    // Nur berechnen, wenn mindestens ein Strich Auto-Mask nutzt — dieselbe
    // Laplace-Varianz-Karte wie `BlurDepthApprox` (siehe
    // `relative_sharpness_map`s Moduldoku zur bewussten Wiederverwendung
    // statt einer `apx-ai`-Abhängigkeit), hier als „lokale
    // Gradientenschwelle" zur Kantenerkennung zweckentfremdet: hohe
    // lokale Laplace-Varianz heißt hohe lokale Kantenaktivität.
    let edge_strength: Option<Vec<f32>> = strokes
        .iter()
        .any(|s| s.auto_mask)
        .then(|| relative_sharpness_map(pixels, w, h));

    (0..w * h)
        .into_par_iter()
        .map(|index| {
            let x = index % w;
            let y = index / w;
            let (u, v) = pixel_uv(w, h, x, y);
            let mut coverage = 0.0f32;
            for stroke in strokes {
                if stroke.points.is_empty() {
                    continue;
                }
                let dist = distance_to_stroke(&stroke.points, u, v);
                let inner = (stroke.radius * (1.0 - stroke.feather.clamp(0.0, 1.0))).max(0.0);
                let outer = stroke.radius.max(inner + 1e-4);
                let mut a = 1.0 - smoothstep(inner, outer, dist);
                if stroke.auto_mask {
                    if let Some(edges) = &edge_strength {
                        a *= 1.0 - AUTO_MASK_EDGE_DAMPING * edges[index];
                    }
                }
                coverage = coverage.max(a);
            }
            coverage
        })
        .collect()
}

/// Zusätzliche globale Weichzeichnung der zusammengesetzten Alpha
/// (`Mask::feather`) — ein separierbarer Box-Weichzeichner (keine echte
/// Gauß-Unschärfe, dieselbe Art Vereinfachung wie `details.rs`s
/// Unsharp-Masking-Referenzweichzeichner), Radius proportional zur
/// längeren Bildkante.
fn feather_alpha(alpha: &[f32], w: usize, h: usize, feather_percent: f32) -> Vec<f32> {
    let radius_px =
        ((feather_percent.clamp(0.0, 100.0) / 100.0) * w.max(h) as f32 * 0.05).round() as i32;
    if radius_px <= 0 {
        return alpha.to_vec();
    }
    let horizontal = box_blur_1d(alpha, w, h, radius_px, true);
    box_blur_1d(&horizontal, w, h, radius_px, false)
}

fn box_blur_1d(src: &[f32], w: usize, h: usize, radius: i32, horizontal: bool) -> Vec<f32> {
    (0..w * h)
        .into_par_iter()
        .map(|index| {
            let x = (index % w) as i32;
            let y = (index / w) as i32;
            let mut sum = 0.0f32;
            let mut count = 0.0f32;
            for offset in -radius..=radius {
                let (sx, sy) = if horizontal {
                    (x + offset, y)
                } else {
                    (x, y + offset)
                };
                if sx < 0 || sy < 0 || sx as usize >= w || sy as usize >= h {
                    continue;
                }
                sum += src[sy as usize * w + sx as usize];
                count += 1.0;
            }
            if count > 0.0 {
                sum / count
            } else {
                0.0
            }
        })
        .collect()
}

/// Die Masken, die tatsächlich angewendet werden sollen: `mask.visible`
/// UND (keine Gruppe zugeordnet ODER die zugeordnete Gruppe ist selbst
/// sichtbar) — eine Gruppe ist rein organisatorisch (`MaskGroup`s
/// Moduldoku), ihr `visible` blendet aber tatsächlich alle
/// zugeordneten Masken aus. Von [`apply_all`] genutzt (Phase 6
/// Schritt 7 — vorher unbenutzt, seit Schritt 2 nur vorbereitet).
pub fn visible_masks<'a>(masks: &'a [Mask], groups: &[MaskGroup]) -> Vec<&'a Mask> {
    masks
        .iter()
        .filter(|mask| {
            mask.visible
                && mask
                    .group_id
                    .as_deref()
                    .and_then(|group_id| groups.iter().find(|g| g.id == group_id))
                    .is_none_or(|group| group.visible)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::edl::v3::{MaskComponent, OverlayColor};

    fn full_coverage_mask(adjustments: MaskAdjustments) -> Mask {
        Mask {
            id: "test".to_string(),
            name: "Test".to_string(),
            // Ein Radialverlauf mit riesigem Radius und Feather 0 deckt
            // das gesamte normierte 0..1-Bildquadrat vollständig ab
            // (maximaler Abstand vom Mittelpunkt ist ~0.71, weit unter
            // dem Radius 10) — Grundlage für den Paritätstest unten,
            // ohne eine eigene "immer alles"-Geometrie einführen zu
            // müssen.
            components: vec![MaskComponent {
                geometry: MaskGeometry::RadialGradient {
                    center_x: 0.5,
                    center_y: 0.5,
                    radius_x: 10.0,
                    radius_y: 10.0,
                    angle_degrees: 0.0,
                    feather: 0.0,
                },
                combine: MaskCombine::Add,
                invert: false,
            }],
            adjustments,
            opacity: 100.0,
            feather: 0.0,
            invert: false,
            blend_mode: BlendMode::Normal,
            visible: true,
            group_id: None,
            overlay_color: OverlayColor::Red,
        }
    }

    fn sample_pixels() -> Vec<f32> {
        vec![
            0.2, 0.3, 0.4, // Pixel 0
            0.6, 0.5, 0.1, // Pixel 1
            0.9, 0.1, 0.05, // Pixel 2
            0.05, 0.6, 0.7, // Pixel 3
        ]
    }

    #[test]
    fn full_coverage_mask_matches_global_application_of_the_same_adjustments() {
        let pixels = sample_pixels();
        let mut adjustments = MaskAdjustments::neutral();
        adjustments.basic.exposure_ev = 0.5;
        adjustments.basic.contrast = 15.0;

        let mask = full_coverage_mask(adjustments.clone());
        let via_mask = apply_all(
            &pixels,
            2,
            2,
            [1.0, 1.0, 1.0, 1.0],
            std::slice::from_ref(&mask),
            &[],
        );

        let wb_gains =
            white_balance::compute_gains([1.0, 1.0, 1.0, 1.0], adjustments.basic.white_balance);
        let via_global = basic_fused::apply_cpu(&pixels, wb_gains, &adjustments.basic);

        for (a, b) in via_mask.iter().zip(via_global.iter()) {
            assert!((a - b).abs() < 1e-5, "erwartet {b}, war {a}");
        }
    }

    #[test]
    fn invisible_mask_leaves_the_image_unchanged() {
        let pixels = sample_pixels();
        let mut mask = full_coverage_mask(MaskAdjustments::neutral());
        mask.visible = false;
        mask.adjustments.basic.exposure_ev = 2.0;

        let result = apply_all(
            &pixels,
            2,
            2,
            [1.0, 1.0, 1.0, 1.0],
            std::slice::from_ref(&mask),
            &[],
        );
        assert_eq!(result, pixels);
    }

    #[test]
    fn a_mask_in_an_invisible_group_is_excluded_even_though_the_mask_itself_is_visible() {
        // Phase 6 Schritt 7: `visible_masks` war seit Schritt 2 unbenutzt
        // (nur vorbereitet) — dieser Test belegt, dass `apply_all` es jetzt
        // tatsächlich aufruft.
        let pixels = sample_pixels();
        let mut mask = full_coverage_mask(MaskAdjustments::neutral());
        mask.group_id = Some("group-1".to_string());
        mask.adjustments.basic.exposure_ev = 2.0;
        let groups = vec![MaskGroup {
            id: "group-1".to_string(),
            name: "Testgruppe".to_string(),
            visible: false,
        }];

        let result = apply_all(
            &pixels,
            2,
            2,
            [1.0, 1.0, 1.0, 1.0],
            std::slice::from_ref(&mask),
            &groups,
        );
        assert_eq!(result, pixels);
    }

    #[test]
    fn zero_opacity_leaves_the_image_unchanged() {
        let pixels = sample_pixels();
        let mut adjustments = MaskAdjustments::neutral();
        adjustments.basic.exposure_ev = 2.0;
        let mut mask = full_coverage_mask(adjustments);
        mask.opacity = 0.0;

        let result = apply_all(
            &pixels,
            2,
            2,
            [1.0, 1.0, 1.0, 1.0],
            std::slice::from_ref(&mask),
            &[],
        );
        for (a, b) in result.iter().zip(pixels.iter()) {
            assert!((a - b).abs() < 1e-5);
        }
    }

    #[test]
    fn linear_gradient_alpha_is_one_at_the_start_point_and_zero_at_the_end_point() {
        // Pixelmittelpunkte liegen bei u = 0.125/0.375/0.625/0.875 (siehe
        // `pixel_uv`), nicht exakt auf 0.0/1.0 — die Grenzwerte hier
        // berücksichtigen das.
        let alpha = linear_gradient_alpha(0.0, 0.5, 1.0, 0.5, 4, 1);
        assert!(alpha[0] > alpha[3], "Alpha soll von Start zu Ende abfallen");
        assert!(alpha[0] > 0.8);
        assert!(alpha[3] < 0.2);
    }

    /// Phase 11 Schritt 7 (siehe `DECISIONS.md` ADR-0038): ein
    /// synthetisches Bild mit einer scharfen Vordergrund-Hälfte
    /// (Schachbrettmuster, hoher lokaler Kontrast) und einer unscharfen
    /// Hintergrund-Hälfte (gleichmäßige Fläche, kein lokaler Kontrast)
    /// muss der scharfen Hälfte eine deutlich höhere Alpha zuweisen —
    /// die Kern-Behauptung der Unschärfe-basierten Tiefennäherung.
    #[test]
    fn blur_depth_approx_alpha_favors_the_sharp_half_over_the_uniform_half() {
        let w = 16usize;
        let h = 16usize;
        let mut pixels = vec![0.0f32; w * h * 3];
        for y in 0..h {
            for x in 0..w {
                let idx = (y * w + x) * 3;
                // Linke Hälfte: Schachbrettmuster (hoher lokaler
                // Kontrast → hohe Laplace-Varianz). Rechte Hälfte:
                // gleichmäßiges Mittelgrau (keine lokale Varianz).
                let v = if x < w / 2 {
                    if (x + y) % 2 == 0 {
                        0.9
                    } else {
                        0.1
                    }
                } else {
                    0.5
                };
                pixels[idx] = v;
                pixels[idx + 1] = v;
                pixels[idx + 2] = v;
            }
        }

        let alpha = blur_depth_approx_alpha(0.3, &pixels, w, h);

        let sharp_avg: f32 = (0..h).map(|y| alpha[y * w + 2]).sum::<f32>() / h as f32;
        let uniform_avg: f32 = (0..h).map(|y| alpha[y * w + (w - 2)]).sum::<f32>() / h as f32;
        assert!(
            sharp_avg > uniform_avg + 0.3,
            "scharfe Hälfte ({sharp_avg}) sollte deutlich höhere Alpha haben als die gleichmäßige Hälfte ({uniform_avg})"
        );
    }

    #[test]
    fn luminance_range_alpha_is_high_inside_the_range_and_low_outside() {
        let bright = vec![0.9, 0.9, 0.9];
        let dark = vec![0.05, 0.05, 0.05];
        let alpha_bright = luminance_range_alpha(0.7, 1.0, 0.05, &bright);
        let alpha_dark = luminance_range_alpha(0.7, 1.0, 0.05, &dark);
        assert!(alpha_bright[0] > 0.9);
        assert!(alpha_dark[0] < 0.1);
    }

    #[test]
    fn color_range_alpha_is_high_near_the_target_color_and_low_far_from_it() {
        let matching = vec![0.8, 0.2, 0.2];
        let different = vec![0.1, 0.8, 0.1];
        let alpha_match = color_range_alpha((0.8, 0.2, 0.2), 0.05, 0.1, &matching);
        let alpha_diff = color_range_alpha((0.8, 0.2, 0.2), 0.05, 0.1, &different);
        assert!(alpha_match[0] > 0.9);
        assert!(alpha_diff[0] < 0.1);
    }

    #[test]
    fn brush_alpha_is_covered_near_a_stroke_point_and_uncovered_far_from_it() {
        let strokes = vec![crate::edl::v3::BrushStroke {
            points: vec![crate::edl::v3::MaskPoint { x: 0.5, y: 0.5 }],
            radius: 0.1,
            feather: 0.5,
            auto_mask: false,
        }];
        let pixels = vec![0.5f32; 3 * 3 * 3];
        let alpha = brush_alpha(&strokes, &pixels, 3, 3);
        // Mittleres Pixel (1,1) liegt exakt auf dem Stützpunkt.
        assert!(
            alpha[4] > 0.9,
            "Mittelpunkt sollte voll gedeckt sein: {}",
            alpha[4]
        );
        // Eckpixel (0,0) ist weit außerhalb des Radius.
        assert!(alpha[0] < 0.1, "Ecke sollte ungedeckt sein: {}", alpha[0]);
    }

    /// Phase 12 Schritt 2 (siehe `DECISIONS.md` ADR-0039): Auto-Mask
    /// dämpft die Deckkraft eines Strichs an starken lokalen Kanten,
    /// lässt sie in flachen Bereichen aber unverändert.
    #[test]
    fn brush_alpha_with_auto_mask_is_dampened_at_a_strong_edge_but_not_in_flat_areas() {
        // Scharfe vertikale Kante bei Spalte 6: linke Hälfte dunkel, rechte hell.
        let (w, h) = (12usize, 12usize);
        let mut pixels = vec![0.0f32; w * h * 3];
        for y in 0..h {
            for x in 0..w {
                let v = if x < w / 2 { 0.1 } else { 0.9 };
                let i = (y * w + x) * 3;
                pixels[i] = v;
                pixels[i + 1] = v;
                pixels[i + 2] = v;
            }
        }
        // Ein einzelner Strich über die Bildmitte, hart (kein Feather),
        // deckt sowohl die Kante (x=6) als auch eine flache Stelle (x=10) ab.
        let base_stroke = crate::edl::v3::MaskPoint { x: 0.5, y: 0.5 };
        let edge_index = 6 * w + 6; // Pixel direkt auf der Kante.
        let flat_index = 6 * w + 10; // Pixel deutlich in der flachen rechten Hälfte.

        let without_auto_mask = vec![crate::edl::v3::BrushStroke {
            points: vec![base_stroke],
            radius: 0.45,
            feather: 0.0,
            auto_mask: false,
        }];
        let alpha_plain = brush_alpha(&without_auto_mask, &pixels, w, h);
        assert!(
            alpha_plain[edge_index] > 0.9 && alpha_plain[flat_index] > 0.9,
            "ohne Auto-Mask sollte die Deckkraft überall im Radius voll sein: Kante={}, flach={}",
            alpha_plain[edge_index],
            alpha_plain[flat_index]
        );

        let with_auto_mask = vec![crate::edl::v3::BrushStroke {
            points: vec![base_stroke],
            radius: 0.45,
            feather: 0.0,
            auto_mask: true,
        }];
        let alpha_auto = brush_alpha(&with_auto_mask, &pixels, w, h);
        assert!(
            alpha_auto[edge_index] < 0.5,
            "Auto-Mask sollte die Deckkraft auf der scharfen Kante deutlich dämpfen: {}",
            alpha_auto[edge_index]
        );
        assert!(
            alpha_auto[flat_index] > 0.8,
            "Auto-Mask sollte die Deckkraft in der flachen Zone kaum verändern: {}",
            alpha_auto[flat_index]
        );
    }

    #[test]
    fn add_combine_takes_the_union_of_two_components() {
        let mask = Mask {
            components: vec![
                MaskComponent {
                    geometry: MaskGeometry::LuminanceRange {
                        range_min: 0.0,
                        range_max: 0.3,
                        feather: 0.01,
                    },
                    combine: MaskCombine::Add,
                    invert: false,
                },
                MaskComponent {
                    geometry: MaskGeometry::LuminanceRange {
                        range_min: 0.7,
                        range_max: 1.0,
                        feather: 0.01,
                    },
                    combine: MaskCombine::Add,
                    invert: false,
                },
            ],
            ..full_coverage_mask(MaskAdjustments::neutral())
        };
        let dark = vec![0.05, 0.05, 0.05];
        let bright = vec![0.95, 0.95, 0.95];
        let mid = vec![0.5, 0.5, 0.5];
        assert!(compose_mask_alpha(&mask, &dark, 1, 1)[0] > 0.9);
        assert!(compose_mask_alpha(&mask, &bright, 1, 1)[0] > 0.9);
        assert!(compose_mask_alpha(&mask, &mid, 1, 1)[0] < 0.1);
    }

    #[test]
    fn subtract_combine_removes_the_second_components_coverage() {
        let mask = Mask {
            components: vec![
                MaskComponent {
                    geometry: MaskGeometry::RadialGradient {
                        center_x: 0.5,
                        center_y: 0.5,
                        radius_x: 10.0,
                        radius_y: 10.0,
                        angle_degrees: 0.0,
                        feather: 0.0,
                    },
                    combine: MaskCombine::Add,
                    invert: false,
                },
                MaskComponent {
                    geometry: MaskGeometry::LuminanceRange {
                        range_min: 0.0,
                        range_max: 1.0,
                        feather: 0.01,
                    },
                    combine: MaskCombine::Subtract,
                    invert: false,
                },
            ],
            ..full_coverage_mask(MaskAdjustments::neutral())
        };
        // Die zweite Komponente deckt den vollen Luminanzbereich ab und
        // subtrahiert damit die gesamte erste Komponente wieder weg.
        let pixels = vec![0.5, 0.5, 0.5];
        assert!(compose_mask_alpha(&mask, &pixels, 1, 1)[0] < 0.1);
    }

    // ---- Ebenen-Mischmodi (Schritt 6) ----------------------------------

    #[test]
    fn normal_blend_mode_ignores_the_base_pixel_entirely() {
        let base = [0.2, 0.3, 0.4];
        let adjusted = [0.9, 0.1, 0.5];
        assert_eq!(blend_pixel(base, adjusted, BlendMode::Normal), adjusted);
    }

    #[test]
    fn multiply_blend_mode_with_a_white_adjusted_layer_leaves_the_base_unchanged() {
        let base = [0.2, 0.3, 0.4];
        let white = [1.0, 1.0, 1.0];
        let blended = blend_pixel(base, white, BlendMode::Multiply);
        for i in 0..3 {
            assert!((blended[i] - base[i]).abs() < 1e-6);
        }
    }

    #[test]
    fn multiply_blend_mode_with_a_black_adjusted_layer_yields_black() {
        let base = [0.2, 0.3, 0.4];
        let black = [0.0, 0.0, 0.0];
        assert_eq!(
            blend_pixel(base, black, BlendMode::Multiply),
            [0.0, 0.0, 0.0]
        );
    }

    #[test]
    fn soft_light_blend_mode_with_mid_gray_adjusted_leaves_the_base_unchanged() {
        // Reine mathematische Eigenschaft der Soft-Light-Formel: bei
        // `cs == 0.5` ist der Term `(1 - 2*cs)` bzw. `(2*cs - 1)` null,
        // das Ergebnis bleibt also exakt `cb`.
        let base = [0.2, 0.3, 0.4];
        let mid_gray = [0.5, 0.5, 0.5];
        let blended = blend_pixel(base, mid_gray, BlendMode::SoftLight);
        for i in 0..3 {
            assert!((blended[i] - base[i]).abs() < 1e-6);
        }
    }

    #[test]
    fn soft_light_blend_mode_stays_within_the_valid_range() {
        for cb in [0.0, 0.1, 0.25, 0.5, 0.9, 1.0] {
            for cs in [0.0, 0.3, 0.5, 0.7, 1.0] {
                let result = soft_light_channel(cb, cs);
                assert!(
                    (-1e-4..=1.0001).contains(&result),
                    "cb={cb} cs={cs} -> {result}"
                );
            }
        }
    }

    #[test]
    fn color_blend_mode_takes_hue_and_saturation_from_adjusted_and_luminance_from_base() {
        let base = [0.8, 0.8, 0.8]; // neutralgrau, hohe Luminanz
        let adjusted = [0.8, 0.2, 0.2]; // gesättigtes Rot, niedrigere Luminanz
        let blended = blend_pixel(base, adjusted, BlendMode::Color);
        // Das Ergebnis übernimmt die Luminanz von `base` (hell)...
        assert!((luminosity(blended) - luminosity(base)).abs() < 1e-4);
        // ...bleibt aber farbig (nicht neutralgrau wie `base`).
        assert!(blended[0] - blended[1] > 0.05);
    }

    #[test]
    fn luminosity_blend_mode_takes_luminance_from_adjusted_and_hue_saturation_from_base() {
        let base = [0.8, 0.2, 0.2]; // gesättigtes Rot
        let adjusted = [0.3, 0.3, 0.3]; // dunkles Neutralgrau
        let blended = blend_pixel(base, adjusted, BlendMode::Luminosity);
        // Das Ergebnis übernimmt die (niedrige) Luminanz von `adjusted`...
        assert!((luminosity(blended) - luminosity(adjusted)).abs() < 1e-4);
        // ...bleibt aber farbig (die Rot-Tönung von `base` bleibt erhalten).
        assert!(blended[0] - blended[1] > 0.05);
    }

    #[test]
    fn set_luminosity_clips_out_of_range_results_back_into_zero_to_one() {
        // Eine sehr hohe Ziel-Luminanz auf einem bereits hellen Pixel
        // würde ohne `clip_color` Kanäle über 1.0 treiben.
        let bright = [0.9, 0.95, 0.99];
        let result = set_luminosity(bright, 1.0);
        for channel in result {
            assert!((-1e-4..=1.0001).contains(&channel), "channel={channel}");
        }
    }
}
