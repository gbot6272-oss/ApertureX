//! Video-Stabilisierung: aus gemessener Kamerabewegung eine geglättete
//! Bahn und daraus die Korrektur je Einzelbild (Phase 17 Schritt 9,
//! siehe `PLAN.md` und `DECISIONS.md` ADR-0062).
//!
//! **Hier steckt nur die Mathematik.** Das Dekodieren, Messen und
//! Neu-Kodieren macht `apx-app`s `stabilize_video`-Command; die
//! Bild-zu-Bild-Messung selbst kommt aus
//! [`crate::homography_stitch::estimate_pairwise_homographies_rgba8`],
//! also demselben Code, der das Panorama-Stitching trägt. Diese Trennung
//! ist der Grund, warum sich die eigentliche Stabilisierung überhaupt
//! testen lässt: eine Bahn aus Zahlen rein, Korrekturen aus Zahlen raus,
//! ohne `ffmpeg` und ohne Videodatei.
//!
//! ## Warum zwei Durchgänge nötig sind
//!
//! Eine Bahn lässt sich nicht glätten, solange man ihre Zukunft nicht
//! kennt. Ein einzelner Durchlauf könnte Bewegung nur **dämpfen** (und
//! würde dabei auch jeden gewollten Schwenk verschleppen). Deshalb:
//! erst alle Bild-zu-Bild-Bewegungen messen, dann glätten, dann
//! korrigieren.
//!
//! ## Warum Ähnlichkeit statt voller Homografie
//!
//! Die Messung liefert eine 8-Freiheitsgrad-Homografie. Die auf eine
//! wackelige Freihandaufnahme direkt anzuwenden erzeugt den berüchtigten
//! „Wackelpudding": perspektivische Anteile, die aus Rauschen in den
//! Merkmalspaaren stammen, lassen Bildkanten schwabbeln. Übliche
//! Stabilisierer beschränken sich deshalb auf vier Freiheitsgrade —
//! Verschiebung, Drehung, Maßstab. [`Similarity::from_homography`]
//! projiziert genau darauf.
//!
//! ## Warum Korrekturen begrenzt werden
//!
//! Jede Korrektur schiebt das Bild und legt am Rand leere Fläche frei.
//! Dagegen hilft nur Hineinzoomen. Statt zu hoffen, dass der gewählte
//! Zoom reicht, **begrenzt** [`stabilize_path`] jede Korrektur auf das,
//! was der Rand hergibt (siehe [`StabilizeParams::crop_zoom`]) — lieber
//! eine leicht verbleibende Restbewegung als ein schwarzer Rand, der im
//! fertigen Video nicht mehr zu reparieren ist.

use nalgebra::Matrix3;

/// Eine Ähnlichkeitstransformation mit vier Freiheitsgraden:
/// `p ↦ scale · R(rotation) · p + (dx, dy)`.
///
/// Bewusst nicht als Matrix gehalten: Glätten heißt hier, Maßstab,
/// Winkel und Verschiebung **einzeln** zu mitteln. Auf Matrizen
/// gemittelt käme etwas heraus, das gar keine Ähnlichkeit mehr ist.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Similarity {
    pub dx: f64,
    pub dy: f64,
    /// Drehwinkel im Bogenmaß.
    pub rotation: f64,
    pub scale: f64,
}

impl Default for Similarity {
    fn default() -> Self {
        Self::IDENTITY
    }
}

impl Similarity {
    pub const IDENTITY: Self = Self {
        dx: 0.0,
        dy: 0.0,
        rotation: 0.0,
        scale: 1.0,
    };

    /// Projiziert eine gemessene Homografie auf ihre
    /// Ähnlichkeits-Anteile — siehe die Moduldoku für die Begründung.
    ///
    /// Maßstab und Winkel kommen aus dem linken oberen 2×2-Block: eine
    /// reine Ähnlichkeit hat dort `s·[[cos, −sin], [sin, cos]]`. Bei
    /// einer verrauschten Messung ist das nur näherungsweise der Fall,
    /// deshalb wird über beide Diagonalen bzw. beide Nebendiagonalen
    /// gemittelt statt einen Eintrag herauszugreifen.
    pub fn from_homography(h: &Matrix3<f64>) -> Self {
        // Perspektivische Zeile normalisieren, sonst hängen die
        // Verschiebungen von einem beliebigen Skalierungsfaktor ab.
        let w = h[(2, 2)];
        let h = if w.abs() > 1e-12 { *h / w } else { *h };

        let a = (h[(0, 0)] + h[(1, 1)]) * 0.5;
        let b = (h[(1, 0)] - h[(0, 1)]) * 0.5;
        let scale = (a * a + b * b).sqrt();
        Self {
            dx: h[(0, 2)],
            dy: h[(1, 2)],
            rotation: b.atan2(a),
            scale: if scale.is_finite() && scale > 1e-6 {
                scale
            } else {
                1.0
            },
        }
    }

