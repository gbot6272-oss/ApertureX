//! Orchestriert den kompletten Entwickeln-Rendervorgang: von linearen
//! Kamera-RGB-Pixeln (`apx_raw::decode_linear`) über die Weißabgleich-
//! Gain-Berechnung und die sieben Regler (`crate::stages`) bis zum
//! fertigen, anzeigefähigen RGBA8-Bildpuffer (`crate::color`) — der
//! einzige Einstiegspunkt, den `apx-app`s Protokoll-Route für
//! `develop/...` aufruft (siehe `PLAN.md` Phase 2 Schritt 5). `apx-app`
//! kennt damit weder `crate::stages` noch `crate::color` einzeln — reine
//! Verdrahtung bleibt reine Verdrahtung (`ARCHITECTURE.md` §4).

use std::borrow::Cow;

use apx_raw::LinearImage;

use crate::color::linear_camera_rgb_to_srgb_rgba8;
use crate::edl::{
    CalibrationAdjustment, ColorGradingAdjustment, CurvesAdjustment, DetailsAdjustment, EdlV2,
    EffectsAdjustment, GeometryAdjustment, HslAdjustment,
};
use crate::error::Result;
use crate::gpu::GpuContext;
use crate::stages::{
    basic_fused, calibration, color_grading, curves, details, effects, geometry, hsl_color_mixer,
    lens_corrections, local_contrast, white_balance,
};

/// Das Ergebnis von [`render_rgba8`] — `width`/`height` beschreiben
/// `pixels`s tatsächliche Größe, die durch Geometrie/Zuschnitt
/// (`stages::geometry`) von `linear.width`/`linear.height` abweichen
/// kann (der einzige Schritt in Phase 4, der die Ausgabegröße ändert,
/// siehe `geometry.rs`s Moduldoku) — Aufrufer dürfen sich NICHT mehr auf
/// `linear.width`/`linear.height` für die Puffergröße verlassen.
pub struct RenderedImage {
    pub width: u32,
    pub height: u32,
    /// Interleaved RGBA8, `4 * width * height` Bytes, Alpha immer `255`.
    pub pixels: Vec<u8>,
}

