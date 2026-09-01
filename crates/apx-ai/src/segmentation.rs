//! Die fünf KI-Masken (`SPEC.md` §3.3: Motiv, Himmel, Hintergrund,
//! Objekte, Personen) — **als echte, deterministische, klassische
//! Bildverarbeitungsheuristiken statt echter tiefer neuronaler Netze**,
//! siehe `DECISIONS.md` ADR-0033 Punkt 1/2 für die ausführliche
//! Begründung (kein legitimer Weg, echte Segmentierungs-Modellgewichte in
//! dieser Umgebung zu beschaffen und mitzuliefern; eine ungetestete
//! „Bring-your-own-Model"-Hülle wäre eine vorgetäuschte statt echten
//! Fähigkeit).
//!
//! Jede Funktion hier ist ein real existierendes, vor der Verbreitung
//! tiefer Netze in echten Foto-Editoren eingesetztes Verfahren:
//! - [`subject_alpha`]: Center-Surround-Saliency (Kontrast eines Pixels
//!   gegen seine weit weichgezeichnete Umgebung, zusätzlich
//!   sättigungsgewichtet) — das klassische Saliency-Grundprinzip.
//! - [`sky_alpha`]: Farbton-/Helligkeits-/Positions-Heuristik (bläulich,
//!   hell, geringer lokaler Kontrast, obere Bildhälfte bevorzugt).
//! - [`background_alpha`]: Komplement von [`subject_alpha`].
//! - [`click_region_alpha`]: Region-Growing (Flood-Fill) ab einem
//!   Klickpunkt, farbtoleranzbasiert.
//! - [`person_alpha`]: Hautton-Erkennung im YCbCr-Farbraum.
//!
//! **Gemeinsame Konventionen:** alle Funktionen arbeiten auf einem
//! interleaved linearen RGB-`f32`-Puffer (`3 * width * height`, wie
//! `apx_raw::LinearImage::pixels`) und geben eine Ein-Kanal-`u8`-Bitmap
//! derselben Auflösung zurück (`0` = außerhalb, `255` = voll erfasst) —
//! genau das Format, das `MaskGeometry::AiGenerated` speichert. Der
//! Aufrufer skaliert das Eingabebild vorher auf die Analyse-Auflösung
//! herunter (siehe [`ANALYSIS_MAX_EDGE`] und
//! `apx_core::raster::fit_within`).

use apx_pipeline::edl::AiMaskKind;

use crate::blur::approximate_gaussian_blur;
use crate::color::{luminance, rgb_to_ycbcr, saturation};
use crate::error::{AiError, Result};

/// Lange Kante der Analyse-Auflösung (siehe `DECISIONS.md` ADR-0033
/// Punkt 3) — die erzeugte Alpha-Bitmap wird beim Rendern bilinear auf
/// die tatsächliche Zielauflösung hochskaliert.
pub const ANALYSIS_MAX_EDGE: u32 = 512;

/// Radius der „Surround"-Weichzeichnung in [`subject_alpha`], als Anteil
/// der langen Bildkante — ein fester Pixelradius würde je nach
/// Analyse-Auflösung unterschiedlich stark wirken.
const SURROUND_RADIUS_FRACTION: f32 = 0.06;

fn analysis_radius(width: u32, height: u32) -> u32 {
    ((width.max(height) as f32 * SURROUND_RADIUS_FRACTION).round() as u32).max(1)
}

fn check_dimensions(pixels: &[f32], width: u32, height: u32) -> Result<()> {
    if width == 0 || height == 0 {
        return Err(AiError::Analysis {
            message: format!("Bild ist {width}×{height} — Analyse braucht mindestens 1×1"),
        });
    }
    let expected = (width as usize) * (height as usize) * 3;
    if pixels.len() != expected {
        return Err(AiError::Analysis {
            message: format!(
                "Pufferlänge {} passt nicht zu {width}×{height} (erwartet {expected})",
                pixels.len()
            ),
        });
    }
    Ok(())
}

