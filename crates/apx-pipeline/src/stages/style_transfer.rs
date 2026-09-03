//! KI-Stiltransfer zwischen Fotos (Phase 14 Schritt 9, siehe
//! `DECISIONS.md` ADR-0041 Nachtrag IX): Lightroom hat dafür kein
//! Äquivalent. Legt das einmalig vorab berechnete stilisierte Ergebnis
//! (`StyleTransferPatch`, siehe dessen Moduldoku) linear mit dem
//! bereits **fertig entwickelten sRGB-RGBA8-Bild** überblendet zurück —
//! läuft in `develop::render_rgba8`s fester Kette nach `composite`, vor
//! `geometry` (derselbe display-referred Farbraum wie
//! `stages::composite`).
//!
//! **Warum ein vorab berechneter Patch statt eines Live-Modellaufrufs:**
//! dieselbe architektonische Beschränkung wie bei `stages::
//! virtual_aperture` (Phase 14 Schritt 8) — `apx-pipeline` darf nicht
//! von `apx-ai` abhängen (die Abhängigkeit verläuft bereits umgekehrt).
//! Die eigentliche `apx_ai::style_transfer`-Inferenz läuft deshalb
//! einmal in `apx-app` (hängt von beiden Crates ab), das Ergebnis kommt
//! hier nur noch als fertige Bitmap an, die pro Rendern lediglich
//! bilinear auf die tatsächliche Bildgröße skaliert wird.
//!
//! **`amount` blendet linear statt über `stages::masks::blend_pixel`**:
//! anders als Compositing (mehrere Blend-Modi zur Auswahl) hat
//! Stiltransfer nur einen sinnvollen Modus — geradliniges Überblenden
//! zwischen unverändertem und vollstilisiertem Bild (Adobes "Normal"-
//! Blend-Modus wäre ohnehin nur diese Interpolation) —, deshalb hier
//! direkt implementiert statt eine Blend-Modus-Auswahl anzubieten, die
//! nie einen zweiten sinnvollen Wert hätte.

use apx_core::raster::bilinear_resize_u8;

use crate::edl::v4::StyleTransferAdjustment;

fn split_rgb(pixels: &[u8], width: u32, height: u32) -> (Vec<u8>, Vec<u8>, Vec<u8>) {
    let n = (width as usize) * (height as usize);
    let mut r = vec![0u8; n];
    let mut g = vec![0u8; n];
    let mut b = vec![0u8; n];
    for i in 0..n.min(pixels.len() / 3) {
        r[i] = pixels[i * 3];
        g[i] = pixels[i * 3 + 1];
        b[i] = pixels[i * 3 + 2];
    }
    (r, g, b)
}

/// Blendet das vorab stilisierte Ergebnis aus `adjustment.patch` mit
/// `amount`-Deckkraft über `base` (RGBA8, `width * height * 4` Bytes) —
/// unverändert durchgereicht, solange kein Patch vorliegt oder `amount`
/// bei `0.0` steht (siehe Moduldoku).
pub fn apply(
    base: &[u8],
    width: u32,
    height: u32,
    adjustment: &StyleTransferAdjustment,
) -> Vec<u8> {
    let Some(patch) = &adjustment.patch else {
        return base.to_vec();
    };
    let opacity = adjustment.amount.clamp(0.0, 1.0);
    if opacity <= 0.0 || patch.bitmap_width == 0 || patch.bitmap_height == 0 {
        return base.to_vec();
    }

    let (src_r, src_g, src_b) = split_rgb(&patch.pixels, patch.bitmap_width, patch.bitmap_height);
    let styled_r = bilinear_resize_u8(
        &src_r,
        patch.bitmap_width,
        patch.bitmap_height,
        width,
        height,
    );
    let styled_g = bilinear_resize_u8(
        &src_g,
        patch.bitmap_width,
        patch.bitmap_height,
        width,
        height,
    );
    let styled_b = bilinear_resize_u8(
        &src_b,
        patch.bitmap_width,
        patch.bitmap_height,
        width,
        height,
    );

    let n = (width as usize) * (height as usize);
    let mut out = base.to_vec();
    for i in 0..n {
        let dst = i * 4;
        for (c, styled) in [&styled_r, &styled_g, &styled_b].into_iter().enumerate() {
            let original = out[dst + c] as f32;
            let value = original + (styled[i] as f32 - original) * opacity;
            out[dst + c] = value.round().clamp(0.0, 255.0) as u8;
        }
        // Alpha bleibt unverändert (`out[dst + 3]`).
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::edl::v4::StyleTransferPatch;

    fn flat_rgba(width: u32, height: u32, value: u8) -> Vec<u8> {
        let mut out = vec![0u8; (width * height * 4) as usize];
        for chunk in out.chunks_exact_mut(4) {
            chunk[0] = value;
            chunk[1] = value;
            chunk[2] = value;
            chunk[3] = 255;
        }
        out
    }

    fn flat_patch(width: u32, height: u32, value: u8) -> StyleTransferPatch {
        StyleTransferPatch {
            bitmap_width: width,
            bitmap_height: height,
            pixels: vec![value; (width * height * 3) as usize],
        }
    }

    #[test]
    fn without_a_patch_is_identity() {
        let base = flat_rgba(4, 4, 100);
        let adjustment = StyleTransferAdjustment {
            amount: 1.0,
            patch: None,
        };
        assert_eq!(apply(&base, 4, 4, &adjustment), base);
    }

    #[test]
    fn zero_amount_is_identity_even_with_a_patch() {
        let base = flat_rgba(4, 4, 100);
        let adjustment = StyleTransferAdjustment {
            amount: 0.0,
            patch: Some(flat_patch(4, 4, 250)),
        };
        assert_eq!(apply(&base, 4, 4, &adjustment), base);
    }

    #[test]
    fn full_amount_replaces_the_base_with_the_resized_patch_color() {
        let base = flat_rgba(4, 4, 100);
        let adjustment = StyleTransferAdjustment {
            amount: 1.0,
            // Andere Auflösung als das Zielbild — muss vor dem Blenden
            // hochskaliert werden.
            patch: Some(flat_patch(2, 2, 200)),
        };
        let out = apply(&base, 4, 4, &adjustment);
        for chunk in out.chunks_exact(4) {
            assert_eq!(chunk[0], 200);
            assert_eq!(chunk[1], 200);
            assert_eq!(chunk[2], 200);
            assert_eq!(chunk[3], 255, "Alpha darf sich nicht ändern");
        }
    }

    #[test]
    fn partial_amount_interpolates_between_base_and_patch() {
        let base = flat_rgba(2, 2, 0);
        let adjustment = StyleTransferAdjustment {
            amount: 0.5,
            patch: Some(flat_patch(2, 2, 200)),
        };
        let out = apply(&base, 2, 2, &adjustment);
        // 0 + (200 - 0) * 0.5 = 100.
        assert_eq!(out[0], 100);
    }
}
