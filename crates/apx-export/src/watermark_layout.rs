//! Größe, Rand, Drehung und Kachelung eines Wasserzeichens (Phase 33 F5).
//!
//! **Das Problem, das dieses Modul löst.** `watermark.rs` platziert ein
//! Overlay in fester Pixelgröße an einer von fünf Ecken, mit einem Rand
//! in festen Pixeln. Das funktioniert für genau eine Exportgröße. Eine
//! Vorlage mit 24 px Schrift und 20 px Rand ist auf einem
//! 6000-px-Export ein unlesbarer Fliegenschiss und auf einem
//! 800-px-Web-Export ein Balken quer durchs Bild. Genau deshalb ließen
//! sich Wasserzeichen bisher nicht sinnvoll als benannte Vorlage
//! speichern: die Zahlen darin gelten immer nur für eine Ausgabegröße.
//!
//! Die Antwort ist, Größe und Rand in Prozent **der kürzeren Kante**
//! anzugeben, nicht der längeren und nicht der Fläche: nur so ist ein
//! Wasserzeichen auf einem Hoch- und einem Querformat gleich groß und
//! gleich weit vom Rand entfernt.
//!
//! **Kachelung** ist der zweite Teil. Ein Wasserzeichen in einer Ecke
//! lässt sich wegschneiden; ein gedreht über das ganze Bild gekacheltes
//! nicht. Das ist die Form, in der Andrucke normalerweise herausgehen,
//! und sie war bisher gar nicht möglich.

use crate::error::{ExportError, Result};
use crate::watermark::WatermarkPosition;

