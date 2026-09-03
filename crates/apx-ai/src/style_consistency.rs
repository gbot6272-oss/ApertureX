//! Automatischer Stil-Konsistenz-Check fürs Shooting (Phase 14 Schritt 5,
//! siehe `DECISIONS.md` ADR-0041 Nachtrag V, Recherche-Tabelle Punkt 6):
//! Lightroom hat dafür kein Äquivalent — nur das manuelle "Sync Settings"
//! zwischen genau zwei Fotos, keinen automatischen Konsistenzabgleich über
//! ein ganzes Shooting.
//!
//! Jedes Foto bekommt eine `StyleSignature` (Mittelwert über CIE-Lab, dem
//! wahrnehmungsnäheren Farbraum gegenüber sRGB — derselbe Grund, warum
//! Adobe seine eigenen Histogramm-/Weißabgleich-Werkzeuge intern auf
//! ähnlichen wahrnehmungsbasierten Räumen aufbaut). [`analyze_group`]
//! vergleicht alle Signaturen einer gewählten Fotomenge (typischerweise
//! ein Ordner/Shooting) gegen deren gemeinsamen Durchschnitt, markiert
//! statistische Ausreißer und schlägt je Foto eine Angleichung über die
//! bereits bestehenden Weißabgleich-/Belichtungs-Regler vor — keine neue
//! EDL-Operation, nur ein Vorschlag für bestehende Regler (dieselbe
//! "berechnet Werte für bestehende Regler" -Philosophie wie
//! `frontend/src/lib/autoTone.ts`).
//!
//! Es gibt in diesem Crate bisher keine sRGB->Lab-Umrechnung (anders als
//! ursprünglich im Plan angenommen: `apx-pipeline::stages::calibration`
//! rechnet nicht in Lab, sondern auf den kameraeigenen Primärfarben) —
//! die Standard-CIE-Formeln (D65-Referenzweiß) werden deshalb hier neu,
//! aber vollständig eigenständig aus öffentlich dokumentierter Mathematik
//! geschrieben, analog zu `stages::effects::hsv_to_rgb` in Schritt 4.

use rayon::prelude::*;

/// CIE-Lab-Signatur eines Fotos: der Pixel-Mittelwert über die (von der
/// aufrufenden Seite bereits auf eine Analyse-Auflösung gedeckelte)
/// Eingabe — dieselbe Grenze wie jede andere Analyse in diesem Crate
/// (siehe `segmentation::ANALYSIS_MAX_EDGE`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StyleSignature {
    pub mean_l: f32,
    pub mean_a: f32,
    pub mean_b: f32,
}

/// Mindestanzahl Fotos, ab der ein Durchschnitt/Streuung statistisch
/// überhaupt aussagekräftig ist — darunter (0-2 Fotos) gibt
/// [`analyze_group`] bewusst keine Ausreißer-Markierung/Vorschläge aus,
/// statt aus einer Streuung von im Extremfall einem einzigen Foto einen
/// bedeutungslosen "Ausreißer" zu erfinden.
pub const MIN_GROUP_SIZE_FOR_ANALYSIS: usize = 3;

/// Kombinierter, auf die jeweilige Achsen-Streuung der Gruppe normierter
/// Abstand (ähnlich einem vereinfachten Mahalanobis-Abstand mit
/// Diagonal-Kovarianz statt der vollen Kovarianzmatrix — für drei grob
/// unabhängige Lab-Achsen eine vertretbare Vereinfachung), ab dem ein
/// Foto als Ausreißer gilt. Bewusst gewählter Schwellenwert (kein
/// statistisch strikt hergeleiteter p-Wert), analog zu
/// `stages::effects::HALATION_THRESHOLD` — grob "spürbar weiter vom
/// Durchschnitt entfernt als die übrige Gruppe".
pub const OUTLIER_DISTANCE_THRESHOLD: f32 = 1.5;