    /// `self ∘ other` — erst `other`, dann `self`.
    pub fn compose(&self, other: &Self) -> Self {
        let (sin, cos) = self.rotation.sin_cos();
        Self {
            dx: self.dx + self.scale * (cos * other.dx - sin * other.dy),
            dy: self.dy + self.scale * (sin * other.dx + cos * other.dy),
            rotation: self.rotation + other.rotation,
            scale: self.scale * other.scale,
        }
    }

    pub fn inverse(&self) -> Self {
        let scale = if self.scale.abs() > 1e-9 {
            1.0 / self.scale
        } else {
            1.0
        };
        let (sin, cos) = (-self.rotation).sin_cos();
        Self {
            dx: -scale * (cos * self.dx - sin * self.dy),
            dy: -scale * (sin * self.dx + cos * self.dy),
            rotation: -self.rotation,
            scale,
        }
    }

    /// Rechnet eine in verkleinerter Auflösung gemessene Bewegung auf
    /// die volle Auflösung hoch.
    ///
    /// Nur die Verschiebung ist auflösungsabhängig: Drehwinkel und
    /// Maßstabsfaktor sind dimensionslos und bleiben unangetastet. Das
    /// ist der Grund, warum überhaupt in verkleinerter Auflösung
    /// gemessen werden darf — die Messung kostet sonst ein Vielfaches,
    /// ohne genauer zu werden (das Wackeln steckt in den groben
    /// Strukturen, nicht im Pixelrauschen).
    pub fn scaled_translation(&self, factor: f64) -> Self {
        Self {
            dx: self.dx * factor,
            dy: self.dy * factor,
            ..*self
        }
    }

    /// Als 3×3-Matrix, wie sie der Warp-Schritt braucht.
    pub fn to_matrix(self) -> Matrix3<f64> {
        let (sin, cos) = self.rotation.sin_cos();
        Matrix3::new(
            self.scale * cos,
            -self.scale * sin,
            self.dx,
            self.scale * sin,
            self.scale * cos,
            self.dy,
            0.0,
            0.0,
            1.0,
        )
    }
}

/// Einstellungen der Stabilisierung.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StabilizeParams {
    /// Halbe Fensterbreite der Glättung in Einzelbildern. Größer =
    /// ruhiger, aber gewollte Schwenks setzen träger ein.
    pub smoothing_radius: usize,
    /// Wie weit hineingezoomt wird, um die freigelegten Ränder zu
    /// verdecken (`1.0` = gar nicht). Bestimmt zugleich, wie groß eine
    /// Korrektur überhaupt werden darf.
    pub crop_zoom: f64,
    /// Bildbreite/-höhe in Pixeln — nötig, um aus `crop_zoom` einen
    /// Grenzwert in Pixeln zu machen.
    pub width: f64,
    pub height: f64,
}

impl Default for StabilizeParams {
    fn default() -> Self {
        Self {
            smoothing_radius: 12,
            crop_zoom: 1.1,
            width: 1920.0,
            height: 1080.0,
        }
    }
}

impl StabilizeParams {
    /// Wie viele Pixel eine Korrektur höchstens verschieben darf, ohne
    /// dass der Zuschnitt-Rand aufreißt. Bei `crop_zoom = 1.1` sind das
    /// je Seite ≈ 4,5 % der Kantenlänge.
    pub fn max_shift(&self) -> (f64, f64) {
        let margin = ((self.crop_zoom - 1.0) / self.crop_zoom / 2.0).max(0.0);
        (self.width * margin, self.height * margin)
    }
}

