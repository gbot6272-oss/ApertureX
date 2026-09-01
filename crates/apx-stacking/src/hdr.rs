//! HDR-Zusammenführung (Phase 9 Schritt 8, siehe `PLAN.md`/`DECISIONS.md`
//! ADR-0035 Punkt 2) — mehrere unterschiedlich belichtete Aufnahmen
//! desselben (bereits ausgerichteten) Motivs zu einem Bild mit größerem
//! Dynamikumfang zusammenführen: eine gewichtete Fusion im linearen
//! Farbraum nach Belichtungszeit (Debevec-artig), anschließend ein
//! klassischer Tonemap-Operator (Reinhard-artig) zurück auf den
//! darstellbaren Bereich.
//!
//! **Voraussetzung**: wie bei `focus::focus_stack_rgba8` bereits
//! ausgerichtete Aufnahmen gleicher Größe (Stativ-Belichtungsreihe).

use crate::error::{Result, StackingError};

/// Eine einzelne Belichtung für die Fusion.
pub struct Exposure<'a> {
    /// RGBA8-Pixel, `width * height * 4` Bytes.
    pub pixels: &'a [u8],
    /// Belichtungszeit in Sekunden (aus EXIF `shutter`) — muss positiv
    /// sein.
    pub exposure_seconds: f32,
}

/// sRGB-Byte (`0..=255`) → ungefähr linear (`^2.2`-Näherung, dieselbe wie
/// die Weißabgleich-Pipette/Auto-Ton in `frontend/src/lib/
/// whiteBalancePicker.ts`/`autoTone.ts` — kein exaktes sRGB-Transfer-
/// polynom, aber für Belichtungsfusion ausreichend).
fn srgb_byte_to_approx_linear(byte: u8) -> f32 {
    (byte as f32 / 255.0).powf(2.2)
}

/// Ungefähr linear → sRGB-Byte (Umkehrung von oben).
fn approx_linear_to_srgb_byte(linear: f32) -> u8 {
    (linear.clamp(0.0, 1.0).powf(1.0 / 2.2) * 255.0).round() as u8
}

/// Debevec-artiges Dreiecksgewicht auf dem rohen sRGB-Byte: Pixel nahe
/// der Mitte (gut belichtet) zählen am meisten, Pixel nahe 0/255
/// (unter-/überbelichtet, ggf. geklemmt) zählen kaum — dieselbe
/// Grundidee wie Debevec & Malik 1997, ohne dessen volle
/// Kamera-Antwortfunktions-Schätzung (hier wird stattdessen ein fester
/// `^2.2`-Näherungswert für die Linearisierung angenommen statt ihn aus
/// den Belichtungen selbst zu rekonstruieren — eine bewusste
/// Vereinfachung, siehe Moduldoku).
fn debevec_weight(byte: u8) -> f32 {
    let z = byte as f32;
    let zmin = 0.0;
    let zmax = 255.0;
    if z <= (zmin + zmax) / 2.0 {
        z - zmin
    } else {
        zmax - z
    }
    .max(1.0) // nie exakt 0, damit ein einzelnes komplett geklemmtes Pixel nicht ganz aus der Summe fällt
}

/// Reinhard-artiger Tonemap-Operator (`L / (1 + L)`) — der einfachste
/// klassische globale Operator, angewendet je Kanal (keine
/// Luminanz-getrennte Farb-erhaltende Variante).
fn reinhard_tonemap(linear: f32) -> f32 {
    linear / (1.0 + linear)
}

