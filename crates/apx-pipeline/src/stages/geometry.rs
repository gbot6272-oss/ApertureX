//! Geometrie (Freistellen/Winkel) — `SPEC.md` §3.2 „Crop/Geometrie". Der
//! einzige Schritt in Phase 4, dessen Ausgabegröße von der Eingabegröße
//! abweicht (siehe `PLAN.md`s Architektur-Grundsatz „größenverändernde
//! Dispatch-Form" — hier bewusst NICHT als GPU-Dispatch umgesetzt,
//! sondern als reiner CPU-Nachschritt auf dem fertigen RGBA8-Puffer,
//! direkt nach den Kurven, als allerletzter Schritt in
//! `develop::render_rgba8`).
//!
//! **Warum CPU-only, kein GPU-Pfad** (analog zu [`super::curves`]):
//! Drehung + Zuschnitt laufen nur einmal pro Regler-Tick auf dem bereits
//! herunterskalierten Vorschaubild, nicht pro Pixel-Kanal wie die
//! übrigen linearen Werkzeuge — ein zusätzlicher GPU-Rundtrip (Upload,
//! Dispatch, Download) wäre hier teurer als der eigentliche
//! CPU-Durchlauf.
//!
//! **Bewusste Vereinfachungen** (siehe `DECISIONS.md` ADR-0028):
//! - **Drehung:** ein einzelner bilinear abgetasteter Dreh-Durchlauf um
//!   den Bildmittelpunkt, Randpixel geklemmt (wie
//!   [`super::lens_corrections`]) statt schwarz gefüllt — bei starken
//!   Winkeln bleiben an den Ecken keine „echten" Bilddaten, das ist
//!   dieselbe bewusste Einschränkung wie dort.
//! - **Zuschnitt:** eine reine, pixel-genau ausgerichtete
//!   Rechteck-Extraktion (kein zusätzliches Resampling) — `crop`s
//!   normierte Koordinaten (`0.0..=1.0`) beziehen sich auf das bereits
//!   gedrehte Bild in seiner ursprünglichen Pixelgröße.
//! - **`aspect_ratio`:** wirkt ausschließlich als Frontend-Ziehgriff-
//!   Einschränkung beim interaktiven Anpassen von `crop` — diese Stufe
//!   liest nur das bereits berechnete `crop`-Rechteck, nicht
//!   `aspect_ratio` selbst.
//! - **`overlay`** (Rasterüberlagerung: Drittel/Goldener Schnitt/...):
//!   eine reine Anzeige-Hilfe im Frontend-Crop-Werkzeug, berührt nie
//!   Pixel — diese Stufe liest das Feld gar nicht.
//! - **`auto_horizon`:** dokumentierter No-op-Platzhalter in dieser
//!   Stufe. Die EXIF-Ausrichtung (Rotation um Vielfache von 90°/
//!   Spiegelung) wird bereits in `apx-raw`s `orientation.rs` *vor* der
//!   EDL-Pipeline angewendet (`apx_raw::decode_linear` liefert bereits
//!   aufrecht stehende Pixel) — ein zusätzliches automatisches
//!   Ausrichten hier bräuchte echte Kantenerkennungs-basierte
//!   Horizont-Schätzung, eine CV-Aufgabe außerhalb des Stacks (siehe
//!   ADR-0028/ADR-0030).

use rayon::prelude::*;

use crate::edl::v2::{CropRect, GeometryAdjustment};

fn sample_rgba_at(rgba: &[u8], width: usize, height: usize, x: i32, y: i32) -> [u8; 4] {
    let cx = x.clamp(0, width as i32 - 1) as usize;
    let cy = y.clamp(0, height as i32 - 1) as usize;
    let idx = (cy * width + cx) * 4;
    [rgba[idx], rgba[idx + 1], rgba[idx + 2], rgba[idx + 3]]
}

