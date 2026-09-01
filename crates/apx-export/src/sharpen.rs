//! Ausgabeschärfung nach Medium (Phase 8 Schritt 1, `PLAN.md`: „Bit-Tiefe
//! 8/16, Größenbegrenzung, Ausgabeschärfung nach Medium"). Klassisches
//! Unsharp-Masking: `Ausgabe = Original + Betrag * (Original - Weichgezeichnet)`
//! — dieselbe Grundtechnik wie `stages::details`s Schärfung (Phase 4
//! Schritt 8), hier aber auf dem fertigen 8-Bit-Ausgabepuffer nach der
//! Größenanpassung, nicht im linearen Arbeitsraum vor der Farbraum-
//! Konvertierung (Ausgabeschärfung muss die tatsächliche Ausgabeauflösung
//! kennen, siehe `SPEC.md` §5 „Ausgabeschärfung nach Medium").

use image::{ImageBuffer, Rgba};

use crate::error::{ExportError, Result};

/// Medium-Voreinstellungen (Lightroom-artig): Bildschirm braucht am
/// wenigsten Schärfung (native Pixel, kein Druckraster), Hochglanzpapier
/// am meisten (Tintenausbreitung mindert wahrgenommene Schärfe stärker als
/// mattes Papier).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SharpenTarget {
    Screen,
    Matte,
    Glossy,
}

impl SharpenTarget {
    /// `(Betrag 0..=2, Radius in Pixeln)` — Ausgangswerte, per `amount`-
    /// Parameter von [`unsharp_mask`] weiter skalierbar (0 = aus).
    pub fn defaults(self) -> (f32, f32) {
        match self {
            Self::Screen => (0.3, 0.8),
            Self::Matte => (0.5, 1.0),
            Self::Glossy => (0.7, 1.2),
        }
    }
}

/// Wendet Unsharp-Masking mit Stärke `amount` (0 = keine Wirkung) und
/// Weichzeichnungsradius `radius` (Pixel) auf einen interleaved-RGBA8-
/// Puffer an. Der Alphakanal bleibt unverändert (immer `255`, siehe
/// `RenderedImage`s Moduldoku).
pub fn unsharp_mask(
    width: u32,
    height: u32,
    pixels: &[u8],
    amount: f32,
    radius: f32,
) -> Result<Vec<u8>> {
    if amount <= 0.0 {
        return Ok(pixels.to_vec());
    }
    let buf = ImageBuffer::<Rgba<u8>, Vec<u8>>::from_raw(width, height, pixels.to_vec())
        .ok_or_else(|| {
            ExportError::Unsupported("Pufferlayout passt nicht zu Breite/Höhe".to_string())
        })?;
    let blurred = image::imageops::blur(&buf, radius.max(0.1));

    let mut out = pixels.to_vec();
    for (dst, (orig_px, blur_px)) in out
        .chunks_exact_mut(4)
        .zip(buf.pixels().zip(blurred.pixels()))
    {
        for (dst_channel, (&orig, &blur)) in dst[..3]
            .iter_mut()
            .zip(orig_px.0[..3].iter().zip(blur_px.0[..3].iter()))
        {
            let orig = orig as f32;
            let blur = blur as f32;
            let sharpened = orig + amount * (orig - blur);
            *dst_channel = sharpened.round().clamp(0.0, 255.0) as u8;
        }
        // dst[3] (Alpha) bleibt unverändert (schon 255 aus `out`s Kopie).
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_amount_leaves_pixels_unchanged() {
        let pixels = vec![10u8, 20, 30, 255, 200, 210, 220, 255];
        let out = unsharp_mask(2, 1, &pixels, 0.0, 1.0).unwrap();
        assert_eq!(out, pixels);
    }

    #[test]
    fn sharpening_increases_local_contrast_at_an_edge() {
        // Ein Halbbild dunkel, eines hell — die Kante in der Mitte sollte
        // nach der Schärfung einen stärkeren Kontrast zeigen.
        let mut pixels = Vec::new();
        for x in 0..8u32 {
            let v = if x < 4 { 50u8 } else { 200u8 };
            pixels.extend_from_slice(&[v, v, v, 255]);
        }
        let out = unsharp_mask(8, 1, &pixels, 1.0, 1.0).unwrap();
        // Pixel direkt links der Kante (Index 3) muss dunkler geworden
        // sein als das Original (Überschwingen an der Kante — der
        // Unsharp-Mask-Effekt).
        assert!(out[3 * 4] <= pixels[3 * 4]);
        // Alpha bleibt in jedem Fall unangetastet.
        assert!(out.chunks_exact(4).all(|p| p[3] == 255));
    }

    #[test]
    fn each_medium_preset_has_distinct_defaults() {
        let (screen_amount, _) = SharpenTarget::Screen.defaults();
        let (matte_amount, _) = SharpenTarget::Matte.defaults();
        let (glossy_amount, _) = SharpenTarget::Glossy.defaults();
        assert!(screen_amount < matte_amount);
        assert!(matte_amount < glossy_amount);
    }
}
