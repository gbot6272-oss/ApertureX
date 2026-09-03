//! Farb-Harmonie-Rad: automatische Paletten-Extraktion (Phase 14
//! Schritt 7, siehe `DECISIONS.md` ADR-0041 Nachtrag VII, Recherche-
//! Tabelle Punkt 10): Lightroom Classic/CC-Color-Grading-Räder sind rein
//! manuell — keine automatische Paletten-Extraktion mit Harmonie-
//! Vorschlag gefunden.
//!
//! Nutzt `kmeans_colors` (real per `cargo add --dry-run` geprüft,
//! v0.7.1, MIT/Apache-2.0, `--no-default-features --features
//! palette_color` — spart die CLI-Abhängigkeiten `app`/`structopt`/
//! `image`, die dieses Projekt nicht braucht) über CIE-Lab-Farben, dem
//! wahrnehmungsnäheren Raum gegenüber sRGB fürs Clustern — dieselbe
//! Grundwahl wie `style_consistency`s Lab-Mittelwert in Schritt 5, hier
//! aber über `palette`s echte Farbraum-Konvertierung statt eigens
//! geschriebener sRGB->Lab-Formeln: `kmeans_colors`s `Calculate`-Trait-
//! Implementierung für `Lab` baut selbst schon auf `palette::Lab` auf,
//! eine zweite eigene Umrechnung wäre hier unverhältnismäßig.
//!
//! Die eigentliche Harmonie-Berechnung (Komplementär/Triade/Split-
//! Komplementär/Analog, Zuordnung der extrahierten Palette zu den acht
//! festen `HslAdjustment`-Bändern aus
//! `apx_pipeline::stages::hsl_color_mixer`) ist reine Farbtheorie-
//! Mathematik ohne Bildzugriff und lebt deshalb im Frontend
//! (`frontend/src/lib/colorHarmony.ts`) — analog zu Schritt 6s
//! Vektorskop/Wellenform, wo ebenfalls nur die eigentliche Bildanalyse
//! Rust braucht, nicht die anschließende reine Zahlen-Mathematik.

// Absoluter Pfad (`::palette`) statt eines schlichten `use palette::...` —
// dieses Modul heißt selbst `palette` (Rust 2018 löst einen bloßen
// `use palette::...`-Import sonst gegen sich selbst auf statt gegen die
// externe Kiste).
use ::palette::{FromColor, IntoColor, Lab, Lch, Srgb};
use kmeans_colors::{get_kmeans, Sort};

/// Anzahl der k-means-Läufe mit unterschiedlichem Seed — k-means++
/// initialisiert zufällig und kann sich in einem suboptimalen lokalen
/// Minimum verfangen, deshalb mehrere Läufe, das Ergebnis mit dem
/// kleinsten `score` gewinnt (dieselbe "mehrere Läufe, bestes Ergebnis
/// behalten"-Empfehlung aus `kmeans_colors`s eigener Moduldoku).
const KMEANS_RUNS: u64 = 3;
const KMEANS_MAX_ITER: usize = 20;
/// `kmeans_colors`s eigener dokumentierter Vorgabewert für den `Lab`-Raum
/// (`Rgb` konvergiert bei einem anderen Wert, hier irrelevant, da nur
/// `Lab` verwendet wird).
const KMEANS_CONVERGE: f32 = 5.0;
const KMEANS_SEED: u64 = 0;

/// Vorgabe-Palettengröße — fünf dominante Farben sind für ein
/// Harmonie-Rad-Widget übersichtlich genug, ohne die Analyse für ein
/// Foto mit wenigen echten Farb-Clustern unnötig zu verlangsamen.
pub const DEFAULT_PALETTE_SIZE: usize = 5;

/// Eine dominante Farbe der extrahierten Palette.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PaletteColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    /// Farbton in Grad (`0.0..360.0`), aus CIE-LCh — Grundlage der
    /// Harmonie-Berechnung im Frontend.
    pub hue_degrees: f32,
    /// Buntheit (`chroma`) — je höher, desto satter die Farbe; nahe `0.0`
    /// bedeutet ein neutrales Grau, dessen Farbton kaum aussagekräftig
    /// ist (das Frontend blendet solche Farben beim Harmonisieren
    /// deshalb aus, siehe `colorHarmony.ts`).
    pub chroma: f32,
    pub lightness: f32,
    /// Anteil dieser Farbe an allen Pixeln, `0.0..=1.0`.
    pub percentage: f32,
}

