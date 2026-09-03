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
    CalibrationAdjustment, ColorGradingAdjustment, CurvesAdjustment, DetailsAdjustment, EdlV4,
    EffectsAdjustment, GeometryAdjustment, HslAdjustment, Treatment,
};
use crate::error::Result;
use crate::gpu::GpuContext;
use crate::stages::{
    basic_fused, bw_mixer, calibration, color_grading, composite, curves, details, effects,
    geometry, hsl_color_mixer, lens_corrections, local_contrast, masks, repair, virtual_aperture,
    white_balance,
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
/// **Phase 4 abgeschlossen (siehe `PLAN.md` Phase 4 Schritt 12):**
/// Reparatur (`stages::repair`, läuft als allererster Schritt auf den
/// unveränderten linearen Sensordaten — siehe `repair.rs`s Moduldoku),
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
///
/// **Phase 6 (siehe `PLAN.md` Phase 6 Schritt 2, `DECISIONS.md`
/// ADR-0032):** Masken (`stages::masks`) laufen direkt nach `effects`,
/// noch vor der Farbraum-Konvertierung — jede sichtbare Maske ist ein
/// eigener Durchlauf, der ihre eigenen ton-/farb-/detailbezogenen
/// Werkzeuge auf eine Kopie des aktuellen Bildzustands anwendet und
/// alpha-gewichtet zurückmischt (siehe `masks.rs`s Moduldoku für die
/// genaue Reihenfolge und die bewusst vereinfachte Kurven-Anwendung im
/// linearen Arbeitsraum).
pub fn render_rgba8(
    ctx: Option<&GpuContext>,
    linear: &LinearImage,
    edl: &EdlV4,
) -> Result<RenderedImage> {
    let stages = &edl.stage_enabled;
    let basic = &edl.basic;
    let wb_gains = white_balance::compute_gains(linear.as_shot_wb_coeffs, basic.white_balance);

    let repaired: Cow<[f32]> = if !stages.repair || edl.repair.is_empty() {
        // Kein zusätzlicher Durchlauf, wenn die Stufe deaktiviert ist
        // (Node-Editor) oder keine Reparatur-Striche vorhanden sind
        // (Regelfall).
        Cow::Borrowed(&linear.pixels)
    } else {
        Cow::Owned(match ctx {
            Some(ctx) => match repair::apply_gpu(
                ctx,
                &linear.pixels,
                linear.width,
                linear.height,
                &edl.repair,
            ) {
                Ok(pixels) => pixels,
                Err(err) => {
                    tracing::warn!(error = %err, "GPU-Rendering (Reparatur) fehlgeschlagen, nutze CPU-Fallback");
                    repair::apply_cpu(&linear.pixels, linear.width, linear.height, &edl.repair)
                }
            },
            None => repair::apply_cpu(&linear.pixels, linear.width, linear.height, &edl.repair),
        })
    };

    let calibrated: Cow<[f32]> = if !stages.calibration
        || edl.calibration == CalibrationAdjustment::NEUTRAL
    {
        // Kein zusätzlicher Durchlauf, wenn die Stufe deaktiviert ist
        // oder Kalibrierung neutral steht (Regelfall).
        Cow::Borrowed(&repaired)
    } else {
        Cow::Owned(match ctx {
            Some(ctx) => match calibration::apply_gpu(ctx, &repaired, &edl.calibration) {
                Ok(pixels) => pixels,
                Err(err) => {
                    tracing::warn!(error = %err, "GPU-Rendering (Kalibrierung) fehlgeschlagen, nutze CPU-Fallback");
                    calibration::apply_cpu(&repaired, &edl.calibration)
                }
            },
            None => calibration::apply_cpu(&repaired, &edl.calibration),
        })
    };

    let tonal = if !stages.basic {
        // Node-Editor: Stufe deaktiviert — reicht das kalibrierte Bild
        // unverändert durch, auch der Weißabgleich-Gain wird dann nicht
        // angewendet (ehrlich „diese Stufe komplett übersprungen", nicht
        // nur „Regler neutral").
        calibrated.into_owned()
    } else {
        match ctx {
            Some(ctx) => match basic_fused::apply_gpu(ctx, &calibrated, wb_gains, basic) {
                Ok(pixels) => pixels,
                Err(err) => {
                    tracing::warn!(error = %err, "GPU-Rendering fehlgeschlagen, nutze CPU-Fallback");
                    basic_fused::apply_cpu(&calibrated, wb_gains, basic)
                }
            },
            None => basic_fused::apply_cpu(&calibrated, wb_gains, basic),
        }
    };

    let textured = if !stages.local_contrast || (basic.texture == 0.0 && basic.clarity == 0.0) {
        // Kein zusätzlicher Durchlauf, wenn die Stufe deaktiviert ist
        // oder beide Regler neutral stehen (Regelfall) — spart den vollen
        // Nachbarschafts-Dispatch.
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

    let detailed = if !stages.details || edl.details == DetailsAdjustment::NEUTRAL {
        // Kein zusätzlicher Durchlauf, wenn die Stufe deaktiviert ist
        // oder Details (Schärfung + Rauschreduzierung) neutral steht
        // (Regelfall).
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

    let hsl_shifted = if !stages.hsl_color_mixer
        || (edl.hsl == HslAdjustment::NEUTRAL && edl.color_mixer.regions.is_empty())
    {
        // Kein zusätzlicher Durchlauf, wenn die Stufe deaktiviert ist
        // oder weder HSL noch Farbmischer etwas zu tun haben (Regelfall).
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

    let graded = if !stages.color_grading || edl.color_grading == ColorGradingAdjustment::NEUTRAL {
        // Kein zusätzlicher Durchlauf, wenn die Stufe deaktiviert ist
        // oder alle vier Farbräder neutral stehen (Regelfall).
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
        if !stages.lens_corrections || params.is_identity() {
            // Kein zusätzlicher Durchlauf, wenn die Stufe deaktiviert ist
            // oder Objektivkorrekturen (nach Auflösung von Profil/Guided-
            // Linien) keine Wirkung hätten (Regelfall).
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

    let effected = if !stages.effects || edl.effects == EffectsAdjustment::NEUTRAL {
        // Kein zusätzlicher Durchlauf, wenn die Stufe deaktiviert ist
        // oder Vignettierung und Körnung beide neutral stehen
        // (Regelfall).
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

    // Halation-/Bloom-Simulation (Phase 14 Schritt 4) — bewusst CPU-only,
    // unabhängig vom GPU-/CPU-Dispatch oben (siehe `stages::effects`s
    // Moduldoku), deshalb ein eigener Kurzschluss statt Teil desselben
    // `apply_gpu`/`apply_cpu`-Aufrufs.
    let effected = if !stages.effects || edl.effects.halation_amount <= 0.0 {
        effected
    } else {
        effects::apply_halation(&effected, linear.width, linear.height, &edl.effects)
    };

    // KI-Tiefenschärfe-Simulator "Virtuelle Blende" (Phase 14 Schritt 8) —
    // bewusst CPU-only, unabhängig vom GPU-/CPU-Dispatch der Vignette/
    // Korn (siehe `stages::virtual_aperture`s Moduldoku, dieselbe
    // Begründung wie Halation in Schritt 4), deshalb ein eigener
    // Kurzschluss statt Teil desselben `apply_gpu`/`apply_cpu`-Aufrufs.
    let effected = if !stages.virtual_aperture || edl.virtual_aperture.amount <= 0.0 {
        effected
    } else {
        virtual_aperture::apply(
            &effected,
            linear.width,
            linear.height,
            &edl.virtual_aperture,
        )
    };

    let masked = if !stages.masks || edl.masks.is_empty() {
        // Kein zusätzlicher Durchlauf, wenn die Stufe deaktiviert ist
        // oder keine Masken vorhanden sind (Regelfall) — siehe
        // `stages::masks`s Moduldoku für die Pipeline-Position (nach
        // `effects`, vor der Farbraum-Konvertierung, noch im linearen
        // Arbeitsraum).
        effected
    } else {
        masks::apply_all(
            &effected,
            linear.width,
            linear.height,
            linear.as_shot_wb_coeffs,
            &edl.masks,
            &edl.mask_groups,
        )
    };

    let rgba = linear_camera_rgb_to_srgb_rgba8(&masked, linear.cam_to_srgb);

    // Schwarzweiß-Mixer (Phase 9 Schritt 5) — wie `curves` bewusst nach
    // der Farbraum-Konvertierung, davor: eine Tonwertkurve auf einem
    // bereits entsättigten Grauwert ergibt dasselbe sichtbare Ergebnis
    // wie auf dem farbigen Original, aber die Reihenfolge erlaubt dem
    // Mixer, noch den vollen Farbton jedes Pixels zu sehen.
    let treated = if stages.treatment && edl.treatment == Treatment::BlackAndWhite {
        bw_mixer::apply_rgba8(&rgba, &edl.bw_mixer)
    } else {
        rgba
    };

    let curved = if !stages.curves || edl.curves == CurvesAdjustment::neutral() {
        // Kein zusätzlicher Durchlauf über den ganzen Puffer, wenn die
        // Stufe deaktiviert ist oder alle fünf Kurven neutral stehen
        // (Regelfall).
        treated
    } else {
        curves::apply_rgba8(&treated, &edl.curves)
    };

    let composited = if !stages.composite || edl.composite_layers.is_empty() {
        // Kein zusätzlicher Durchlauf, wenn die Stufe deaktiviert ist
        // oder keine Compositing-Ebenen vorhanden sind (Regelfall) —
        // siehe `stages::composite`s Moduldoku für die Pipeline-Position
        // (nach `curves`, im fertig entwickelten sRGB-RGBA8-Bild).
        curved
    } else {
        composite::apply_all(&curved, linear.width, linear.height, &edl.composite_layers)
    };

    let (width, height, pixels) = if !stages.geometry || edl.geometry == GeometryAdjustment::NEUTRAL
    {
        // Kein zusätzlicher Durchlauf, wenn die Stufe deaktiviert ist
        // oder weder Drehung noch Zuschnitt etwas zu tun haben
        // (Regelfall).
        (linear.width, linear.height, composited)
    } else {
        geometry::apply(&composited, linear.width, linear.height, &edl.geometry)
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
    use crate::edl::{BasicAdjustments, EdlV4, WhiteBalanceAdjustment};

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

    /// Phase 9 Schritt 7 (Node-Editor): eine deaktivierte Stufe muss sich
    /// nicht auswirken, selbst wenn ihre Regler nicht neutral stehen —
    /// hier am Beispiel der Belichtungsstufe (`stage_enabled.basic`),
    /// die als einzige unabhängig vom Kurzschluss-Kommentar in
    /// `render_rgba8` sonst immer läuft.
    #[test]
    fn disabling_the_basic_stage_ignores_its_non_neutral_exposure() {
        let linear = flat_gray_linear_image(0.5);
        let exposed_edl = EdlV4 {
            basic: BasicAdjustments {
                exposure_ev: -2.0,
                ..BasicAdjustments::NEUTRAL
            },
            ..EdlV4::neutral()
        };
        let with_stage_on = render_rgba8(None, &linear, &exposed_edl).expect("rendern");

        let disabled_edl = EdlV4 {
            stage_enabled: crate::edl::StageEnabled {
                basic: false,
                ..crate::edl::StageEnabled::ALL
            },
            ..exposed_edl
        };
        let with_stage_off = render_rgba8(None, &linear, &disabled_edl).expect("rendern");
        let neutral = render_rgba8(None, &linear, &EdlV4::neutral()).expect("rendern");

        assert_ne!(
            with_stage_on.pixels[0], with_stage_off.pixels[0],
            "die aktive Stufe muss weiterhin abdunkeln"
        );
        assert_eq!(
            with_stage_off.pixels[0], neutral.pixels[0],
            "eine deaktivierte Stufe darf keine Wirkung mehr haben, egal was ihre Regler sagen"
        );
    }

    /// Phase 14 Schritt 3 (Mehrfachbelichtung/Compositing): eine
    /// Compositing-Ebene aus `edl.composite_layers` muss tatsächlich in
    /// `render_rgba8`s fester Kette ankommen (nicht nur in
    /// `stages::composite`s eigenen isolierten Tests funktionieren) —
    /// und `stage_enabled.composite = false` muss sie wieder abschalten,
    /// derselbe Node-Editor-Vertrag wie jede andere Stufe.
    #[test]
    fn a_composite_layer_reaches_the_final_render_and_can_be_disabled() {
        let linear = flat_gray_linear_image(0.2);
        let layer = crate::edl::CompositeLayer {
            visible: true,
            blend_mode: crate::edl::BlendMode::Normal,
            opacity: 1.0,
            scale: 1.0,
            offset_x: 0.5,
            offset_y: 0.5,
            source: crate::edl::CompositeLayerSource {
                bitmap_width: 1,
                bitmap_height: 1,
                pixels: vec![250, 250, 250],
            },
        };
        let edl = EdlV4 {
            composite_layers: vec![layer],
            ..EdlV4::neutral()
        };
        let rendered = render_rgba8(None, &linear, &edl).expect("rendern");
        assert!(
            rendered.pixels[0] > 200,
            "die volldeckende Compositing-Ebene sollte das dunkle Basisbild überschreiben, war {}",
            rendered.pixels[0]
        );

        let disabled_edl = EdlV4 {
            stage_enabled: crate::edl::StageEnabled {
                composite: false,
                ..crate::edl::StageEnabled::ALL
            },
            ..edl
        };
        let with_stage_off = render_rgba8(None, &linear, &disabled_edl).expect("rendern");
        let neutral = render_rgba8(None, &linear, &EdlV4::neutral()).expect("rendern");
        assert_eq!(
            with_stage_off.pixels[0], neutral.pixels[0],
            "deaktiviertes Compositing darf keine Wirkung mehr haben"
        );
    }

    #[test]
    fn neutral_edl_produces_correctly_sized_opaque_output() {
        let linear = flat_gray_linear_image(0.5);
        let rendered = render_rgba8(None, &linear, &EdlV4::neutral()).expect("sollte rendern");
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
        let neutral = render_rgba8(None, &linear, &EdlV4::neutral()).expect("rendern");
        let darker_edl = EdlV4 {
            basic: BasicAdjustments {
                exposure_ev: -2.0,
                ..BasicAdjustments::NEUTRAL
            },
            ..EdlV4::neutral()
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
        let edl = EdlV4 {
            basic: BasicAdjustments {
                exposure_ev: 0.3,
                contrast: 15.0,
                ..BasicAdjustments::NEUTRAL
            },
            ..EdlV4::neutral()
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
        let edl = EdlV4 {
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
            ..EdlV4::neutral()
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

    /// Phase-4-Nachmessung (`PLAN.md` Phase 4 Schritt 13, analog zum
    /// Phase-2-Schritt-7-Vorbild oben): dieselbe Messmethode, aber mit
    /// jeder einzelnen der zehn Phase-4-Werkzeugkategorien auf einen
    /// spürbar von neutral abweichenden Wert gesetzt — die obige Messung
    /// oben trifft für die meisten Stufen den „Regelfall überspringen"-
    /// Kurzschluss (siehe `render_rgba8`s Moduldoku) und misst damit nur
    /// den ursprünglichen Phase-2-Kern. Dieselben Ehrlichkeits-Einschränkungen
    /// gelten (keine echte Fenster-/IPC-/Compositing-Umgebung in dieser
    /// Sandbox, generöse Zeitschranke nur als Regressionswächter).
    #[test]
    fn render_rgba8_timing_with_all_phase4_stages_active() {
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

        use crate::edl::v2::{CropRect, ManualTransform};
        use crate::edl::{
            CalibrationAdjustment, ColorGradingAdjustment, ColorGradingWheel, ColorMixerAdjustment,
            ColorMixerRegion, CurveChannel, CurvesAdjustment, DetailsAdjustment, EffectsAdjustment,
            GeometryAdjustment, GridOverlay, HslAdjustment, HslBand, LensCorrectionAdjustment,
            PrimaryColorAdjustment, RepairLayer, RepairMode, RepairPoint, RepairStroke,
        };

        let edl = EdlV4 {
            basic: BasicAdjustments {
                exposure_ev: 0.4,
                contrast: 15.0,
                highlights: -10.0,
                shadows: 10.0,
                whites: 5.0,
                blacks: -5.0,
                texture: 20.0,
                clarity: 15.0,
                dehaze: 10.0,
                vibrance: 10.0,
                saturation: 5.0,
                white_balance: WhiteBalanceAdjustment {
                    temp_shift_kelvin: 200.0,
                    tint_shift: -5.0,
                },
            },
            curves: CurvesAdjustment {
                rgb: CurveChannel::Parametric {
                    shadows: 10.0,
                    darks: 5.0,
                    lights: -5.0,
                    highlights: -10.0,
                },
                ..CurvesAdjustment::neutral()
            },
            hsl: HslAdjustment {
                red: HslBand {
                    hue: 10.0,
                    saturation: 15.0,
                    luminance: -5.0,
                },
                ..HslAdjustment::NEUTRAL
            },
            color_mixer: ColorMixerAdjustment {
                regions: vec![ColorMixerRegion {
                    target_hue_degrees: 30.0,
                    bandwidth_degrees: 40.0,
                    feather: 15.0,
                    hue_shift: 10.0,
                    saturation_shift: 10.0,
                    luminance_shift: 0.0,
                }],
            },
            color_grading: ColorGradingAdjustment {
                shadows: ColorGradingWheel {
                    hue_degrees: 220.0,
                    saturation: 20.0,
                    luminance: -5.0,
                },
                balance: 10.0,
                ..ColorGradingAdjustment::NEUTRAL
            },
            details: DetailsAdjustment {
                sharpen_amount: 40.0,
                sharpen_radius: 1.0,
                luminance_nr_amount: 20.0,
                color_nr_amount: 20.0,
                ..DetailsAdjustment::NEUTRAL
            },
            lens_corrections: LensCorrectionAdjustment {
                ca_red_cyan: 10.0,
                ca_blue_yellow: -10.0,
                vignette_amount: 15.0,
                distortion_amount: 10.0,
                manual_transform: ManualTransform {
                    rotate_degrees: 1.0,
                    ..ManualTransform::NEUTRAL
                },
                ..LensCorrectionAdjustment::NEUTRAL
            },
            effects: EffectsAdjustment {
                post_vignette_amount: -20.0,
                grain_amount: 15.0,
                ..EffectsAdjustment::NEUTRAL
            },
            calibration: CalibrationAdjustment {
                shadow_tint: 10.0,
                red_primary: PrimaryColorAdjustment {
                    hue: 5.0,
                    saturation: 10.0,
                },
                ..CalibrationAdjustment::NEUTRAL
            },
            geometry: GeometryAdjustment {
                crop: CropRect {
                    x: 0.02,
                    y: 0.02,
                    width: 0.96,
                    height: 0.96,
                },
                angle_degrees: 2.0,
                overlay: GridOverlay::Thirds,
                ..GeometryAdjustment::NEUTRAL
            },
            repair: vec![RepairStroke {
                mode: RepairMode::Clone,
                source: RepairPoint { x: 0.1, y: 0.1 },
                target_path: vec![
                    RepairPoint { x: 0.5, y: 0.5 },
                    RepairPoint { x: 0.52, y: 0.51 },
                ],
                radius: 0.03,
                feather: 0.01,
                opacity: 1.0,
                ai_fill: None,
                layer: RepairLayer::Normal,
            }],
            masks: Vec::new(),
            mask_groups: Vec::new(),
            treatment: crate::edl::Treatment::Color,
            bw_mixer: crate::edl::BlackAndWhiteMixerAdjustment::NEUTRAL,
            stage_enabled: crate::edl::StageEnabled::ALL,
            composite_layers: Vec::new(),
            virtual_aperture: crate::edl::v4::VirtualApertureAdjustment::NEUTRAL,
        };

        if let Some(ctx) = &ctx {
            let started = std::time::Instant::now();
            let _ = render_rgba8(Some(ctx), &linear, &edl).expect("GPU-Rendering");
            let elapsed = started.elapsed();
            eprintln!(
                "render_rgba8 mit allen Phase-4-Stufen aktiv (GPU, {width}x{height}, Adapter '{}'): {:.2} ms",
                ctx.adapter_info.name,
                elapsed.as_secs_f64() * 1000.0
            );
            // Großzügige Schranke — mehrere zusätzliche sequenzielle
            // Durchläufe (u. a. Details/Objektivkorrekturen/Effekte/
            // Reparatur) statt eines einzigen Fused-Passes sind hier
            // erwartbar teurer als der Phase-2-Kern oben, siehe
            // `develop.rs`s Moduldoku für die Begründung je Stufe.
            // In dieser Sandbox läuft „GPU" auf `llvmpipe` (siehe die
            // Adapter-Ausgabe oben) — einem Software-Rasterisierer, der
            // neun sequenzielle Dispatches spürbar langsamer ausführt als
            // echte GPU-Hardware. Die Schranke ist entsprechend großzügig
            // gewählt (reiner Regressionswächter, keine Aussage über das
            // 16-ms-Ziel auf echter Hardware, siehe Moduldoku oben).
            assert!(
                elapsed.as_millis() < 10_000,
                "GPU-Rendering mit allen Phase-4-Stufen ungewöhnlich langsam: {elapsed:?}"
            );
        }

        let started = std::time::Instant::now();
        let _ = render_rgba8(None, &linear, &edl);
        let elapsed = started.elapsed();
        eprintln!(
            "render_rgba8 mit allen Phase-4-Stufen aktiv (CPU-Fallback, {width}x{height}): {:.2} ms",
            elapsed.as_secs_f64() * 1000.0
        );
        assert!(
            elapsed.as_millis() < 10_000,
            "CPU-Fallback mit allen Phase-4-Stufen ungewöhnlich langsam: {elapsed:?}"
        );
    }

    /// Phase-6-Nachmessung (`PLAN.md` Phase 6 Schritt 11, ADR-0032 Punkt 4
    /// nannte dies als offenes Risiko): dieselbe Messmethode wie oben, aber
    /// mit mehreren gleichzeitig sichtbaren, komplexen Masken statt der
    /// globalen Phase-4-Werkzeuge — jede Maske ist ein eigener sequenzieller
    /// Pipeline-Durchlauf durch alle sechs Masken-Werkzeuge (siehe
    /// `stages/masks.rs`), im Gegensatz zum einmaligen globalen Fused-Pass
    /// oben also die architektonisch teuerste Form dieser Phase. Dieselben
    /// Ehrlichkeits-Einschränkungen gelten (keine echte Fenster-/IPC-/
    /// Compositing-Umgebung in dieser Sandbox, generöse Zeitschranke nur als
    /// Regressionswächter, kein hartes 16-ms-Versprechen).
    #[test]
    fn render_rgba8_timing_with_several_masks_active() {
        use crate::edl::{
            BlendMode, ColorGradingAdjustment, ColorGradingWheel, ColorMixerAdjustment,
            ColorMixerRegion, CurveChannel, CurvesAdjustment, DetailsAdjustment, HslAdjustment,
            HslBand, Mask, MaskAdjustments, MaskCombine, MaskComponent, MaskGeometry,
        };

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

        // Nicht-neutrale Anpassungen über alle sechs Masken-Werkzeuge
        // hinweg — soll denselben "keine Stufe darf sich per Kurzschluss
        // selbst überspringen"-Grundsatz wie die Phase-4-Messung oben
        // erfüllen (siehe `develop.rs`s Moduldoku).
        fn busy_mask_adjustments() -> MaskAdjustments {
            MaskAdjustments {
                basic: BasicAdjustments {
                    exposure_ev: 0.3,
                    contrast: 10.0,
                    ..BasicAdjustments::NEUTRAL
                },
                curves: CurvesAdjustment {
                    rgb: CurveChannel::Parametric {
                        shadows: 10.0,
                        darks: 5.0,
                        lights: -5.0,
                        highlights: -10.0,
                    },
                    ..CurvesAdjustment::neutral()
                },
                hsl: HslAdjustment {
                    red: HslBand {
                        hue: 10.0,
                        saturation: 15.0,
                        luminance: -5.0,
                    },
                    ..HslAdjustment::NEUTRAL
                },
                color_mixer: ColorMixerAdjustment {
                    regions: vec![ColorMixerRegion {
                        target_hue_degrees: 30.0,
                        bandwidth_degrees: 40.0,
                        feather: 15.0,
                        hue_shift: 10.0,
                        saturation_shift: 10.0,
                        luminance_shift: 0.0,
                    }],
                },
                color_grading: ColorGradingAdjustment {
                    shadows: ColorGradingWheel {
                        hue_degrees: 220.0,
                        saturation: 20.0,
                        luminance: -5.0,
                    },
                    balance: 10.0,
                    ..ColorGradingAdjustment::NEUTRAL
                },
                details: DetailsAdjustment {
                    sharpen_amount: 40.0,
                    sharpen_radius: 1.0,
                    luminance_nr_amount: 20.0,
                    color_nr_amount: 20.0,
                    ..DetailsAdjustment::NEUTRAL
                },
            }
        }

        fn mask_with_geometry(id: &str, geometry: MaskGeometry, blend_mode: BlendMode) -> Mask {
            Mask {
                id: id.to_string(),
                name: id.to_string(),
                components: vec![MaskComponent {
                    geometry,
                    combine: MaskCombine::Add,
                    invert: false,
                }],
                adjustments: busy_mask_adjustments(),
                opacity: 80.0,
                feather: 10.0,
                invert: false,
                blend_mode,
                visible: true,
                group_id: None,
                overlay_color: crate::edl::OverlayColor::Red,
            }
        }

        // Fünf gleichzeitig sichtbare Masken, alle fünf Geometrietypen und
        // drei der teureren (Ganz-Pixel-)Mischmodi vertreten — der
        // realistische "viele/komplexe Masken aktiv"-Extremfall, den
        // ADR-0032 Punkt 4 als offenes Risiko benannt hat.
        let masks = vec![
            mask_with_geometry(
                "brush",
                MaskGeometry::Brush {
                    strokes: vec![crate::edl::BrushStroke {
                        points: vec![
                            crate::edl::MaskPoint { x: 0.2, y: 0.2 },
                            crate::edl::MaskPoint { x: 0.4, y: 0.3 },
                        ],
                        radius: 0.1,
                        feather: 0.05,
                        auto_mask: false,
                    }],
                },
                BlendMode::Multiply,
            ),
            mask_with_geometry(
                "linear_gradient",
                MaskGeometry::LinearGradient {
                    x1: 0.0,
                    y1: 0.0,
                    x2: 1.0,
                    y2: 1.0,
                },
                BlendMode::SoftLight,
            ),
            mask_with_geometry(
                "radial_gradient",
                MaskGeometry::RadialGradient {
                    center_x: 0.5,
                    center_y: 0.5,
                    radius_x: 0.3,
                    radius_y: 0.3,
                    angle_degrees: 0.0,
                    feather: 0.2,
                },
                BlendMode::Color,
            ),
            mask_with_geometry(
                "color_range",
                MaskGeometry::ColorRange {
                    target_r: 0.6,
                    target_g: 0.4,
                    target_b: 0.3,
                    tolerance: 0.2,
                    feather: 0.1,
                },
                BlendMode::Luminosity,
            ),
            mask_with_geometry(
                "luminance_range",
                MaskGeometry::LuminanceRange {
                    range_min: 0.3,
                    range_max: 0.7,
                    feather: 0.1,
                },
                BlendMode::Normal,
            ),
        ];

        let edl = EdlV4 {
            masks,
            ..EdlV4::neutral()
        };

        if let Some(ctx) = &ctx {
            let started = std::time::Instant::now();
            let _ = render_rgba8(Some(ctx), &linear, &edl).expect("GPU-Rendering");
            let elapsed = started.elapsed();
            eprintln!(
                "render_rgba8 mit fünf aktiven Masken (GPU, {width}x{height}, Adapter '{}'): {:.2} ms",
                ctx.adapter_info.name,
                elapsed.as_secs_f64() * 1000.0
            );
            // Großzügige Schranke, aus demselben Grund wie bei der
            // Phase-4-Messung oben (kein GPU-Pfad für die Maskenstufe
            // selbst — läuft also ohnehin komplett CPU-seitig, egal ob
            // `ctx` gesetzt ist; die Schranke deckt trotzdem den Fall ab,
            // in dem ein künftiger GPU-Pfad hinzukommt).
            assert!(
                elapsed.as_millis() < 20_000,
                "GPU-Rendering mit fünf aktiven Masken ungewöhnlich langsam: {elapsed:?}"
            );
        }

        let started = std::time::Instant::now();
        let _ = render_rgba8(None, &linear, &edl);
        let elapsed = started.elapsed();
        eprintln!(
            "render_rgba8 mit fünf aktiven Masken (CPU-Fallback, {width}x{height}): {:.2} ms",
            elapsed.as_secs_f64() * 1000.0
        );
        assert!(
            elapsed.as_millis() < 20_000,
            "CPU-Fallback mit fünf aktiven Masken ungewöhnlich langsam: {elapsed:?}"
        );
    }
}
