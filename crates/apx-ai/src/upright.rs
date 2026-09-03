//! Automatische Perspektive/Upright-Kantenerkennung (Phase 13 Schritt 4,
//! siehe `DECISIONS.md` ADR-0040-Nachtrag II und `PLAN.md`) — macht die
//! vier bisherigen No-op-Platzhalter `UprightMode::Auto`/`Level`/
//! `Vertical`/`Full` (siehe `apx_pipeline::stages::lens_corrections`s
//! Moduldoku) real: `imageproc::edges::canny` findet Kanten, `imageproc::
//! hough::detect_lines` findet darin gerade Linien, und aus deren Winkeln
//! wird — genau wie beim bereits bestehenden „Guided"-Modus, nur
//! automatisch statt vom Nutzer markiert — eine Dreh-/Scherungskorrektur
//! berechnet.
//!
//! **Zwei erkannte Effekte, nicht vier unabhängige:**
//! - **Level** (Horizont/Kanten begradigen): der Mittelwert der Winkel
//!   aller nahezu waagerechten erkannten Linien ergibt `rotate_degrees` —
//!   dieselbe Rechnung wie `lens_corrections::guided_rotation_degrees`,
//!   nur mit automatisch gefundenen statt vom Nutzer gezogenen Linien
//!   (siehe [`rotate_degrees_for_horizontal_lines`] für die Herleitung).
//! - **Vertical** (stürzende Linien/Gebäudekanten begradigen): der
//!   Mittelwert der Abweichung aller nahezu senkrechten erkannten Linien
//!   von der wahren Senkrechten ergibt den `horizontal`-Scherungsregler
//!   aus `ManualTransform` (trotz des Namens der Regler, der die
//!   *senkrechte* Kantenkonvergenz korrigiert — siehe
//!   [`horizontal_shear_for_vertical_lines`] für die vollständige
//!   Herleitung aus `lens_corrections.rs`s `undo_manual_transform`).
//! - **Auto**/**Full**: beide Effekte kombiniert. Eine echte Trennung
//!   zwischen "Auto" (Lightrooms moderate Automatik) und "Full" (volle
//!   Vier-Parameter-Homografie) bräuchte eine echte Homografie-Schätzung
//!   aus mehreren, unabhängig konvergierenden Linienscharen — außerhalb
//!   des Umfangs dieses bereits in ADR-0028/-0030 auf ein einziges
//!   Scherungspaar vereinfachten Objektivkorrektur-Modells. `Full`
//!   verhält sich hier deshalb bewusst identisch zu `Auto`, statt eine
//!   nicht durch echte Daten gedeckte zusätzliche Korrektur zu erfinden.
//!
//! **Kein gelerntes Modell** — dieselbe Handschrift wie
//! [`crate::lens_calibration`]: eine echte, deterministische Berechnung
//! aus echten Bilddaten, keine LLM-„Schätzung".

use apx_pipeline::edl::UprightMode;
use image::{GrayImage, Luma};
use imageproc::edges::canny;
use imageproc::hough::{detect_lines, LineDetectionOptions, PolarLine};

/// Untere/obere Hysterese-Schwelle für `canny` — feste Heuristik statt
/// eines adaptiven (z. B. Otsu-basierten) Schwellwerts: einfacher, und
/// für die hier gesuchten kontraststarken geraden Kanten (Horizont,
/// Gebäudeecken, Türrahmen) in der Praxis ausreichend. Siehe
/// `imageproc::edges::canny`s Doku: die größtmögliche Kantenstärke ist
/// `sqrt(5)·2·255 ≈ 1140`; diese Werte liegen bewusst im unteren Drittel,
/// damit auch mittelstarke Kanten in normal belichteten Fotos anschlagen.
const CANNY_LOW_THRESHOLD: f32 = 80.0;
const CANNY_HIGH_THRESHOLD: f32 = 160.0;