/// Extrahiert die `k` dominantesten Farben aus `0.0..=1.0`-normierten
/// sRGB-Pixeln (dieselbe Eingabekonvention wie
/// `style_consistency::compute_style_signature`, drei Kanäle pro Pixel,
/// keine Alpha-Ebene) per k-means-Clustering im CIE-Lab-Raum. Ergebnis
/// absteigend nach Häufigkeit sortiert. Liefert eine leere Liste für ein
/// leeres Bild oder `k = 0`.
pub fn extract_palette(pixels: &[f32], width: u32, height: u32, k: usize) -> Vec<PaletteColor> {
    let n = (width as usize) * (height as usize);
    if n == 0 || k == 0 {
        return Vec::new();
    }

    let lab: Vec<Lab> = (0..n)
        .map(|i| {
            let idx = i * 3;
            let srgb = Srgb::new(pixels[idx], pixels[idx + 1], pixels[idx + 2]);
            srgb.into_linear().into_color()
        })
        .collect();

    let k = k.min(lab.len());
    let mut best = get_kmeans(
        k,
        KMEANS_MAX_ITER,
        KMEANS_CONVERGE,
        false,
        &lab,
        KMEANS_SEED,
    );
    for run in 1..KMEANS_RUNS {
        let candidate = get_kmeans(
            k,
            KMEANS_MAX_ITER,
            KMEANS_CONVERGE,
            false,
            &lab,
            KMEANS_SEED + run,
        );
        if candidate.score < best.score {
            best = candidate;
        }
    }

    let sorted = Lab::sort_indexed_colors(&best.centroids, &best.indices);
    let mut colors: Vec<PaletteColor> = sorted
        .into_iter()
        .map(|entry| {
            let lab_color = entry.centroid;
            // `Srgb::from_color` wendet die volle Lab->Xyz->Rgb-Kette
            // inklusive Gamma-Kodierung an (siehe `palette::Rgb`s
            // `FromColorUnclamped<Xyz>`-Implementierung: intern
            // `Self::from_linear(...)`) — kein separater manueller
            // `from_linear`-Schritt nötig, anders als beim umgekehrten
            // Weg oben.
            let srgb: Srgb<u8> = Srgb::from_color(lab_color).into_format();
            let lch: Lch = Lch::from_color(lab_color);
            PaletteColor {
                r: srgb.red,
                g: srgb.green,
                b: srgb.blue,
                hue_degrees: lch.hue.into_positive_degrees(),
                chroma: lch.chroma,
                lightness: lch.l,
                percentage: entry.percentage,
            }
        })
        .collect();
    colors.sort_by(|a, b| b.percentage.total_cmp(&a.percentage));
    colors
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flat_image(r: f32, g: f32, b: f32, count: usize) -> Vec<f32> {
        [r, g, b].repeat(count)
    }

    #[test]
    fn empty_image_yields_no_colors() {
        assert!(extract_palette(&[], 0, 0, DEFAULT_PALETTE_SIZE).is_empty());
    }

    #[test]
    fn k_zero_yields_no_colors() {
        let pixels = flat_image(1.0, 0.0, 0.0, 4);
        assert!(extract_palette(&pixels, 2, 2, 0).is_empty());
    }

    #[test]
    fn a_uniformly_red_image_yields_a_single_dominant_red_color() {
        let pixels = flat_image(0.8, 0.1, 0.1, 16);
        let palette = extract_palette(&pixels, 4, 4, 1);
        assert_eq!(palette.len(), 1);
        let red = palette[0];
        assert!(
            (red.percentage - 1.0).abs() < 1e-4,
            "percentage={}",
            red.percentage
        );
        assert!(
            red.r > red.g && red.r > red.b,
            "r={} g={} b={}",
            red.r,
            red.g,
            red.b
        );
        // Rot liegt in CIE-LCh nahe 0/360 Grad — auf beiden Seiten des
        // Wraparounds prüfen statt eines einzelnen Bereichs.
        assert!(
            red.hue_degrees < 40.0 || red.hue_degrees > 320.0,
            "hue={}",
            red.hue_degrees
        );
    }

    #[test]
    fn a_half_red_half_blue_image_yields_two_roughly_equal_colors_with_clearly_different_hues() {
        let mut pixels = flat_image(0.8, 0.1, 0.1, 8);
        pixels.extend(flat_image(0.1, 0.1, 0.8, 8));
        let palette = extract_palette(&pixels, 4, 4, 2);
        assert_eq!(palette.len(), 2);

        for color in &palette {
            assert!(
                (color.percentage - 0.5).abs() < 0.05,
                "percentage={}",
                color.percentage
            );
        }

        let hue_diff = (palette[0].hue_degrees - palette[1].hue_degrees).abs();
        let hue_diff = hue_diff.min(360.0 - hue_diff);
        assert!(hue_diff > 60.0, "hue_diff={hue_diff}");

        let dominant_channel = |color: &PaletteColor| -> usize {
            let values = [color.r, color.g, color.b];
            values
                .iter()
                .enumerate()
                .max_by_key(|(_, &v)| v)
                .map(|(i, _)| i)
                .unwrap()
        };
        let channels: Vec<usize> = palette.iter().map(dominant_channel).collect();
        assert!(
            channels.contains(&0),
            "erwartet einen rot-dominanten Cluster: {channels:?}"
        );
        assert!(
            channels.contains(&2),
            "erwartet einen blau-dominanten Cluster: {channels:?}"
        );
    }

    #[test]
    fn results_are_sorted_by_percentage_descending() {
        let mut pixels = flat_image(0.8, 0.1, 0.1, 12);
        pixels.extend(flat_image(0.1, 0.1, 0.8, 4));
        let palette = extract_palette(&pixels, 4, 4, 2);
        assert_eq!(palette.len(), 2);
        assert!(palette[0].percentage >= palette[1].percentage);
    }
}
