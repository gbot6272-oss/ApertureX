//! Reparatur-Erweiterungen (Phase 7, `DECISIONS.md` ADR-0032 Punkt 8 hat
//! sie hierher vorgemerkt, ADR-0033 Punkt 4 legt die Umsetzung fest):
//! **einmalige Analyse-Befehle**, im Unterschied zum render-zeitlichen
//! `RepairMode::ContentAwareFill`, das in
//! `apx_pipeline::stages::repair` bleibt (es läuft bei *jedem* Rendering,
//! nicht nur einmal auf Knopfdruck).
//!
//! - [`suggest_source_point`]: Auto-Quellenfindung — sucht in einem Ring
//!   um den Zielpunkt das Patch mit der höchsten normierten
//!   Kreuzkorrelation zur Zielumgebung (das klassische Template-Matching-
//!   Maß, invariant gegen Helligkeits-/Kontrast-Unterschiede).
//! - [`detect_spots`]: Sensorflecken-Visualisierung — Blob-Erkennung per
//!   lokaler Kontrast-Anomalie gegen ein weichgezeichnetes Referenzbild
//!   (Sensorflecken sind kleine, dunkle, kontrastarme Abweichungen in
//!   sonst glatten Flächen, typischerweise im Himmel).
//!
//! Beide arbeiten wie [`crate::segmentation`] auf einem interleaved
//! linearen RGB-`f32`-Puffer und in normierten Bildkoordinaten
//! (`0.0..=1.0`), passend zu `apx_pipeline::edl::RepairPoint`.

use crate::blur::approximate_gaussian_blur;
use crate::color::luminance;
use crate::error::{AiError, Result};

/// Ein erkannter Sensorfleck-Kandidat in normierten Bildkoordinaten.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpotCandidate {
    pub x: f32,
    pub y: f32,
    /// Geschätzter Radius in normierten Koordinaten (Anteil der langen
    /// Bildkante) — als Vorgabe für den Reparatur-Pinsel gedacht.
    pub radius: f32,
    /// Wie stark der Fleck vom Umfeld abweicht (`0.0..=1.0`, höher =
    /// auffälliger). Erlaubt dem Frontend, nur die deutlichsten zu zeigen.
    pub strength: f32,
}

fn luma_plane(pixels: &[f32], width: u32, height: u32) -> Result<Vec<f32>> {
    if width == 0 || height == 0 {
        return Err(AiError::Analysis {
            message: format!("Bild ist {width}×{height}"),
        });
    }
    let count = (width as usize) * (height as usize);
    if pixels.len() != count * 3 {
        return Err(AiError::Analysis {
            message: format!(
                "Pufferlänge {} passt nicht zu {width}×{height}",
                pixels.len()
            ),
        });
    }
    Ok((0..count)
        .map(|i| luminance(pixels[i * 3], pixels[i * 3 + 1], pixels[i * 3 + 2]))
        .collect())
}

/// Normierte Kreuzkorrelation zweier gleich großer Patches (`-1.0..=1.0`,
/// `1.0` = identisches Muster bis auf Helligkeit/Kontrast). Zwei
/// vollkommen flache Patches gelten als perfekt ähnlich (`1.0`) — ohne
/// Struktur gibt es nichts zu unterscheiden.
fn normalized_cross_correlation(a: &[f32], b: &[f32]) -> f32 {
    debug_assert_eq!(a.len(), b.len());
    let n = a.len() as f32;
    if n == 0.0 {
        return 0.0;
    }
    let mean_a = a.iter().sum::<f32>() / n;
    let mean_b = b.iter().sum::<f32>() / n;

    let mut cov = 0.0f32;
    let mut var_a = 0.0f32;
    let mut var_b = 0.0f32;
    for (va, vb) in a.iter().zip(b.iter()) {
        let da = va - mean_a;
        let db = vb - mean_b;
        cov += da * db;
        var_a += da * da;
        var_b += db * db;
    }
    let denom = (var_a * var_b).sqrt();
    if denom <= 1e-9 {
        // Beide strukturlos → perfekt austauschbar.
        return 1.0;
    }
    cov / denom
}

/// Liest ein quadratisches Patch der Kantenlänge `2 * radius + 1` um
/// `(cx, cy)` aus `plane`, Randpixel geklemmt (dieselbe Konvention wie
/// [`crate::blur`]).
fn patch_at(plane: &[f32], width: usize, height: usize, cx: i64, cy: i64, radius: i64) -> Vec<f32> {
    let side = (2 * radius + 1) as usize;
    let mut patch = Vec::with_capacity(side * side);
    for dy in -radius..=radius {
        for dx in -radius..=radius {
            let x = (cx + dx).clamp(0, width as i64 - 1) as usize;
            let y = (cy + dy).clamp(0, height as i64 - 1) as usize;
            patch.push(plane[y * width + x]);
        }
    }
    patch
}

