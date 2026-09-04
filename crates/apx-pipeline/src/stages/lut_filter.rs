//! Filter-/LUT-Bibliothek (Phase 16 Schritt 1, siehe `DECISIONS.md`
//! ADR-0043). Wendet ein per `lut_cube::parse_cube_bytes` geparstes und
//! im EDL abgelegtes 3D-`.cube`-Raster ([`crate::edl::LutFilterData`])
//! per trilinearer Interpolation auf das bereits fertig entwickelte
//! sRGB-RGBA8-Bild an — läuft in `develop::render_rgba8`s fester Kette
//! nach `sky_replace`, vor `liquify` (siehe `edl::v4::StageEnabled::
//! lut_filter`s Moduldoku für die Begründung der Position: letzte
//! Farb-Stufe vor den rein geometrischen/verformenden Stufen, wie ein
//! abschließender "Look"-Pass in professionellen Grading-Werkzeugen).
//!
//! **`strength` blendet linear** zwischen unverändertem und vollem
//! LUT-Ergebnis, dieselbe Deckkraft-Konvention wie `stages::
//! style_transfer`/`stages::skin_smoothing`.
//!
//! **Trilineare statt tetraedrische Interpolation**: die einfachere,
//! auch von vielen Referenzimplementierungen (u. a. `pycubelut`)
//! genutzte Variante — für "Foto-Filter/Looks" (im Gegensatz zu
//! technischer Farbraum-Konvertierung) ist der Unterschied zur
//! aufwendigeren tetraedrischen Interpolation visuell nicht relevant.

use crate::edl::v4::LutFilterAdjustment;

/// Interpoliert einen einzelnen Farbwert `(r, g, b)` (jeweils `0.0..=1.0`,
/// bereits auf den LUT-Wertebereich normiert) trilinear aus `table`.
fn sample_lut(table: &[f32], size: u32, r: f32, g: f32, b: f32) -> [f32; 3] {
    let n = size.max(2);
    let max_idx = n - 1;
    let scale = max_idx as f32;

    let rf = r.clamp(0.0, 1.0) * scale;
    let gf = g.clamp(0.0, 1.0) * scale;
    let bf = b.clamp(0.0, 1.0) * scale;

    let r0 = rf.floor() as u32;
    let g0 = gf.floor() as u32;
    let b0 = bf.floor() as u32;
    let r1 = (r0 + 1).min(max_idx);
    let g1 = (g0 + 1).min(max_idx);
    let b1 = (b0 + 1).min(max_idx);

    let fr = rf - r0 as f32;
    let fg = gf - g0 as f32;
    let fb = bf - b0 as f32;

    let at = |ri: u32, gi: u32, bi: u32, ch: usize| -> f32 {
        let idx = (((bi * n + gi) * n + ri) as usize) * 3 + ch;
        table[idx]
    };

    let mut out = [0.0f32; 3];
    for (ch, slot) in out.iter_mut().enumerate() {
        let c000 = at(r0, g0, b0, ch);
        let c100 = at(r1, g0, b0, ch);
        let c010 = at(r0, g1, b0, ch);
        let c110 = at(r1, g1, b0, ch);
        let c001 = at(r0, g0, b1, ch);
        let c101 = at(r1, g0, b1, ch);
        let c011 = at(r0, g1, b1, ch);
        let c111 = at(r1, g1, b1, ch);

        let c00 = c000 + (c100 - c000) * fr;
        let c10 = c010 + (c110 - c010) * fr;
        let c01 = c001 + (c101 - c001) * fr;
        let c11 = c011 + (c111 - c011) * fr;

        let c0 = c00 + (c10 - c00) * fg;
        let c1 = c01 + (c11 - c01) * fg;

        *slot = c0 + (c1 - c0) * fb;
    }
    out
}

