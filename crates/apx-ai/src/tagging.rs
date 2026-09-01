//! Auto-Tagging (Phase 7 Schritt 5, `SPEC.md` §5, `DECISIONS.md`
//! ADR-0033) — regelbasierte Schlagwort-Vorschläge, kein Klassifikations-
//! modell: Flächenanteil der `segmentation`-Heuristiken (Himmel-/
//! Personen-Maske) kombiniert mit EXIF-Kennwerten (ISO, Blende,
//! Brennweite). Der Aufrufer (`apx-app`s `suggest_tags`-Command) reicht
//! die Vorschläge über die bestehende `photo_keywords`-Infrastruktur aus
//! Phase 3 weiter (`add_photo_keyword`) — dieses Modul schlägt nur vor,
//! es schreibt nichts in den Katalog.
//!
//! **Bewusste Vereinfachung:** die Schwellenwerte sind grobe, feste
//! Faustregeln (kein aus echten Fotos gelernter Klassifikator) — dieselbe
//! Art Näherung wie die fünf KI-Masken selbst (siehe `segmentation`s
//! Moduldoku).

use crate::error::Result;
use crate::segmentation::{person_alpha, sky_alpha};

/// EXIF-Kennwerte, die für Tag-Vorschläge herangezogen werden — eine
/// bewusst kleine Teilmenge von `apx_catalog::Photo`, damit dieses Modul
/// nicht von `apx-catalog` abhängen muss.
#[derive(Debug, Clone, Copy, Default)]
pub struct TagExifInput {
    pub iso: Option<u32>,
    pub aperture: Option<f32>,
    pub focal_length: Option<f32>,
}

/// Flächenanteil (`0.0..=1.0`), ab dem eine Segmentierungs-Heuristik als
/// „deutlich vorhanden" gilt — unterhalb dieser Schwelle ist die
/// Heuristik zu unsicher für einen Tag-Vorschlag (vgl. `detect_spots`s
/// `sensitivity`-Schwelle in `repair_analysis`).
const AREA_FRACTION_THRESHOLD: f32 = 0.15;

fn mean_alpha_fraction(alpha: &[u8]) -> f32 {
    if alpha.is_empty() {
        return 0.0;
    }
    let sum: u64 = alpha.iter().map(|&v| v as u64).sum();
    (sum as f32 / alpha.len() as f32) / 255.0
}

/// Schlägt Schlagworte für ein Foto vor — Bildanalyse (Himmel-/
/// Personen-Flächenanteil) plus EXIF-Faustregeln. Reihenfolge ist stabil
/// (Bildanalyse zuerst), Duplikate sind ausgeschlossen.
pub fn suggest_tags(
    pixels: &[f32],
    width: u32,
    height: u32,
    exif: &TagExifInput,
) -> Result<Vec<String>> {
    let mut tags = Vec::new();

    let sky_fraction = mean_alpha_fraction(&sky_alpha(pixels, width, height)?);
    if sky_fraction > AREA_FRACTION_THRESHOLD {
        tags.push("Himmel".to_string());
        if sky_fraction > 0.4 {
            tags.push("Landschaft".to_string());
        }
    }

    let person_fraction = mean_alpha_fraction(&person_alpha(pixels, width, height)?);
    if person_fraction > AREA_FRACTION_THRESHOLD {
        tags.push("Personen".to_string());
        if person_fraction > 0.3 {
            tags.push("Porträt".to_string());
        }
    }

    if let Some(iso) = exif.iso {
        if iso >= 1600 {
            tags.push("Wenig Licht".to_string());
        }
    }
    if let Some(aperture) = exif.aperture {
        if aperture <= 2.8 {
            tags.push("Freistellung".to_string());
        }
    }
    if let Some(focal_length) = exif.focal_length {
        if focal_length >= 100.0 {
            tags.push("Tele".to_string());
        } else if focal_length <= 24.0 {
            tags.push("Weitwinkel".to_string());
        }
    }

    tags.dedup();
    Ok(tags)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flat_gray(width: u32, height: u32, value: f32) -> Vec<f32> {
        vec![value; (width * height * 3) as usize]
    }

    #[test]
    fn bright_blue_upper_half_suggests_sky_and_landscape() {
        let size = 64u32;
        let mut pixels = flat_gray(size, size, 0.3);
        // Obere Hälfte hell und bläulich einfärben.
        for y in 0..(size / 2) {
            for x in 0..size {
                let i = ((y * size + x) * 3) as usize;
                pixels[i] = 0.6; // R
                pixels[i + 1] = 0.75; // G
                pixels[i + 2] = 0.95; // B
            }
        }
        let tags =
            suggest_tags(&pixels, size, size, &TagExifInput::default()).expect("sollte gelingen");
        assert!(tags.contains(&"Himmel".to_string()), "Tags waren: {tags:?}");
    }

    #[test]
    fn high_iso_suggests_low_light() {
        let size = 16u32;
        let pixels = flat_gray(size, size, 0.5);
        let exif = TagExifInput {
            iso: Some(6400),
            aperture: None,
            focal_length: None,
        };
        let tags = suggest_tags(&pixels, size, size, &exif).expect("sollte gelingen");
        assert!(tags.contains(&"Wenig Licht".to_string()));
    }

    #[test]
    fn wide_aperture_suggests_shallow_depth_of_field() {
        let size = 16u32;
        let pixels = flat_gray(size, size, 0.5);
        let exif = TagExifInput {
            iso: None,
            aperture: Some(1.8),
            focal_length: None,
        };
        let tags = suggest_tags(&pixels, size, size, &exif).expect("sollte gelingen");
        assert!(tags.contains(&"Freistellung".to_string()));
    }

    #[test]
    fn long_focal_length_suggests_tele_and_short_suggests_wide_angle() {
        let size = 16u32;
        let pixels = flat_gray(size, size, 0.5);
        let tele = suggest_tags(
            &pixels,
            size,
            size,
            &TagExifInput {
                focal_length: Some(200.0),
                ..Default::default()
            },
        )
        .expect("sollte gelingen");
        assert!(tele.contains(&"Tele".to_string()));
        let wide = suggest_tags(
            &pixels,
            size,
            size,
            &TagExifInput {
                focal_length: Some(16.0),
                ..Default::default()
            },
        )
        .expect("sollte gelingen");
        assert!(wide.contains(&"Weitwinkel".to_string()));
    }

    #[test]
    fn flat_gray_image_without_exif_suggests_nothing() {
        let size = 16u32;
        let pixels = flat_gray(size, size, 0.5);
        let tags =
            suggest_tags(&pixels, size, size, &TagExifInput::default()).expect("sollte gelingen");
        assert!(tags.is_empty(), "Tags waren: {tags:?}");
    }
}