/// Normiert ein `f32`-Raster auf seinen eigenen Wertebereich (Min→0,
/// Max→255) und gibt es als `u8`-Bitmap zurück. Ein konstantes Raster
/// (Min == Max) ergibt überall `0` — eine Maske ohne erkennbare Struktur
/// soll nichts auswählen, statt willkürlich alles.
fn normalize_to_u8(values: &[f32]) -> Vec<u8> {
    let mut min = f32::INFINITY;
    let mut max = f32::NEG_INFINITY;
    for &v in values {
        if v < min {
            min = v;
        }
        if v > max {
            max = v;
        }
    }
    let span = max - min;
    if !span.is_finite() || span <= 1e-6 {
        return vec![0u8; values.len()];
    }
    values
        .iter()
        .map(|&v| (((v - min) / span) * 255.0).round().clamp(0.0, 255.0) as u8)
        .collect()
}

/// **Motiv-Maske** — Center-Surround-Saliency: je stärker ein Pixel in
/// Helligkeit *und* Farbigkeit von seiner weit weichgezeichneten Umgebung
/// abweicht, desto eher gehört er zum Bildmotiv. Zusätzlich mit der
/// eigenen Sättigung gewichtet (kräftige Farben ziehen den Blick, ein
/// gleichmäßig grauer Bereich nicht) und zum Schluss selbst leicht
/// weichgezeichnet, damit die Maske keine Einzelpixel-Sprenkel enthält.
pub fn subject_alpha(pixels: &[f32], width: u32, height: u32) -> Result<Vec<u8>> {
    check_dimensions(pixels, width, height)?;
    let count = (width as usize) * (height as usize);

    let mut luma = Vec::with_capacity(count);
    let mut sat = Vec::with_capacity(count);
    for i in 0..count {
        let (r, g, b) = (pixels[i * 3], pixels[i * 3 + 1], pixels[i * 3 + 2]);
        luma.push(luminance(r, g, b));
        sat.push(saturation(r, g, b));
    }

    let radius = analysis_radius(width, height);
    let luma_surround = approximate_gaussian_blur(&luma, width, height, radius);
    let sat_surround = approximate_gaussian_blur(&sat, width, height, radius);

    let saliency: Vec<f32> = (0..count)
        .map(|i| {
            let luma_contrast = (luma[i] - luma_surround[i]).abs();
            let sat_contrast = (sat[i] - sat_surround[i]).abs();
            // Eigene Sättigung als zusätzlicher Faktor (+0.25 Grundwert,
            // damit ein sehr kontrastreicher grauer Bereich — z. B. eine
            // Silhouette — nicht komplett verworfen wird).
            (luma_contrast + sat_contrast) * (0.25 + sat[i])
        })
        .collect();

    let smoothed = approximate_gaussian_blur(&saliency, width, height, (radius / 2).max(1));
    Ok(normalize_to_u8(&smoothed))
}

/// **Himmel-Maske** — kombiniert vier klassische Indizien: bläulich
/// (Blaukanal über Rot-/Grünkanal), hell, geringer lokaler Kontrast
/// (Himmel ist glatt, Laub/Architektur nicht) und weiter oben im Bild.
/// Jedes Indiz ist ein Faktor in `0.0..=1.0`, das Produkt ist die
/// Zugehörigkeit.
pub fn sky_alpha(pixels: &[f32], width: u32, height: u32) -> Result<Vec<u8>> {
    check_dimensions(pixels, width, height)?;
    let count = (width as usize) * (height as usize);

    let luma: Vec<f32> = (0..count)
        .map(|i| luminance(pixels[i * 3], pixels[i * 3 + 1], pixels[i * 3 + 2]))
        .collect();
    let radius = analysis_radius(width, height);
    let luma_smooth = approximate_gaussian_blur(&luma, width, height, radius);

    let scores: Vec<f32> = (0..count)
        .map(|i| {
            let (r, g, b) = (pixels[i * 3], pixels[i * 3 + 1], pixels[i * 3 + 2]);
            // Blau-Dominanz gegenüber dem Mittel aus Rot und Grün.
            let blueness = (b - (r + g) * 0.5).max(0.0);
            let brightness = luma[i].clamp(0.0, 1.0);
            // Glattheit: je geringer der Unterschied zum weichgezeichneten
            // Bild, desto himmelstypischer.
            let flatness = 1.0 - ((luma[i] - luma_smooth[i]).abs() * 8.0).clamp(0.0, 1.0);
            // Vertikale Position: oben 1.0, unten 0.0 (linear).
            let row = i / (width as usize);
            let vertical = 1.0 - (row as f32 / (height as f32 - 1.0).max(1.0));

            // Blau-Dominanz und Helligkeit sind die harten Kriterien, die
            // beiden anderen nur abschwächende Gewichte (0.3-Grundwert),
            // damit ein heller blauer Himmel auch im unteren Bildteil oder
            // mit leichter Wolkenstruktur erkannt wird.
            (blueness * 4.0).clamp(0.0, 1.0)
                * brightness
                * (0.3 + 0.7 * flatness)
                * (0.3 + 0.7 * vertical)
        })
        .collect();

    let smoothed = approximate_gaussian_blur(&scores, width, height, (radius / 2).max(1));
    // Absolute (nicht selbstnormierte) Skalierung: anders als bei der
    // Saliency ist „nirgends Himmel" ein sinnvolles, häufiges Ergebnis —
    // eine Selbstnormierung würde in einem Bild ganz ohne Himmel den
    // bläulichsten Fleck künstlich auf 255 hochziehen.
    Ok(smoothed
        .iter()
        .map(|&v| (v * 255.0).round().clamp(0.0, 255.0) as u8)
        .collect())
}