/// Rechnet ein `0.0..=1.0`-normiertes sRGB-Tripel in CIE-Lab um
/// (D65-Referenzweiß, Standardformeln). `L*` liegt in `0.0..=100.0`,
/// `a*`/`b*` typischerweise in etwa `-128.0..=127.0`.
pub fn srgb_to_lab(r: f32, g: f32, b: f32) -> (f32, f32, f32) {
    let (x, y, z) = srgb_to_xyz(r, g, b);
    // D65-Referenzweiß (sRGBs eigener Weißpunkt).
    const XN: f32 = 0.950_47;
    const YN: f32 = 1.0;
    const ZN: f32 = 1.088_83;
    let fx = lab_f(x / XN);
    let fy = lab_f(y / YN);
    let fz = lab_f(z / ZN);
    let l = 116.0 * fy - 16.0;
    let a = 500.0 * (fx - fy);
    let b = 200.0 * (fy - fz);
    (l, a, b)
}

fn srgb_channel_to_linear(c: f32) -> f32 {
    if c <= 0.040_45 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

/// sRGB (D65) -> CIE-XYZ, Standard-Umwandlungsmatrix.
fn srgb_to_xyz(r: f32, g: f32, b: f32) -> (f32, f32, f32) {
    let r = srgb_channel_to_linear(r);
    let g = srgb_channel_to_linear(g);
    let b = srgb_channel_to_linear(b);
    let x = 0.412_456_4 * r + 0.357_576_1 * g + 0.180_437_5 * b;
    let y = 0.212_672_9 * r + 0.715_152_2 * g + 0.072_175_0 * b;
    let z = 0.019_333_9 * r + 0.119_192 * g + 0.950_304_1 * b;
    (x, y, z)
}

/// CIE-Lab-Hilfsfunktion `f(t)`, siehe CIE-Standard-Definition.
fn lab_f(t: f32) -> f32 {
    const DELTA: f32 = 6.0 / 29.0;
    if t > DELTA * DELTA * DELTA {
        t.cbrt()
    } else {
        t / (3.0 * DELTA * DELTA) + 4.0 / 29.0
    }
}

/// Berechnet die [`StyleSignature`] eines Fotos aus dessen bereits
/// dekodierten, `0.0..=1.0`-normierten RGB-Pixeln (drei Kanäle pro
/// Pixel, keine Alpha-Ebene — dieselbe Konvention wie
/// `stages::effects::apply_halation`).
pub fn compute_style_signature(pixels: &[f32], width: u32, height: u32) -> StyleSignature {
    let n = (width as usize) * (height as usize);
    if n == 0 {
        return StyleSignature {
            mean_l: 0.0,
            mean_a: 0.0,
            mean_b: 0.0,
        };
    }
    let (sum_l, sum_a, sum_b) = (0..n)
        .into_par_iter()
        .map(|i| {
            let idx = i * 3;
            srgb_to_lab(pixels[idx], pixels[idx + 1], pixels[idx + 2])
        })
        .reduce(
            || (0.0f32, 0.0f32, 0.0f32),
            |acc, lab| (acc.0 + lab.0, acc.1 + lab.1, acc.2 + lab.2),
        );
    let n = n as f32;
    StyleSignature {
        mean_l: sum_l / n,
        mean_a: sum_a / n,
        mean_b: sum_b / n,
    }
}

/// Vorschlag zur Angleichung eines Fotos an den Shooting-Durchschnitt —
/// Deltas für die bereits bestehenden Grundeinstellungs-Regler
/// (`WhiteBalanceAdjustment`/`BasicAdjustment::exposure_ev`), keine neue
/// Pixel-Transformation. Die aufrufende Seite addiert diese Deltas auf
/// den *aktuellen* Wert des jeweiligen Reglers (siehe `apx-app`s
/// `analyze_style_consistency`-Kommandodoku) — nicht absolut gesetzt,
/// damit ein Foto, das schon manuell nachjustiert wurde, nicht
/// überschrieben wird.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StyleAlignmentSuggestion {
    pub exposure_ev_delta: f32,
    pub temp_shift_kelvin_delta: f32,
    pub tint_shift_delta: f32,
}

/// Ergebnis der Gruppenanalyse für ein einzelnes Foto.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StylePhotoAnalysis {
    pub signature: StyleSignature,
    pub distance_from_group: f32,
    pub is_outlier: bool,
    pub suggestion: StyleAlignmentSuggestion,
}

/// Mindest-Streuung, unter der eine Gruppenachse als "praktisch
/// identisch" statt als eine durch Rundungsfehler beinahe null geteilte
/// Fehlerquelle behandelt wird.
const MIN_STD: f32 = 1e-3;