/// Absolute Kamerabahn aus den Bild-zu-Bild-Bewegungen.
///
/// `None` bedeutet „für dieses Einzelbild ließ sich nichts messen" — dann
/// wird **keine Bewegung** angenommen statt geraten. Ein geratener Sprung
/// wäre im fertigen Video als Ruck sichtbar und schlimmer als ein
/// ausgelassenes Bild.
fn absolute_path(frame_to_previous: &[Option<Similarity>]) -> Vec<Similarity> {
    let mut path = Vec::with_capacity(frame_to_previous.len());
    let mut current = Similarity::IDENTITY;
    for step in frame_to_previous {
        current = current.compose(&step.unwrap_or(Similarity::IDENTITY));
        path.push(current);
    }
    path
}

/// Gleitendes Mittel über die Bahn, je Komponente einzeln. Am Rand wird
/// das Fenster geklemmt statt verkürzt — sonst wären die ersten und
/// letzten Bilder spürbar unruhiger als der Rest.
fn smooth_path(path: &[Similarity], radius: usize) -> Vec<Similarity> {
    if path.is_empty() {
        return Vec::new();
    }
    let last = path.len() - 1;
    (0..path.len())
        .map(|i| {
            let lo = i.saturating_sub(radius);
            let hi = (i + radius).min(last);
            let mut sum = Similarity {
                dx: 0.0,
                dy: 0.0,
                rotation: 0.0,
                scale: 0.0,
            };
            let mut count = 0.0;
            for entry in &path[lo..=hi] {
                sum.dx += entry.dx;
                sum.dy += entry.dy;
                sum.rotation += entry.rotation;
                sum.scale += entry.scale;
                count += 1.0;
            }
            Similarity {
                dx: sum.dx / count,
                dy: sum.dy / count,
                rotation: sum.rotation / count,
                scale: sum.scale / count,
            }
        })
        .collect()
}

/// Begrenzt eine Korrektur auf das, was der Zuschnitt-Rand verdecken
/// kann (siehe Moduldoku).
fn clamp_correction(correction: Similarity, params: &StabilizeParams) -> Similarity {
    let (max_x, max_y) = params.max_shift();
    // Drehung und Maßstab ebenfalls begrenzen: eine aus Messrauschen
    // entstandene starke Drehung fällt genauso auf wie ein Ruck.
    let max_rotation = 0.05_f64; // ≈ 2,9°
    Similarity {
        dx: correction.dx.clamp(-max_x, max_x),
        dy: correction.dy.clamp(-max_y, max_y),
        rotation: correction.rotation.clamp(-max_rotation, max_rotation),
        scale: correction.scale.clamp(0.95, 1.05),
    }
}

/// Berechnet je Einzelbild die Transformation, die es auf die geglättete
/// Kamerabahn zieht.
///
/// Eingabe ist die Bewegung **von Bild `i` zum vorherigen Bild** (das
/// erste Element gehört zum ersten Bild und ist üblicherweise
/// [`Similarity::IDENTITY`]). Ausgabe ist gleich lang.
pub fn stabilize_path(
    frame_to_previous: &[Option<Similarity>],
    params: &StabilizeParams,
) -> Vec<Similarity> {
    let path = absolute_path(frame_to_previous);
    let smoothed = smooth_path(&path, params.smoothing_radius);
    path.iter()
        .zip(&smoothed)
        .map(|(actual, target)| clamp_correction(target.compose(&actual.inverse()), params))
        .collect()
}

/// Der Hineinzoom, der die von der Korrektur freigelegten Ränder
/// verdeckt — Skalierung um die **Bildmitte**, nicht um den Ursprung.
///
/// Ohne die Mitte als Fixpunkt würde ein Zoom das Bild zusätzlich zur
/// Ecke hin verschieben und damit genau den Rand aufreißen, den er
/// zudecken soll.
pub fn zoom_about_center(zoom: f64, width: f64, height: f64) -> Similarity {
    let (cx, cy) = (width * 0.5, height * 0.5);
    Similarity {
        dx: cx * (1.0 - zoom),
        dy: cy * (1.0 - zoom),
        rotation: 0.0,
        scale: zoom,
    }
}

/// Die vollständige Abbildung für ein Einzelbild: erst die Korrektur auf
/// die geglättete Bahn, dann der Zuschnitt-Zoom.
pub fn frame_transform(correction: &Similarity, params: &StabilizeParams) -> Similarity {
    zoom_about_center(params.crop_zoom, params.width, params.height).compose(correction)
}

