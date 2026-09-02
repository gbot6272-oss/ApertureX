//! Objektiv-Kalibrierung aus eigenen Fotos (Phase 12 Schritt 3 Teil B,
//! siehe `DECISIONS.md` ADR-0039) — für Objektive außerhalb der echten
//! LensFun-Datenbank (siehe `apx_pipeline::lens_profiles`).
//!
//! **Ehrliche Vereinfachung ggü. dem ursprünglichen Plan-Text** ("Zhang's
//! Methode... Eckenerkennung per Harris-artigem Detektor + Homografie-
//! Schätzung + nichtlineare Verfeinerung"): eine robuste automatische
//! Schachbrett-Eckenerkennung plus eine volle Mehrparameter-Kamera-
//! kalibrierung ist ein eigenständiges, fehleranfälliges Computer-
//! Vision-Projekt für sich — für unser Ein-Wert-Verzeichnungsmodell
//! (siehe `apx_pipeline::stages::lens_corrections`) überdimensioniert
//! und in diesem Umfang nicht seriös umsetzbar.
//!
//! **Stattdessen:** der Nutzer markiert selbst mehrere Punkte entlang
//! einer in der Realität geraden Linie (Schachbrett-Gitterlinie,
//! Wandkante, Horizont — dieselbe Grundidee wie der bestehende
//! „Guided"-Aufrichtungsmodus in `lens_corrections.rs`, nur mit beliebig
//! vielen Punkten statt zwei Linienenden, und für mehrere Linien
//! gleichzeitig statt nur einer Rotation). [`calibrate_distortion_k1`]
//! sucht dann den *einen* Verzeichnungskoeffizienten, der alle markierten
//! Linien nach der Entzerrung gemeinsam am geradesten macht — klassische
//! Optimierung (Rasterverfeinerung über einen einzigen Skalar), **kein
//! gelerntes Modell**. Eine echte Berechnung aus echten Nutzerdaten,
//! keine Fabrikation — anders als eine LLM-„Schätzung" von Objektiv-
//! Koeffizienten ohne echte Kalibrierdatengrundlage (siehe ADR-0039).
//!
//! **Ergebnis ist bewusst auf `distortion_k1` beschränkt** (Vignettierung/
//! chromatische Aberration bräuchten ein anderes Kalibriervorgehen —
//! Helligkeits- bzw. Kanal-Registrierungsmessungen statt Geradheit — und
//! sind hier nicht Teil des Umfangs). Ein resultierendes Profil trägt
//! `vignette_amount = 0.0`/`ca_red_cyan = 0.0`/`ca_blue_yellow = 0.0`.

/// Ein normierter (`0.0..=1.0`, Bildbruchteil, dieselbe Konvention wie
/// `apx_pipeline::edl::v3::MaskPoint`) Bildpunkt, den der Nutzer entlang
/// einer in der Realität geraden Linie markiert hat.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StraightLinePoint {
    pub x: f32,
    pub y: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum LensCalibrationError {
    #[error("mindestens eine markierte Linie mit mindestens drei Punkten wird gebraucht")]
    NotEnoughPoints,
}

/// Suchgrenzen für `k1` — deckt großzügig ab, was `LensCorrectionAdjustment`s
/// manueller Verzeichnungsregler selbst erreichen kann (siehe
/// `lens_corrections.rs`s `MANUAL_K1_SCALE`: `±100` am Regler ergibt
/// `±0.3`), mit Sicherheitsabstand für real vorkommende starke
/// Weitwinkel-/Fisheye-Verzeichnung.
const SEARCH_MIN: f32 = -0.5;
const SEARCH_MAX: f32 = 0.5;

/// `0..1`-Bildbruchteil → zentrierte `±1`-Koordinate, wie
/// `lens_corrections.rs`s `process_pixel` sie für `nx`/`ny` verwendet
/// (`nx = (px - half_w) / half_w`, was für den Bruchteil `x` exakt
/// `2·x − 1` ist — die Bildbreite kürzt sich heraus, kein Bildmaß nötig).
fn to_centered(p: StraightLinePoint) -> (f32, f32) {
    (p.x * 2.0 - 1.0, p.y * 2.0 - 1.0)
}

