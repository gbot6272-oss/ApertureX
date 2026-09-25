//! Belichtung eines Fotos an ein Referenzfoto angleichen
//! (Phase 34 F2, siehe `DECISIONS.md` ADR-0070).
//!
//! **Wozu.** In einer Serie aus demselben Licht schwanken die
//! Belichtungen oft um einen halben Blendenwert, weil die Automatik auf
//! unterschiedlich helle Motive reagiert hat. Von Hand gleicht man das
//! Foto für Foto am Regler an — und trifft es nie genau. Hier rechnet es
//! die Maschine aus: wie viele EV muss die Belichtung des Ziels
//! verschoben werden, damit seine mittlere Helligkeit der des
//! Referenzfotos entspricht.
//!
//! **Warum im Logarithmus, und warum linearisiert.** Eine Blendenstufe
//! ist eine Verdopplung der Lichtmenge, also ein konstanter Abstand im
//! `log2` der LINEAREN Helligkeit. Die Vorschaubilder liegen aber
//! gammakodiert vor (sRGB). Ohne Linearisierung wäre die berechnete
//! Differenz keine Blendenstufe, sondern ein motivabhängiger Fantasiewert
//! — hell fotografierte Motive bekämen systematisch zu wenig, dunkle zu
//! viel.
//!
//! **Warum beschnittene Pixel draußen bleiben.** Ein ausgefressener
//! Himmel bleibt bei 255, egal wie stark man abdunkelt; ein
//! abgesoffener Schatten bleibt bei 0. Solche Pixel tragen keine
//! Information über die Belichtung, ziehen den Mittelwert aber kräftig
//! in ihre Richtung. Sie werden deshalb übersprungen.

/// Kanalwerte bis einschließlich dieser Grenze gelten als abgesoffen.
pub const CLIP_LOW: u8 = 4;
/// Kanalwerte ab einschließlich dieser Grenze gelten als ausgefressen.
pub const CLIP_HIGH: u8 = 251;
/// Grenze der vorgeschlagenen Korrektur — derselbe Bereich, den der
/// Belichtungsregler im Entwickeln-Panel hat (`BASIC_SLIDER_SPECS`).
pub const MAX_DELTA_EV: f32 = 5.0;

/// Kleinster Wert, der in den Logarithmus geht. `log2(0)` wäre `-inf`
/// und würde den Mittelwert zerstören.
const EPSILON: f32 = 1.0 / 4096.0;

/// sRGB-Gammakurve rückwärts: kodierter 0..1-Wert → lineares Licht.
fn srgb_to_linear(value: f32) -> f32 {
    if value <= 0.04045 {
        value / 12.92
    } else {
        ((value + 0.055) / 1.055).powf(2.4)
    }
}

/// Mittlerer `log2` der linearen Leuchtdichte über alle verwertbaren
/// Pixel — `None`, wenn keines übrig bleibt (vollständig beschnittenes
/// oder leeres Bild).
///
/// `rgba` ist ein RGBA8-Puffer. Vollständig durchsichtige Pixel zählen
/// nicht mit: sie zeigen nichts, ihr Farbwert ist beliebig.
pub fn mean_log_luminance(rgba: &[u8]) -> Option<f32> {
    let mut sum = 0.0_f64;
    let mut count = 0_u64;
    for px in rgba.chunks_exact(4) {
        if px[3] == 0 {
            continue;
        }
        let clipped = px[..3].iter().any(|&c| c <= CLIP_LOW || c >= CLIP_HIGH);
        if clipped {
            continue;
        }
        let r = srgb_to_linear(f32::from(px[0]) / 255.0);
        let g = srgb_to_linear(f32::from(px[1]) / 255.0);
        let b = srgb_to_linear(f32::from(px[2]) / 255.0);
        // Rec.709-Leuchtdichte, dieselben Gewichte wie in
        // `apx-pipeline`s Tonwert-Stufen.
        let luminance = 0.2126 * r + 0.7152 * g + 0.0722 * b;
        sum += f64::from(luminance.max(EPSILON).log2());
        count += 1;
    }
    if count == 0 {
        return None;
    }
    Some((sum / count as f64) as f32)
}

