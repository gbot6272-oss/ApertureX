//! Himmelsaustausch (Phase 14 Schritt 10): `apx_ai::sky_replace` liefert
//! bereits das fertige, belichtungsangeglichene Vollbild als `patch` —
//! diese Stufe skaliert ihn nur noch auf die tatsächliche Bildgröße und
//! ersetzt die RGB-Kanäle vollständig (kein Deckkraft-Regler, Alpha
//! bleibt unverändert).

use apx_core::raster::bilinear_resize_u8;

use crate::edl::v4::SkyReplacePatch;

pub fn apply(base: &[u8], width: u32, height: u32, patch: &Option<SkyReplacePatch>) -> Vec<u8> {
    let Some(patch) = patch else {
        return base.to_vec();
    };
    if patch.bitmap_width == 0 || patch.bitmap_height == 0 {
        return base.to_vec();
    }
    let n = (patch.bitmap_width as usize) * (patch.bitmap_height as usize);
    let mut r = vec![0u8; n];
    let mut g = vec![0u8; n];
    let mut b = vec![0u8; n];
    for i in 0..n.min(patch.pixels.len() / 3) {
        r[i] = patch.pixels[i * 3];
        g[i] = patch.pixels[i * 3 + 1];
        b[i] = patch.pixels[i * 3 + 2];
    }
    let r = bilinear_resize_u8(&r, patch.bitmap_width, patch.bitmap_height, width, height);
    let g = bilinear_resize_u8(&g, patch.bitmap_width, patch.bitmap_height, width, height);
    let b = bilinear_resize_u8(&b, patch.bitmap_width, patch.bitmap_height, width, height);

    let mut out = base.to_vec();
    let n = (width as usize) * (height as usize);
    for i in 0..n {
        out[i * 4] = r[i];
        out[i * 4 + 1] = g[i];
        out[i * 4 + 2] = b[i];
    }
    out
}