/// Löst die implizite Entzerrungsgleichung — die Umkehrung von
/// `lens_corrections.rs`s `apply_distortion` (`quelle = ziel · (1 + k1·
/// |ziel|²)`) nach `ziel`, für einen gegebenen `quelle`-Punkt (die
/// tatsächlich markierte, verzeichnete Bildposition) — per Fixpunkt-
/// Iteration statt einer geschlossenen kubischen Lösungsformel;
/// konvergiert für die hier relevanten moderaten `k1`-Werte in wenigen
/// Schritten zuverlässig.
fn undistort_point(source: (f32, f32), k1: f32) -> (f32, f32) {
    let mut target = source;
    for _ in 0..6 {
        let r2 = target.0 * target.0 + target.1 * target.1;
        let factor = 1.0 + k1 * r2;
        if factor.abs() < 1e-4 {
            break;
        }
        target = (source.0 / factor, source.1 / factor);
    }
    target
}

/// Summe der quadrierten senkrechten Abstände der Punkte einer Linie zu
/// ihrer eigenen Ausgleichsgerade — totale Kleinste-Quadrate (PCA-Fit):
/// robust gegenüber beliebiger Linienrichtung, anders als eine simple
/// y-auf-x-Regression, die bei einer fast senkrechten Linie entartet.
fn straightness_residual(points: &[(f32, f32)]) -> f32 {
    let n = points.len() as f32;
    let mean_x = points.iter().map(|p| p.0).sum::<f32>() / n;
    let mean_y = points.iter().map(|p| p.1).sum::<f32>() / n;
    let (mut sxx, mut sxy, mut syy) = (0.0f32, 0.0f32, 0.0f32);
    for p in points {
        let dx = p.0 - mean_x;
        let dy = p.1 - mean_y;
        sxx += dx * dx;
        sxy += dx * dy;
        syy += dy * dy;
    }
    // Hauptrichtung der Punktwolke: Eigenvektor des größeren Eigenwerts
    // der 2×2-Kovarianzmatrix [[sxx,sxy],[sxy,syy]] (geschlossene Form
    // für 2×2 statt einer allgemeinen Eigenwertzerlegung).
    let trace = sxx + syy;
    let det = sxx * syy - sxy * sxy;
    let discriminant = (trace * trace / 4.0 - det).max(0.0).sqrt();
    let lambda_max = trace / 2.0 + discriminant;
    let (normal_x, normal_y) = if sxy.abs() > 1e-9 {
        let dir_x = lambda_max - syy;
        let dir_y = sxy;
        let len = (dir_x * dir_x + dir_y * dir_y).sqrt().max(1e-9);
        (-dir_y / len, dir_x / len) // senkrecht zur Hauptrichtung
    } else if sxx >= syy {
        (0.0, 1.0) // Hauptrichtung ≈ x-Achse, Normale ist y
    } else {
        (1.0, 0.0)
    };
    points
        .iter()
        .map(|p| {
            let d = normal_x * (p.0 - mean_x) + normal_y * (p.1 - mean_y);
            d * d
        })
        .sum()
}

fn total_cost(lines: &[Vec<(f32, f32)>], k1: f32) -> f32 {
    lines
        .iter()
        .map(|line| {
            let undistorted: Vec<(f32, f32)> =
                line.iter().map(|&p| undistort_point(p, k1)).collect();
            straightness_residual(&undistorted)
        })
        .sum()
}