/// Rendert `linear` mit den in `edl` beschriebenen Anpassungen zu einem
/// [`RenderedImage`].
///
/// Nutzt `ctx`, falls vorhanden — schlägt die GPU-Ausführung dennoch fehl
/// (z. B. Treiberfehler zur Laufzeit), fällt diese Funktion automatisch
/// auf den rayon-CPU-Pfad zurück, statt den ganzen Aufruf scheitern zu
/// lassen (siehe `SPEC.md` §2.2 „GPU→CPU-Fallback muss existieren",
/// `DECISIONS.md` ADR-0012) — der Aufrufer muss diese Entscheidung nicht
/// selbst treffen.
///
/// **Phase-4-Übergangsstand (siehe `PLAN.md` Phase 4 Schritt 11):**
/// Kalibrierung (`stages::calibration`, läuft *vor* Weißabgleich/den
/// Grundeinstellungen — siehe `calibration.rs`s Moduldoku), alle zwölf
/// Grundeinstellungs-Regler (`stages::basic_fused` /
/// `stages::local_contrast`), Details/Schärfung+Rauschreduzierung
/// (`stages::details`, läuft direkt nach Textur/Klarheit — siehe
/// `details.rs`s Moduldoku), HSL + Farbmischer erweitert
/// (`stages::hsl_color_mixer`), Color Grading (`stages::color_grading`,
/// alle im linearen Arbeitsraum wie `basic_fused`), Objektivkorrekturen
/// (`stages::lens_corrections`, läuft nach Color Grading — siehe
/// `lens_corrections.rs`s Moduldoku für die geometrische Abbildung) und
/// Effekte (`stages::effects`, Vignettierung + Körnung, läuft direkt
/// danach, ebenfalls noch vor der Farbraum-Konvertierung) sowie die
/// Gradationskurven (`stages::curves`, laufen bewusst *nach* der
/// Farbraum-Konvertierung auf dem fertigen RGBA8-Puffer) und Geometrie
/// (`stages::geometry`, Drehung + Zuschnitt — der einzige Schritt, der
/// die Ausgabegröße ändert, siehe `geometry.rs`s Moduldoku und
/// [`RenderedImage`] — läuft als allerletzter Schritt) sind verdrahtet.
/// Reparatur ist noch inert — der letzte Schritt verdrahtet es.
pub fn render_rgba8(
    ctx: Option<&GpuContext>,
    linear: &LinearImage,
    edl: &EdlV2,
) -> Result<RenderedImage> {
    let basic = &edl.basic;
    let wb_gains = white_balance::compute_gains(linear.as_shot_wb_coeffs, basic.white_balance);

    let calibrated: Cow<[f32]> = if edl.calibration == CalibrationAdjustment::NEUTRAL {
        // Kein zusätzlicher Durchlauf, wenn Kalibrierung neutral steht
        // (Regelfall).
        Cow::Borrowed(&linear.pixels)
    } else {
        Cow::Owned(match ctx {
            Some(ctx) => match calibration::apply_gpu(ctx, &linear.pixels, &edl.calibration) {
                Ok(pixels) => pixels,
                Err(err) => {
                    tracing::warn!(error = %err, "GPU-Rendering (Kalibrierung) fehlgeschlagen, nutze CPU-Fallback");
                    calibration::apply_cpu(&linear.pixels, &edl.calibration)
                }
            },
            None => calibration::apply_cpu(&linear.pixels, &edl.calibration),
        })
    };

    let tonal = match ctx {
        Some(ctx) => match basic_fused::apply_gpu(ctx, &calibrated, wb_gains, basic) {
            Ok(pixels) => pixels,
            Err(err) => {
                tracing::warn!(error = %err, "GPU-Rendering fehlgeschlagen, nutze CPU-Fallback");
                basic_fused::apply_cpu(&calibrated, wb_gains, basic)
            }
        },
        None => basic_fused::apply_cpu(&calibrated, wb_gains, basic),
    };

    let textured = if basic.texture == 0.0 && basic.clarity == 0.0 {
        // Kein zusätzlicher Durchlauf, wenn beide Regler neutral stehen —
        // spart den vollen Nachbarschafts-Dispatch im Regelfall.
        tonal
    } else {
        match ctx {
            Some(ctx) => match local_contrast::apply_gpu(
                ctx,
                &tonal,
                linear.width,
                linear.height,
                basic.texture,
                basic.clarity,
            ) {
                Ok(pixels) => pixels,
                Err(err) => {
                    tracing::warn!(error = %err, "GPU-Rendering (Textur/Klarheit) fehlgeschlagen, nutze CPU-Fallback");
                    local_contrast::apply_cpu(
                        &tonal,
                        linear.width,
                        linear.height,
                        basic.texture,
                        basic.clarity,
                    )
                }
            },
            None => local_contrast::apply_cpu(
                &tonal,
                linear.width,
                linear.height,
                basic.texture,
                basic.clarity,
            ),
        }
    };

    let detailed = if edl.details == DetailsAdjustment::NEUTRAL {
        // Kein zusätzlicher Durchlauf, wenn Details (Schärfung +
        // Rauschreduzierung) neutral steht (Regelfall).
        textured
    } else {
        match ctx {
            Some(ctx) => {
                match details::apply_gpu(ctx, &textured, linear.width, linear.height, &edl.details)
                {
                    Ok(pixels) => pixels,
                    Err(err) => {
                        tracing::warn!(error = %err, "GPU-Rendering (Details) fehlgeschlagen, nutze CPU-Fallback");
                        details::apply_cpu(&textured, linear.width, linear.height, &edl.details)
                    }
                }
            }
            None => details::apply_cpu(&textured, linear.width, linear.height, &edl.details),
        }
    };

    let hsl_shifted = if edl.hsl == HslAdjustment::NEUTRAL && edl.color_mixer.regions.is_empty() {
        // Kein zusätzlicher Durchlauf, wenn weder HSL noch Farbmischer
        // etwas zu tun haben (Regelfall).
        detailed
    } else {
        match ctx {
            Some(ctx) => {
                match hsl_color_mixer::apply_gpu(ctx, &detailed, &edl.hsl, &edl.color_mixer) {
                    Ok(pixels) => pixels,
                    Err(err) => {
                        tracing::warn!(error = %err, "GPU-Rendering (HSL/Farbmischer) fehlgeschlagen, nutze CPU-Fallback");
                        hsl_color_mixer::apply_cpu(&detailed, &edl.hsl, &edl.color_mixer)
                    }
                }
            }
            None => hsl_color_mixer::apply_cpu(&detailed, &edl.hsl, &edl.color_mixer),
        }
    };

    let graded = if edl.color_grading == ColorGradingAdjustment::NEUTRAL {
        // Kein zusätzlicher Durchlauf, wenn alle vier Farbräder neutral
        // stehen (Regelfall).
        hsl_shifted
    } else {
        match ctx {
            Some(ctx) => match color_grading::apply_gpu(ctx, &hsl_shifted, &edl.color_grading) {
                Ok(pixels) => pixels,
                Err(err) => {
                    tracing::warn!(error = %err, "GPU-Rendering (Color Grading) fehlgeschlagen, nutze CPU-Fallback");
                    color_grading::apply_cpu(&hsl_shifted, &edl.color_grading)
                }
            },
            None => color_grading::apply_cpu(&hsl_shifted, &edl.color_grading),
        }
    };

    let lens_corrected = {
        let params = lens_corrections::LensCorrectionParams::new(
            linear.width,
            linear.height,
            &edl.lens_corrections,
        );
        if params.is_identity() {
            // Kein zusätzlicher Durchlauf, wenn Objektivkorrekturen (nach
            // Auflösung von Profil/Guided-Linien) keine Wirkung hätten
            // (Regelfall).
            graded
        } else {
            match ctx {
                Some(ctx) => match lens_corrections::apply_gpu(
                    ctx,
                    &graded,
                    linear.width,
                    linear.height,
                    &edl.lens_corrections,
                ) {
                    Ok(pixels) => pixels,
                    Err(err) => {
                        tracing::warn!(error = %err, "GPU-Rendering (Objektivkorrekturen) fehlgeschlagen, nutze CPU-Fallback");
                        lens_corrections::apply_cpu(
                            &graded,
                            linear.width,
                            linear.height,
                            &edl.lens_corrections,
                        )
                    }
                },
                None => lens_corrections::apply_cpu(
                    &graded,
                    linear.width,
                    linear.height,
                    &edl.lens_corrections,
                ),
            }
        }
    };

    let effected = if edl.effects == EffectsAdjustment::NEUTRAL {
        // Kein zusätzlicher Durchlauf, wenn Vignettierung und Körnung
        // beide neutral stehen (Regelfall).
        lens_corrected
    } else {
        match ctx {
            Some(ctx) => match effects::apply_gpu(
                ctx,
                &lens_corrected,
                linear.width,
                linear.height,
                &edl.effects,
            ) {
                Ok(pixels) => pixels,
                Err(err) => {
                    tracing::warn!(error = %err, "GPU-Rendering (Effekte) fehlgeschlagen, nutze CPU-Fallback");
                    effects::apply_cpu(&lens_corrected, linear.width, linear.height, &edl.effects)
                }
            },
            None => effects::apply_cpu(&lens_corrected, linear.width, linear.height, &edl.effects),
        }
    };

    let rgba = linear_camera_rgb_to_srgb_rgba8(&effected, linear.cam_to_srgb);

    let curved = if edl.curves == CurvesAdjustment::neutral() {
        // Kein zusätzlicher Durchlauf über den ganzen Puffer, wenn alle
        // fünf Kurven neutral stehen (Regelfall).
        rgba
    } else {
        curves::apply_rgba8(&rgba, &edl.curves)
    };

    let (width, height, pixels) = if edl.geometry == GeometryAdjustment::NEUTRAL {
        // Kein zusätzlicher Durchlauf, wenn weder Drehung noch Zuschnitt
        // etwas zu tun haben (Regelfall).
        (linear.width, linear.height, curved)
    } else {
        geometry::apply(&curved, linear.width, linear.height, &edl.geometry)
    };

    Ok(RenderedImage {
        width,
        height,
        pixels,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::edl::{BasicAdjustments, EdlV2, WhiteBalanceAdjustment};

    const IDENTITY: [[f32; 3]; 3] = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];

    fn flat_gray_linear_image(value: f32) -> LinearImage {
        LinearImage {
            width: 2,
            height: 2,
            pixels: vec![value; 2 * 2 * 3],
            as_shot_wb_coeffs: [1.0, 1.0, 1.0, 1.0],
            cam_to_srgb: IDENTITY,
        }
    }

    #[test]
    fn neutral_edl_produces_correctly_sized_opaque_output() {
        let linear = flat_gray_linear_image(0.5);
        let rendered = render_rgba8(None, &linear, &EdlV2::neutral()).expect("sollte rendern");
        assert_eq!(rendered.width, 2);
        assert_eq!(rendered.height, 2);
        assert_eq!(rendered.pixels.len(), 2 * 2 * 4);
        for pixel in rendered.pixels.chunks_exact(4) {
            assert_eq!(pixel[3], 255, "Alpha muss immer undurchsichtig sein");
        }
    }

    #[test]
    fn negative_exposure_darkens_output() {
        let linear = flat_gray_linear_image(0.5);
        let neutral = render_rgba8(None, &linear, &EdlV2::neutral()).expect("rendern");
        let darker_edl = EdlV2 {
            basic: BasicAdjustments {
                exposure_ev: -2.0,
                ..BasicAdjustments::NEUTRAL
            },
            ..EdlV2::neutral()
        };
        let darker = render_rgba8(None, &linear, &darker_edl).expect("rendern");
        assert!(
            darker.pixels[0] < neutral.pixels[0],
            "negative Belichtung sollte den Rot-Kanal absenken (neutral={}, darker={})",
            neutral.pixels[0],
            darker.pixels[0]
        );
    }

    #[test]
    fn gpu_context_produces_same_result_as_cpu_fallback() {
        let ctx = match GpuContext::new_blocking() {
            Ok(ctx) => ctx,
            Err(_) => {
                eprintln!("übersprungen: kein GPU-Adapter in dieser Umgebung verfügbar");
                return;
            }
        };
        let linear = flat_gray_linear_image(0.4);
        let edl = EdlV2 {
            basic: BasicAdjustments {
                exposure_ev: 0.3,
                contrast: 15.0,
                ..BasicAdjustments::NEUTRAL
            },
            ..EdlV2::neutral()
        };
        let cpu = render_rgba8(None, &linear, &edl).expect("CPU-Rendering");
        let gpu = render_rgba8(Some(&ctx), &linear, &edl).expect("GPU-Rendering");
        assert_eq!((cpu.width, cpu.height), (gpu.width, gpu.height));
        for (c, g) in cpu.pixels.iter().zip(gpu.pixels.iter()) {
            // Toleranz von 1, da CPU/GPU getrennt auf f32 runden, bevor
            // hier auf u8 quantisiert wird.
            assert!((*c as i16 - *g as i16).abs() <= 1, "CPU={c} GPU={g}");
        }
    }

    /// Ehrliche Performance-Messung für das 16-ms-Ziel (`SPEC.md` §2.4,
    /// `PLAN.md` Phase 2 Schritt 7): misst nur `render_rgba8` selbst (der
    /// Teil, der bei *jedem* Regler-Tick läuft — der teure Dekodier-Schritt
    /// läuft dank `TileCache` nur einmal pro Foto, siehe `tile_cache.rs`
    /// und `crates/apx-app/src/protocol/mod.rs`s `compute_develop`), auf
    /// einem synthetischen Bild bei `STANDARD_EDGE`-ähnlicher Auflösung
    /// (2048×1365, 3:2-Seitenverhältnis). Ausgabe nur mit `--nocapture`
    /// sichtbar; die generöse Zeitschranke unten ist ein
    /// Regressionswächter gegen eine grobe Verlangsamung, keine scharfe
    /// Behauptung über das 16-ms-Ziel selbst — dafür fehlt in dieser
    /// Sandbox eine echte Fenster-/IPC-/Compositing-Umgebung (siehe
    /// `DECISIONS.md`, Ehrlichkeits-Hinweis unten).
    #[test]
    fn render_rgba8_timing_on_synthetic_standard_edge_image() {
        let ctx = GpuContext::new_blocking().ok();
        if ctx.is_none() {
            eprintln!("übersprungen: kein GPU-Adapter in dieser Umgebung verfügbar");
        }

        let width = 2048;
        let height = 1365;
        let pixels = crate::test_support::gray_gradient((width * height) as usize);
        let linear = LinearImage {
            width,
            height,
            pixels,
            as_shot_wb_coeffs: [1.05, 1.0, 0.9, 1.0],
            cam_to_srgb: IDENTITY,
        };
        let edl = EdlV2 {
            basic: BasicAdjustments {
                exposure_ev: 0.4,
                contrast: 15.0,
                highlights: -10.0,
                shadows: 10.0,
                whites: 5.0,
                blacks: -5.0,
                white_balance: WhiteBalanceAdjustment {
                    temp_shift_kelvin: 200.0,
                    tint_shift: -5.0,
                },
                ..BasicAdjustments::NEUTRAL
            },
            ..EdlV2::neutral()
        };

        if let Some(ctx) = &ctx {
            let started = std::time::Instant::now();
            let _ = render_rgba8(Some(ctx), &linear, &edl).expect("GPU-Rendering");
            let elapsed = started.elapsed();
            eprintln!(
                "render_rgba8 (GPU, {width}x{height}, Adapter '{}'): {:.2} ms",
                ctx.adapter_info.name,
                elapsed.as_secs_f64() * 1000.0
            );
            // Sehr großzügige Schranke (kein hartes 16-ms-Versprechen,
            // siehe Doku oben) — soll nur eine eklatante Regression
            // fangen, nicht auf dieser Sandbox-Hardware kalibriert sein.
            assert!(
                elapsed.as_millis() < 2000,
                "GPU-Rendering ungewöhnlich langsam: {elapsed:?}"
            );
        }

        let started = std::time::Instant::now();
        let _ = render_rgba8(None, &linear, &edl);
        let elapsed = started.elapsed();
        eprintln!(
            "render_rgba8 (CPU-Fallback, {width}x{height}): {:.2} ms",
            elapsed.as_secs_f64() * 1000.0
        );
        assert!(
            elapsed.as_millis() < 2000,
            "CPU-Fallback ungewöhnlich langsam: {elapsed:?}"
        );
    }
}