/// Wendet `adjustment` auf `base` (RGBA8, `width * height * 4` Bytes) an
/// — unverändert durchgereicht, solange kein Filter gewählt ist, dessen
/// Rasterdaten zu klein/unvollständig sind, oder `strength` bei `0.0`
/// steht (siehe Moduldoku).
pub fn apply(base: &[u8], width: u32, height: u32, adjustment: &LutFilterAdjustment) -> Vec<u8> {
    let Some(lut) = &adjustment.lut else {
        return base.to_vec();
    };
    let strength = adjustment.strength.clamp(0.0, 1.0);
    let expected_len = (lut.size as usize).pow(3) * 3;
    if strength <= 0.0 || lut.size < 2 || lut.table.len() < expected_len {
        return base.to_vec();
    }

    let dmin = lut.domain_min;
    let dspan = [
        (lut.domain_max[0] - dmin[0]).max(1e-6),
        (lut.domain_max[1] - dmin[1]).max(1e-6),
        (lut.domain_max[2] - dmin[2]).max(1e-6),
    ];

    let n = (width as usize) * (height as usize);
    let mut out = base.to_vec();
    for i in 0..n.min(base.len() / 4) {
        let px = i * 4;
        let r = base[px] as f32 / 255.0;
        let g = base[px + 1] as f32 / 255.0;
        let b = base[px + 2] as f32 / 255.0;

        let nr = ((r - dmin[0]) / dspan[0]).clamp(0.0, 1.0);
        let ng = ((g - dmin[1]) / dspan[1]).clamp(0.0, 1.0);
        let nb = ((b - dmin[2]) / dspan[2]).clamp(0.0, 1.0);

        let looked_up = sample_lut(&lut.table, lut.size, nr, ng, nb);

        out[px] = ((r + (looked_up[0] - r) * strength).clamp(0.0, 1.0) * 255.0).round() as u8;
        out[px + 1] = ((g + (looked_up[1] - g) * strength).clamp(0.0, 1.0) * 255.0).round() as u8;
        out[px + 2] = ((b + (looked_up[2] - b) * strength).clamp(0.0, 1.0) * 255.0).round() as u8;
        // Alpha bleibt unverändert (`out[px + 3]`).
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::edl::v4::LutFilterData;

    /// Kantenlänge-2-Identitäts-LUT: jeder Punkt bildet auf sich selbst
    /// ab, also darf `apply` bei voller Stärke am Bild nichts ändern.
    fn identity_lut_2() -> LutFilterData {
        let mut table = Vec::with_capacity(8 * 3);
        for b in 0..2u32 {
            for g in 0..2u32 {
                for r in 0..2u32 {
                    table.push(r as f32);
                    table.push(g as f32);
                    table.push(b as f32);
                }
            }
        }
        LutFilterData {
            name: "Identity".to_string(),
            size: 2,
            table,
            domain_min: [0.0, 0.0, 0.0],
            domain_max: [1.0, 1.0, 1.0],
        }
    }

    /// Invertiert jeden Kanal (`v -> 1 - v`) auf einem 2er-Raster.
    fn invert_lut_2() -> LutFilterData {
        let mut table = Vec::with_capacity(8 * 3);
        for b in 0..2u32 {
            for g in 0..2u32 {
                for r in 0..2u32 {
                    table.push(1.0 - r as f32);
                    table.push(1.0 - g as f32);
                    table.push(1.0 - b as f32);
                }
            }
        }
        LutFilterData {
            name: "Invert".to_string(),
            size: 2,
            table,
            domain_min: [0.0, 0.0, 0.0],
            domain_max: [1.0, 1.0, 1.0],
        }
    }

    #[test]
    fn no_lut_is_a_no_op() {
        let base = vec![10, 20, 30, 255, 200, 100, 50, 128];
        let adjustment = LutFilterAdjustment::NEUTRAL;
        let out = apply(&base, 2, 1, &adjustment);
        assert_eq!(out, base);
    }

    #[test]
    fn zero_strength_is_a_no_op() {
        let base = vec![10, 20, 30, 255];
        let adjustment = LutFilterAdjustment {
            strength: 0.0,
            lut: Some(invert_lut_2()),
        };
        let out = apply(&base, 1, 1, &adjustment);
        assert_eq!(out, base);
    }

    #[test]
    fn identity_lut_leaves_pixels_unchanged() {
        let base = vec![10, 20, 30, 255, 200, 100, 50, 128];
        let adjustment = LutFilterAdjustment {
            strength: 1.0,
            lut: Some(identity_lut_2()),
        };
        let out = apply(&base, 2, 1, &adjustment);
        // Trilineare Interpolation einer echten Identität ist exakt,
        // Rundung auf u8 kann höchstens +-1 abweichen.
        for (a, b) in out.iter().zip(base.iter()) {
            assert!((*a as i32 - *b as i32).abs() <= 1);
        }
    }

    #[test]
    fn invert_lut_at_full_strength_inverts_rgb_not_alpha() {
        let base = vec![0, 64, 255, 200];
        let adjustment = LutFilterAdjustment {
            strength: 1.0,
            lut: Some(invert_lut_2()),
        };
        let out = apply(&base, 1, 1, &adjustment);
        assert_eq!(out[0], 255);
        assert!((out[1] as i32 - 191).abs() <= 1);
        assert_eq!(out[2], 0);
        assert_eq!(out[3], 200); // Alpha unverändert
    }

    #[test]
    fn half_strength_blends_halfway() {
        let base = vec![0, 0, 0, 255];
        let adjustment = LutFilterAdjustment {
            strength: 0.5,
            lut: Some(invert_lut_2()),
        };
        let out = apply(&base, 1, 1, &adjustment);
        // 0 -> 255 bei voller Stärke, also ~128 bei halber.
        assert!((out[0] as i32 - 128).abs() <= 2);
    }

    #[test]
    fn undersized_table_is_a_no_op() {
        let base = vec![10, 20, 30, 255];
        let mut lut = identity_lut_2();
        lut.table.truncate(4); // absichtlich zu wenig Daten
        let adjustment = LutFilterAdjustment {
            strength: 1.0,
            lut: Some(lut),
        };
        let out = apply(&base, 1, 1, &adjustment);
        assert_eq!(out, base);
    }
}