/// Sucht den Verzeichnungskoeffizienten `k1` (direkt kompatibel mit
/// `LensProfile::distortion_k1`/`LensCorrectionAdjustment`s Regler, siehe
/// [`to_centered`]s Konvention), der `lines` nach der Entzerrung
/// gemeinsam am geradesten macht — grobe, dann zunehmend feinere
/// Rastersuche über den plausiblen `k1`-Bereich (ein einzelner Skalar,
/// dafür reicht eine einfache Rasterverfeinerung ohne einen allgemeinen
/// Gradientenverfahren-Löser).
///
/// Braucht mindestens eine markierte Linie mit mindestens drei Punkten
/// (zwei Punkte definieren immer eine Gerade, egal welches `k1` — die
/// Krümmung wird erst ab drei Punkten überhaupt messbar).
pub fn calibrate_distortion_k1(
    lines: &[Vec<StraightLinePoint>],
) -> Result<f32, LensCalibrationError> {
    let sequences: Vec<Vec<(f32, f32)>> = lines
        .iter()
        .filter(|line| line.len() >= 3)
        .map(|line| line.iter().map(|&p| to_centered(p)).collect())
        .collect();
    if sequences.is_empty() {
        return Err(LensCalibrationError::NotEnoughPoints);
    }

    let mut center = 0.0f32;
    let mut half_width = (SEARCH_MAX - SEARCH_MIN) / 2.0;
    const GRID_STEPS: usize = 40;
    const REFINEMENT_PASSES: usize = 6;

    for _ in 0..REFINEMENT_PASSES {
        let lo = (center - half_width).max(SEARCH_MIN);
        let hi = (center + half_width).min(SEARCH_MAX);
        let mut best_k1 = center;
        let mut best_cost = f32::MAX;
        for step in 0..=GRID_STEPS {
            let k1 = lo + (hi - lo) * (step as f32 / GRID_STEPS as f32);
            let cost = total_cost(&sequences, k1);
            if cost < best_cost {
                best_cost = cost;
                best_k1 = k1;
            }
        }
        center = best_k1;
        half_width /= 4.0;
    }

    Ok(center)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Vorwärtsverzeichnung — dieselbe Formel wie `lens_corrections.rs`s
    /// `apply_distortion`, hier nur zum Erzeugen synthetischer
    /// "verzeichneter" Testpunkte aus einer bekannt geraden Linie.
    fn distort(target: (f32, f32), k1: f32) -> (f32, f32) {
        let r2 = target.0 * target.0 + target.1 * target.1;
        let factor = 1.0 + k1 * r2;
        (target.0 * factor, target.1 * factor)
    }

    fn marked_line_from_straight_targets(
        targets: &[(f32, f32)],
        k1_true: f32,
    ) -> Vec<StraightLinePoint> {
        targets
            .iter()
            .map(|&t| {
                let (dx, dy) = distort(t, k1_true);
                // Zurück in 0..1-Bildbruchteile, wie ein Nutzer sie im
                // Viewer markieren würde (Umkehrung von `to_centered`).
                StraightLinePoint {
                    x: (dx + 1.0) / 2.0,
                    y: (dy + 1.0) / 2.0,
                }
            })
            .collect()
    }

    #[test]
    fn calibrate_distortion_k1_recovers_a_known_coefficient_from_synthetic_distorted_lines() {
        let k1_true = -0.15f32;
        // Zwei in der Realität gerade Linien (eine horizontale, eine
        // vertikale), beide abseits vom Bildzentrum — nur dort wird die
        // Krümmung durch Verzeichnung überhaupt sichtbar.
        let horizontal_target: Vec<(f32, f32)> =
            (0..7).map(|i| (-0.9 + i as f32 * 0.3, 0.6)).collect();
        let vertical_target: Vec<(f32, f32)> =
            (0..7).map(|i| (0.6, -0.9 + i as f32 * 0.3)).collect();

        let lines = vec![
            marked_line_from_straight_targets(&horizontal_target, k1_true),
            marked_line_from_straight_targets(&vertical_target, k1_true),
        ];

        let recovered = calibrate_distortion_k1(&lines).expect("sollte konvergieren");
        assert!(
            (recovered - k1_true).abs() < 0.01,
            "sollte den wahren Koeffizienten wiederfinden (wahr={k1_true}, gefunden={recovered})"
        );
    }

    #[test]
    fn calibrate_distortion_k1_rejects_lines_with_fewer_than_three_points() {
        let lines = vec![vec![
            StraightLinePoint { x: 0.1, y: 0.1 },
            StraightLinePoint { x: 0.9, y: 0.1 },
        ]];
        assert_eq!(
            calibrate_distortion_k1(&lines),
            Err(LensCalibrationError::NotEnoughPoints)
        );
    }
}