/// **Hintergrund-Maske** — das Komplement der Motiv-Maske (siehe
/// `DECISIONS.md` ADR-0033 Punkt 2: „kein eigener Algorithmus nötig").
pub fn background_alpha(pixels: &[f32], width: u32, height: u32) -> Result<Vec<u8>> {
    let subject = subject_alpha(pixels, width, height)?;
    Ok(subject.into_iter().map(|v| 255 - v).collect())
}

/// **Objekte-Maske (Klick-Segmentierung)** — Region-Growing ab
/// `(click_x, click_y)` (normierte Bildkoordinaten `0.0..=1.0`, dieselbe
/// Konvention wie die übrige Maskengeometrie): benachbarte Pixel werden
/// aufgenommen, solange ihr Farbabstand zur Saatfarbe unter `tolerance`
/// liegt. `feather_radius` weicht die Kante danach auf.
///
/// Teilt die Toleranz-/Weichzeichnungs-Grundidee mit der bestehenden
/// Farbbereich-Maske aus Phase 6 (`apx_pipeline::stages::masks::
/// color_range_alpha`), wächst aber von einem *Saatpunkt* aus statt jeden
/// Pixel global gegen einen Zielfarbwert zu vergleichen — dadurch werden
/// gleichfarbige, aber räumlich getrennte Bereiche nicht miterfasst.
pub fn click_region_alpha(
    pixels: &[f32],
    width: u32,
    height: u32,
    click_x: f32,
    click_y: f32,
    tolerance: f32,
) -> Result<Vec<u8>> {
    check_dimensions(pixels, width, height)?;
    let w = width as usize;
    let h = height as usize;

    let seed_x = ((click_x.clamp(0.0, 1.0) * width as f32) as usize).min(w - 1);
    let seed_y = ((click_y.clamp(0.0, 1.0) * height as f32) as usize).min(h - 1);
    let seed_index = seed_y * w + seed_x;
    let seed = [
        pixels[seed_index * 3],
        pixels[seed_index * 3 + 1],
        pixels[seed_index * 3 + 2],
    ];

    let tolerance = tolerance.max(1e-4);
    let mut visited = vec![false; w * h];
    let mut alpha = vec![0.0f32; w * h];
    let mut stack = vec![seed_index];
    visited[seed_index] = true;

    while let Some(index) = stack.pop() {
        let dr = pixels[index * 3] - seed[0];
        let dg = pixels[index * 3 + 1] - seed[1];
        let db = pixels[index * 3 + 2] - seed[2];
        let distance = (dr * dr + dg * dg + db * db).sqrt();
        if distance > tolerance {
            continue;
        }
        // Innerhalb der Toleranz: voll erfasst, zum Rand hin abfallend.
        alpha[index] = 1.0 - (distance / tolerance).clamp(0.0, 1.0) * 0.35;

        let x = index % w;
        let y = index / w;
        let push = |nx: usize, ny: usize, stack: &mut Vec<usize>, visited: &mut Vec<bool>| {
            let n = ny * w + nx;
            if !visited[n] {
                visited[n] = true;
                stack.push(n);
            }
        };
        if x > 0 {
            push(x - 1, y, &mut stack, &mut visited);
        }
        if x + 1 < w {
            push(x + 1, y, &mut stack, &mut visited);
        }
        if y > 0 {
            push(x, y - 1, &mut stack, &mut visited);
        }
        if y + 1 < h {
            push(x, y + 1, &mut stack, &mut visited);
        }
    }

    let feather_radius = (analysis_radius(width, height) / 3).max(1);
    let smoothed = approximate_gaussian_blur(&alpha, width, height, feather_radius);
    Ok(smoothed
        .iter()
        .map(|&v| (v * 255.0).round().clamp(0.0, 255.0) as u8)
        .collect())
}

