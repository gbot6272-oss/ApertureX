//! Gradationskurven (`SPEC.md` §3.2 „Kurven") — RGB-Verbundkurve, R/G/B
//! einzeln und eine Luminanz-Kurve, je Kanal entweder als Punktkurve
//! (monotone kubische Spline durch frei gesetzte Kontrollpunkte) oder als
//! parametrische Kurve (vier Tonwertzonen-Regler).
//!
//! **Sequenzierung (siehe `PLAN.md` Phase 4 Schritt 4, dort als offene
//! Frage aus Schritt 2 übernommen):** Kurven laufen — wie `contrast.rs`s
//! Moduldoku bereits für Phase 4 ankündigt — *nach* Farbmanagement/
//! Output-Transform, also auf dem bereits gamma-kodierten,
//! quantisierten RGBA8-Ausgabepuffer (`crate::color::
//! linear_camera_rgb_to_srgb_rgba8`s Ergebnis), nicht auf den linearen
//! Kamera-RGB-Werten davor. Damit entfällt für Kurven jede GPU-Variante:
//! eine 256-Einträge-Lookup-Tabelle pro Kanal ist so billig (ein
//! Feld-Zugriff plus eine Fließkomma-Interpolation je Pixel), dass ein
//! zusätzlicher GPU-Dispatch nur zusätzliche Rundlauf-Kosten hätte, ohne
//! nennenswerten Geschwindigkeitsgewinn — `rayon` über den u8-Puffer
//! reicht fürs 16-ms-Ziel bequem aus. Dieselbe CPU-Nachschritt-Idee trägt
//! später auch Schritt 11s Crop/Geometrie (siehe dortige Planung).
//!
//! **Verkettungsreihenfolge der fünf Kurven** (eigene, hier getroffene
//! Festlegung — es gibt dafür keinen universellen Standard): zuerst die
//! Luminanz-Kurve (ihr Effekt wird als gleicher Delta-Betrag auf alle
//! drei Kanäle addiert, damit Farbton/Sättigung erhalten bleiben statt
//! sich durch unterschiedliche Kanal-Verhältnisse zu verschieben), dann
//! die RGB-Verbundkurve identisch auf alle drei Kanäle, zuletzt die
//! individuellen R-/G-/B-Kurven auf ihren jeweils eigenen Kanal.

use rayon::prelude::*;

use crate::edl::v2::{CurveChannel, CurvePoint, CurvesAdjustment};

const LUT_SIZE: usize = 256;

fn identity_lut() -> [f32; LUT_SIZE] {
    let mut lut = [0.0f32; LUT_SIZE];
    for (i, entry) in lut.iter_mut().enumerate() {
        *entry = i as f32 / (LUT_SIZE - 1) as f32;
    }
    lut
}

/// Fritsch-Carlson-Tangenten je Kontrollpunkt — verhindert das Über-/
/// Unterschwingen, das eine naive kubische Spline bei steilen lokalen
/// Anstiegen zeigen würde (siehe Modultests
/// `spline_stays_monotonic_for_a_steep_local_rise`).
fn fritsch_carlson_tangents(points: &[CurvePoint]) -> Vec<f32> {
    let n = points.len();
    let mut secants = vec![0.0f32; n - 1];
    for i in 0..n - 1 {
        let dx = (points[i + 1].input - points[i].input).max(1e-6);
        secants[i] = (points[i + 1].output - points[i].output) / dx;
    }

    let mut tangents = vec![0.0f32; n];
    tangents[0] = secants[0];
    tangents[n - 1] = secants[n - 2];
    for i in 1..n - 1 {
        tangents[i] = if secants[i - 1] * secants[i] <= 0.0 {
            0.0 // lokales Extremum — waagrechte Tangente vermeidet Überschwingen
        } else {
            (secants[i - 1] + secants[i]) / 2.0
        };
    }

    for i in 0..n - 1 {
        if secants[i] == 0.0 {
            tangents[i] = 0.0;
            tangents[i + 1] = 0.0;
            continue;
        }
        if tangents[i] / secants[i] < 0.0 {
            tangents[i] = 0.0;
        }
        if tangents[i + 1] / secants[i] < 0.0 {
            tangents[i + 1] = 0.0;
        }
        let alpha = tangents[i] / secants[i];
        let beta = tangents[i + 1] / secants[i];
        let sum_sq = alpha * alpha + beta * beta;
        if sum_sq > 9.0 {
            let tau = 3.0 / sum_sq.sqrt();
            tangents[i] = tau * alpha * secants[i];
            tangents[i + 1] = tau * beta * secants[i];
        }
    }

    tangents
}

