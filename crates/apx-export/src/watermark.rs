//! Wasserzeichen (Phase 8 Schritt 2, `PLAN.md`: „Bild-/Text-Overlay auf
//! dem gerenderten RGBA8-Puffer vor der Kodierung"). Zwei Varianten:
//!
//! - **Bild-Wasserzeichen** ([`apply_image_watermark`]): komponiert eine
//!   bereits dekodierte RGBA8-Bildquelle (z. B. ein Logo-PNG) alpha-
//!   gewichtet in eine Bildecke.
//! - **Text-Wasserzeichen** ([`apply_text_watermark`]): rasterisiert
//!   echten Text über `ab_glyph` (reines Rust, kein Systemschrift-API)
//!   und komponiert ihn genauso.
//!
//! **Bewusste Vereinfachung:** Text-Wasserzeichen brauchen eine vom
//! Nutzer ausgewählte `.ttf`/`.otf`-Schriftdatei (`font_bytes`) — es wird
//! keine Schriftart mitgeliefert/eingebettet (würde eine zusätzliche
//! Binärdatei plus deren Lizenzeintrag in `THIRD_PARTY.md` bedeuten, für
//! eine reine „Vorhanden oder nicht"-Zusatzfunktion nicht gerechtfertigt).
//! Der Export-Dialog bietet dafür einen Datei-Auswahldialog an
//! („Schriftdatei wählen…"), keinen Schriftart-Namen aus einer
//! eingebauten Liste.

use ab_glyph::{point, Font, FontRef, PxScale, ScaleFont};