/// **Personen-Maske** — Hautton-Erkennung im YCbCr-Farbraum, ein reales,
/// weit verbreitetes klassisches Verfahren: menschliche Haut fällt
/// unabhängig von der Hautfarbe in ein enges Chrominanz-Fenster
/// (`Cb` leicht negativ, `Cr` positiv), während die Luminanz stark
/// variiert.
///
/// **Bewusste Einschränkung** (`DECISIONS.md` ADR-0033 Punkt 2): liefert
/// **eine einzelne zusammenhängende Hautregion**, nicht die in `SPEC.md`
/// §3.3 genannten Einzelteile (Augen, Brauen, Lippen, Zähne, Haare,
/// Kleidung) — die brauchen echte Gesichts-/Körper-Landmark-Erkennung und
/// damit ein trainiertes Modell.
pub fn person_alpha(pixels: &[f32], width: u32, height: u32) -> Result<Vec<u8>> {
    check_dimensions(pixels, width, height)?;
    let count = (width as usize) * (height as usize);

    // Chrominanz-Fenster für Hauttöne (BT.601, Vollbereich, auf
    // `-0.5..=0.5` normiert). Entspricht dem in der Literatur üblichen
    // 8-Bit-Bereich Cb ∈ [77, 127], Cr ∈ [133, 173] relativ zu 128.
    const CB_MIN: f32 = -0.20;
    const CB_MAX: f32 = -0.004;
    const CR_MIN: f32 = 0.02;
    const CR_MAX: f32 = 0.18;

    let scores: Vec<f32> = (0..count)
        .map(|i| {
            let (r, g, b) = (pixels[i * 3], pixels[i * 3 + 1], pixels[i * 3 + 2]);
            let (y, cb, cr) = rgb_to_ycbcr(r, g, b);
            // Sehr dunkle oder ausgebrannte Pixel tragen keine
            // verlässliche Chrominanz-Information.
            if !(0.05..=0.98).contains(&y) {
                return 0.0;
            }
            let in_cb = (CB_MIN..=CB_MAX).contains(&cb);
            let in_cr = (CR_MIN..=CR_MAX).contains(&cr);
            if in_cb && in_cr {
                // Abstand zur Fenstermitte als weiche Gewichtung, damit
                // die Maskenkante nicht binär hart ausfällt.
                let cb_center = (CB_MIN + CB_MAX) * 0.5;
                let cr_center = (CR_MIN + CR_MAX) * 0.5;
                let cb_norm = ((cb - cb_center) / ((CB_MAX - CB_MIN) * 0.5)).abs();
                let cr_norm = ((cr - cr_center) / ((CR_MAX - CR_MIN) * 0.5)).abs();
                (1.0 - (cb_norm.max(cr_norm)) * 0.5).clamp(0.0, 1.0)
            } else {
                0.0
            }
        })
        .collect();

    let radius = (analysis_radius(width, height) / 3).max(1);
    let smoothed = approximate_gaussian_blur(&scores, width, height, radius);
    Ok(smoothed
        .iter()
        .map(|&v| (v * 255.0).round().clamp(0.0, 255.0) as u8)
        .collect())
}