/// Mindestanteil der Bilddiagonale an Hough-"Stimmen", damit eine Linie
/// als echter Kandidat zählt — skaliert mit der Bildgröße statt eines
/// festen Pixelwerts, der bei kleinen Analyse-Auflösungen zu viele und
/// bei großen zu wenige Linien fände.
const VOTE_THRESHOLD_DIAGONAL_FRACTION: f32 = 0.12;
/// Unterdrückt benachbarte Hough-Zellen im Umkreis dieses Radius auf die
/// jeweils stärkste — verhindert, dass eine einzelne reale Kante als
/// mehrere fast identische "Linien" gezählt wird.
const HOUGH_SUPPRESSION_RADIUS: u32 = 8;

/// Nahezu waagerecht: `angle_in_degrees` (Hough-Normalenwinkel, `90°` =
/// exakt waagerecht, siehe Moduldoku) innerhalb `90° ± toleranz`.
const HORIZONTAL_TOLERANCE_DEGREES: f32 = 30.0;
/// Nahezu senkrecht: `angle_in_degrees` innerhalb `0°`/`180°` ±
/// Toleranz (die zwei Enden des `0..180`-Wertebereichs sind für eine
/// Linienrichtung dieselbe Ausrichtung, siehe [`vertical_deviation_degrees`]).
const VERTICAL_TOLERANCE_DEGREES: f32 = 30.0;

/// Skaliert eine erkannte Winkelabweichung (Grad) auf den `±100`-
/// Scherungsregler `ManualTransform::horizontal`. Hergeleitet aus
/// `lens_corrections.rs`s `undo_manual_transform`
/// (`sheared_x = rx - (horizontal/100)·SHEAR_STRENGTH·ry`, `SHEAR_STRENGTH
/// = 0.5`): für eine im Quellbild um den kleinen Winkel `θ` (im Bogenmaß)
/// von der Senkrechten abweichende Kante ergibt sich `horizontal = 100·θ
/// /SHEAR_STRENGTH = 200·θ` (Herleitung: eine im *Ziel*bild exakt
/// senkrechte Linie `rx = x0` muss auf die tatsächliche, um `θ` geneigte
/// Quelllinie `sheared_x = x0 + θ·ry` abgebildet werden — Koeffizientenvergleich
/// mit obiger Formel ergibt `-(horizontal/100)·0.5 = θ`).
const DEGREES_TO_HORIZONTAL_SLIDER: f32 = 200.0;

/// Ergebnis der automatischen Erkennung — direkt kompatibel mit
/// `ManualTransform`s gleichnamigen Feldern (die übrigen fünf Felder
/// bleiben unberührt, siehe Aufrufer in `apx-app`s
/// `detect_upright_correction`-Command).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct UprightCorrection {
    pub rotate_degrees: f32,
    pub horizontal: f32,
}

impl UprightCorrection {
    pub const NEUTRAL: Self = Self {
        rotate_degrees: 0.0,
        horizontal: 0.0,
    };
}

/// Richtungswinkel (Grad, `atan2`-Konvention: `0°` = waagerecht nach
/// rechts, wächst im Uhrzeigersinn, da Bildkoordinaten `y` nach unten
/// zählen) einer erkannten Hough-Linie — die Umkehrung der Normalform
/// `x·cos(m°) + y·sin(m°) = r`: die Liniennormale zeigt in Richtung `m`,
/// die Linie selbst also senkrecht dazu, `φ = m − 90°`. Geprüft gegen
/// `imageproc::hough`s eigene Spezialfälle (`m=90` → exakt waagerecht,
/// `φ=0`; `m=0` → exakt senkrecht, `φ=±90`, siehe Moduldoku-Kommentar
/// zu `intersection_points`).
fn line_direction_degrees(line: &PolarLine) -> f32 {
    line.angle_in_degrees as f32 - 90.0
}

