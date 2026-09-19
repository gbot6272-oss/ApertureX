//! Ähnliche Fotos zu einem Referenzfoto (Phase 33 F9).
//!
//! **Abgrenzung zur Duplikatsuche.** `list_perceptual_duplicate_groups`
//! beantwortet „welche Fotos sind dasselbe Bild?" — eine
//! Alle-gegen-alle-Gruppierung mit fester Schwelle. Hier geht es um die
//! andere Frage: „welche Fotos sind *diesem hier* ähnlich?", mit einer
//! Rangfolge und einem Regler. Das ist kein Sonderfall der ersten: eine
//! Gruppierung kennt keine Referenz und damit keine Rangfolge.
//!
//! **Warum zwei Maße und nicht eines.** Ein Perceptual Hash vergleicht
//! die grobe Helligkeitsstruktur — er beantwortet „ist das dasselbe
//! Bild?" und ist dabei fast farbenblind: ein Foto und seine
//! Schwarzweiß-Fassung sind für ihn nahezu identisch. Ein Fotograf, der
//! „zeig mir ähnliche" sagt, meint aber oft das Gegenteil: nicht
//! dasselbe Motiv, sondern denselben Look — dieselbe Farbstimmung,
//! dieselbe Tonalität. Deshalb gibt es hier beides, ein
//! Struktur- und ein Farbmaß, und einen Regler dazwischen. Ein einziges
//! festverdrahtetes Mischverhältnis hätte die halbe Frage nicht
//! beantworten können.
//!
//! Beide Maße laufen auf `0.0..=1.0` (0 = identisch), damit sie sich
//! überhaupt mischen lassen — ohne das hinge das Ergebnis daran, dass
//! der Hash zufällig 64 Bit hat.

/// Kantenlänge des Farbwürfels je Kanal. 4 ergibt 64 Klassen — grob
/// genug, dass eine leicht andere Belichtung nicht in eine andere Klasse
/// rutscht, fein genug, um einen blauen von einem goldenen Abend zu
/// unterscheiden.
pub const COLOR_BINS: usize = 4;
pub const COLOR_HISTOGRAM_LEN: usize = COLOR_BINS * COLOR_BINS * COLOR_BINS;

/// Der Fingerabdruck eines Fotos: Struktur plus Farbe.
#[derive(Debug, Clone, PartialEq)]
pub struct Fingerprint {
    /// Perceptual-Hash-Bits, wie `image_hasher` sie liefert.
    pub hash_bits: Vec<u8>,
    /// Normiertes Farbhistogramm (Summe 1.0, oder alles 0 bei einem
    /// leeren Bild).
    pub color: Vec<f32>,
}

/// Baut das Farbhistogramm eines RGBA8-Puffers.
///
/// Vollständig durchsichtige Pixel zählen nicht mit: ein freigestelltes
/// Motiv soll nicht über die Farbe seines leeren Hintergrunds verglichen
/// werden.
pub fn color_histogram(pixels: &[u8]) -> Vec<f32> {
    let mut bins = vec![0.0f32; COLOR_HISTOGRAM_LEN];
    let mut counted = 0.0f32;
    for px in pixels.chunks_exact(4) {
        if px[3] == 0 {
            continue;
        }
        let bin = |value: u8| (value as usize * COLOR_BINS / 256).min(COLOR_BINS - 1);
        let index = bin(px[0]) * COLOR_BINS * COLOR_BINS + bin(px[1]) * COLOR_BINS + bin(px[2]);
        bins[index] += 1.0;
        counted += 1.0;
    }
    if counted > 0.0 {
        for value in bins.iter_mut() {
            *value /= counted;
        }
    }
    bins
}

/// Strukturabstand aus zwei Hash-Bitfolgen, normiert auf `0.0..=1.0`.
///
/// Unterschiedlich lange Folgen (verschiedene Hash-Größen) ergeben den
/// Höchstabstand statt eines Fehlers: das kann nur bei gemischt alten
/// Daten passieren, und „maximal unähnlich" ist dort die harmlose
/// Antwort — das Foto fällt aus den Treffern, statt die ganze Suche
/// abzubrechen.
pub fn structure_distance(a: &[u8], b: &[u8]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 1.0;
    }
    let differing: u32 = a
        .iter()
        .zip(b.iter())
        .map(|(x, y)| (x ^ y).count_ones())
        .sum();
    differing as f32 / (a.len() as f32 * 8.0)
}

/// Farbabstand zweier normierter Histogramme, `0.0..=1.0`.
///
/// Die halbierte L1-Distanz — bei zwei Verteilungen mit Summe 1 liegt
/// sie genau in `0..=1`, mit 1 für „keine einzige gemeinsame Farbklasse".
/// Anschaulicher als eine euklidische Distanz, die bei 64 Klassen von
/// niemandem mehr eingeordnet werden kann.
pub fn color_distance(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 1.0;
    }
    let sum: f32 = a.iter().zip(b.iter()).map(|(x, y)| (x - y).abs()).sum();
    (sum / 2.0).clamp(0.0, 1.0)
}