/// Ruft die zu `kind` gehörende Heuristik auf. `click` (normierte
/// Bildkoordinaten) wird nur von [`AiMaskKind::ClickRegion`] gebraucht —
/// fehlt es dort, ist das ein Aufruferfehler und wird als solcher
/// gemeldet statt still auf die Bildmitte auszuweichen.
pub fn generate(
    kind: AiMaskKind,
    pixels: &[f32],
    width: u32,
    height: u32,
    click: Option<(f32, f32)>,
    tolerance: f32,
) -> Result<Vec<u8>> {
    match kind {
        AiMaskKind::Subject => subject_alpha(pixels, width, height),
        AiMaskKind::Sky => sky_alpha(pixels, width, height),
        AiMaskKind::Background => background_alpha(pixels, width, height),
        AiMaskKind::Person => person_alpha(pixels, width, height),
        AiMaskKind::ClickRegion => {
            let (x, y) = click.ok_or_else(|| AiError::Analysis {
                message: "Objekte-Maske braucht einen Klickpunkt".to_string(),
            })?;
            click_region_alpha(pixels, width, height, x, y, tolerance)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Bild mit einem farbigen Rechteck auf gleichmäßigem Grund — das
    /// Rechteck ist das „Motiv".
    fn image_with_patch(
        width: u32,
        height: u32,
        background: [f32; 3],
        patch: [f32; 3],
        patch_rect: (u32, u32, u32, u32),
    ) -> Vec<f32> {
        let (px, py, pw, ph) = patch_rect;
        let mut pixels = Vec::with_capacity((width * height * 3) as usize);
        for y in 0..height {
            for x in 0..width {
                let inside = x >= px && x < px + pw && y >= py && y < py + ph;
                let c = if inside { patch } else { background };
                pixels.extend_from_slice(&c);
            }
        }
        pixels
    }

    fn at(alpha: &[u8], width: u32, x: u32, y: u32) -> u8 {
        alpha[(y * width + x) as usize]
    }

    #[test]
    fn rejects_mismatched_buffer_length() {
        let err = subject_alpha(&[0.0; 5], 4, 4).unwrap_err();
        assert!(matches!(err, AiError::Analysis { .. }));
    }

    #[test]
    fn rejects_zero_sized_image() {
        assert!(subject_alpha(&[], 0, 0).is_err());
    }

    #[test]
    fn subject_mask_is_stronger_on_a_saturated_patch_than_on_flat_background() {
        let pixels = image_with_patch(32, 32, [0.4, 0.4, 0.4], [0.9, 0.1, 0.1], (12, 12, 8, 8));
        let alpha = subject_alpha(&pixels, 32, 32).expect("Analyse");
        assert_eq!(alpha.len(), 32 * 32);
        // Der Rand des Rechtecks ist der kontrastreichste Bereich.
        assert!(at(&alpha, 32, 12, 16) > at(&alpha, 32, 2, 2));
    }

    #[test]
    fn subject_mask_of_a_completely_flat_image_selects_nothing() {
        let pixels = vec![0.5f32; 16 * 16 * 3];
        let alpha = subject_alpha(&pixels, 16, 16).expect("Analyse");
        assert!(alpha.iter().all(|&v| v == 0));
    }

    #[test]
    fn background_mask_is_the_complement_of_the_subject_mask() {
        let pixels = image_with_patch(24, 24, [0.3, 0.3, 0.3], [0.9, 0.2, 0.2], (8, 8, 8, 8));
        let subject = subject_alpha(&pixels, 24, 24).expect("Analyse");
        let background = background_alpha(&pixels, 24, 24).expect("Analyse");
        for (s, b) in subject.iter().zip(background.iter()) {
            assert_eq!(*s as u16 + *b as u16, 255);
        }
    }

    #[test]
    fn sky_mask_is_high_on_bright_blue_top_and_low_on_dark_green_bottom() {
        // Obere Hälfte heller Himmel, untere Hälfte dunkles Grün.
        let width = 16u32;
        let height = 16u32;
        let mut pixels = Vec::new();
        for y in 0..height {
            for _ in 0..width {
                if y < height / 2 {
                    pixels.extend_from_slice(&[0.45, 0.6, 0.95]);
                } else {
                    pixels.extend_from_slice(&[0.1, 0.25, 0.08]);
                }
            }
        }
        let alpha = sky_alpha(&pixels, width, height).expect("Analyse");
        assert!(
            at(&alpha, width, 8, 2) > 100,
            "Himmel oben muss erkannt werden"
        );
        assert!(
            at(&alpha, width, 8, 14) < 30,
            "Laub unten darf nicht erkannt werden"
        );
    }

    #[test]
    fn sky_mask_of_an_image_without_sky_stays_near_zero() {
        // Gleichmäßiges Rot — nirgends Himmel. Eine Selbstnormierung
        // würde hier fälschlich etwas auf 255 hochziehen (siehe die
        // Begründung in `sky_alpha`).
        let pixels = [0.8f32, 0.2, 0.2].repeat(16 * 16);
        let alpha = sky_alpha(&pixels, 16, 16).expect("Analyse");
        assert!(alpha.iter().all(|&v| v < 20));
    }

    #[test]
    fn click_region_grows_into_the_clicked_patch_and_stops_at_its_edge() {
        let pixels = image_with_patch(32, 32, [0.1, 0.1, 0.1], [0.9, 0.9, 0.9], (8, 8, 16, 16));
        // Klick mitten ins helle Rechteck (normierte Koordinaten).
        let alpha = click_region_alpha(&pixels, 32, 32, 0.5, 0.5, 0.2).expect("Analyse");
        assert!(at(&alpha, 32, 16, 16) > 200, "Klickpunkt muss erfasst sein");
        assert!(
            at(&alpha, 32, 2, 2) < 30,
            "Dunkler Grund darf nicht erfasst sein"
        );
    }

    #[test]
    fn click_region_does_not_leak_into_a_disconnected_patch_of_the_same_color() {
        // Zwei gleichfarbige, räumlich getrennte Rechtecke: Region-Growing
        // darf nur das angeklickte erfassen (Unterschied zur globalen
        // Farbbereich-Maske aus Phase 6).
        let width = 40u32;
        let height = 16u32;
        let mut pixels = Vec::new();
        for y in 0..height {
            for x in 0..width {
                let in_left = (4..12).contains(&x) && (4..12).contains(&y);
                let in_right = (28..36).contains(&x) && (4..12).contains(&y);
                if in_left || in_right {
                    pixels.extend_from_slice(&[0.9, 0.9, 0.9]);
                } else {
                    pixels.extend_from_slice(&[0.1, 0.1, 0.1]);
                }
            }
        }
        // Klick ins linke Rechteck.
        let alpha = click_region_alpha(&pixels, width, height, 8.0 / 40.0, 0.5, 0.2).expect("A");
        assert!(at(&alpha, width, 8, 8) > 200, "linkes Rechteck erfasst");
        assert!(
            at(&alpha, width, 32, 8) < 30,
            "rechtes Rechteck NICHT erfasst"
        );
    }

    #[test]
    fn person_mask_responds_to_skin_tone_and_ignores_blue() {
        // Linke Hälfte typischer Hautton, rechte Hälfte kräftiges Blau.
        let width = 16u32;
        let height = 8u32;
        let mut pixels = Vec::new();
        for _ in 0..height {
            for x in 0..width {
                if x < width / 2 {
                    pixels.extend_from_slice(&[0.75, 0.55, 0.45]);
                } else {
                    pixels.extend_from_slice(&[0.1, 0.2, 0.8]);
                }
            }
        }
        let alpha = person_alpha(&pixels, width, height).expect("Analyse");
        assert!(at(&alpha, width, 2, 4) > 100, "Hautton muss erkannt werden");
        assert!(
            at(&alpha, width, 14, 4) < 30,
            "Blau darf nicht erkannt werden"
        );
    }

    #[test]
    fn generate_dispatches_by_kind_and_requires_a_click_point_for_click_region() {
        let pixels = image_with_patch(16, 16, [0.3, 0.3, 0.3], [0.9, 0.2, 0.2], (4, 4, 8, 8));
        for kind in [
            AiMaskKind::Subject,
            AiMaskKind::Sky,
            AiMaskKind::Background,
            AiMaskKind::Person,
        ] {
            let alpha = generate(kind, &pixels, 16, 16, None, 0.2).expect("Analyse");
            assert_eq!(alpha.len(), 16 * 16);
        }
        assert!(generate(AiMaskKind::ClickRegion, &pixels, 16, 16, None, 0.2).is_err());
        assert!(generate(
            AiMaskKind::ClickRegion,
            &pixels,
            16,
            16,
            Some((0.5, 0.5)),
            0.2
        )
        .is_ok());
    }
}