/// Kleinste Kantenlänge, auf die ein Overlay skaliert wird — unter einem
/// Pixel gibt es nichts mehr zu komponieren.
const MIN_OVERLAY_EDGE: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TileSpec {
    /// Abstand zwischen zwei Kacheln, in Prozent der kürzeren Kante.
    /// `0` heißt: Kachel an Kachel.
    pub spacing_percent: f32,
    /// Drehung jeder Kachel in Grad, gegen den Uhrzeigersinn.
    pub rotation_degrees: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RelativePlacement {
    /// Breite des Wasserzeichens in Prozent der kürzeren Bildkante.
    pub size_percent: f32,
    /// Abstand zum Bildrand in Prozent der kürzeren Bildkante.
    pub margin_percent: f32,
    pub position: WatermarkPosition,
    /// `Some` = über das ganze Bild kacheln; `position`/`margin_percent`
    /// spielen dann keine Rolle mehr.
    pub tile: Option<TileSpec>,
}

impl Default for RelativePlacement {
    fn default() -> Self {
        Self {
            size_percent: 20.0,
            margin_percent: 3.0,
            position: WatermarkPosition::BottomRight,
            tile: None,
        }
    }
}

/// Die Zielgröße eines Overlays auf einem `canvas_w × canvas_h` großen
/// Bild.
///
/// Das Seitenverhältnis bleibt erhalten; die Breite bestimmt die Skalierung,
/// weil ein Wasserzeichen fast immer breiter als hoch ist und man seine
/// Breite im Bild abschätzt, nicht seine Höhe. Auf null skaliert wird nie —
/// ein verschwundenes Wasserzeichen wäre schlimmer als ein winziges,
/// weil man den Fehler nicht sieht.
pub fn scaled_overlay_size(
    canvas_w: u32,
    canvas_h: u32,
    overlay_w: u32,
    overlay_h: u32,
    size_percent: f32,
) -> (u32, u32) {
    if overlay_w == 0 || overlay_h == 0 {
        return (MIN_OVERLAY_EDGE, MIN_OVERLAY_EDGE);
    }
    let short_edge = canvas_w.min(canvas_h) as f32;
    let target_w = (short_edge * size_percent.clamp(0.1, 200.0) / 100.0).round();
    let scale = (target_w / overlay_w as f32).max(f32::MIN_POSITIVE);
    let width = (overlay_w as f32 * scale).round() as u32;
    let height = (overlay_h as f32 * scale).round() as u32;
    (width.max(MIN_OVERLAY_EDGE), height.max(MIN_OVERLAY_EDGE))
}

/// Der Rand in Pixeln.
pub fn margin_px(canvas_w: u32, canvas_h: u32, margin_percent: f32) -> u32 {
    let short_edge = canvas_w.min(canvas_h) as f32;
    (short_edge * margin_percent.clamp(0.0, 45.0) / 100.0).round() as u32
}

/// Die Schriftgröße in Pixeln für ein Text-Wasserzeichen.
///
/// Eigene Funktion statt [`scaled_overlay_size`], weil bei Text nicht die
/// Breite des Ergebnisses vorgegeben wird, sondern die Höhe der Zeile:
/// dieselbe Vorlage soll über einem kurzen und einem langen Namen gleich
/// hoch schreiben, nicht gleich breit.
pub fn font_size_px(canvas_w: u32, canvas_h: u32, size_percent: f32) -> f32 {
    let short_edge = canvas_w.min(canvas_h) as f32;
    (short_edge * size_percent.clamp(0.1, 50.0) / 100.0).max(1.0)
}

/// Die Ursprünge aller Kacheln, die das Bild bedecken.
///
/// Das Muster wird über dem Bild **zentriert**: ein bei (0,0)
/// beginnendes Raster ergäbe oben links eine ganze und unten rechts eine
/// angeschnittene Kachel, was wie ein Fehler aussieht. Zentriert sind die
/// Anschnitte auf beiden Seiten gleich.
///
/// Es wird jeweils eine Kachel über den Rand hinaus erzeugt, damit die
/// Ränder wirklich bedeckt sind — `apply_image_at` schneidet ab, was
/// draußen liegt.
pub fn tile_origins(
    canvas_w: u32,
    canvas_h: u32,
    tile_w: u32,
    tile_h: u32,
    spacing_px: u32,
) -> Vec<(i64, i64)> {
    if tile_w == 0 || tile_h == 0 {
        return Vec::new();
    }
    let step_x = (tile_w + spacing_px) as i64;
    let step_y = (tile_h + spacing_px) as i64;
    let cols = (canvas_w as i64).div_euclid(step_x) + 2;
    let rows = (canvas_h as i64).div_euclid(step_y) + 2;

    let pattern_w = cols * step_x - spacing_px as i64;
    let pattern_h = rows * step_y - spacing_px as i64;
    let start_x = (canvas_w as i64 - pattern_w) / 2;
    let start_y = (canvas_h as i64 - pattern_h) / 2;

    let mut origins = Vec::with_capacity((cols * rows) as usize);
    for row in 0..rows {
        for col in 0..cols {
            origins.push((start_x + col * step_x, start_y + row * step_y));
        }
    }
    origins
}

/// Dreht ein RGBA8-Overlay um seinen Mittelpunkt und liefert es in einem
/// Puffer, der groß genug ist, dass nichts abgeschnitten wird.
///
/// Rückwärts abgetastet (für jeden Zielpixel wird die Quellposition
/// berechnet) und bilinear interpoliert — vorwärts gedreht blieben
/// Löcher zwischen den Pixeln stehen. Außerhalb der Quelle liegende
/// Abtastungen werden vollständig transparent, nicht schwarz: das
/// Ergebnis wird gleich alpha-komponiert, und ein schwarzer Rahmen um
/// jede Kachel wäre das sichtbarste denkbare Artefakt.
pub fn rotate_rgba8(
    width: u32,
    height: u32,
    pixels: &[u8],
    degrees: f32,
) -> Result<(u32, u32, Vec<u8>)> {
    let expected = width as usize * height as usize * 4;
    if pixels.len() != expected {
        return Err(ExportError::Unsupported(format!(
            "Overlay-Pufferlänge {} passt nicht zu {width}x{height} RGBA8",
            pixels.len()
        )));
    }
    if width == 0 || height == 0 {
        return Err(ExportError::Unsupported(
            "Overlay ohne Ausdehnung kann nicht gedreht werden".to_string(),
        ));
    }

    let radians = degrees.to_radians();
    let (sin, cos) = radians.sin_cos();
    if sin.abs() < 1e-6 && cos > 0.0 {
        // Keine Drehung — Puffer unverändert durchreichen, statt ihn
        // durch eine Interpolation zu schicken, die ihn nur weicher
        // machen würde.
        return Ok((width, height, pixels.to_vec()));
    }

    let (w, h) = (width as f32, height as f32);
    // Aufgerundet, damit bei einer schrägen Drehung nichts abgeschnitten
    // wird — aber erst nach Abzug einer kleinen Toleranz: bei 90° ist
    // `cos` in `f32` nicht exakt null, und ohne die Toleranz bekäme ein
    // 4×10-Overlay eine 5 Pixel breite Zielfläche mit einer leeren
    // Spalte darin.
    let ceil_with_tolerance = |value: f32| (value - 1e-3).ceil().max(1.0) as u32;
    let out_w = ceil_with_tolerance(w * cos.abs() + h * sin.abs());
    let out_h = ceil_with_tolerance(w * sin.abs() + h * cos.abs());

    let src_cx = w / 2.0;
    let src_cy = h / 2.0;
    let dst_cx = out_w as f32 / 2.0;
    let dst_cy = out_h as f32 / 2.0;

    let mut out = vec![0u8; out_w as usize * out_h as usize * 4];
    for y in 0..out_h {
        for x in 0..out_w {
            let dx = x as f32 + 0.5 - dst_cx;
            let dy = y as f32 + 0.5 - dst_cy;
            // Rückwärts: die inverse Drehung.
            let sx = dx * cos + dy * sin + src_cx - 0.5;
            let sy = -dx * sin + dy * cos + src_cy - 0.5;
            let sample = sample_bilinear(width, height, pixels, sx, sy);
            let idx = (y as usize * out_w as usize + x as usize) * 4;
            out[idx..idx + 4].copy_from_slice(&sample);
        }
    }
    Ok((out_w, out_h, out))
}

fn sample_bilinear(width: u32, height: u32, pixels: &[u8], x: f32, y: f32) -> [u8; 4] {
    let x0 = x.floor() as i64;
    let y0 = y.floor() as i64;
    let fx = x - x0 as f32;
    let fy = y - y0 as f32;

    let at = |px: i64, py: i64| -> [f32; 4] {
        if px < 0 || py < 0 || px >= width as i64 || py >= height as i64 {
            // Vollständig transparent, siehe Moduldoku.
            return [0.0; 4];
        }
        let idx = (py as usize * width as usize + px as usize) * 4;
        [
            pixels[idx] as f32,
            pixels[idx + 1] as f32,
            pixels[idx + 2] as f32,
            pixels[idx + 3] as f32,
        ]
    };

    let p00 = at(x0, y0);
    let p10 = at(x0 + 1, y0);
    let p01 = at(x0, y0 + 1);
    let p11 = at(x0 + 1, y0 + 1);

    let mut out = [0u8; 4];
    for channel in 0..4 {
        let top = p00[channel] * (1.0 - fx) + p10[channel] * fx;
        let bottom = p01[channel] * (1.0 - fx) + p11[channel] * fx;
        out[channel] = (top * (1.0 - fy) + bottom * fy).round().clamp(0.0, 255.0) as u8;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn die_groesse_haengt_an_der_kuerzeren_kante() {
        // Hoch- und Querformat mit derselben kürzeren Kante ergeben
        // dasselbe Wasserzeichen — genau darum geht es.
        let quer = scaled_overlay_size(4000, 2000, 100, 50, 10.0);
        let hoch = scaled_overlay_size(2000, 4000, 100, 50, 10.0);
        assert_eq!(quer, hoch);
        assert_eq!(quer, (200, 100));
    }

    #[test]
    fn das_seitenverhaeltnis_bleibt_erhalten() {
        let (w, h) = scaled_overlay_size(1000, 1000, 300, 100, 30.0);
        assert_eq!(w, 300);
        assert_eq!(h, 100);
    }

    #[test]
    fn dieselbe_vorlage_ergibt_auf_zwei_exportgroessen_denselben_bildanteil() {
        // Der eigentliche Zweck: eine gespeicherte Vorlage soll auf
        // 800 px und auf 6000 px gleich aussehen.
        let (klein, _) = scaled_overlay_size(800, 600, 200, 50, 25.0);
        let (gross, _) = scaled_overlay_size(8000, 6000, 200, 50, 25.0);
        let anteil_klein = klein as f32 / 600.0;
        let anteil_gross = gross as f32 / 6000.0;
        assert!((anteil_klein - anteil_gross).abs() < 0.01);
    }

    #[test]
    fn ein_wasserzeichen_verschwindet_nie_ganz() {
        let (w, h) = scaled_overlay_size(100, 100, 10, 10, 0.1);
        assert!(w >= 1 && h >= 1);
    }

    #[test]
    fn ein_leeres_overlay_ergibt_keine_division_durch_null() {
        assert_eq!(scaled_overlay_size(1000, 1000, 0, 0, 10.0), (1, 1));
    }

    #[test]
    fn der_rand_haengt_ebenfalls_an_der_kuerzeren_kante() {
        assert_eq!(margin_px(4000, 2000, 5.0), 100);
        assert_eq!(margin_px(2000, 4000, 5.0), 100);
    }

    #[test]
    fn ein_unsinnig_grosser_rand_wird_begrenzt() {
        // Ohne Begrenzung läge das Wasserzeichen außerhalb des Bildes.
        assert_eq!(margin_px(1000, 1000, 500.0), 450);
    }

    #[test]
    fn die_schriftgroesse_bemisst_sich_an_der_hoehe() {
        assert_eq!(font_size_px(4000, 2000, 5.0), 100.0);
        assert_eq!(font_size_px(2000, 4000, 5.0), 100.0);
    }

    #[test]
    fn kacheln_bedecken_das_ganze_bild() {
        let origins = tile_origins(100, 100, 30, 30, 0);
        assert!(!origins.is_empty());
        // Links/oben beginnt das Muster außerhalb oder am Rand, rechts/
        // unten reicht es darüber hinaus.
        let min_x = origins.iter().map(|(x, _)| *x).min().unwrap();
        let max_x = origins.iter().map(|(x, _)| *x).max().unwrap();
        assert!(min_x <= 0, "min_x = {min_x}");
        assert!(max_x + 30 >= 100, "max_x = {max_x}");
    }

    #[test]
    fn das_kachelmuster_ist_zentriert() {
        let origins = tile_origins(100, 100, 30, 30, 0);
        let min_x = origins.iter().map(|(x, _)| *x).min().unwrap();
        let max_x = origins.iter().map(|(x, _)| *x).max().unwrap();
        let ueberstand_links = -min_x;
        let ueberstand_rechts = max_x + 30 - 100;
        assert_eq!(ueberstand_links, ueberstand_rechts);
    }

    #[test]
    fn der_kachelabstand_wird_eingehalten() {
        let origins = tile_origins(200, 50, 20, 20, 10);
        let mut xs: Vec<i64> = origins.iter().map(|(x, _)| *x).collect();
        xs.sort_unstable();
        xs.dedup();
        assert_eq!(xs[1] - xs[0], 30);
    }

    #[test]
    fn eine_kachel_ohne_ausdehnung_ergibt_kein_muster() {
        assert!(tile_origins(100, 100, 0, 10, 0).is_empty());
    }

    #[test]
    fn eine_drehung_um_null_grad_laesst_den_puffer_unangetastet() {
        let pixels = vec![1u8, 2, 3, 4, 5, 6, 7, 8];
        let (w, h, out) = rotate_rgba8(2, 1, &pixels, 0.0).unwrap();
        assert_eq!((w, h), (2, 1));
        assert_eq!(out, pixels);
    }

    #[test]
    fn eine_drehung_um_90_grad_vertauscht_breite_und_hoehe() {
        let pixels = vec![255u8; 4 * 10 * 4];
        let (w, h, _) = rotate_rgba8(4, 10, &pixels, 90.0).unwrap();
        assert_eq!((w, h), (10, 4));
    }

    #[test]
    fn eine_drehung_um_45_grad_vergroessert_die_flaeche() {
        let pixels = vec![255u8; 10 * 10 * 4];
        let (w, h, out) = rotate_rgba8(10, 10, &pixels, 45.0).unwrap();
        assert!(w > 10 && h > 10, "{w}x{h}");
        assert_eq!(out.len(), w as usize * h as usize * 4);
    }

    #[test]
    fn die_ecken_einer_drehung_bleiben_transparent_statt_schwarz() {
        // Der Test zum Moduldoku-Punkt: ein schwarzer Rahmen um jede
        // Kachel wäre das sichtbarste denkbare Artefakt.
        let pixels = vec![255u8; 20 * 20 * 4];
        let (w, _h, out) = rotate_rgba8(20, 20, &pixels, 45.0).unwrap();
        // Oberste linke Ecke des vergrößerten Puffers liegt außerhalb
        // des gedrehten Quadrats.
        assert_eq!(out[3], 0, "Alpha der Ecke");
        // Und der Mittelpunkt ist weiterhin deckend.
        let center = ((w as usize / 2) + (w as usize) * (w as usize / 2)) * 4;
        assert!(
            out[center + 3] > 200,
            "Alpha der Mitte: {}",
            out[center + 3]
        );
    }

    #[test]
    fn ein_falsch_grosser_puffer_wird_abgelehnt() {
        assert!(rotate_rgba8(4, 4, &[0, 0, 0, 255], 45.0).is_err());
    }

    #[test]
    fn die_vorgabe_ist_ein_dezentes_wasserzeichen_unten_rechts() {
        let placement = RelativePlacement::default();
        assert_eq!(placement.position, WatermarkPosition::BottomRight);
        assert!(placement.tile.is_none());
        assert!(placement.size_percent > 0.0 && placement.size_percent < 50.0);
    }
}