/// Mittelt die Richtungswinkel aller Linien, deren Hough-Normalenwinkel
/// nahe `90°` liegt (nahezu waagerechte Linien) — `None`, wenn keine
/// gefunden wurde.
fn mean_horizontal_direction_degrees(lines: &[PolarLine]) -> Option<f32> {
    let matching: Vec<f32> = lines
        .iter()
        .filter(|l| (l.angle_in_degrees as f32 - 90.0).abs() <= HORIZONTAL_TOLERANCE_DEGREES)
        .map(line_direction_degrees)
        .collect();
    if matching.is_empty() {
        return None;
    }
    Some(matching.iter().sum::<f32>() / matching.len() as f32)
}

/// Abweichung einer Linie von der wahren Senkrechten, in Grad — anders
/// als [`line_direction_degrees`] auf den Bereich `±90°` verschoben statt
/// `0..180°`, damit eine fast senkrechte Linie mit Normalenwinkel nahe
/// `0°` (leicht nach links geneigt) und eine mit Normalenwinkel nahe
/// `180°` (dieselbe Neigung, andere Zählrichtung derselben Gerade) auf
/// denselben kleinen Wert abbilden statt an der `0°`/`180°`-Kante
/// auseinanderzureißen (unverzichtbar für eine sinnvolle Mittelung).
fn vertical_deviation_degrees(line: &PolarLine) -> f32 {
    let m = line.angle_in_degrees as f32;
    if m > 90.0 {
        m - 180.0
    } else {
        m
    }
}

/// Mittelt die Senkrecht-Abweichungen aller nahezu senkrechten Linien
/// (Hough-Normalenwinkel nahe `0°`/`180°`) — `None`, wenn keine gefunden
/// wurde.
fn mean_vertical_deviation_degrees(lines: &[PolarLine]) -> Option<f32> {
    let matching: Vec<f32> = lines
        .iter()
        .filter(|l| vertical_deviation_degrees(l).abs() <= VERTICAL_TOLERANCE_DEGREES)
        .map(vertical_deviation_degrees)
        .collect();
    if matching.is_empty() {
        return None;
    }
    Some(matching.iter().sum::<f32>() / matching.len() as f32)
}

/// Dreh-Korrektur (Grad) aus den erkannten nahezu waagerechten Linien —
/// dieselbe Vorzeichen-Konvention wie `lens_corrections::
/// guided_rotation_degrees` (`-Kippwinkel`), siehe Modul-Doku.
fn rotate_degrees_for_horizontal_lines(lines: &[PolarLine]) -> f32 {
    mean_horizontal_direction_degrees(lines).map_or(0.0, |tilt| -tilt)
}

/// `ManualTransform::horizontal`-Wert aus den erkannten nahezu
/// senkrechten Linien — siehe [`DEGREES_TO_HORIZONTAL_SLIDER`]s
/// Herleitung.
fn horizontal_shear_for_vertical_lines(lines: &[PolarLine]) -> f32 {
    mean_vertical_deviation_degrees(lines)
        .map_or(0.0, |dev| dev.to_radians() * DEGREES_TO_HORIZONTAL_SLIDER)
}

/// Findet gerade Kanten in `gray` (Canny + Hough, siehe Moduldoku) und
/// berechnet daraus die zu `mode` passende Korrektur. Liefert
/// [`UprightCorrection::NEUTRAL`] für `Off`/`Guided` (dort gilt der
/// bestehende manuelle bzw. `guided_lines`-Mechanismus) sowie, wenn keine
/// passende Linie gefunden wurde, für die jeweils betroffene Komponente.
pub fn detect(gray: &GrayImage, mode: UprightMode) -> UprightCorrection {
    if mode == UprightMode::Off || mode == UprightMode::Guided {
        return UprightCorrection::NEUTRAL;
    }

    let edges = canny(gray, CANNY_LOW_THRESHOLD, CANNY_HIGH_THRESHOLD);
    let diagonal = ((gray.width() * gray.width() + gray.height() * gray.height()) as f32).sqrt();
    let options = LineDetectionOptions {
        vote_threshold: (diagonal * VOTE_THRESHOLD_DIAGONAL_FRACTION).round() as u32,
        suppression_radius: HOUGH_SUPPRESSION_RADIUS,
    };
    let lines = detect_lines(&edges, options);

    match mode {
        UprightMode::Level => UprightCorrection {
            rotate_degrees: rotate_degrees_for_horizontal_lines(&lines),
            horizontal: 0.0,
        },
        UprightMode::Vertical => UprightCorrection {
            rotate_degrees: 0.0,
            horizontal: horizontal_shear_for_vertical_lines(&lines),
        },
        // Auto/Full: siehe Moduldoku zur bewussten Gleichbehandlung.
        UprightMode::Auto | UprightMode::Full => UprightCorrection {
            rotate_degrees: rotate_degrees_for_horizontal_lines(&lines),
            horizontal: horizontal_shear_for_vertical_lines(&lines),
        },
        UprightMode::Off | UprightMode::Guided => unreachable!("oben bereits behandelt"),
    }
}