fn evaluate_spline(points: &[CurvePoint], tangents: &[f32], x: f32) -> f32 {
    let n = points.len();
    if x <= points[0].input {
        return points[0].output.clamp(0.0, 1.0);
    }
    if x >= points[n - 1].input {
        return points[n - 1].output.clamp(0.0, 1.0);
    }

    let mut segment = 0;
    for i in 0..n - 1 {
        if x >= points[i].input && x <= points[i + 1].input {
            segment = i;
            break;
        }
    }

    let x0 = points[segment].input;
    let x1 = points[segment + 1].input;
    let y0 = points[segment].output;
    let y1 = points[segment + 1].output;
    let h = (x1 - x0).max(1e-6);
    let t = (x - x0) / h;

    let h00 = 2.0 * t.powi(3) - 3.0 * t.powi(2) + 1.0;
    let h10 = t.powi(3) - 2.0 * t.powi(2) + t;
    let h01 = -2.0 * t.powi(3) + 3.0 * t.powi(2);
    let h11 = t.powi(3) - t.powi(2);

    let y = h00 * y0 + h10 * h * tangents[segment] + h01 * y1 + h11 * h * tangents[segment + 1];
    y.clamp(0.0, 1.0)
}

fn build_points_lut(points: &[CurvePoint]) -> [f32; LUT_SIZE] {
    let mut sorted: Vec<CurvePoint> = points.to_vec();
    sorted.sort_by(|a, b| {
        a.input
            .partial_cmp(&b.input)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let mut unique: Vec<CurvePoint> = Vec::with_capacity(sorted.len());
    for point in sorted {
        match unique.last_mut() {
            Some(last) if (last.input - point.input).abs() < 1e-6 => *last = point,
            _ => unique.push(point),
        }
    }

    if unique.len() < 2 {
        // Entartungsfall (sollte durch die Frontend-Validierung nie
        // eintreten, siehe `frontend/src/lib/edl.ts`) — Identität als
        // sicherer Rückfall statt eines Absturzes.
        return identity_lut();
    }

    let tangents = fritsch_carlson_tangents(&unique);
    let mut lut = [0.0f32; LUT_SIZE];
    for (i, entry) in lut.iter_mut().enumerate() {
        let x = i as f32 / (LUT_SIZE - 1) as f32;
        *entry = evaluate_spline(&unique, &tangents, x);
    }
    lut
}

/// Vereinfachtes parametrisches Modell: statt echter, im Editor
/// verschiebbarer Split-Punkte vier feste, Gauß-gewichtete Tonwertzonen
/// um 0/⅓/⅔/1 — konsistent mit dem Gewichtungsprinzip aus
/// `highlights_shadows.rs`/`whites_blacks.rs` (dieselbe `* 0.3`-Skala für
/// den maximalen Effekt eines voll ausgeschlagenen Reglers).
fn build_parametric_lut(shadows: f32, darks: f32, lights: f32, highlights: f32) -> [f32; LUT_SIZE] {
    const SIGMA: f32 = 0.25;
    let regions = [
        (0.0, shadows),
        (1.0 / 3.0, darks),
        (2.0 / 3.0, lights),
        (1.0, highlights),
    ];

    let mut lut = [0.0f32; LUT_SIZE];
    for (i, entry) in lut.iter_mut().enumerate() {
        let v = i as f32 / (LUT_SIZE - 1) as f32;
        let mut delta = 0.0;
        for (center, amount) in regions {
            let d = v - center;
            let weight = (-(d * d) / (2.0 * SIGMA * SIGMA)).exp();
            delta += (amount / 100.0) * weight * 0.3;
        }
        *entry = (v + delta).clamp(0.0, 1.0);
    }
    lut
}

fn build_lut(channel: &CurveChannel) -> [f32; LUT_SIZE] {
    match channel {
        CurveChannel::Points { points } => build_points_lut(points),
        CurveChannel::Parametric {
            shadows,
            darks,
            lights,
            highlights,
        } => build_parametric_lut(*shadows, *darks, *lights, *highlights),
    }
}

/// Liest `lut` an `value` (`0.0..=1.0`) mit linearer Interpolation
/// zwischen den beiden nächstgelegenen der 256 Stützstellen — glättet die
/// sonst sichtbare Bandbildung, da `value` selbst nicht auf ein
/// Achtel-Bit-Raster fällt.
fn sample_lut(lut: &[f32; LUT_SIZE], value: f32) -> f32 {
    let scaled = value.clamp(0.0, 1.0) * (LUT_SIZE - 1) as f32;
    let low = scaled.floor() as usize;
    let high = (low + 1).min(LUT_SIZE - 1);
    let frac = scaled - low as f32;
    lut[low] * (1.0 - frac) + lut[high] * frac
}

fn to_u8(value: f32) -> u8 {
    (value.clamp(0.0, 1.0) * 255.0).round() as u8
}

/// Wendet alle fünf Kurven (siehe Moduldoku für die Verkettungsreihenfolge)
/// auf einen interleaved RGBA8-Puffer an — Alpha bleibt unverändert.
pub fn apply_rgba8(pixels: &[u8], curves: &CurvesAdjustment) -> Vec<u8> {
    let luminance_lut = build_lut(&curves.luminance);
    let rgb_lut = build_lut(&curves.rgb);
    let red_lut = build_lut(&curves.red);
    let green_lut = build_lut(&curves.green);
    let blue_lut = build_lut(&curves.blue);

    pixels
        .par_chunks_exact(4)
        .flat_map_iter(|rgba| {
            let r0 = rgba[0] as f32 / 255.0;
            let g0 = rgba[1] as f32 / 255.0;
            let b0 = rgba[2] as f32 / 255.0;

            let luminance = 0.299 * r0 + 0.587 * g0 + 0.114 * b0;
            let delta = sample_lut(&luminance_lut, luminance) - luminance;

            let r1 = (r0 + delta).clamp(0.0, 1.0);
            let g1 = (g0 + delta).clamp(0.0, 1.0);
            let b1 = (b0 + delta).clamp(0.0, 1.0);

            let r2 = sample_lut(&rgb_lut, r1);
            let g2 = sample_lut(&rgb_lut, g1);
            let b2 = sample_lut(&rgb_lut, b1);

            let r3 = sample_lut(&red_lut, r2);
            let g3 = sample_lut(&green_lut, g2);
            let b3 = sample_lut(&blue_lut, b2);

            [to_u8(r3), to_u8(g3), to_u8(b3), rgba[3]]
        })
        .collect()
}

/// Wendet dieselben fünf Kurven wie [`apply_rgba8`] an, aber auf einen
/// interleaved *linearen* RGB-f32-Puffer (3 Kanäle je Pixel, kein Alpha)
/// — für Masken (`stages::masks`), deren Werkzeuge bewusst alle im
/// selben linearen Arbeitsraum laufen statt für die Kurve allein einen
/// Umweg über Farbraum-Konvertierung + Rückkonvertierung zu nehmen
/// (siehe `masks.rs`s Moduldoku). Dieselbe LUT-Aufbau-/Sample-Logik wie
/// `apply_rgba8`, nur ohne die u8-Quantisierung am Ein-/Ausgang — die
/// Kurve wirkt hier auf dem linearen Wert selbst statt auf dem
/// display-referred Tonwert wie beim globalen Kurven-Werkzeug, eine
/// bewusste Vereinfachung (siehe `DECISIONS.md` ADR-0032).
pub fn apply_linear_rgb(pixels: &[f32], curves: &CurvesAdjustment) -> Vec<f32> {
    let luminance_lut = build_lut(&curves.luminance);
    let rgb_lut = build_lut(&curves.rgb);
    let red_lut = build_lut(&curves.red);
    let green_lut = build_lut(&curves.green);
    let blue_lut = build_lut(&curves.blue);

    pixels
        .par_chunks_exact(3)
        .flat_map_iter(|rgb| {
            let r0 = rgb[0];
            let g0 = rgb[1];
            let b0 = rgb[2];

            let luminance = 0.299 * r0 + 0.587 * g0 + 0.114 * b0;
            let delta = sample_lut(&luminance_lut, luminance) - luminance;

            let r1 = (r0 + delta).max(0.0);
            let g1 = (g0 + delta).max(0.0);
            let b1 = (b0 + delta).max(0.0);

            let r2 = sample_lut(&rgb_lut, r1);
            let g2 = sample_lut(&rgb_lut, g1);
            let b2 = sample_lut(&rgb_lut, b1);

            [
                sample_lut(&red_lut, r2),
                sample_lut(&green_lut, g2),
                sample_lut(&blue_lut, b2),
            ]
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn points(pairs: &[(f32, f32)]) -> CurveChannel {
        CurveChannel::Points {
            points: pairs
                .iter()
                .map(|&(input, output)| CurvePoint { input, output })
                .collect(),
        }
    }

    #[test]
    fn identity_points_curve_is_identity() {
        let lut = build_points_lut(&[
            CurvePoint {
                input: 0.0,
                output: 0.0,
            },
            CurvePoint {
                input: 1.0,
                output: 1.0,
            },
        ]);
        for (i, &value) in lut.iter().enumerate() {
            let expected = i as f32 / (LUT_SIZE - 1) as f32;
            assert!(
                (value - expected).abs() < 1e-4,
                "i={i} lut={value} expected={expected}"
            );
        }
    }

    #[test]
    fn points_curve_raises_midpoint_toward_its_control_point() {
        let lut = build_points_lut(&[
            CurvePoint {
                input: 0.0,
                output: 0.0,
            },
            CurvePoint {
                input: 0.5,
                output: 0.7,
            },
            CurvePoint {
                input: 1.0,
                output: 1.0,
            },
        ]);
        let mid = sample_lut(&lut, 0.5);
        assert!(
            (mid - 0.7).abs() < 0.01,
            "Mittelpunkt sollte nahe am gesetzten Kontrollpunkt liegen, war {mid}"
        );
    }

    /// Beweist die eigentliche Existenzberechtigung der Fritsch-Carlson-
    /// Korrektur: eine naive (nicht monotonie-korrigierte) kubische Spline
    /// würde bei diesem steilen lokalen Anstieg überschwingen und stellen-
    /// weise wieder fallen — das darf hier nicht passieren.
    #[test]
    fn spline_stays_monotonic_for_a_steep_local_rise() {
        let lut = build_points_lut(&[
            CurvePoint {
                input: 0.0,
                output: 0.0,
            },
            CurvePoint {
                input: 0.3,
                output: 0.3,
            },
            CurvePoint {
                input: 0.35,
                output: 0.9,
            },
            CurvePoint {
                input: 1.0,
                output: 1.0,
            },
        ]);
        for i in 1..LUT_SIZE {
            assert!(
                lut[i] + 1e-4 >= lut[i - 1],
                "LUT sollte monoton steigen, fiel aber bei i={i}: {} -> {}",
                lut[i - 1],
                lut[i]
            );
        }
    }

    #[test]
    fn unsorted_and_duplicate_input_points_are_handled_defensively() {
        let lut = build_points_lut(&[
            CurvePoint {
                input: 1.0,
                output: 1.0,
            },
            CurvePoint {
                input: 0.0,
                output: 0.0,
            },
            CurvePoint {
                input: 0.5,
                output: 0.6,
            },
            CurvePoint {
                input: 0.5,
                output: 0.6,
            }, // Duplikat derselben Stelle
        ]);
        assert!((sample_lut(&lut, 0.5) - 0.6).abs() < 0.01);
    }

    #[test]
    fn parametric_shadows_lifts_the_shadow_region_more_than_the_highlight_region() {
        let lut = build_parametric_lut(50.0, 0.0, 0.0, 0.0);
        let shadow_delta = lut[10] - 10.0 / 255.0;
        let highlight_delta = lut[245] - 245.0 / 255.0;
        assert!(
            shadow_delta > highlight_delta,
            "positive Schatten-Regler sollte dunkle Werte stärker anheben (shadow_delta={shadow_delta} highlight_delta={highlight_delta})"
        );
    }

    #[test]
    fn neutral_curves_are_identity_on_rgba8() {
        let neutral = CurvesAdjustment::neutral();
        let pixels = [12u8, 200, 77, 255, 0, 128, 255, 255];
        let result = apply_rgba8(&pixels, &neutral);
        for (input, output) in pixels.iter().zip(result.iter()) {
            assert!(
                (*input as i16 - *output as i16).abs() <= 1,
                "input={input} output={output}"
            );
        }
    }

    #[test]
    fn rgb_curve_lifts_all_three_channels_similarly() {
        let mut curves = CurvesAdjustment::neutral();
        curves.rgb = points(&[(0.0, 0.0), (0.5, 0.7), (1.0, 1.0)]);
        let pixels = [128u8, 128, 128, 255];
        let result = apply_rgba8(&pixels, &curves);
        assert!(result[0] > pixels[0] && result[1] > pixels[1] && result[2] > pixels[2]);
        assert_eq!(result[3], 255, "Alpha darf sich nicht ändern");
    }

    #[test]
    fn red_curve_only_affects_the_red_channel() {
        let mut curves = CurvesAdjustment::neutral();
        curves.red = points(&[(0.0, 0.0), (0.5, 0.9), (1.0, 1.0)]);
        let pixels = [128u8, 128, 128, 255];
        let result = apply_rgba8(&pixels, &curves);
        assert!(result[0] > pixels[0], "Rot sollte angehoben werden");
        assert_eq!(result[1], pixels[1], "Grün sollte unverändert bleiben");
        assert_eq!(result[2], pixels[2], "Blau sollte unverändert bleiben");
    }

    #[test]
    fn luminance_curve_shifts_a_colored_pixel_while_roughly_preserving_its_hue() {
        let mut curves = CurvesAdjustment::neutral();
        curves.luminance = points(&[(0.0, 0.0), (0.5, 0.7), (1.0, 1.0)]);
        // Ein deutlich rötliches Pixel statt eines neutralen Graus.
        let pixels = [180u8, 100, 80, 255];
        let result = apply_rgba8(&pixels, &curves);
        let original_rg_gap = pixels[0] as i16 - pixels[1] as i16;
        let result_rg_gap = result[0] as i16 - result[1] as i16;
        assert!(result[0] > pixels[0], "Helligkeit sollte insgesamt steigen");
        assert!(
            (result_rg_gap - original_rg_gap).abs() <= 2,
            "Rot/Grün-Abstand sollte durch eine reine Luminanz-Anhebung ungefähr erhalten bleiben (vorher={original_rg_gap} nachher={result_rg_gap})"
        );
    }
}
