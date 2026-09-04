//! Automatisches Hautglätten (Phase 15 Schritt 5, siehe `DECISIONS.md`
//! ADR-0042 — Lightroom hat kein automatisches, gesichtserkennungs-
//! gestütztes Hautglätten, nur den manuellen Anpassungspinsel). Legt das
//! einmalig vorab berechnete geglättete Ergebnis (`SkinSmoothingPatch`)
//! linear mit dem bereits **fertig entwickelten sRGB-RGBA8-Bild**
//! überblendet zurück — läuft in `develop::render_rgba8`s fester Kette
//! nach `style_transfer`, vor `sky_replace` (derselbe display-referred
//! Farbraum wie `stages::style_transfer`).
//!
//! **Vorab berechneter Patch statt Live-Berechnung**: dieselbe
//! architektonische Beschränkung wie bei `stages::style_transfer`/
//! `stages::virtual_aperture` — `apx-pipeline` darf nicht von `apx-ai`
//! abhängen. Die eigentliche Gesichtserkennung + gesichtsbewusste
//! Frequenztrennung läuft deshalb einmal in `apx-app`s `smooth_skin`-
//! Command, das Ergebnis kommt hier nur noch als fertige Bitmap an.
//!
//! **`amount` blendet linear statt über `stages::masks::blend_pixel`**:
//! wie bei Stiltransfer gibt es nur einen sinnvollen Modus (Deckkraft-
//! Überblendung zwischen unverändertem und voll geglättetem Bild).

use apx_core::raster::bilinear_resize_u8;

use crate::edl::v4::SkinSmoothingAdjustment;

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

/// Blendet das vorab geglättete Ergebnis aus `adjustment.patch` mit
/// `amount`-Deckkraft über `base` (RGBA8, `width * height * 4` Bytes) —
/// unverändert durchgereicht, solange kein Patch vorliegt oder `amount`
/// bei `0.0` steht (siehe Moduldoku).
pub fn apply(
    base: &[u8],
    width: u32,
    height: u32,
    adjustment: &SkinSmoothingAdjustment,
) -> Vec<u8> {
    let Some(patch) = &adjustment.patch else {
        return base.to_vec();
    };
    let opacity = adjustment.amount.clamp(0.0, 1.0);
    if opacity <= 0.0 || patch.bitmap_width == 0 || patch.bitmap_height == 0 {
        return base.to_vec();
    }

    let (src_r, src_g, src_b) = split_rgb(&patch.pixels, patch.bitmap_width, patch.bitmap_height);
    let smoothed_r = bilinear_resize_u8(
        &src_r,
        patch.bitmap_width,
        patch.bitmap_height,
        width,
        height,
    );
    let smoothed_g = bilinear_resize_u8(
        &src_g,
        patch.bitmap_width,
        patch.bitmap_height,
        width,
        height,
    );
    let smoothed_b = bilinear_resize_u8(
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
        for (c, smoothed) in [&smoothed_r, &smoothed_g, &smoothed_b]
            .into_iter()
            .enumerate()
        {
            let original = out[dst + c] as f32;
            let value = original + (smoothed[i] as f32 - original) * opacity;
            out[dst + c] = value.round().clamp(0.0, 255.0) as u8;
        }
        // Alpha bleibt unverändert (`out[dst + 3]`).
    }
    out
}