/// Mischt beide Abstände zu einer Ähnlichkeit `0.0..=1.0` (1 = gleich).
///
/// `color_weight` 0 heißt „nur Motiv", 1 heißt „nur Farbe".
pub fn similarity(reference: &Fingerprint, other: &Fingerprint, color_weight: f32) -> f32 {
    let weight = color_weight.clamp(0.0, 1.0);
    let structure = structure_distance(&reference.hash_bits, &other.hash_bits);
    let color = color_distance(&reference.color, &other.color);
    let distance = structure * (1.0 - weight) + color * weight;
    (1.0 - distance).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rgba(pixels: &[[u8; 3]]) -> Vec<u8> {
        pixels
            .iter()
            .flat_map(|[r, g, b]| [*r, *g, *b, 255])
            .collect()
    }

    #[test]
    fn ein_histogramm_summiert_sich_zu_eins() {
        let hist = color_histogram(&rgba(&[[10, 10, 10], [250, 250, 250], [10, 200, 10]]));
        let sum: f32 = hist.iter().sum();
        assert!((sum - 1.0).abs() < 1e-5, "Summe {sum}");
    }

    #[test]
    fn durchsichtige_pixel_zaehlen_nicht_mit() {
        // Ein freigestelltes Motiv soll nicht über die Farbe seines
        // leeren Hintergrunds verglichen werden.
        let mut pixels = rgba(&[[255, 0, 0]]);
        pixels.extend_from_slice(&[0, 0, 255, 0]);
        let hist = color_histogram(&pixels);
        let sum: f32 = hist.iter().sum();
        assert!((sum - 1.0).abs() < 1e-5);
        // Das rote Pixel muss die einzige belegte Klasse sein.
        assert_eq!(hist.iter().filter(|v| **v > 0.0).count(), 1);
    }

    #[test]
    fn ein_leeres_bild_ergibt_ein_leeres_histogramm_statt_einer_division_durch_null() {
        let hist = color_histogram(&[]);
        assert_eq!(hist.len(), COLOR_HISTOGRAM_LEN);
        assert!(hist.iter().all(|v| *v == 0.0));
    }

    #[test]
    fn gleiche_bilder_haben_den_abstand_null() {
        assert_eq!(structure_distance(&[0b1010_1010], &[0b1010_1010]), 0.0);
        let hist = color_histogram(&rgba(&[[10, 10, 10]]));
        assert_eq!(color_distance(&hist, &hist), 0.0);
    }

    #[test]
    fn gegensaetzliche_bits_ergeben_den_hoechstabstand() {
        assert_eq!(structure_distance(&[0x00, 0x00], &[0xFF, 0xFF]), 1.0);
    }

    #[test]
    fn der_strukturabstand_ist_auf_die_hash_laenge_normiert() {
        // Ein einzelnes abweichendes Bit in 16 Bit sind 1/16.
        let d = structure_distance(&[0x00, 0x00], &[0x01, 0x00]);
        assert!((d - 1.0 / 16.0).abs() < 1e-6, "{d}");
    }

    #[test]
    fn unterschiedlich_lange_hashes_gelten_als_maximal_unaehnlich() {
        assert_eq!(structure_distance(&[0x00], &[0x00, 0x00]), 1.0);
        assert_eq!(structure_distance(&[], &[]), 1.0);
    }

    #[test]
    fn zwei_voellig_verschiedene_farben_haben_den_farbabstand_eins() {
        let rot = color_histogram(&rgba(&[[255, 0, 0]]));
        let blau = color_histogram(&rgba(&[[0, 0, 255]]));
        assert!((color_distance(&rot, &blau) - 1.0).abs() < 1e-5);
    }

    #[test]
    fn eine_halbe_ueberlappung_ergibt_den_halben_farbabstand() {
        let a = color_histogram(&rgba(&[[255, 0, 0], [0, 0, 255]]));
        let b = color_histogram(&rgba(&[[255, 0, 0], [0, 255, 0]]));
        let d = color_distance(&a, &b);
        assert!((d - 0.5).abs() < 1e-5, "{d}");
    }

    #[test]
    fn der_regler_entscheidet_welches_mass_zaehlt() {
        // Gleiche Struktur, völlig andere Farbe — genau der Fall einer
        // Schwarzweiß-Fassung desselben Fotos.
        let reference = Fingerprint {
            hash_bits: vec![0xAA, 0xAA],
            color: color_histogram(&rgba(&[[255, 0, 0]])),
        };
        let other = Fingerprint {
            hash_bits: vec![0xAA, 0xAA],
            color: color_histogram(&rgba(&[[0, 0, 255]])),
        };

        // Nur Motiv: identisch.
        assert!((similarity(&reference, &other, 0.0) - 1.0).abs() < 1e-5);
        // Nur Farbe: maximal unähnlich.
        assert!(similarity(&reference, &other, 1.0) < 1e-5);
        // Dazwischen: dazwischen.
        let mitte = similarity(&reference, &other, 0.5);
        assert!(mitte > 0.4 && mitte < 0.6, "{mitte}");
    }

    #[test]
    fn ein_foto_ist_sich_selbst_maximal_aehnlich() {
        let fingerprint = Fingerprint {
            hash_bits: vec![0x12, 0x34],
            color: color_histogram(&rgba(&[[1, 2, 3], [200, 100, 50]])),
        };
        for weight in [0.0, 0.5, 1.0] {
            assert!((similarity(&fingerprint, &fingerprint, weight) - 1.0).abs() < 1e-5);
        }
    }

    #[test]
    fn ein_regler_ausserhalb_des_bereichs_wird_begrenzt_statt_zu_kippen() {
        let a = Fingerprint {
            hash_bits: vec![0x00],
            color: color_histogram(&rgba(&[[255, 0, 0]])),
        };
        let b = Fingerprint {
            hash_bits: vec![0xFF],
            color: color_histogram(&rgba(&[[255, 0, 0]])),
        };
        // Ohne Begrenzung ergäbe ein negatives Gewicht eine Ähnlichkeit
        // über 1 oder unter 0.
        let low = similarity(&a, &b, -5.0);
        let high = similarity(&a, &b, 5.0);
        assert!((0.0..=1.0).contains(&low));
        assert!((0.0..=1.0).contains(&high));
    }
}
