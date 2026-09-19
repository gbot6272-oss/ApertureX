//! Schärfe-Bewertung eines ganzen Bildes (Phase 33 F3) — die Grundlage
//! für „welche Aufnahme dieser Serie ist die beste?".
//!
//! **Abgrenzung zum Fokus-Stacking nebenan.** `focus.rs` berechnet
//! dieselbe Laplace-Antwort, aber *pro Pixel* und um daraus ein Bild
//! zusammenzusetzen. Hier geht es um eine einzige Zahl pro Bild, mit der
//! sich zwanzig Aufnahmen derselben Sekunde sortieren lassen. Die
//! Laplace-Berechnung ist bewusst noch einmal hingeschrieben statt
//! wiederverwendet: `focus.rs` braucht die volle Karte im Speicher,
//! hier reicht ein laufender Mittelwert pro Kachel — bei zwanzig
//! 2048px-Bildern ist das der Unterschied zwischen 20 × 16 MB und
//! nichts.
//!
//! **Drei Entscheidungen, die das Ergebnis brauchbar machen:**
//!
//! 1. **Kacheln statt Gesamtmittel.** Ein Porträt mit offener Blende ist
//!    zu 80 % unscharf — und genau so soll es sein. Ein Mittelwert über
//!    das ganze Bild würde es gegen einen durchgehend mittelmäßigen
//!    Schnappschuss verlieren lassen. Deshalb wird je Kachel gemessen
//!    und daraus ein hohes Perzentil genommen: die Frage ist nicht „wie
//!    scharf ist das Bild im Schnitt", sondern „wie scharf ist es da, wo
//!    es scharf sein soll".
//! 2. **Kontrast herausgerechnet.** Die Laplace-Energie wächst mit dem
//!    Quadrat des Kontrasts. Ohne Normierung gewinnt jedes Mal die
//!    hellere, kontrastreichere Aufnahme — auch die unschärfere.
//!    Geteilt wird deshalb durch die Varianz derselben Kachel; beide
//!    skalieren mit dem Quadrat des Kontrasts, das Verhältnis bleibt.
//! 3. **Nicht das Perzentil über alle Pixel, sondern über die Kacheln.**
//!    Über Pixel gerechnet wäre das Ergebnis vom Rauschen bestimmt —
//!    einzelne Ausreißer sind in einer hochgezogenen ISO-Aufnahme immer
//!    da. Eine Kachel mittelt sie weg.
//!
//! **Was diese Zahl nicht ist:** ein absolutes Schärfemaß. Sie hängt an
//! Auflösung, Motiv und Rauschen. Sie taugt zum Vergleich von
//! Aufnahmen *desselben Motivs in derselben Größe* — also genau für die
//! Serie, für die sie gebaut ist.

use crate::error::{Result, StackingError};
use crate::luma::rgba8_to_luma_f32;

/// Kantenlänge einer Messkachel in Pixeln. 64 ist groß genug, dass
/// Rauschen herausmittelt, und klein genug, dass ein scharfes Auge in
/// einem sonst unscharfen Porträt eine eigene Kachel bekommt.
pub const TILE_SIZE: u32 = 64;

/// Anteil der Kacheln, der unter dem Ergebniswert liegt. 0.9 heißt: die
/// zehn Prozent schärfsten Kacheln entscheiden. Nicht 1.0 (das Maximum
/// wäre wieder ein einzelner Ausreißer) und nicht 0.5 (der Median wäre
/// wieder ein Gesamtmittel mit Zwischenschritt).
pub const SHARPNESS_PERCENTILE: f32 = 0.9;

/// Verhindert eine Division durch null in einer völlig einfarbigen
/// Kachel. Auf der 0..255-Luminanzskala vernachlässigbar klein.
const VARIANCE_EPSILON: f32 = 1.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SharpnessScore {
    /// Der Vergleichswert: hohes Perzentil über die Kachelwerte.
    pub score: f32,
    /// Mittelwert über alle Kacheln — sagt, wie *großflächig* scharf das
    /// Bild ist. Zusammen mit `score` unterscheidbar: hoher `score` bei
    /// niedrigem `mean` heißt „selektiv scharf" (offene Blende), beides
    /// hoch heißt „durchgehend scharf".
    pub mean: f32,
    /// Anzahl der ausgewerteten Kacheln.
    pub tiles: usize,
}

/// Misst die Schärfe eines RGBA8-Bildes.
///
/// Fehler nur bei einem Puffer, der nicht zu `width * height` passt —
/// ein zu kleines Bild (weniger als eine Kachel) ist kein Fehler,
/// sondern wird als eine einzige Kachel gemessen.
pub fn score_rgba8(pixels: &[u8], width: u32, height: u32) -> Result<SharpnessScore> {
    let expected = width as usize * height as usize * 4;
    if pixels.len() != expected {
        return Err(StackingError::InvalidInput(format!(
            "RGBA8-Puffer hat {} Bytes, erwartet waren {expected} für {width}×{height}",
            pixels.len()
        )));
    }
    if width == 0 || height == 0 {
        return Err(StackingError::InvalidInput(
            "Bild ohne Ausdehnung kann nicht bewertet werden".to_string(),
        ));
    }

    let luma = rgba8_to_luma_f32(pixels);
    let mut tile_scores = tile_scores(&luma, width, height);
    if tile_scores.is_empty() {
        return Ok(SharpnessScore {
            score: 0.0,
            mean: 0.0,
            tiles: 0,
        });
    }

    let mean = tile_scores.iter().sum::<f32>() / tile_scores.len() as f32;
    tile_scores.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let index = ((tile_scores.len() - 1) as f32 * SHARPNESS_PERCENTILE).round() as usize;
    Ok(SharpnessScore {
        score: tile_scores[index],
        mean,
        tiles: tile_scores.len(),
    })
}