/// **Auto-Quellenfindung** — sucht rings um `(target_x, target_y)` (in
/// normierten Bildkoordinaten) die Position, deren Umgebung der
/// Zielumgebung am ähnlichsten ist, und liefert sie als Quellpunkt für
/// einen Klon-/Reparatur-Strich zurück.
///
/// **Bewusste Vereinfachung:** durchsucht nur einen Ring von Kandidaten
/// in wenigen festen Abständen und Winkeln (statt jeder möglichen
/// Position im ganzen Bild) — eine erschöpfende Suche wäre bei
/// Vorschau-Auflösung bereits Millionen Patch-Vergleiche und für einen
/// interaktiven Knopfdruck zu langsam. Genau dieselbe Art Beschränkung,
/// die auch echte Editoren für ihre „Quelle automatisch wählen"-Funktion
/// nutzen.
pub fn suggest_source_point(
    pixels: &[f32],
    width: u32,
    height: u32,
    target_x: f32,
    target_y: f32,
    brush_radius: f32,
) -> Result<(f32, f32)> {
    let plane = luma_plane(pixels, width, height)?;
    let w = width as usize;
    let h = height as usize;
    let long_edge = width.max(height) as f32;

    let cx = (target_x.clamp(0.0, 1.0) * width as f32) as i64;
    let cy = (target_y.clamp(0.0, 1.0) * height as f32) as i64;

    // Patch-Radius aus dem Pinselradius, aber nach oben gedeckelt — ein
    // sehr großer Pinsel würde sonst pro Kandidat ein riesiges Patch
    // vergleichen.
    let patch_radius = ((brush_radius * long_edge).round() as i64).clamp(2, 16);
    let target_patch = patch_at(&plane, w, h, cx, cy, patch_radius);

    // Kandidaten-Ringe: der Quellpunkt soll weit genug weg liegen, um
    // nicht denselben Defekt zu erwischen, aber nah genug, um zur
    // Umgebung zu passen.
    let base = (patch_radius * 3).max(8);
    let distances = [base, base * 2, base * 3];
    // 16 Winkel — feiner als nötig für ein brauchbares Ergebnis, aber
    // immer noch nur ~48 Patch-Vergleiche insgesamt.
    let angle_steps = 16;

    let mut best: Option<((f32, f32), f32)> = None;
    for &distance in &distances {
        for step in 0..angle_steps {
            let angle = (step as f32 / angle_steps as f32) * std::f32::consts::TAU;
            let sx = cx + (angle.cos() * distance as f32).round() as i64;
            let sy = cy + (angle.sin() * distance as f32).round() as i64;
            // Kandidaten außerhalb des Bildes überspringen statt zu
            // klemmen — geklemmte Kandidaten würden alle auf denselben
            // Randpunkt zusammenfallen.
            if sx < 0 || sy < 0 || sx >= w as i64 || sy >= h as i64 {
                continue;
            }
            let candidate = patch_at(&plane, w, h, sx, sy, patch_radius);
            let score = normalized_cross_correlation(&target_patch, &candidate);
            if best.is_none_or(|(_, best_score)| score > best_score) {
                let nx = (sx as f32 + 0.5) / width as f32;
                let ny = (sy as f32 + 0.5) / height as f32;
                best = Some(((nx, ny), score));
            }
        }
    }

    best.map(|(point, _)| point)
        .ok_or_else(|| AiError::Analysis {
            message: "kein gültiger Quellpunkt im Bild gefunden (Bild zu klein?)".to_string(),
        })
}