/// Näherungs-Exponent für die Umrechnung von CIE-`L*` (wahrnehmungsnahe
/// Helligkeit) in eine Blendenstufen-Korrektur — dieselbe bereits im
/// Projekt etablierte "^2.2-Näherung" wie `frontend/src/lib/autoTone.ts`
/// (dort für die Umrechnung zwischen gamma-kodierter und linearer
/// Helligkeit verwendet). `L*` ist selbst schon eine kubikwurzelartige
/// Wahrnehmungskurve, keine reine Gammakurve — die 2.2-Näherung ist
/// deshalb wie beim Auto-Ton-Regler eine bewusste Vereinfachung, keine
/// exakte photometrische Herleitung.
const EXPOSURE_GAMMA_APPROX: f32 = 2.2;

/// Heuristische Skalierung von CIE-`b*` (Blau/Gelb-Achse) auf
/// `temp_shift_kelvin` — nicht photometrisch hergeleitet, sondern so
/// gewählt, dass ein deutlicher Lab-`b*`-Unterschied (grob 5-10 Einheiten,
/// wie er zwischen z. B. Tageslicht- und Kunstlichtaufnahmen typisch
/// auftritt) eine im Wertebereich der `temp_shift_kelvin`-Regler
/// (`-2000..=2000`) spürbare, aber nicht den ganzen Bereich sofort
/// ausreizende Korrektur ergibt.
const TEMP_KELVIN_PER_LAB_B: f32 = 40.0;

/// Analoge heuristische Skalierung von CIE-`a*` (Grün/Magenta-Achse) auf
/// `tint_shift` (Wertebereich `-100..=100`).
const TINT_PER_LAB_A: f32 = 4.0;

/// Deltas werden auf diesen Bereich gekappt, damit ein extremer
/// Ausreißer (z. B. ein versehentlich mitfotografiertes komplett anderes
/// Motiv) keinen die Regler-Obergrenze sprengenden Vorschlag erzeugt —
/// der Nutzer sieht danach an den Reglern selbst, ob weitere manuelle
/// Korrektur nötig ist.
const MAX_EXPOSURE_EV_DELTA: f32 = 3.0;
const MAX_TEMP_KELVIN_DELTA: f32 = 800.0;
const MAX_TINT_DELTA: f32 = 50.0;

fn suggest_alignment(
    signature: StyleSignature,
    group_mean: StyleSignature,
) -> StyleAlignmentSuggestion {
    let exposure_ev_delta = (EXPOSURE_GAMMA_APPROX
        * (group_mean.mean_l.max(0.01) / signature.mean_l.max(0.01)).log2())
    .clamp(-MAX_EXPOSURE_EV_DELTA, MAX_EXPOSURE_EV_DELTA);
    let temp_shift_kelvin_delta = ((group_mean.mean_b - signature.mean_b) * TEMP_KELVIN_PER_LAB_B)
        .clamp(-MAX_TEMP_KELVIN_DELTA, MAX_TEMP_KELVIN_DELTA);
    let tint_shift_delta = ((group_mean.mean_a - signature.mean_a) * TINT_PER_LAB_A)
        .clamp(-MAX_TINT_DELTA, MAX_TINT_DELTA);
    StyleAlignmentSuggestion {
        exposure_ev_delta,
        temp_shift_kelvin_delta,
        tint_shift_delta,
    }
}