/// Verzerrt ein RGB8-Bild nach `transform`.
///
/// `transform` ist die **Vorwärts**-Abbildung (Quelle → Ziel); abgetastet
/// wird rückwärts über ihre Inverse, damit im Ergebnis kein Pixel
/// unbesetzt bleibt. Zwischen den Stützstellen wird bilinear gemittelt,
/// am Rand auf das letzte gültige Pixel geklemmt — bei einem
/// hineingezoomten Bild liegt der Rand ohnehin außerhalb des
/// Sichtbaren, und ein geklemmter Rand ist harmloser als ein schwarzer.
pub fn warp_rgb8(frame: &[u8], width: u32, height: u32, transform: &Similarity) -> Vec<u8> {
    let (w, h) = (width as usize, height as usize);
    let inverse = transform.inverse();
    let (sin, cos) = inverse.rotation.sin_cos();
    let mut out = vec![0u8; w * h * 3];

    for y in 0..h {
        for x in 0..w {
            let (fx, fy) = (x as f64, y as f64);
            let sx = inverse.dx + inverse.scale * (cos * fx - sin * fy);
            let sy = inverse.dy + inverse.scale * (sin * fx + cos * fy);
            let pixel = sample_bilinear_rgb8(frame, w, h, sx, sy);
            let base = (y * w + x) * 3;
            out[base..base + 3].copy_from_slice(&pixel);
        }
    }
    out
}

