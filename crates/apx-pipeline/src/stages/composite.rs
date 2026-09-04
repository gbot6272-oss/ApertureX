//! Mehrfachbelichtung/Layer-Compositing (Phase 14 Schritt 3, siehe
//! `DECISIONS.md` ADR-0041 — Lightroom Classic hat "keine klassischen
//! Ebenen-Kompositionsfähigkeiten wie Photoshop"). Legt beliebig viele
//! [`CompositeLayer`]s sequenziell über das bereits entwickelte,
//! **fertig entwickelte sRGB-RGBA8-Bild** — läuft in `develop::
//! render_rgba8`s fester Kette nach `curves`, vor `geometry` (siehe
//! `EdlV4::composite_layers`s Doku), im selben Farbraum wie Photoshops
//! eigene Ebenen-Überblendung (display-referred, nicht linear) — dieselbe
//! Erwartung, die Nutzer von einem "Multiplizieren"/"Ineinanderkopieren"-
//! Regler mitbringen.
//!
//! Wiederverwendet [`super::masks::blend_pixel`] unverändert (dieselben
//! fünf Blend-Modi wie die Masken-Stufe) — hier auf ganze Ebenen
//! angewendet statt nur auf lokale Masken-Anpassungen. Jede Ebene trägt
//! ihre Quellpixel bereits als fertige Bitmap (`CompositeLayerSource`,
//! siehe deren Doku für den Grund: diese Crate hat keinen Katalog-/
//! Dateisystemzugriff).

use apx_core::raster::bilinear_resize_u8;

use super::masks::blend_pixel;
use crate::edl::v4::CompositeLayer;

/// Zerlegt eine interleaved-RGB-`u8`-Bitmap in drei Ein-Kanal-Puffer —
/// dieselbe Aufteilung wie `stages::repair`s private `split_patch_channels`
/// bzw. `stages::geometry`s `split_patch_rgb`, hier wieder separat
/// gehalten statt geteilt (kleine, in sich geschlossene Funktion, siehe
/// deren Begründung).
/// Photoshop-/W3C-Compositing-„Lum"-Gewichtung (Rec.601-Luminanzgewichte)
/// — eigene Kopie statt Wiederverwendung von `stages::masks::luminosity`
/// (`fn`-privat dort, dieselbe "jedes Modul reimplementiert seine
/// kleinen Helfer statt Sichtbarkeit querzuschneiden"-Konvention wie
/// überall sonst in diesem Projekt, siehe `SPEC.md` §6).
fn luminosity(c: [f32; 3]) -> f32 {
    0.3 * c[0] + 0.59 * c[1] + 0.11 * c[2]
}

fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    if edge1 <= edge0 {
        return if x < edge0 { 0.0 } else { 1.0 };
    }
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Feste Rampenbreite für Blend-If (Photoshop-Funktion, Phase 15
/// Schritt 2) — Photoshops einfacher Ein-Schieberegler-Modus schneidet
/// hart ab (nur Alt-Ziehen zum Split-Regler erzeugt einen weichen
/// Übergang); hier bewusst immer weich (fester Rampenbreite statt
/// zweier unabhängiger Regler pro Seite, siehe `DECISIONS.md`
/// ADR-0042) — vermeidet sichtbare Treppenstufen-Kanten im Ergebnis.
/// Die Rampe liegt jeweils *innerhalb* des ausgeblendeten Bereichs
/// (unterhalb von `shadow_cutoff`, oberhalb von `highlight_cutoff`),
/// deshalb bleiben die Neutralwerte `0.0`/`1.0` ein echtes No-Op.
const BLEND_IF_RAMP: f32 = 0.08;

/// Gewichtet `opacity` zusätzlich nach der Luminanz des Basis-Pixels
/// (Blend-If) — `1.0` bei Neutralwerten (`shadow_cutoff = 0.0`,
/// `highlight_cutoff = 1.0`), siehe [`BLEND_IF_RAMP`].
fn blend_if_weight(base_rgb: [f32; 3], shadow_cutoff: f32, highlight_cutoff: f32) -> f32 {
    let lum = luminosity(base_rgb);
    let shadow_weight = smoothstep(shadow_cutoff - BLEND_IF_RAMP, shadow_cutoff, lum);
    let highlight_weight =
        1.0 - smoothstep(highlight_cutoff, highlight_cutoff + BLEND_IF_RAMP, lum);
    shadow_weight * highlight_weight
}

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

/// Legt eine einzelne Ebene über `base` (RGBA8) — die Ebene wird auf
/// `scale * (width, height)` skaliert und um ihren normierten
/// Mittelpunkt (`offset_x`/`offset_y`) platziert; außerhalb ihrer
/// Grenzen (kleinere `scale` oder ein Versatz, der sie teilweise über
/// den Rand schiebt) bleibt `base` unverändert — dieselbe „nur innerhalb
/// blenden"-Logik wie ein Ebenen-Rechteck in jedem gängigen
/// Bildbearbeitungsprogramm.
fn composite_layer(base: &[u8], width: u32, height: u32, layer: &CompositeLayer) -> Vec<u8> {
    if !layer.visible || layer.opacity <= 0.0 {
        return base.to_vec();
    }
    let source = &layer.source;
    if source.bitmap_width == 0 || source.bitmap_height == 0 {
        return base.to_vec();
    }

    let scale = layer.scale.max(0.01);
    let layer_w = ((width as f32) * scale).round().max(1.0) as u32;
    let layer_h = ((height as f32) * scale).round().max(1.0) as u32;

    let (src_r, src_g, src_b) =
        split_rgb(&source.pixels, source.bitmap_width, source.bitmap_height);
    let layer_r = bilinear_resize_u8(
        &src_r,
        source.bitmap_width,
        source.bitmap_height,
        layer_w,
        layer_h,
    );
    let layer_g = bilinear_resize_u8(
        &src_g,
        source.bitmap_width,
        source.bitmap_height,
        layer_w,
        layer_h,
    );
    let layer_b = bilinear_resize_u8(
        &src_b,
        source.bitmap_width,
        source.bitmap_height,
        layer_w,
        layer_h,
    );

    let center_x = layer.offset_x * width as f32;
    let center_y = layer.offset_y * height as f32;
    let origin_x = (center_x - layer_w as f32 / 2.0).round() as i64;
    let origin_y = (center_y - layer_h as f32 / 2.0).round() as i64;

    let opacity = layer.opacity.clamp(0.0, 1.0);
    let w = width as usize;
    let mut out = base.to_vec();
    for y in 0..layer_h {
        let img_y = origin_y + y as i64;
        if img_y < 0 || img_y >= height as i64 {
            continue;
        }
        for x in 0..layer_w {
            let img_x = origin_x + x as i64;
            if img_x < 0 || img_x >= width as i64 {
                continue;
            }
            let dst = (img_y as usize * w + img_x as usize) * 4;
            let src_i = y as usize * layer_w as usize + x as usize;

            let base_rgb = [
                out[dst] as f32 / 255.0,
                out[dst + 1] as f32 / 255.0,
                out[dst + 2] as f32 / 255.0,
            ];
            let layer_rgb = [
                layer_r[src_i] as f32 / 255.0,
                layer_g[src_i] as f32 / 255.0,
                layer_b[src_i] as f32 / 255.0,
            ];
            let blended = blend_pixel(base_rgb, layer_rgb, layer.blend_mode);
            let blend_if = blend_if_weight(
                base_rgb,
                layer.blend_if_shadow_cutoff,
                layer.blend_if_highlight_cutoff,
            );
            let final_opacity = opacity * blend_if;
            for c in 0..3 {
                let value = base_rgb[c] + (blended[c] - base_rgb[c]) * final_opacity;
                out[dst + c] = (value.clamp(0.0, 1.0) * 255.0).round() as u8;
            }
            // Alpha bleibt unverändert (`out[dst + 3]` war bereits das
            // Basisbild-Alpha).
        }
    }
    out
}

/// Wendet alle `layers` nacheinander an (siehe Moduldoku) — die einzige
/// Funktion, die `develop::render_rgba8` aus diesem Modul aufruft.
pub fn apply_all(base: &[u8], width: u32, height: u32, layers: &[CompositeLayer]) -> Vec<u8> {
    let mut current = base.to_vec();
    for layer in layers {
        current = composite_layer(&current, width, height, layer);
    }
    current
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::edl::v3::BlendMode;
    use crate::edl::v4::CompositeLayerSource;

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

    fn neutral_layer(source: CompositeLayerSource) -> CompositeLayer {
        CompositeLayer {
            visible: true,
            blend_mode: BlendMode::Normal,
            opacity: 1.0,
            scale: 1.0,
            offset_x: 0.5,
            offset_y: 0.5,
            source,
            blend_if_shadow_cutoff: 0.0,
            blend_if_highlight_cutoff: 1.0,
        }
    }

    #[test]
    fn empty_layer_list_is_identity() {
        let base = flat_rgba(8, 8, 100);
        let result = apply_all(&base, 8, 8, &[]);
        assert_eq!(result, base);
    }

    #[test]
    fn invisible_layer_is_a_no_op() {
        let base = flat_rgba(8, 8, 100);
        let layer = CompositeLayer {
            visible: false,
            ..neutral_layer(CompositeLayerSource {
                bitmap_width: 1,
                bitmap_height: 1,
                pixels: vec![255, 0, 0],
            })
        };
        let result = apply_all(&base, 8, 8, std::slice::from_ref(&layer));
        assert_eq!(result, base);
    }

    #[test]
    fn full_opacity_normal_blend_at_full_scale_replaces_the_whole_canvas() {
        let base = flat_rgba(10, 10, 50);
        let layer = neutral_layer(CompositeLayerSource {
            bitmap_width: 1,
            bitmap_height: 1,
            pixels: vec![200, 200, 200],
        });
        let result = apply_all(&base, 10, 10, std::slice::from_ref(&layer));
        // Scale 1.0 deckt die gesamte Leinwand ab — jedes Pixel sollte
        // jetzt der Ebenenfarbe entsprechen (Normal-Blend, volle
        // Deckkraft).
        for chunk in result.chunks_exact(4) {
            assert_eq!(chunk[0], 200);
            assert_eq!(chunk[1], 200);
            assert_eq!(chunk[2], 200);
            assert_eq!(chunk[3], 255);
        }
    }

    #[test]
    fn a_small_scaled_layer_only_affects_its_own_region() {
        let base = flat_rgba(20, 20, 50);
        let layer = CompositeLayer {
            scale: 0.2,
            ..neutral_layer(CompositeLayerSource {
                bitmap_width: 1,
                bitmap_height: 1,
                pixels: vec![255, 0, 0],
            })
        };
        let result = apply_all(&base, 20, 20, std::slice::from_ref(&layer));
        // Bildmitte (unter der kleinen zentrierten Ebene): rot.
        let center_idx = ((10 * 20 + 10) * 4) as usize;
        assert_eq!(result[center_idx], 255);
        assert_eq!(result[center_idx + 1], 0);
        // Bildecke: unverändert.
        assert_eq!(&result[0..4], &[50, 50, 50, 255]);
    }

    #[test]
    fn half_opacity_blends_halfway_between_base_and_layer() {
        let base = flat_rgba(4, 4, 0);
        let layer = CompositeLayer {
            opacity: 0.5,
            ..neutral_layer(CompositeLayerSource {
                bitmap_width: 1,
                bitmap_height: 1,
                pixels: vec![200, 200, 200],
            })
        };
        let result = apply_all(&base, 4, 4, std::slice::from_ref(&layer));
        // Normal-Blend bei 50 % Deckkraft: Mittelwert aus Basis (0) und
        // Ebene (200), also 100.
        assert_eq!(result[0], 100);
    }

    #[test]
    fn blend_if_shadow_cutoff_hides_the_layer_over_dark_base_pixels() {
        let base = flat_rgba(4, 4, 10);
        let layer = CompositeLayer {
            blend_if_shadow_cutoff: 0.5,
            ..neutral_layer(CompositeLayerSource {
                bitmap_width: 1,
                bitmap_height: 1,
                pixels: vec![200, 200, 200],
            })
        };
        let result = apply_all(&base, 4, 4, std::slice::from_ref(&layer));
        // Die Basis-Luminanz (~0.04) liegt weit unter `shadow_cutoff`
        // (0.5) — die Ebene bleibt hier vollständig ausgeblendet.
        assert_eq!(result, base);
    }

    #[test]
    fn blend_if_highlight_cutoff_hides_the_layer_over_bright_base_pixels() {
        let base = flat_rgba(4, 4, 250);
        let layer = CompositeLayer {
            blend_if_highlight_cutoff: 0.5,
            ..neutral_layer(CompositeLayerSource {
                bitmap_width: 1,
                bitmap_height: 1,
                pixels: vec![10, 10, 10],
            })
        };
        let result = apply_all(&base, 4, 4, std::slice::from_ref(&layer));
        // Die Basis-Luminanz (~0.98) liegt weit über `highlight_cutoff`
        // (0.5) — die Ebene bleibt hier vollständig ausgeblendet.
        assert_eq!(result, base);
    }
}