/// Führt `exposures` zu einem Bild mit erweitertem Dynamikumfang zusammen.
pub fn hdr_merge_rgba8(exposures: &[Exposure], width: u32, height: u32) -> Result<Vec<u8>> {
    if exposures.len() < 2 {
        return Err(StackingError::TooFewImages {
            message: format!(
                "HDR-Zusammenführung braucht mindestens 2 Belichtungen, {} übergeben",
                exposures.len()
            ),
        });
    }
    let expected_len = (width as usize) * (height as usize) * 4;
    for (index, exposure) in exposures.iter().enumerate() {
        if exposure.pixels.len() != expected_len {
            return Err(StackingError::DimensionMismatch {
                message: format!(
                    "Belichtung {index} hat {} Bytes, erwartet wurden {expected_len} ({width}x{height} RGBA8)",
                    exposure.pixels.len()
                ),
            });
        }
        if !(exposure.exposure_seconds.is_finite() && exposure.exposure_seconds > 0.0) {
            return Err(StackingError::DimensionMismatch {
                message: format!(
                    "Belichtung {index} hat eine ungültige Belichtungszeit ({} s)",
                    exposure.exposure_seconds
                ),
            });
        }
    }

    let pixel_count = (width as usize) * (height as usize);
    let mut out = vec![0u8; expected_len];

    for pixel in 0..pixel_count {
        for channel in 0..3 {
            let byte_index = pixel * 4 + channel;
            let mut weighted_sum = 0.0f32;
            let mut weight_total = 0.0f32;
            let mut unweighted_sum = 0.0f32;
            for exposure in exposures {
                let byte = exposure.pixels[byte_index];
                let linear_radiance = srgb_byte_to_approx_linear(byte) / exposure.exposure_seconds;
                let weight = debevec_weight(byte);
                weighted_sum += weight * linear_radiance;
                weight_total += weight;
                unweighted_sum += linear_radiance;
            }
            let radiance = if weight_total > 0.0 {
                weighted_sum / weight_total
            } else {
                unweighted_sum / exposures.len() as f32
            };
            out[byte_index] = approx_linear_to_srgb_byte(reinhard_tonemap(radiance));
        }
        out[pixel * 4 + 3] = 255;
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn solid(width: u32, height: u32, r: u8, g: u8, b: u8) -> Vec<u8> {
        let mut pixels = vec![0u8; (width * height * 4) as usize];
        for chunk in pixels.chunks_exact_mut(4) {
            chunk[0] = r;
            chunk[1] = g;
            chunk[2] = b;
            chunk[3] = 255;
        }
        pixels
    }

    #[test]
    fn rejects_a_single_exposure() {
        let image = solid(4, 4, 100, 100, 100);
        let result = hdr_merge_rgba8(
            &[Exposure {
                pixels: &image,
                exposure_seconds: 0.01,
            }],
            4,
            4,
        );
        assert!(matches!(result, Err(StackingError::TooFewImages { .. })));
    }

    #[test]
    fn rejects_mismatched_dimensions() {
        let a = solid(4, 4, 100, 100, 100);
        let b = solid(2, 2, 100, 100, 100);
        let result = hdr_merge_rgba8(
            &[
                Exposure {
                    pixels: &a,
                    exposure_seconds: 0.01,
                },
                Exposure {
                    pixels: &b,
                    exposure_seconds: 0.04,
                },
            ],
            4,
            4,
        );
        assert!(matches!(
            result,
            Err(StackingError::DimensionMismatch { .. })
        ));
    }

    #[test]
    fn recovers_shadow_detail_lost_to_black_in_the_short_exposure() {
        // Kurz belichtet: eine dunkle Bildstelle wird zu reinem Schwarz
        // (0) — lange belichtet (4x): dieselbe Stelle ist gut sichtbar
        // (mittleres Grau). Die Fusion soll die Schattendetails aus der
        // langen Belichtung übernehmen statt beim Schwarz der kurzen zu
        // bleiben.
        let width = 4u32;
        let height = 4u32;
        let short = solid(width, height, 0, 0, 0);
        let long = solid(width, height, 130, 130, 130);
        let merged = hdr_merge_rgba8(
            &[
                Exposure {
                    pixels: &short,
                    exposure_seconds: 0.01,
                },
                Exposure {
                    pixels: &long,
                    exposure_seconds: 0.04,
                },
            ],
            width,
            height,
        )
        .expect("sollte fusionieren");
        assert!(
            merged[0] > 0,
            "Schattendetail aus der langen Belichtung sollte durchkommen (bekam {})",
            merged[0]
        );
    }

    #[test]
    fn recovers_highlight_detail_lost_to_white_in_the_long_exposure() {
        // Umgekehrter Fall: lange Belichtung klemmt eine helle Stelle auf
        // 255, kurze Belichtung zeigt sie gut belichtet.
        let width = 4u32;
        let height = 4u32;
        let long = solid(width, height, 255, 255, 255);
        let short = solid(width, height, 120, 120, 120);
        let merged = hdr_merge_rgba8(
            &[
                Exposure {
                    pixels: &long,
                    exposure_seconds: 0.04,
                },
                Exposure {
                    pixels: &short,
                    exposure_seconds: 0.01,
                },
            ],
            width,
            height,
        )
        .expect("sollte fusionieren");
        assert!(
            merged[0] < 255,
            "Lichterdetail aus der kurzen Belichtung sollte durchkommen (bekam {})",
            merged[0]
        );
    }

    #[test]
    fn alpha_channel_stays_opaque() {
        let width = 3u32;
        let height = 3u32;
        let a = solid(width, height, 10, 20, 30);
        let b = solid(width, height, 40, 50, 60);
        let merged = hdr_merge_rgba8(
            &[
                Exposure {
                    pixels: &a,
                    exposure_seconds: 0.01,
                },
                Exposure {
                    pixels: &b,
                    exposure_seconds: 0.02,
                },
            ],
            width,
            height,
        )
        .expect("sollte fusionieren");
        for pixel in merged.chunks_exact(4) {
            assert_eq!(pixel[3], 255);
        }
    }

    #[test]
    fn rejects_a_non_positive_exposure_time() {
        let a = solid(2, 2, 10, 10, 10);
        let b = solid(2, 2, 20, 20, 20);
        let result = hdr_merge_rgba8(
            &[
                Exposure {
                    pixels: &a,
                    exposure_seconds: 0.0,
                },
                Exposure {
                    pixels: &b,
                    exposure_seconds: 0.02,
                },
            ],
            2,
            2,
        );
        assert!(result.is_err());
    }
}