fn bilinear_sample_rgba(rgba: &[u8], width: usize, height: usize, x: f32, y: f32) -> [u8; 4] {
    let x0 = x.floor();
    let y0 = y.floor();
    let fx = x - x0;
    let fy = y - y0;
    let x0i = x0 as i32;
    let y0i = y0 as i32;
    let p00 = sample_rgba_at(rgba, width, height, x0i, y0i);
    let p10 = sample_rgba_at(rgba, width, height, x0i + 1, y0i);
    let p01 = sample_rgba_at(rgba, width, height, x0i, y0i + 1);
    let p11 = sample_rgba_at(rgba, width, height, x0i + 1, y0i + 1);
    let mut out = [0u8; 4];
    for c in 0..4 {
        let top = p00[c] as f32 + (p10[c] as f32 - p00[c] as f32) * fx;
        let bottom = p01[c] as f32 + (p11[c] as f32 - p01[c] as f32) * fx;
        let v = top + (bottom - top) * fy;
        out[c] = v.round().clamp(0.0, 255.0) as u8;
    }
    out
}

/// Dreht `rgba` um den Bildmittelpunkt (`angle_degrees`, im Uhrzeigersinn
/// positiv) — Ausgabegröße bleibt unverändert, Randpixel werden geklemmt
/// (siehe Moduldoku).
fn rotate_rgba(rgba: &[u8], width: u32, height: u32, angle_degrees: f32) -> Vec<u8> {
    if angle_degrees == 0.0 {
        return rgba.to_vec();
    }
    let w = width as usize;
    let h = height as usize;
    let half_w = width as f32 / 2.0;
    let half_h = height as f32 / 2.0;
    // Inverse Abbildung: für jedes Ziel-Pixel die Quellposition im
    // ungedrehten Bild bestimmen (dieselbe Herangehensweise wie
    // `lens_corrections.rs`).
    let angle = -angle_degrees.to_radians();
    let (sin_a, cos_a) = angle.sin_cos();

    (0..h)
        .into_par_iter()
        .flat_map_iter(move |y| {
            let dy = y as f32 - half_h;
            (0..w).flat_map(move |x| {
                let dx = x as f32 - half_w;
                let src_x = dx * cos_a - dy * sin_a + half_w;
                let src_y = dx * sin_a + dy * cos_a + half_h;
                bilinear_sample_rgba(rgba, w, h, src_x, src_y)
            })
        })
        .collect()
}

/// Extrahiert das durch `crop` beschriebene Rechteck (normierte
/// Koordinaten, siehe Moduldoku) als eigenständigen, pixel-genauen
/// Ausschnitt — kein Resampling, reine Rechteck-Kopie. Ein Rechteck, das
/// über den Bildrand hinausragen würde, wird geklemmt statt zu einem
/// Absturz zu führen.
fn crop_rgba(rgba: &[u8], width: u32, height: u32, crop: &CropRect) -> (u32, u32, Vec<u8>) {
    let w = width as usize;
    let h = height as usize;

    let start_x = ((crop.x.clamp(0.0, 1.0)) * width as f32).round() as usize;
    let start_y = ((crop.y.clamp(0.0, 1.0)) * height as f32).round() as usize;
    let start_x = start_x.min(w.saturating_sub(1));
    let start_y = start_y.min(h.saturating_sub(1));

    let raw_w = (crop.width.max(0.0) * width as f32).round() as usize;
    let raw_h = (crop.height.max(0.0) * height as f32).round() as usize;
    let out_w = raw_w.clamp(1, w - start_x);
    let out_h = raw_h.clamp(1, h - start_y);

    let mut out = Vec::with_capacity(out_w * out_h * 4);
    for row in 0..out_h {
        let src_row = start_y + row;
        let src_start = (src_row * w + start_x) * 4;
        let src_end = src_start + out_w * 4;
        out.extend_from_slice(&rgba[src_start..src_end]);
    }
    (out_w as u32, out_h as u32, out)
}