/// Um wie viele EV muss `target` verschoben werden, damit seine mittlere
/// Helligkeit der von `reference` entspricht?
///
/// Positiv = aufhellen. Das Ergebnis ist auf [`MAX_DELTA_EV`] begrenzt:
/// ein größerer Wert wäre ohnehin nicht einstellbar, und eine Differenz
/// dieser Größenordnung heißt meist, dass die beiden Bilder nichts
/// miteinander zu tun haben.
///
/// `None`, wenn eines der beiden Bilder keine verwertbaren Pixel hat.
pub fn exposure_delta_ev(reference: &[u8], target: &[u8]) -> Option<f32> {
    let reference_mean = mean_log_luminance(reference)?;
    let target_mean = mean_log_luminance(target)?;
    Some((reference_mean - target_mean).clamp(-MAX_DELTA_EV, MAX_DELTA_EV))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Ein einfarbiges RGBA8-Bild mit `n` Pixeln.
    fn solid(value: u8, n: usize) -> Vec<u8> {
        std::iter::repeat_n([value, value, value, 255], n)
            .flatten()
            .collect()
    }

    #[test]
    fn identical_images_need_no_correction() {
        let img = solid(128, 16);
        assert_eq!(exposure_delta_ev(&img, &img), Some(0.0));
    }

    #[test]
    fn a_doubling_of_linear_light_is_exactly_one_ev() {
        // 0.25 und 0.5 lineares Licht liegen genau eine Blendenstufe
        // auseinander. Die gammakodierten Bytes dazu:
        let dark = (0.25_f32.powf(1.0 / 2.4) * 1.055 - 0.055) * 255.0;
        let bright = (0.5_f32.powf(1.0 / 2.4) * 1.055 - 0.055) * 255.0;
        let delta = exposure_delta_ev(
            &solid(bright.round() as u8, 4),
            &solid(dark.round() as u8, 4),
        )
        .expect("beide Bilder sind verwertbar");
        assert!(
            (delta - 1.0).abs() < 0.05,
            "eine Verdopplung sollte ~1 EV ergeben, war {delta}"
        );
    }

    #[test]
    fn brightening_is_positive_and_darkening_negative() {
        let dark = solid(60, 4);
        let bright = solid(200, 4);
        assert!(exposure_delta_ev(&bright, &dark).expect("ok") > 0.0);
        assert!(exposure_delta_ev(&dark, &bright).expect("ok") < 0.0);
    }

    #[test]
    fn the_result_is_antisymmetric() {
        let a = solid(70, 4);
        let b = solid(180, 4);
        let forward = exposure_delta_ev(&a, &b).expect("ok");
        let backward = exposure_delta_ev(&b, &a).expect("ok");
        assert!((forward + backward).abs() < 1e-4);
    }

    #[test]
    fn clipped_pixels_do_not_drag_the_mean() {
        // Dasselbe Motiv, einmal mit einem ausgefressenen Himmel daneben.
        // Wuerden die 255er mitzaehlen, waere die Differenz deutlich von
        // null verschieden.
        let plain = solid(128, 8);
        let mut with_sky = solid(128, 8);
        with_sky.extend(solid(255, 8));
        let delta = exposure_delta_ev(&plain, &with_sky).expect("ok");
        assert!(delta.abs() < 1e-4, "war {delta}");
    }

    #[test]
    fn a_fully_clipped_image_has_no_usable_pixels() {
        assert_eq!(mean_log_luminance(&solid(255, 4)), None);
        assert_eq!(mean_log_luminance(&solid(0, 4)), None);
        assert_eq!(mean_log_luminance(&[]), None);
    }

    #[test]
    fn transparent_pixels_are_ignored() {
        // Ein durchsichtiges schwarzes Pixel darf das Ergebnis nicht
        // verdunkeln — sein Farbwert zeigt nichts an.
        let opaque = solid(128, 4);
        let mut with_hole = solid(128, 4);
        with_hole.extend_from_slice(&[0, 0, 0, 0]);
        assert_eq!(mean_log_luminance(&opaque), mean_log_luminance(&with_hole));
    }

    #[test]
    fn the_correction_stays_within_the_slider_range() {
        let almost_black = solid(CLIP_LOW + 1, 4);
        let almost_white = solid(CLIP_HIGH - 1, 4);
        let delta = exposure_delta_ev(&almost_white, &almost_black).expect("ok");
        assert!(delta <= MAX_DELTA_EV);
        assert!(delta >= -MAX_DELTA_EV);
    }

    #[test]
    fn a_partial_buffer_at_the_end_is_ignored() {
        // `chunks_exact` laesst einen unvollstaendigen Rest liegen —
        // ein abgeschnittener Puffer darf nicht in einen Absturz laufen.
        let mut broken = solid(128, 2);
        broken.extend_from_slice(&[128, 128]);
        assert!(mean_log_luminance(&broken).is_some());
    }
}