/// **Sensorflecken-Visualisierung** — findet kleine, dunkle
/// Kontrast-Anomalien in sonst glatten Flächen: Sensorstaub zeigt sich als
/// lokaler Abfall gegenüber der weichgezeichneten Umgebung, dort wo die
/// Umgebung selbst kaum Struktur hat (im Himmel, nicht im Laub).
///
/// `sensitivity` (`0.0..=1.0`) steuert die Erkennungsschwelle, `max_spots`
/// deckelt die Ergebnisliste (die stärksten zuerst).
pub fn detect_spots(
    pixels: &[f32],
    width: u32,
    height: u32,
    sensitivity: f32,
    max_spots: usize,
) -> Result<Vec<SpotCandidate>> {
    let plane = luma_plane(pixels, width, height)?;
    let w = width as usize;
    let h = height as usize;
    let long_edge = width.max(height) as f32;

    // Ein Fleck ist klein: Referenz-Weichzeichnung deutlich größer als
    // der erwartete Fleckdurchmesser, damit der Fleck selbst darin
    // „verschwindet" und die Differenz ihn hervorhebt.
    let spot_radius_px = ((long_edge * 0.006).round() as u32).max(2);
    let reference_radius = spot_radius_px * 4;
    let reference = approximate_gaussian_blur(&plane, width, height, reference_radius);
    // Zweite, noch weitere Weichzeichnung der *Abweichungsbeträge* misst,
    // wie strukturiert die Umgebung insgesamt ist.
    let deviation: Vec<f32> = plane
        .iter()
        .zip(reference.iter())
        .map(|(v, r)| (v - r).abs())
        .collect();
    let busyness = approximate_gaussian_blur(&deviation, width, height, reference_radius * 2);

    // Schwelle: bei sensitivity=1.0 reicht eine sehr kleine Abweichung,
    // bei 0.0 nur eine sehr deutliche.
    let sensitivity = sensitivity.clamp(0.0, 1.0);
    let threshold = 0.05 - 0.045 * sensitivity;

    let mut candidates: Vec<SpotCandidate> = Vec::new();
    let step = spot_radius_px.max(1) as usize;
    for y in (spot_radius_px as usize..h.saturating_sub(spot_radius_px as usize)).step_by(step) {
        for x in (spot_radius_px as usize..w.saturating_sub(spot_radius_px as usize)).step_by(step)
        {
            let i = y * w + x;
            // Nur *dunklere* Abweichungen — Sensorstaub verdunkelt.
            let drop = reference[i] - plane[i];
            if drop < threshold {
                continue;
            }
            // In strukturierter Umgebung (Laub, Kanten) ist eine
            // Abweichung normal und kein Fleck. Verglichen wird
            // *relativ zum Abfall selbst*, nicht gegen einen absoluten
            // Wert: der Fleck trägt zwangsläufig auch zu seiner eigenen
            // Umgebungs-Aktivität bei (das war in einer früheren Fassung
            // ein Fehler — ein deutlicher Fleck hat sich damit selbst
            // aussortiert). Ein echter Fleck sticht klar aus der
            // allgemeinen lokalen Aktivität heraus, in Laub sind beide
            // Werte vergleichbar groß.
            if busyness[i] > drop * 0.35 {
                continue;
            }
            candidates.push(SpotCandidate {
                x: (x as f32 + 0.5) / width as f32,
                y: (y as f32 + 0.5) / height as f32,
                radius: spot_radius_px as f32 / long_edge,
                strength: (drop / 0.2).clamp(0.0, 1.0),
            });
        }
    }

    // Stärkste zuerst, dann räumlich ausdünnen (ein Fleck erzeugt sonst
    // mehrere benachbarte Treffer).
    candidates.sort_by(|a, b| {
        b.strength
            .partial_cmp(&a.strength)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let min_separation = (spot_radius_px as f32 * 3.0) / long_edge;
    let mut kept: Vec<SpotCandidate> = Vec::new();
    for candidate in candidates {
        if kept.len() >= max_spots {
            break;
        }
        let too_close = kept.iter().any(|k| {
            let dx = k.x - candidate.x;
            let dy = k.y - candidate.y;
            (dx * dx + dy * dy).sqrt() < min_separation
        });
        if !too_close {
            kept.push(candidate);
        }
    }
    Ok(kept)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flat_image(width: u32, height: u32, value: f32) -> Vec<f32> {
        vec![value; (width * height * 3) as usize]
    }

    #[test]
    fn cross_correlation_of_identical_patches_is_one() {
        let a = vec![0.1, 0.5, 0.9, 0.3];
        assert!((normalized_cross_correlation(&a, &a) - 1.0).abs() < 1e-5);
    }

    #[test]
    fn cross_correlation_ignores_brightness_offset() {
        let a = vec![0.1, 0.5, 0.9, 0.3];
        let b: Vec<f32> = a.iter().map(|v| v + 0.2).collect();
        assert!((normalized_cross_correlation(&a, &b) - 1.0).abs() < 1e-5);
    }

    #[test]
    fn cross_correlation_of_inverted_patch_is_negative() {
        let a = vec![0.1, 0.5, 0.9, 0.3];
        let b: Vec<f32> = a.iter().map(|v| 1.0 - v).collect();
        assert!(normalized_cross_correlation(&a, &b) < -0.9);
    }

    #[test]
    fn suggest_source_point_rejects_a_mismatched_buffer() {
        assert!(suggest_source_point(&[0.0; 5], 8, 8, 0.5, 0.5, 0.05).is_err());
    }

    #[test]
    fn suggest_source_point_returns_a_point_inside_the_image() {
        let pixels = flat_image(64, 64, 0.5);
        let (x, y) = suggest_source_point(&pixels, 64, 64, 0.5, 0.5, 0.05).expect("Quellpunkt");
        assert!((0.0..=1.0).contains(&x));
        assert!((0.0..=1.0).contains(&y));
        // Der Vorschlag darf nicht der Zielpunkt selbst sein.
        assert!((x - 0.5).abs() > 1e-3 || (y - 0.5).abs() > 1e-3);
    }

    #[test]
    fn suggest_source_point_prefers_the_matching_half_of_a_two_tone_image() {
        // Linke Hälfte glatt, rechte Hälfte stark gestreift. Ein Ziel in
        // der linken Hälfte muss eine Quelle in der glatten Hälfte
        // bevorzugen (höhere Korrelation als zum Streifenmuster).
        let width = 96u32;
        let height = 48u32;
        let mut pixels = Vec::new();
        for y in 0..height {
            for x in 0..width {
                let v = if x < width / 2 {
                    0.5
                } else if (y / 2) % 2 == 0 {
                    0.1
                } else {
                    0.9
                };
                pixels.extend_from_slice(&[v, v, v]);
            }
        }
        let (sx, _sy) =
            suggest_source_point(&pixels, width, height, 0.25, 0.5, 0.03).expect("Quellpunkt");
        assert!(
            sx < 0.5,
            "Quelle sollte in der glatten linken Hälfte liegen, war {sx}"
        );
    }

    #[test]
    fn detect_spots_finds_nothing_in_a_perfectly_flat_image() {
        let pixels = flat_image(64, 64, 0.6);
        let spots = detect_spots(&pixels, 64, 64, 0.8, 20).expect("Analyse");
        assert!(spots.is_empty(), "flaches Bild darf keine Flecken liefern");
    }

    #[test]
    fn detect_spots_finds_a_dark_blob_on_a_smooth_background() {
        let width = 96u32;
        let height = 96u32;
        let mut pixels = flat_image(width, height, 0.7);
        // Dunkler Fleck bei (48, 48), Radius ~3px.
        for y in 45..52u32 {
            for x in 45..52u32 {
                let i = ((y * width + x) * 3) as usize;
                pixels[i] = 0.35;
                pixels[i + 1] = 0.35;
                pixels[i + 2] = 0.35;
            }
        }
        let spots = detect_spots(&pixels, width, height, 0.9, 20).expect("Analyse");
        assert!(!spots.is_empty(), "Fleck muss gefunden werden");
        let found = spots
            .iter()
            .any(|s| (s.x - 0.5).abs() < 0.1 && (s.y - 0.5).abs() < 0.1);
        assert!(found, "Fleck muss nahe der Bildmitte liegen: {spots:?}");
    }

    #[test]
    fn detect_spots_ignores_dark_pixels_inside_a_busy_textured_area() {
        // Hochfrequentes Schachbrett („Laub"): jeder dunkle Pixel weicht
        // stark vom weichgezeichneten Mittel ab, ist aber kein Fleck —
        // genau der Fall, den die Busyness-Prüfung aussortieren muss.
        // Ohne sie wäre der Test rot (die Prüfung ist also nicht leer).
        let width = 96u32;
        let height = 96u32;
        let mut pixels = Vec::new();
        for y in 0..height {
            for x in 0..width {
                let v = if (x / 2 + y / 2) % 2 == 0 { 0.25 } else { 0.75 };
                pixels.extend_from_slice(&[v, v, v]);
            }
        }
        let spots = detect_spots(&pixels, width, height, 0.9, 20).expect("Analyse");
        assert!(
            spots.is_empty(),
            "strukturierte Fläche darf keine Flecken liefern, waren {}",
            spots.len()
        );
    }

    #[test]
    fn detect_spots_respects_the_max_spots_cap() {
        let width = 96u32;
        let height = 96u32;
        let mut pixels = flat_image(width, height, 0.7);
        // Mehrere dunkle Flecken in einem Raster.
        for cy in [20u32, 48, 76] {
            for cx in [20u32, 48, 76] {
                for y in cy - 2..cy + 3 {
                    for x in cx - 2..cx + 3 {
                        let i = ((y * width + x) * 3) as usize;
                        pixels[i] = 0.3;
                        pixels[i + 1] = 0.3;
                        pixels[i + 2] = 0.3;
                    }
                }
            }
        }
        let spots = detect_spots(&pixels, width, height, 0.9, 4).expect("Analyse");
        assert!(spots.len() <= 4);
    }
}