/// Kontrastnormierte Laplace-Energie je Kachel.
fn tile_scores(luma: &[f32], width: u32, height: u32) -> Vec<f32> {
    let w = width as i32;
    let h = height as i32;
    let at = |x: i32, y: i32| -> f32 {
        let cx = x.clamp(0, w - 1);
        let cy = y.clamp(0, h - 1);
        luma[(cy * w + cx) as usize]
    };

    let tiles_x = width.div_ceil(TILE_SIZE);
    let tiles_y = height.div_ceil(TILE_SIZE);
    let mut scores = Vec::with_capacity((tiles_x * tiles_y) as usize);

    for ty in 0..tiles_y {
        for tx in 0..tiles_x {
            let x0 = (tx * TILE_SIZE) as i32;
            let y0 = (ty * TILE_SIZE) as i32;
            let x1 = ((tx + 1) * TILE_SIZE).min(width) as i32;
            let y1 = ((ty + 1) * TILE_SIZE).min(height) as i32;

            let mut count = 0.0f32;
            let mut sum = 0.0f32;
            let mut sum_sq = 0.0f32;
            let mut laplace_energy = 0.0f32;
            for y in y0..y1 {
                for x in x0..x1 {
                    let center = at(x, y);
                    let laplacian =
                        4.0 * center - at(x - 1, y) - at(x + 1, y) - at(x, y - 1) - at(x, y + 1);
                    laplace_energy += laplacian * laplacian;
                    sum += center;
                    sum_sq += center * center;
                    count += 1.0;
                }
            }
            if count == 0.0 {
                continue;
            }
            let mean = sum / count;
            let variance = (sum_sq / count - mean * mean).max(0.0);
            scores.push((laplace_energy / count) / (variance + VARIANCE_EPSILON));
        }
    }
    scores
}