/// Wandelt einen linearen `f32`-RGB-Bildpuffer (0.0..=1.0, wie ihn
/// `apx_raw::decode_linear`/`apx-app`s `TileCache` liefern) in ein
/// `GrayImage` und ruft [`detect`] darauf auf — dünner Adapter, den
/// `apx-app`s Tauri-Command direkt verwendet, ohne die Pixel-Konvertierung
/// selbst duplizieren zu müssen. Rec.-709-Luma-Gewichte, dieselbe
/// Konvention wie `apx_stacking::luma`.
pub fn detect_from_linear_rgb(
    pixels: &[f32],
    width: u32,
    height: u32,
    mode: UprightMode,
) -> UprightCorrection {
    let gray = GrayImage::from_fn(width, height, |x, y| {
        let idx = (y as usize * width as usize + x as usize) * 3;
        let luma = 0.2126 * pixels[idx] + 0.7152 * pixels[idx + 1] + 0.0722 * pixels[idx + 2];
        Luma([(luma.clamp(0.0, 1.0) * 255.0).round() as u8])
    });
    detect(&gray, mode)
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{GrayImage, Luma};
    use imageproc::drawing::draw_line_segment_mut;

    fn blank(width: u32, height: u32) -> GrayImage {
        GrayImage::from_pixel(width, height, Luma([20u8]))
    }

    /// Zeichnet eine mehrere Pixel breite Linie (Canny braucht eine
    /// gewisse Kantenausdehnung, um nach dem Gauß-Weichzeichner noch
    /// zuverlässig anzuschlagen) hohen Kontrasts.
    fn draw_thick_line(image: &mut GrayImage, start: (f32, f32), end: (f32, f32)) {
        for offset in -1..=1 {
            let o = offset as f32;
            draw_line_segment_mut(
                image,
                (start.0 + o, start.1),
                (end.0 + o, end.1),
                Luma([250u8]),
            );
            draw_line_segment_mut(
                image,
                (start.0, start.1 + o),
                (end.0, end.1 + o),
                Luma([250u8]),
            );
        }
    }

    #[test]
    fn off_and_guided_are_untouched_no_ops() {
        let image = blank(64, 64);
        assert_eq!(detect(&image, UprightMode::Off), UprightCorrection::NEUTRAL);
        assert_eq!(
            detect(&image, UprightMode::Guided),
            UprightCorrection::NEUTRAL
        );
    }

    #[test]
    fn a_blank_image_has_no_detectable_lines_and_falls_back_to_neutral() {
        let image = blank(128, 128);
        assert_eq!(
            detect(&image, UprightMode::Auto),
            UprightCorrection::NEUTRAL
        );
    }

    #[test]
    fn level_detects_a_tilted_horizon_and_computes_a_leveling_rotation() {
        let mut image = blank(200, 200);
        // Ein "Horizont", der von links nach rechts leicht nach unten
        // kippt (10° Neigung) — quer über das ganze Bild, wie ein echter
        // Horizont es tun würde.
        let tilt_degrees = 10.0f32;
        let dy = 100.0 * tilt_degrees.to_radians().tan();
        draw_thick_line(&mut image, (10.0, 100.0 - dy), (190.0, 100.0 + dy));

        let result = detect(&image, UprightMode::Level);
        // Erwartete Korrektur: -10° (siehe rotate_degrees_for_horizontal_lines).
        // Hough rastert Winkel nur in 1°-Schritten, daher Toleranz.
        assert!(
            (result.rotate_degrees - (-tilt_degrees)).abs() < 2.0,
            "erwartet ≈ -10°, war {}",
            result.rotate_degrees
        );
        assert_eq!(result.horizontal, 0.0);
    }

    #[test]
    fn vertical_detects_converging_building_edges_and_computes_a_shear() {
        let mut image = blank(200, 200);
        // Zwei "Gebäudekanten", die beide um 8° von der Senkrechten
        // geneigt sind (stürzende Linien, wie bei einer von unten
        // fotografierten Fassade).
        let tilt_degrees = 8.0f32;
        let dx = 180.0 * tilt_degrees.to_radians().tan();
        draw_thick_line(
            &mut image,
            (40.0 - dx / 2.0, 10.0),
            (40.0 + dx / 2.0, 190.0),
        );
        draw_thick_line(
            &mut image,
            (160.0 - dx / 2.0, 10.0),
            (160.0 + dx / 2.0, 190.0),
        );

        let result = detect(&image, UprightMode::Vertical);
        assert_eq!(result.rotate_degrees, 0.0);
        assert!(
            result.horizontal.abs() > 1.0,
            "erwartet spürbare Scherungskorrektur, war {}",
            result.horizontal
        );

        // Vorzeichen-/Größenprobe, direkt aus `undo_manual_transform`s
        // Formel nachgerechnet: eine im *Ausgabebild* exakt senkrechte
        // Ziellinie bei konstantem `rx0` muss auf genau die tatsächlich
        // gezeichnete geneigte Quelllinie abgebildet werden — nicht nur
        // "irgendeine Verkleinerung", sondern (bis auf Houghs 1°-Rasterung
        // und die Kantendicke beim Zeichnen) die tatsächliche Neigung.
        let shear = (result.horizontal / 100.0) * 0.5; // SHEAR_STRENGTH aus lens_corrections.rs
        let rx0 = 40.0 / 200.0 * 2.0 - 1.0; // konstant — dieselbe Ziel-x für beide y
        let ry_top = 10.0 / 200.0 * 2.0 - 1.0;
        let ry_bottom = 190.0 / 200.0 * 2.0 - 1.0;
        let sheared_span = (rx0 - shear * ry_bottom) - (rx0 - shear * ry_top);
        let actual_span =
            ((40.0 + dx / 2.0) / 200.0 * 2.0 - 1.0) - ((40.0 - dx / 2.0) / 200.0 * 2.0 - 1.0);
        assert!(
            (sheared_span - actual_span).abs() < actual_span.abs() * 0.5,
            "Scherung sollte die tatsächliche Neigung (Spanne {actual_span}) \
             ungefähr reproduzieren, ergab stattdessen {sheared_span}"
        );
    }

    #[test]
    fn auto_and_full_combine_both_effects_identically() {
        let mut image = blank(200, 200);
        let tilt_degrees = 10.0f32;
        let dy = 100.0 * tilt_degrees.to_radians().tan();
        draw_thick_line(&mut image, (10.0, 100.0 - dy), (190.0, 100.0 + dy));
        let dx = 180.0 * 8f32.to_radians().tan();
        draw_thick_line(
            &mut image,
            (40.0 - dx / 2.0, 10.0),
            (40.0 + dx / 2.0, 190.0),
        );
        draw_thick_line(
            &mut image,
            (160.0 - dx / 2.0, 10.0),
            (160.0 + dx / 2.0, 190.0),
        );

        let auto = detect(&image, UprightMode::Auto);
        let full = detect(&image, UprightMode::Full);
        assert_eq!(auto, full);
        assert!(auto.rotate_degrees != 0.0 && auto.horizontal != 0.0);
    }
}
