//! Bilineares Hoch-/Herunterskalieren einer Ein-Kanal-`u8`-Bitmap — ab
//! Phase 7 (`DECISIONS.md` ADR-0033) die Brücke zwischen `apx-ai`s
//! niedrig aufgelösten KI-Masken-Heuristiken und `apx-pipeline`s
//! `MaskGeometry::AiGenerated`, das dieselbe Bitmap bei jeder
//! tatsächlichen Render-Auflösung braucht. Lebt bewusst hier statt in
//! einem der beiden Crates: `apx-ai` hängt von `apx-pipeline` ab (für
//! EDL-Typen/Rendering), ein Import in der umgekehrten Richtung
//! (`apx-pipeline` → `apx-ai`) wäre ein Abhängigkeitszyklus — `apx-core`
//! ist die gemeinsame Grundlage, von der beide ohnehin abhängen.

/// Skaliert `src` (`src_w * src_h` Bytes, ein Wert je Pixel) auf
/// `dst_w * dst_h` per bilinearer Interpolation. `src_w`/`src_h` müssen
/// mit `src.len()` übereinstimmen; ein leeres oder nullgroßes Quellbild
/// ergibt eine komplett schwarze Ausgabe statt eines Absturzes.
pub fn bilinear_resize_u8(src: &[u8], src_w: u32, src_h: u32, dst_w: u32, dst_h: u32) -> Vec<u8> {
    if src_w == 0 || src_h == 0 || dst_w == 0 || dst_h == 0 || src.len() != (src_w * src_h) as usize
    {
        return vec![0u8; (dst_w * dst_h) as usize];
    }
    if src_w == dst_w && src_h == dst_h {
        return src.to_vec();
    }

    let mut out = vec![0u8; (dst_w * dst_h) as usize];
    // Skalenfaktor über die Pixel-*Zentren*, nicht die Pixel-Ecken — die
    // übliche Konvention (dieselbe wie der `image`-Crate-Resampler),
    // sonst würden Rand-Reihen/-Spalten systematisch heller/dunkler
    // abgetastet.
    let scale_x = src_w as f32 / dst_w as f32;
    let scale_y = src_h as f32 / dst_h as f32;

    for dy in 0..dst_h {
        let sy = ((dy as f32 + 0.5) * scale_y - 0.5).clamp(0.0, (src_h - 1) as f32);
        let y0 = sy.floor() as u32;
        let y1 = (y0 + 1).min(src_h - 1);
        let fy = sy - y0 as f32;

        for dx in 0..dst_w {
            let sx = ((dx as f32 + 0.5) * scale_x - 0.5).clamp(0.0, (src_w - 1) as f32);
            let x0 = sx.floor() as u32;
            let x1 = (x0 + 1).min(src_w - 1);
            let fx = sx - x0 as f32;

            let p00 = src[(y0 * src_w + x0) as usize] as f32;
            let p10 = src[(y0 * src_w + x1) as usize] as f32;
            let p01 = src[(y1 * src_w + x0) as usize] as f32;
            let p11 = src[(y1 * src_w + x1) as usize] as f32;

            let top = p00 * (1.0 - fx) + p10 * fx;
            let bottom = p01 * (1.0 - fx) + p11 * fx;
            let value = top * (1.0 - fy) + bottom * fy;

            out[(dy * dst_w + dx) as usize] = value.round().clamp(0.0, 255.0) as u8;
        }
    }
    out
}

/// Begrenzt eine Bildgröße auf `max_edge` an der langen Kante (mit
/// Rundung), Seitenverhältnis erhalten — für die Analyse-Auflösung der
/// KI-Masken (siehe `DECISIONS.md` ADR-0033 Punkt 3: „lange Kante auf
/// 512px begrenzt").
pub fn fit_within(width: u32, height: u32, max_edge: u32) -> (u32, u32) {
    let long_edge = width.max(height);
    if long_edge <= max_edge || long_edge == 0 {
        return (width, height);
    }
    let scale = max_edge as f32 / long_edge as f32;
    (
        ((width as f32 * scale).round() as u32).max(1),
        ((height as f32 * scale).round() as u32).max(1),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_resize_returns_input_unchanged() {
        let src = vec![10, 20, 30, 40];
        let out = bilinear_resize_u8(&src, 2, 2, 2, 2);
        assert_eq!(out, src);
    }

    #[test]
    fn uniform_input_stays_uniform_after_resize() {
        let src = vec![128u8; 4]; // 2x2
        let out = bilinear_resize_u8(&src, 2, 2, 8, 8);
        assert_eq!(out.len(), 64);
        assert!(out.iter().all(|&v| v == 128));
    }

    #[test]
    fn empty_source_yields_all_zero_output_instead_of_panicking() {
        let out = bilinear_resize_u8(&[], 0, 0, 4, 4);
        assert_eq!(out, vec![0u8; 16]);
    }

    #[test]
    fn fit_within_preserves_aspect_ratio_and_caps_long_edge() {
        let (w, h) = fit_within(4000, 2000, 512);
        assert_eq!(w, 512);
        assert_eq!(h, 256);
    }

    #[test]
    fn fit_within_is_a_no_op_when_already_smaller() {
        assert_eq!(fit_within(100, 80, 512), (100, 80));
    }
}