/// Wendet Drehung + Zuschnitt an — die einzige Funktion, die
/// `develop::render_rgba8` aus diesem Modul aufruft. Gibt die
/// tatsächliche (u. U. gegenüber `width`/`height` verkleinerte)
/// Ausgabegröße zurück.
pub fn apply(
    rgba: &[u8],
    width: u32,
    height: u32,
    adjustment: &GeometryAdjustment,
) -> (u32, u32, Vec<u8>) {
    let rotated = rotate_rgba(rgba, width, height, adjustment.angle_degrees);
    if adjustment.crop == CropRect::FULL {
        (width, height, rotated)
    } else {
        crop_rgba(&rotated, width, height, &adjustment.crop)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Baut ein `size`×`size`-RGBA8-Testbild mit einer eindeutigen
    /// Markierung an einer bestimmten Pixelposition.
    fn marked_image(size: u32, mark_x: u32, mark_y: u32) -> Vec<u8> {
        let s = size as usize;
        let mut pixels = vec![128u8; s * s * 4];
        let idx = ((mark_y as usize) * s + mark_x as usize) * 4;
        pixels[idx] = 255;
        pixels[idx + 1] = 0;
        pixels[idx + 2] = 0;
        pixels[idx + 3] = 255;
        pixels
    }

    #[test]
    fn neutral_is_identity() {
        let pixels = marked_image(10, 4, 4);
        let (w, h, result) = apply(&pixels, 10, 10, &GeometryAdjustment::NEUTRAL);
        assert_eq!((w, h), (10, 10));
        assert_eq!(result, pixels);
    }

    #[test]
    fn crop_reduces_output_dimensions() {
        let pixels = marked_image(20, 10, 10);
        let adjustment = GeometryAdjustment {
            crop: CropRect {
                x: 0.25,
                y: 0.25,
                width: 0.5,
                height: 0.5,
            },
            ..GeometryAdjustment::NEUTRAL
        };
        let (w, h, result) = apply(&pixels, 20, 20, &adjustment);
        assert_eq!((w, h), (10, 10));
        assert_eq!(result.len(), (10 * 10 * 4) as usize);
    }

    #[test]
    fn crop_keeps_the_expected_region() {
        let pixels = marked_image(20, 10, 10);
        let adjustment = GeometryAdjustment {
            crop: CropRect {
                x: 0.25,
                y: 0.25,
                width: 0.5,
                height: 0.5,
            },
            ..GeometryAdjustment::NEUTRAL
        };
        let (w, _h, result) = apply(&pixels, 20, 20, &adjustment);
        // Die Markierung bei (10,10) im Original liegt nach dem Zuschnitt
        // bei (10 - 5, 10 - 5) = (5, 5) im neuen, kleineren Bild.
        let idx = ((5 * w as usize) + 5) * 4;
        assert_eq!(&result[idx..idx + 4], &[255, 0, 0, 255]);
    }

    #[test]
    fn crop_rect_extending_past_the_edge_is_clamped_without_panicking() {
        let pixels = marked_image(20, 5, 5);
        let adjustment = GeometryAdjustment {
            crop: CropRect {
                x: 0.8,
                y: 0.8,
                width: 0.5, // würde über den Rand hinausragen
                height: 0.5,
            },
            ..GeometryAdjustment::NEUTRAL
        };
        let (w, h, result) = apply(&pixels, 20, 20, &adjustment);
        assert!(w >= 1 && h >= 1);
        assert_eq!(result.len(), (w * h * 4) as usize);
    }

    #[test]
    fn rotation_changes_pixel_values_without_changing_dimensions() {
        let pixels = marked_image(21, 15, 5);
        let adjustment = GeometryAdjustment {
            angle_degrees: 20.0,
            ..GeometryAdjustment::NEUTRAL
        };
        let (w, h, result) = apply(&pixels, 21, 21, &adjustment);
        assert_eq!((w, h), (21, 21));
        assert_ne!(result, pixels, "Drehung sollte den Pixelinhalt verändern");
    }

    #[test]
    fn rotation_and_crop_combine_to_the_cropped_size() {
        let pixels = marked_image(30, 15, 15);
        let adjustment = GeometryAdjustment {
            angle_degrees: 5.0,
            crop: CropRect {
                x: 0.1,
                y: 0.1,
                width: 0.4,
                height: 0.3,
            },
            ..GeometryAdjustment::NEUTRAL
        };
        let (w, h, result) = apply(&pixels, 30, 30, &adjustment);
        assert_eq!(w, (0.4 * 30.0_f32).round() as u32);
        assert_eq!(h, (0.3 * 30.0_f32).round() as u32);
        assert_eq!(result.len(), (w * h * 4) as usize);
    }
}