use crate::error::{ExportError, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WatermarkPosition {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
    Center,
}

fn origin_for(
    position: WatermarkPosition,
    canvas_w: u32,
    canvas_h: u32,
    overlay_w: u32,
    overlay_h: u32,
    margin: u32,
) -> (i64, i64) {
    let (cw, ch, ow, oh, m) = (
        canvas_w as i64,
        canvas_h as i64,
        overlay_w as i64,
        overlay_h as i64,
        margin as i64,
    );
    match position {
        WatermarkPosition::TopLeft => (m, m),
        WatermarkPosition::TopRight => (cw - ow - m, m),
        WatermarkPosition::BottomLeft => (m, ch - oh - m),
        WatermarkPosition::BottomRight => (cw - ow - m, ch - oh - m),
        WatermarkPosition::Center => ((cw - ow) / 2, (ch - oh) / 2),
    }
}

/// Komponiert `overlay_rgba` (interleaved RGBA8, `overlay_width` ×
/// `overlay_height`) alpha-gewichtet mit zusätzlichem `opacity`-Faktor
/// (`0.0..=1.0`, multipliziert auf `overlay_rgba`s eigenen Alphakanal) in
/// `pixels` hinein — Pixel, die (teilweise) außerhalb der Zielfläche
/// liegen, werden übersprungen statt einen Fehler auszulösen (ein zu
/// großes Wasserzeichen wird einfach abgeschnitten).
#[allow(clippy::too_many_arguments)]
pub fn apply_image_watermark(
    width: u32,
    height: u32,
    pixels: &mut [u8],
    overlay_width: u32,
    overlay_height: u32,
    overlay_rgba: &[u8],
    position: WatermarkPosition,
    opacity: f32,
    margin: u32,
) -> Result<()> {
    let expected_len = width as usize * height as usize * 4;
    if pixels.len() != expected_len {
        return Err(ExportError::Unsupported(format!(
            "Pufferlänge {} passt nicht zu {width}x{height} RGBA8",
            pixels.len()
        )));
    }
    let expected_overlay_len = overlay_width as usize * overlay_height as usize * 4;
    if overlay_rgba.len() != expected_overlay_len {
        return Err(ExportError::Unsupported(format!(
            "Overlay-Pufferlänge {} passt nicht zu {overlay_width}x{overlay_height} RGBA8",
            overlay_rgba.len()
        )));
    }

    let (origin_x, origin_y) = origin_for(
        position,
        width,
        height,
        overlay_width,
        overlay_height,
        margin,
    );
    let opacity = opacity.clamp(0.0, 1.0);

    for oy in 0..overlay_height as i64 {
        let dst_y = origin_y + oy;
        if dst_y < 0 || dst_y >= height as i64 {
            continue;
        }
        for ox in 0..overlay_width as i64 {
            let dst_x = origin_x + ox;
            if dst_x < 0 || dst_x >= width as i64 {
                continue;
            }
            let src_idx = (oy as usize * overlay_width as usize + ox as usize) * 4;
            let alpha = (overlay_rgba[src_idx + 3] as f32 / 255.0) * opacity;
            if alpha <= 0.0 {
                continue;
            }
            let dst_idx = (dst_y as usize * width as usize + dst_x as usize) * 4;
            for channel in 0..3 {
                let src = overlay_rgba[src_idx + channel] as f32;
                let dst = pixels[dst_idx + channel] as f32;
                pixels[dst_idx + channel] = (src * alpha + dst * (1.0 - alpha))
                    .round()
                    .clamp(0.0, 255.0) as u8;
            }
            // Alpha des Zielpuffers bleibt 255 (undurchsichtiges Foto).
        }
    }
    Ok(())
}

/// Rasterisiert `text` in `color` mit `font_size_px` (per `ab_glyph`,
/// `font_bytes` = Inhalt einer `.ttf`/`.otf`-Datei) und komponiert ihn wie
/// [`apply_image_watermark`] in `pixels` hinein.
#[allow(clippy::too_many_arguments)]
pub fn apply_text_watermark(
    width: u32,
    height: u32,
    pixels: &mut [u8],
    font_bytes: &[u8],
    text: &str,
    font_size_px: f32,
    color: [u8; 3],
    position: WatermarkPosition,
    opacity: f32,
    margin: u32,
) -> Result<()> {
    let font = FontRef::try_from_slice(font_bytes).map_err(|err| {
        ExportError::Unsupported(format!("Schriftdatei konnte nicht gelesen werden: {err}"))
    })?;
    let scale = PxScale::from(font_size_px);
    let scaled = font.as_scaled(scale);

    // Glyphen entlang der Grundlinie platzieren, Gesamtbreite/-höhe
    // dabei mitverfolgen (siehe `ab_glyph`s Layout-Beispiel).
    let mut glyphs = Vec::new();
    let mut caret = point(0.0, scaled.ascent());
    for ch in text.chars() {
        let glyph_id = font.glyph_id(ch);
        let glyph = glyph_id.with_scale_and_position(scale, caret);
        caret.x += scaled.h_advance(glyph_id);
        glyphs.push(glyph);
    }
    let text_width = caret.x.ceil().max(1.0) as u32;
    let text_height = (scaled.ascent() - scaled.descent()).ceil().max(1.0) as u32;

    // Deckungs-Puffer (nur Alpha) auf Textgröße, dann in ein RGBA8-Overlay
    // mit `color` gefüllt — dieselbe Kompositions-Funktion wie Bild-
    // Wasserzeichen wiederverwendbar.
    let mut coverage = vec![0.0f32; text_width as usize * text_height as usize];
    for glyph in glyphs {
        if let Some(outlined) = font.outline_glyph(glyph) {
            let bounds = outlined.px_bounds();
            outlined.draw(|gx, gy, value| {
                let px = bounds.min.x as i64 + gx as i64;
                let py = bounds.min.y as i64 + gy as i64;
                if px >= 0 && py >= 0 && (px as u32) < text_width && (py as u32) < text_height {
                    let idx = py as usize * text_width as usize + px as usize;
                    coverage[idx] = coverage[idx].max(value);
                }
            });
        }
    }

    let overlay_rgba: Vec<u8> = coverage
        .iter()
        .flat_map(|&v| {
            [
                color[0],
                color[1],
                color[2],
                (v.clamp(0.0, 1.0) * 255.0).round() as u8,
            ]
        })
        .collect();

    apply_image_watermark(
        width,
        height,
        pixels,
        text_width,
        text_height,
        &overlay_rgba,
        position,
        opacity,
        margin,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn solid_overlay(width: u32, height: u32, rgba: [u8; 4]) -> Vec<u8> {
        (0..width * height).flat_map(|_| rgba).collect()
    }

    /// Schwarze, undurchsichtige Leinwand (Alpha 255) — dieselbe
    /// Invariante wie `RenderedImage`s Moduldoku ("Alpha immer 255"), die
    /// [`apply_image_watermark`] voraussetzt (es fasst den Zielalphakanal
    /// nie an). Ein `vec![0u8; …]` allein wäre KEIN gültiges Testbild
    /// dafür (Alpha 0 statt 255).
    fn opaque_black_canvas(width: u32, height: u32) -> Vec<u8> {
        solid_overlay(width, height, [0, 0, 0, 255])
    }

    #[test]
    fn opaque_overlay_replaces_target_pixels() {
        let mut pixels = opaque_black_canvas(4, 4);
        let overlay = solid_overlay(2, 2, [255, 0, 0, 255]);
        apply_image_watermark(
            4,
            4,
            &mut pixels,
            2,
            2,
            &overlay,
            WatermarkPosition::TopLeft,
            1.0,
            0,
        )
        .unwrap();
        assert_eq!(&pixels[0..4], &[255, 0, 0, 255]);
        // Außerhalb des Overlays unverändert.
        assert_eq!(&pixels[3 * 4..3 * 4 + 4], &[0, 0, 0, 255]);
    }

    #[test]
    fn zero_opacity_leaves_pixels_unchanged() {
        let mut pixels = opaque_black_canvas(4, 4);
        let original = pixels.clone();
        let overlay = solid_overlay(2, 2, [255, 0, 0, 255]);
        apply_image_watermark(
            4,
            4,
            &mut pixels,
            2,
            2,
            &overlay,
            WatermarkPosition::Center,
            0.0,
            0,
        )
        .unwrap();
        assert_eq!(pixels, original);
    }

    #[test]
    fn bottom_right_position_offsets_from_canvas_edge() {
        let mut pixels = opaque_black_canvas(4, 4);
        let overlay = solid_overlay(1, 1, [255, 255, 255, 255]);
        apply_image_watermark(
            4,
            4,
            &mut pixels,
            1,
            1,
            &overlay,
            WatermarkPosition::BottomRight,
            1.0,
            0,
        )
        .unwrap();
        let last_pixel_idx = (3 * 4 + 3) * 4;
        assert_eq!(
            &pixels[last_pixel_idx..last_pixel_idx + 4],
            &[255, 255, 255, 255]
        );
    }

    #[test]
    fn overlay_partially_outside_canvas_is_clipped_not_an_error() {
        let mut pixels = opaque_black_canvas(2, 2);
        let overlay = solid_overlay(4, 4, [255, 0, 0, 255]);
        // Overlay ist groesser als die Zielflaeche -- darf nicht abstürzen.
        apply_image_watermark(
            2,
            2,
            &mut pixels,
            4,
            4,
            &overlay,
            WatermarkPosition::TopLeft,
            1.0,
            0,
        )
        .unwrap();
        assert_eq!(&pixels[0..4], &[255, 0, 0, 255]);
    }

    #[test]
    fn mismatched_buffer_length_is_rejected() {
        let mut pixels = vec![0u8; 3];
        let overlay = solid_overlay(1, 1, [0, 0, 0, 255]);
        let err = apply_image_watermark(
            2,
            2,
            &mut pixels,
            1,
            1,
            &overlay,
            WatermarkPosition::TopLeft,
            1.0,
            0,
        )
        .unwrap_err();
        assert!(matches!(err, ExportError::Unsupported(_)));
    }

    /// Nutzt eine echte, auf Linux-Entwicklungs-/CI-Maschinen übliche
    /// Systemschrift (Liberation Sans) statt einer im Repo mitgelieferten
    /// Testschrift — bewusst auf Linux beschränkt (`cfg(target_os =
    /// "linux")`), damit macOS-/Windows-CI-Läufe nicht auf einen
    /// plattformspezifischen Schriftpfad angewiesen sind. Der eigentliche
    /// Rasterisierungscode selbst ist plattformunabhängig (reines
    /// `ab_glyph`) — dieser Test prüft nur, dass die Aufruf-Verdrahtung
    /// (Glyph-Layout, Deckungspuffer, Kompositions-Aufruf) tatsächlich
    /// sichtbare Pixel erzeugt, nicht stillschweigend leer bleibt.
    #[cfg(target_os = "linux")]
    #[test]
    fn real_font_produces_visible_glyph_coverage() {
        let font_bytes =
            std::fs::read("/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf")
                .expect("Liberation Sans sollte auf dieser Maschine vorhanden sein");
        let mut pixels = vec![0u8; 64 * 32 * 4]; // schwarze Leinwand
        apply_text_watermark(
            64,
            32,
            &mut pixels,
            &font_bytes,
            "Aa",
            20.0,
            [255, 255, 255],
            WatermarkPosition::TopLeft,
            1.0,
            0,
        )
        .unwrap();
        assert!(
            pixels.chunks_exact(4).any(|p| p[0] > 0),
            "mindestens ein Pixel sollte vom Text eingefärbt sein"
        );
    }

    #[test]
    fn invalid_font_bytes_are_a_clean_error() {
        let mut pixels = vec![0u8; 4 * 4 * 4];
        let err = apply_text_watermark(
            4,
            4,
            &mut pixels,
            b"nicht-echte-schriftdaten",
            "Test",
            12.0,
            [255, 255, 255],
            WatermarkPosition::Center,
            1.0,
            0,
        )
        .unwrap_err();
        assert!(matches!(err, ExportError::Unsupported(_)));
    }
}