/// Sortiert Bewertungen von der schärfsten zur unschärfsten Aufnahme und
/// gibt die Ursprungs-Indizes zurück.
///
/// Bei gleichem Wert gewinnt der kleinere Index — die Reihenfolge ist
/// damit auch bei identischen Aufnahmen stabil, statt von der
/// Sortierimplementierung abzuhängen.
pub fn rank_best_first(scores: &[SharpnessScore]) -> Vec<usize> {
    let mut order: Vec<usize> = (0..scores.len()).collect();
    order.sort_by(|&a, &b| {
        scores[b]
            .score
            .partial_cmp(&scores[a].score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.cmp(&b))
    });
    order
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Schachbrett mit `cell`-großen Feldern — je kleiner `cell`, desto
    /// höherfrequenter und damit „schärfer".
    fn checkerboard(width: u32, height: u32, cell: u32, amplitude: u8) -> Vec<u8> {
        let mut out = Vec::with_capacity((width * height * 4) as usize);
        for y in 0..height {
            for x in 0..width {
                let on = ((x / cell) + (y / cell)).is_multiple_of(2);
                let value = if on { amplitude } else { 0 };
                out.extend_from_slice(&[value, value, value, 255]);
            }
        }
        out
    }

    /// Weichzeichnet ein RGBA8-Bild mit einem 3×3-Boxfilter — das
    /// „unscharfe" Gegenstück zu einem Testbild.
    fn blur(pixels: &[u8], width: u32, height: u32) -> Vec<u8> {
        let w = width as i32;
        let h = height as i32;
        let at = |x: i32, y: i32, c: usize| -> f32 {
            let cx = x.clamp(0, w - 1);
            let cy = y.clamp(0, h - 1);
            pixels[((cy * w + cx) as usize) * 4 + c] as f32
        };
        let mut out = vec![0u8; pixels.len()];
        for y in 0..h {
            for x in 0..w {
                for c in 0..3 {
                    let mut sum = 0.0;
                    for dy in -1..=1 {
                        for dx in -1..=1 {
                            sum += at(x + dx, y + dy, c);
                        }
                    }
                    out[((y * w + x) as usize) * 4 + c] = (sum / 9.0).round() as u8;
                }
                out[((y * w + x) as usize) * 4 + 3] = 255;
            }
        }
        out
    }

    fn flat(width: u32, height: u32, value: u8) -> Vec<u8> {
        vec![value; (width * height * 4) as usize]
            .chunks_exact(4)
            .flat_map(|_| [value, value, value, 255])
            .collect()
    }

    #[test]
    fn ein_scharfes_bild_schlaegt_seine_weichgezeichnete_fassung() {
        let sharp = checkerboard(128, 128, 4, 255);
        let soft = blur(&sharp, 128, 128);
        let sharp_score = score_rgba8(&sharp, 128, 128).unwrap();
        let soft_score = score_rgba8(&soft, 128, 128).unwrap();
        assert!(
            sharp_score.score > soft_score.score * 1.5,
            "scharf {sharp_score:?} vs. weich {soft_score:?}"
        );
    }

    #[test]
    fn eine_einfarbige_flaeche_hat_keine_schaerfe() {
        let score = score_rgba8(&flat(128, 128, 128), 128, 128).unwrap();
        assert!(score.score < 1e-3, "{score:?}");
    }

    #[test]
    fn der_kontrast_faellt_aus_der_bewertung_heraus() {
        // Dasselbe Motiv, einmal mit vollem und einmal mit halbem
        // Kontrast. Ohne die Varianznormierung wäre der zweite Wert etwa
        // ein Viertel des ersten.
        let full = checkerboard(128, 128, 4, 255);
        let half = checkerboard(128, 128, 4, 128);
        let a = score_rgba8(&full, 128, 128).unwrap().score;
        let b = score_rgba8(&half, 128, 128).unwrap().score;
        let ratio = a / b;
        assert!(
            ratio > 0.9 && ratio < 1.1,
            "Verhältnis {ratio} (a={a}, b={b})"
        );
    }

    #[test]
    fn ein_selektiv_scharfes_bild_schlaegt_ein_durchgehend_unscharfes() {
        // Genau der Porträt-Fall: links scharf, rechts Bokeh. Gegen ein
        // Bild, das überall gleich weich ist.
        let sharp = checkerboard(256, 128, 4, 255);
        let soft = blur(&sharp, 256, 128);
        let mut selective = soft.clone();
        for y in 0..128u32 {
            for x in 0..64u32 {
                let i = ((y * 256 + x) * 4) as usize;
                selective[i..i + 4].copy_from_slice(&sharp[i..i + 4]);
            }
        }
        let selective_score = score_rgba8(&selective, 256, 128).unwrap();
        let soft_score = score_rgba8(&soft, 256, 128).unwrap();
        assert!(
            selective_score.score > soft_score.score,
            "selektiv {selective_score:?} vs. weich {soft_score:?}"
        );
    }

    #[test]
    fn selektive_und_durchgehende_schaerfe_lassen_sich_unterscheiden() {
        let sharp = checkerboard(256, 128, 4, 255);
        let soft = blur(&sharp, 256, 128);
        let mut selective = soft.clone();
        for y in 0..128u32 {
            for x in 0..64u32 {
                let i = ((y * 256 + x) * 4) as usize;
                selective[i..i + 4].copy_from_slice(&sharp[i..i + 4]);
            }
        }
        let selective_score = score_rgba8(&selective, 256, 128).unwrap();
        let full_score = score_rgba8(&sharp, 256, 128).unwrap();
        // Der Spitzenwert ist vergleichbar, der Mittelwert nicht — genau
        // dafür gibt es `mean` neben `score`.
        assert!(
            selective_score.mean < full_score.mean * 0.8,
            "{selective_score:?} / {full_score:?}"
        );
    }

    #[test]
    fn ein_bild_kleiner_als_eine_kachel_wird_als_eine_kachel_gemessen() {
        let small = checkerboard(10, 10, 2, 255);
        let score = score_rgba8(&small, 10, 10).unwrap();
        assert_eq!(score.tiles, 1);
        assert!(score.score > 0.0);
    }

    #[test]
    fn eine_nicht_durch_die_kachelgroesse_teilbare_breite_verliert_keine_pixel() {
        let odd = checkerboard(130, 70, 4, 255);
        let score = score_rgba8(&odd, 130, 70).unwrap();
        // 130/64 -> 3 Spalten, 70/64 -> 2 Zeilen.
        assert_eq!(score.tiles, 6);
    }

    #[test]
    fn ein_falsch_grosser_puffer_ist_ein_fehler() {
        assert!(score_rgba8(&[0, 0, 0, 255], 4, 4).is_err());
    }

    #[test]
    fn die_rangfolge_geht_vom_schaerfsten_zum_unschaerfsten() {
        let make = |score: f32| SharpnessScore {
            score,
            mean: score,
            tiles: 1,
        };
        let order = rank_best_first(&[make(1.0), make(5.0), make(3.0)]);
        assert_eq!(order, vec![1, 2, 0]);
    }

    #[test]
    fn bei_gleichstand_gewinnt_die_fruehere_aufnahme() {
        let make = |score: f32| SharpnessScore {
            score,
            mean: score,
            tiles: 1,
        };
        let order = rank_best_first(&[make(2.0), make(2.0), make(2.0)]);
        assert_eq!(order, vec![0, 1, 2]);
    }

    #[test]
    fn eine_leere_liste_hat_keine_rangfolge() {
        assert!(rank_best_first(&[]).is_empty());
    }
}