/// Bilineare Abtastung mit Rand-Klemmung (siehe [`warp_rgb8`]).
fn sample_bilinear_rgb8(frame: &[u8], width: usize, height: usize, x: f64, y: f64) -> [u8; 3] {
    if width == 0 || height == 0 {
        return [0, 0, 0];
    }
    let clamped_x = x.clamp(0.0, (width - 1) as f64);
    let clamped_y = y.clamp(0.0, (height - 1) as f64);
    let x0 = clamped_x.floor() as usize;
    let y0 = clamped_y.floor() as usize;
    let x1 = (x0 + 1).min(width - 1);
    let y1 = (y0 + 1).min(height - 1);
    let tx = clamped_x - x0 as f64;
    let ty = clamped_y - y0 as f64;

    let mut pixel = [0u8; 3];
    for (channel, value) in pixel.iter_mut().enumerate() {
        let at = |px: usize, py: usize| frame[(py * width + px) * 3 + channel] as f64;
        let top = at(x0, y0) * (1.0 - tx) + at(x1, y0) * tx;
        let bottom = at(x0, y1) * (1.0 - tx) + at(x1, y1) * tx;
        *value = (top * (1.0 - ty) + bottom * ty).round().clamp(0.0, 255.0) as u8;
    }
    pixel
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shift(dx: f64, dy: f64) -> Option<Similarity> {
        Some(Similarity {
            dx,
            dy,
            ..Similarity::IDENTITY
        })
    }

    fn params() -> StabilizeParams {
        StabilizeParams {
            smoothing_radius: 5,
            crop_zoom: 1.2,
            width: 1000.0,
            height: 1000.0,
        }
    }

    #[test]
    fn composing_with_the_inverse_yields_the_identity() {
        let a = Similarity {
            dx: 12.0,
            dy: -7.0,
            rotation: 0.3,
            scale: 1.4,
        };
        let round = a.compose(&a.inverse());
        assert!(round.dx.abs() < 1e-9, "dx {}", round.dx);
        assert!(round.dy.abs() < 1e-9, "dy {}", round.dy);
        assert!(round.rotation.abs() < 1e-9);
        assert!((round.scale - 1.0).abs() < 1e-9);
    }

    /// Die Projektion muss eine echte Ähnlichkeit unverändert
    /// zurückgeben — sonst verfälschte sie schon die Messung.
    #[test]
    fn projecting_a_pure_similarity_recovers_it_exactly() {
        let original = Similarity {
            dx: 25.0,
            dy: -13.0,
            rotation: 0.2,
            scale: 1.15,
        };
        let recovered = Similarity::from_homography(&original.to_matrix());
        assert!((recovered.dx - original.dx).abs() < 1e-9);
        assert!((recovered.dy - original.dy).abs() < 1e-9);
        assert!((recovered.rotation - original.rotation).abs() < 1e-9);
        assert!((recovered.scale - original.scale).abs() < 1e-9);
    }

    /// Perspektivische Anteile — die Quelle des „Wackelpuddings" —
    /// müssen bei der Projektion verschwinden.
    #[test]
    fn projecting_discards_the_perspective_part() {
        let mut wobbly = Similarity {
            dx: 10.0,
            dy: 5.0,
            ..Similarity::IDENTITY
        }
        .to_matrix();
        wobbly[(2, 0)] = 0.0004;
        wobbly[(2, 1)] = -0.0003;
        let projected = Similarity::from_homography(&wobbly);
        let back = projected.to_matrix();
        assert_eq!(back[(2, 0)], 0.0);
        assert_eq!(back[(2, 1)], 0.0);
        assert!((projected.dx - 10.0).abs() < 1e-9);
    }

    #[test]
    fn a_perfectly_still_camera_needs_no_correction() {
        let still = vec![Some(Similarity::IDENTITY); 40];
        for correction in stabilize_path(&still, &params()) {
            assert!(correction.dx.abs() < 1e-9);
            assert!(correction.dy.abs() < 1e-9);
            assert!((correction.scale - 1.0).abs() < 1e-9);
        }
    }

    /// **Der entscheidende Test.** Ein gewollter, gleichmäßiger Schwenk
    /// darf NICHT weggeregelt werden — ein Stabilisierer, der jede
    /// Bewegung entfernt, macht aus einem Schwenk ein Standbild mit
    /// wanderndem Rand.
    #[test]
    fn a_steady_pan_is_left_alone() {
        let pan: Vec<Option<Similarity>> = (0..60).map(|_| shift(4.0, 0.0)).collect();
        let corrections = stabilize_path(&pan, &params());
        // In der Mitte (weit weg von den Rändern des Glättungsfensters)
        // muss die Korrektur praktisch verschwinden.
        for correction in &corrections[20..40] {
            assert!(
                correction.dx.abs() < 0.5,
                "der Schwenk wird weggeregelt: dx = {}",
                correction.dx
            );
        }
    }

    /// **Der zweite entscheidende Test.** Zittern auf einem Schwenk muss
    /// verschwinden, der Schwenk bleiben. Gemessen wird die
    /// verbleibende Unruhe der KORRIGIERTEN Bahn gegen die der rohen.
    #[test]
    fn jitter_on_top_of_a_pan_is_removed_while_the_pan_survives() {
        let mut steps = Vec::new();
        for i in 0..80 {
            // Gleichmäßiger Schwenk plus ein hochfrequentes Zittern.
            let jitter = if i % 2 == 0 { 9.0 } else { -9.0 };
            steps.push(shift(3.0 + jitter, jitter * 0.5));
        }
        let corrections = stabilize_path(&steps, &params());
        let path = absolute_path(&steps);

        // Unruhe = mittlere Änderung der Bild-zu-Bild-Bewegung.
        let roughness = |positions: &[(f64, f64)]| -> f64 {
            positions
                .windows(3)
                .map(|w| {
                    let a = (w[1].0 - w[0].0, w[1].1 - w[0].1);
                    let b = (w[2].0 - w[1].0, w[2].1 - w[1].1);
                    ((b.0 - a.0).powi(2) + (b.1 - a.1).powi(2)).sqrt()
                })
                .sum::<f64>()
                / (positions.len().max(3) - 2) as f64
        };

        let raw: Vec<(f64, f64)> = path[10..70].iter().map(|p| (p.dx, p.dy)).collect();
        let fixed: Vec<(f64, f64)> = path[10..70]
            .iter()
            .zip(&corrections[10..70])
            .map(|(p, c)| {
                let corrected = c.compose(p);
                (corrected.dx, corrected.dy)
            })
            .collect();

        let before = roughness(&raw);
        let after = roughness(&fixed);
        assert!(
            after < before * 0.25,
            "das Zittern bleibt: Unruhe vorher {before:.2}, nachher {after:.2}"
        );
        // Der Schwenk muss erhalten bleiben: Anfang und Ende der
        // korrigierten Bahn liegen weiterhin weit auseinander.
        let travelled = fixed.last().unwrap().0 - fixed.first().unwrap().0;
        assert!(
            travelled > 100.0,
            "der Schwenk wurde mit weggeregelt: nur {travelled:.1} px zurückgelegt"
        );
    }

    /// Keine Korrektur darf über den Zuschnitt-Rand hinausgehen, sonst
    /// erscheint im fertigen Video ein schwarzer Rand.
    #[test]
    fn corrections_never_exceed_the_crop_margin() {
        // Absichtlich heftige, sprunghafte Bewegung.
        let wild: Vec<Option<Similarity>> = (0..50)
            .map(|i| shift(if i % 2 == 0 { 200.0 } else { -180.0 }, 150.0))
            .collect();
        let p = params();
        let (max_x, max_y) = p.max_shift();
        for correction in stabilize_path(&wild, &p) {
            assert!(
                correction.dx.abs() <= max_x + 1e-9 && correction.dy.abs() <= max_y + 1e-9,
                "Korrektur {correction:?} sprengt den Rand ({max_x}, {max_y})"
            );
        }
    }

    /// Eine fehlgeschlagene Messung darf keinen Ruck erzeugen: ohne
    /// Messwert wird Stillstand angenommen, nicht extrapoliert.
    #[test]
    fn a_failed_measurement_does_not_cause_a_jump() {
        let mut steps: Vec<Option<Similarity>> = (0..40).map(|_| shift(2.0, 0.0)).collect();
        steps[20] = None;
        let corrections = stabilize_path(&steps, &params());
        for pair in corrections.windows(2) {
            let jump = (pair[1].dx - pair[0].dx).abs();
            assert!(jump < 5.0, "Ruck von {jump:.2} px an der Messlücke");
        }
    }

    #[test]
    fn the_crop_margin_follows_the_zoom() {
        let none = StabilizeParams {
            crop_zoom: 1.0,
            width: 1000.0,
            height: 500.0,
            ..StabilizeParams::default()
        };
        assert_eq!(none.max_shift(), (0.0, 0.0));
        let some = StabilizeParams {
            crop_zoom: 1.2,
            ..none
        };
        let (x, y) = some.max_shift();
        // (1.2 - 1) / 1.2 / 2 = 8,33 %
        assert!((x - 83.33).abs() < 0.1, "x = {x}");
        assert!((y - 41.67).abs() < 0.1, "y = {y}");
    }

    #[test]
    fn an_empty_clip_produces_no_corrections() {
        assert!(stabilize_path(&[], &params()).is_empty());
    }

    // --- Zuschnitt-Zoom und Verzerrung -----------------------------

    /// Ein Bild mit einem hellen 3×3-Fleck an einer bekannten Stelle.
    fn frame_with_spot(width: usize, height: usize, cx: usize, cy: usize) -> Vec<u8> {
        let mut frame = vec![0u8; width * height * 3];
        for y in cy.saturating_sub(1)..=(cy + 1).min(height - 1) {
            for x in cx.saturating_sub(1)..=(cx + 1).min(width - 1) {
                let base = (y * width + x) * 3;
                frame[base..base + 3].copy_from_slice(&[255, 255, 255]);
            }
        }
        frame
    }

    /// Helligkeits-Schwerpunkt — nicht das hellste Pixel: der Fleck ist
    /// 3×3 groß und wird beim Verzerren bilinear verschmiert, ein
    /// einzelnes Maximum wäre also weder eindeutig noch aussagekräftig.
    fn bright_centroid(frame: &[u8], width: usize, height: usize) -> (f64, f64) {
        let (mut sum_x, mut sum_y, mut weight) = (0.0, 0.0, 0.0);
        for y in 0..height {
            for x in 0..width {
                let value = frame[(y * width + x) * 3] as f64;
                sum_x += x as f64 * value;
                sum_y += y as f64 * value;
                weight += value;
            }
        }
        assert!(weight > 0.0, "Bild ist komplett schwarz");
        (sum_x / weight, sum_y / weight)
    }

    #[test]
    fn the_zoom_keeps_the_image_centre_where_it_is() {
        let zoom = zoom_about_center(1.2, 100.0, 80.0);
        // Die Mitte ist der Fixpunkt: (50, 40) muss auf sich selbst fallen.
        let (sin, cos) = zoom.rotation.sin_cos();
        let x = zoom.dx + zoom.scale * (cos * 50.0 - sin * 40.0);
        let y = zoom.dy + zoom.scale * (sin * 50.0 + cos * 40.0);
        assert!((x - 50.0).abs() < 1e-9, "x {x}");
        assert!((y - 40.0).abs() < 1e-9, "y {y}");
    }

    #[test]
    fn warping_by_the_identity_returns_the_frame_unchanged() {
        let frame = frame_with_spot(24, 16, 10, 6);
        let warped = warp_rgb8(&frame, 24, 16, &Similarity::IDENTITY);
        assert_eq!(warped, frame);
    }

    #[test]
    fn a_translation_moves_the_content_by_exactly_that_amount() {
        let frame = frame_with_spot(40, 30, 12, 9);
        let warped = warp_rgb8(
            &frame,
            40,
            30,
            &Similarity {
                dx: 5.0,
                dy: -3.0,
                ..Similarity::IDENTITY
            },
        );
        let (x, y) = bright_centroid(&warped, 40, 30);
        assert!((x - 17.0).abs() < 1e-6, "x {x}");
        assert!((y - 6.0).abs() < 1e-6, "y {y}");
    }

    #[test]
    fn a_shaking_frame_lands_where_the_steady_frames_land() {
        // Die Kamera steht still, nur Bild 1 ist um 6 px verrutscht.
        // Nach der Stabilisierung müssen beide Bilder denselben Fleck an
        // derselben Stelle zeigen — verglichen wird ein ruhiges gegen das
        // wackelige Bild, beide durch dieselbe Kette aus Korrektur UND
        // Zuschnitt-Zoom. Die alternative Prüfung „liegt nahe 21,2" wäre
        // schwächer: sie hinge daran, wie der Zoom den Wert nachträglich
        // noch verschiebt.
        let steps = vec![
            shift(0.0, 0.0),
            shift(6.0, 0.0),
            shift(-6.0, 0.0),
            shift(0.0, 0.0),
            shift(0.0, 0.0),
        ];
        let params = StabilizeParams {
            smoothing_radius: 4,
            crop_zoom: 1.2,
            width: 60.0,
            height: 40.0,
        };
        let corrections = stabilize_path(&steps, &params);

        let steady = frame_with_spot(60, 40, 20, 20);
        let shaken = frame_with_spot(60, 40, 26, 20); // um 6 px verrutscht

        // Unbehandelt liegen die beiden um 6 px auseinander — ohne
        // Stabilisierung könnte dieser Test gar nicht bestehen.
        let raw_gap = bright_centroid(&shaken, 60, 40).0 - bright_centroid(&steady, 60, 40).0;
        assert!((raw_gap - 6.0).abs() < 1e-6, "Ausgangslage {raw_gap}");

        let fixed_steady = warp_rgb8(&steady, 60, 40, &frame_transform(&corrections[0], &params));
        let fixed_shaken = warp_rgb8(&shaken, 60, 40, &frame_transform(&corrections[1], &params));
        let gap =
            bright_centroid(&fixed_shaken, 60, 40).0 - bright_centroid(&fixed_steady, 60, 40).0;
        assert!(
            gap.abs() < 0.5,
            "nach der Stabilisierung noch {gap} px Versatz"
        );
    }

    #[test]
    fn without_a_crop_margin_nothing_may_be_corrected() {
        // `crop_zoom = 1.0` heißt: kein Rand zum Verdecken da. Jede
        // Verschiebung würde schwarze Kanten freilegen, also wird auf
        // null begrenzt — das ist Absicht, kein Ausfall.
        let params = StabilizeParams {
            smoothing_radius: 4,
            crop_zoom: 1.0,
            width: 60.0,
            height: 40.0,
        };
        let corrections = stabilize_path(&[shift(0.0, 0.0), shift(6.0, 0.0)], &params);
        assert!(corrections.iter().all(|c| c.dx == 0.0 && c.dy == 0.0));
    }

    #[test]
    fn a_measurement_taken_at_half_resolution_scales_up_to_full_pixels() {
        let measured = Similarity {
            dx: 4.0,
            dy: -2.5,
            rotation: 0.2,
            scale: 1.05,
        };
        let full = measured.scaled_translation(2.0);
        assert!((full.dx - 8.0).abs() < 1e-12);
        assert!((full.dy + 5.0).abs() < 1e-12);
        // Winkel und Maßstab sind dimensionslos und dürfen sich NICHT ändern.
        assert!((full.rotation - measured.rotation).abs() < 1e-12);
        assert!((full.scale - measured.scale).abs() < 1e-12);
    }
}