/// Vergleicht alle übergebenen Signaturen gegen ihren gemeinsamen
/// Durchschnitt und markiert statistische Ausreißer — siehe Moduldoku.
/// Bei weniger als [`MIN_GROUP_SIZE_FOR_ANALYSIS`] Fotos werden weder
/// Ausreißer markiert noch Angleichungen vorgeschlagen (jedes Ergebnis
/// trägt trotzdem seine eigene Signatur, Distanz `0.0`).
pub fn analyze_group(signatures: &[StyleSignature]) -> Vec<StylePhotoAnalysis> {
    if signatures.len() < MIN_GROUP_SIZE_FOR_ANALYSIS {
        return signatures
            .iter()
            .map(|&signature| StylePhotoAnalysis {
                signature,
                distance_from_group: 0.0,
                is_outlier: false,
                suggestion: StyleAlignmentSuggestion {
                    exposure_ev_delta: 0.0,
                    temp_shift_kelvin_delta: 0.0,
                    tint_shift_delta: 0.0,
                },
            })
            .collect();
    }

    let n = signatures.len() as f32;
    let mean_l = signatures.iter().map(|s| s.mean_l).sum::<f32>() / n;
    let mean_a = signatures.iter().map(|s| s.mean_a).sum::<f32>() / n;
    let mean_b = signatures.iter().map(|s| s.mean_b).sum::<f32>() / n;
    let group_mean = StyleSignature {
        mean_l,
        mean_a,
        mean_b,
    };

    let variance_of = |pick: fn(&StyleSignature) -> f32, mean: f32| {
        signatures
            .iter()
            .map(|s| (pick(s) - mean).powi(2))
            .sum::<f32>()
            / n
    };
    let std_l = variance_of(|s| s.mean_l, mean_l).sqrt().max(MIN_STD);
    let std_a = variance_of(|s| s.mean_a, mean_a).sqrt().max(MIN_STD);
    let std_b = variance_of(|s| s.mean_b, mean_b).sqrt().max(MIN_STD);

    signatures
        .iter()
        .map(|&signature| {
            let dl = (signature.mean_l - mean_l) / std_l;
            let da = (signature.mean_a - mean_a) / std_a;
            let db = (signature.mean_b - mean_b) / std_b;
            let distance_from_group = (dl * dl + da * da + db * db).sqrt();
            StylePhotoAnalysis {
                signature,
                distance_from_group,
                is_outlier: distance_from_group > OUTLIER_DISTANCE_THRESHOLD,
                suggestion: suggest_alignment(signature, group_mean),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flat_signature(l: f32, a: f32, b: f32) -> StyleSignature {
        StyleSignature {
            mean_l: l,
            mean_a: a,
            mean_b: b,
        }
    }

    #[test]
    fn srgb_to_lab_of_pure_white_is_near_l100_a0_b0() {
        let (l, a, b) = srgb_to_lab(1.0, 1.0, 1.0);
        assert!((l - 100.0).abs() < 0.5, "L*={l}");
        assert!(a.abs() < 0.5, "a*={a}");
        assert!(b.abs() < 0.5, "b*={b}");
    }

    #[test]
    fn srgb_to_lab_of_pure_black_is_near_zero() {
        let (l, a, b) = srgb_to_lab(0.0, 0.0, 0.0);
        assert!(l.abs() < 0.5, "L*={l}");
        assert!(a.abs() < 0.5, "a*={a}");
        assert!(b.abs() < 0.5, "b*={b}");
    }

    #[test]
    fn srgb_to_lab_of_a_warm_yellow_orange_has_positive_b_and_a() {
        // Ein warmes Orange (viel Rot, mittleres Grün, wenig Blau) muss
        // auf der Gelb/Blau-Achse deutlich Richtung Gelb (positives b*)
        // und auf der Grün/Magenta-Achse Richtung Rot/Magenta (positives
        // a*) liegen — der Kern der Wärme-Semantik, auf die
        // `TEMP_KELVIN_PER_LAB_B`/`TINT_PER_LAB_A` sich verlassen.
        let (_, a, b) = srgb_to_lab(0.9, 0.6, 0.2);
        assert!(a > 5.0, "a*={a}");
        assert!(b > 20.0, "b*={b}");
    }

    #[test]
    fn compute_style_signature_of_a_uniform_image_matches_the_single_pixel_conversion() {
        let (l, a, b) = srgb_to_lab(0.4, 0.5, 0.6);
        let pixels = vec![0.4, 0.5, 0.6].repeat(9); // 3x3 uniform image
        let signature = compute_style_signature(&pixels, 3, 3);
        assert!((signature.mean_l - l).abs() < 1e-3);
        assert!((signature.mean_a - a).abs() < 1e-3);
        assert!((signature.mean_b - b).abs() < 1e-3);
    }

    #[test]
    fn too_few_photos_yields_no_outliers_and_no_suggestions() {
        let signatures = vec![
            flat_signature(50.0, 0.0, 0.0),
            flat_signature(90.0, 20.0, 20.0),
        ];
        let analyses = analyze_group(&signatures);
        assert_eq!(analyses.len(), 2);
        for analysis in analyses {
            assert!(!analysis.is_outlier);
            assert_eq!(analysis.distance_from_group, 0.0);
            assert_eq!(analysis.suggestion.exposure_ev_delta, 0.0);
        }
    }

    #[test]
    fn a_photo_far_from_an_otherwise_consistent_shoot_is_flagged_as_an_outlier() {
        // Vier Fotos desselben, konsistent belichteten Shootings plus ein
        // deutlich dunkleres, kühleres fünftes Foto (z. B. versehentlich
        // im Schatten oder mit falschem Weißabgleich aufgenommen).
        let signatures = vec![
            flat_signature(60.0, 5.0, 15.0),
            flat_signature(61.0, 4.5, 14.5),
            flat_signature(59.5, 5.2, 15.5),
            flat_signature(60.5, 4.8, 14.8),
            flat_signature(25.0, -10.0, -20.0),
        ];
        let analyses = analyze_group(&signatures);
        assert!(
            analyses[4].is_outlier,
            "distance={}",
            analyses[4].distance_from_group
        );
        for analysis in &analyses[0..4] {
            assert!(
                !analysis.is_outlier,
                "distance={}",
                analysis.distance_from_group
            );
        }
        assert!(analyses[4].distance_from_group > analyses[0].distance_from_group);
    }

    #[test]
    fn outlier_photo_gets_a_suggestion_that_brightens_and_warms_it_toward_the_group() {
        // Neun beinahe identische Fotos plus ein deutlicher Ausreißer —
        // bewusst neun statt vier "konsistente" Fotos, damit der einzelne
        // Ausreißer den Gruppendurchschnitt nicht selbst so stark
        // verschiebt, dass die "konsistenten" Fotos ihrerseits eine
        // nennenswerte Korrektur vorgeschlagen bekämen (siehe die eigene
        // Assertion unten).
        let mut signatures = vec![
            flat_signature(60.0, 5.0, 15.0),
            flat_signature(61.0, 4.5, 14.5),
            flat_signature(59.5, 5.2, 15.5),
            flat_signature(60.5, 4.8, 14.8),
            flat_signature(60.2, 5.0, 15.1),
            flat_signature(59.8, 4.9, 14.9),
            flat_signature(60.3, 5.1, 15.2),
            flat_signature(59.7, 4.7, 14.7),
            flat_signature(60.1, 5.0, 15.0),
        ];
        signatures.push(flat_signature(25.0, -10.0, -20.0));
        let outlier_index = signatures.len() - 1;
        let analyses = analyze_group(&signatures);
        let outlier = &analyses[outlier_index];
        // Dunkler als die Gruppe -> positive Belichtungskorrektur.
        assert!(
            outlier.suggestion.exposure_ev_delta > 0.0,
            "delta={}",
            outlier.suggestion.exposure_ev_delta
        );
        // Kühler/blauer (niedrigeres b*) als die Gruppe -> positive
        // (wärmende) Temperaturkorrektur.
        assert!(
            outlier.suggestion.temp_shift_kelvin_delta > 0.0,
            "delta={}",
            outlier.suggestion.temp_shift_kelvin_delta
        );
        // Grüner (niedrigeres a*) als die Gruppe -> positive (Richtung
        // Magenta) Tint-Korrektur.
        assert!(
            outlier.suggestion.tint_shift_delta > 0.0,
            "delta={}",
            outlier.suggestion.tint_shift_delta
        );

        // Ein Foto exakt auf dem Gruppendurchschnitt braucht (fast) keine
        // Korrektur.
        let consistent = &analyses[0];
        assert!(
            consistent.suggestion.exposure_ev_delta.abs() < 0.2,
            "delta={}",
            consistent.suggestion.exposure_ev_delta
        );
    }

    #[test]
    fn suggested_deltas_stay_within_the_documented_clamp_even_for_an_extreme_outlier() {
        let mut signatures = vec![flat_signature(60.0, 0.0, 0.0); 4];
        signatures.push(flat_signature(0.5, 100.0, -120.0));
        let analyses = analyze_group(&signatures);
        let outlier = analyses.last().unwrap();
        assert!(outlier.suggestion.exposure_ev_delta <= MAX_EXPOSURE_EV_DELTA);
        assert!(outlier.suggestion.temp_shift_kelvin_delta.abs() <= MAX_TEMP_KELVIN_DELTA);
        assert!(outlier.suggestion.tint_shift_delta.abs() <= MAX_TINT_DELTA);
    }
}
