//! Himmelsaustausch mit grober automatischer Neubelichtung (Phase 14
//! Schritt 10). Klassischer Algorithmus, kein ONNX-Modell: die
//! Himmel-Maske kommt aus [`crate::segmentation::sky_alpha`].

use apx_core::raster::bilinear_resize_u8;

fn split(px: &[u8], n: usize) -> (Vec<u8>, Vec<u8>, Vec<u8>) {
    let mut r = vec![0u8; n];
    let mut g = vec![0u8; n];
    let mut b = vec![0u8; n];
    for i in 0..n.min(px.len() / 3) {
        r[i] = px[i * 3];
        g[i] = px[i * 3 + 1];
        b[i] = px[i * 3 + 2];
    }
    (r, g, b)
}

/// Ersetzt den per `alpha` markierten Himmel durch `sky_rgb` und
/// skaliert den Vordergrund je Kanal grob auf die mittlere Helligkeit
/// des neuen Himmels (einfache Belichtungs-/Weißabgleichsangleichung).
pub fn composite(
    photo_rgb: &[u8],
    width: u32,
    height: u32,
    alpha: &[u8],
    sky_rgb: &[u8],
    sky_width: u32,
    sky_height: u32,
) -> Vec<u8> {
    let n = (width as usize) * (height as usize);
    let (pr, pg, pb) = split(photo_rgb, n);
    let (sr0, sg0, sb0) = split(sky_rgb, (sky_width as usize) * (sky_height as usize));
    let sr = bilinear_resize_u8(&sr0, sky_width, sky_height, width, height);
    let sg = bilinear_resize_u8(&sg0, sky_width, sky_height, width, height);
    let sb = bilinear_resize_u8(&sb0, sky_width, sky_height, width, height);

    let mut old_sum = [0f64; 3];
    let mut old_w = 0f64;
    let mut new_sum = [0f64; 3];
    for i in 0..n {
        let a = alpha[i] as f64 / 255.0;
        old_sum[0] += pr[i] as f64 * a;
        old_sum[1] += pg[i] as f64 * a;
        old_sum[2] += pb[i] as f64 * a;
        old_w += a;
        new_sum[0] += sr[i] as f64;
        new_sum[1] += sg[i] as f64;
        new_sum[2] += sb[i] as f64;
    }
    let new_avg = new_sum.map(|v| v / n.max(1) as f64);
    let ratios: [f32; 3] = std::array::from_fn(|c| {
        if old_w > 1.0 && old_sum[c] > 1.0 {
            ((new_avg[c] / (old_sum[c] / old_w)) as f32).clamp(0.5, 2.0)
        } else {
            1.0
        }
    });

    let mut out = vec![0u8; n * 3];
    for i in 0..n {
        let a = alpha[i] as f32 / 255.0;
        for (c, (p, s)) in [(pr[i], sr[i]), (pg[i], sg[i]), (pb[i], sb[i])]
            .into_iter()
            .enumerate()
        {
            let fg = (p as f32 * ratios[c]).clamp(0.0, 255.0);
            let v = fg + (s as f32 - fg) * a;
            out[i * 3 + c] = v.round().clamp(0.0, 255.0) as u8;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_alpha_leaves_the_photo_unchanged() {
        let photo = vec![100u8; 2 * 2 * 3];
        let alpha = vec![0u8; 2 * 2];
        let sky = vec![200u8; 2 * 2 * 3];
        assert_eq!(composite(&photo, 2, 2, &alpha, &sky, 2, 2), photo);
    }

    #[test]
    fn full_alpha_replaces_with_the_new_sky() {
        let photo = vec![100u8; 2 * 2 * 3];
        let alpha = vec![255u8; 2 * 2];
        let sky = vec![200u8; 2 * 2 * 3];
        assert_eq!(composite(&photo, 2, 2, &alpha, &sky, 2, 2), sky);
    }
}
