//! Orchestriert den kompletten Entwickeln-Rendervorgang: von linearen
//! Kamera-RGB-Pixeln (`apx_raw::decode_linear`) über die Weißabgleich-
//! Gain-Berechnung und die sieben Regler (`crate::stages`) bis zum
//! fertigen, anzeigefähigen RGBA8-Bildpuffer (`crate::color`) — der
//! einzige Einstiegspunkt, den `apx-app`s Protokoll-Route für
//! `develop/...` aufruft (siehe `PLAN.md` Phase 2 Schritt 5). `apx-app`
//! kennt damit weder `crate::stages` noch `crate::color` einzeln — reine
//! Verdrahtung bleibt reine Verdrahtung (`ARCHITECTURE.md` §4).

use apx_raw::LinearImage;

use crate::color::linear_camera_rgb_to_srgb_rgba8;
use crate::edl::EdlV1;
use crate::error::Result;
use crate::gpu::GpuContext;
use crate::stages::{basic_fused, white_balance};

/// Rendert `linear` mit den in `edl` beschriebenen Anpassungen zu einem
/// interleaved RGBA8-Puffer (`4 * linear.width * linear.height` Bytes,
/// Alpha immer `255`).
///
/// Nutzt `ctx`, falls vorhanden — schlägt die GPU-Ausführung dennoch fehl
/// (z. B. Treiberfehler zur Laufzeit), fällt diese Funktion automatisch
/// auf den rayon-CPU-Pfad zurück, statt den ganzen Aufruf scheitern zu
/// lassen (siehe `SPEC.md` §2.2 „GPU→CPU-Fallback muss existieren",
/// `DECISIONS.md` ADR-0012) — der Aufrufer muss diese Entscheidung nicht
/// selbst treffen.
pub fn render_rgba8(
    ctx: Option<&GpuContext>,
    linear: &LinearImage,
    edl: &EdlV1,
) -> Result<Vec<u8>> {
    let wb_gains = white_balance::compute_gains(linear.as_shot_wb_coeffs, edl.basic.white_balance);

    let tonal = match ctx {
        Some(ctx) => match basic_fused::apply_gpu(ctx, &linear.pixels, wb_gains, &edl.basic) {
            Ok(pixels) => pixels,
            Err(err) => {
                tracing::warn!(error = %err, "GPU-Rendering fehlgeschlagen, nutze CPU-Fallback");
                basic_fused::apply_cpu(&linear.pixels, wb_gains, &edl.basic)
            }
        },
        None => basic_fused::apply_cpu(&linear.pixels, wb_gains, &edl.basic),
    };

    Ok(linear_camera_rgb_to_srgb_rgba8(&tonal, linear.cam_to_srgb))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::edl::BasicAdjustments;

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
        let rgba = render_rgba8(None, &linear, &EdlV1::NEUTRAL).expect("sollte rendern");
        assert_eq!(rgba.len(), 2 * 2 * 4);
        for pixel in rgba.chunks_exact(4) {
            assert_eq!(pixel[3], 255, "Alpha muss immer undurchsichtig sein");
        }
    }

    #[test]
    fn negative_exposure_darkens_output() {
        let linear = flat_gray_linear_image(0.5);
        let neutral = render_rgba8(None, &linear, &EdlV1::NEUTRAL).expect("rendern");
        let darker_edl = EdlV1 {
            basic: BasicAdjustments {
                exposure_ev: -2.0,
                ..BasicAdjustments::NEUTRAL
            },
        };
        let darker = render_rgba8(None, &linear, &darker_edl).expect("rendern");
        assert!(
            darker[0] < neutral[0],
            "negative Belichtung sollte den Rot-Kanal absenken (neutral={}, darker={})",
            neutral[0],
            darker[0]
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
        let edl = EdlV1 {
            basic: BasicAdjustments {
                exposure_ev: 0.3,
                contrast: 15.0,
                ..BasicAdjustments::NEUTRAL
            },
        };
        let cpu = render_rgba8(None, &linear, &edl).expect("CPU-Rendering");
        let gpu = render_rgba8(Some(&ctx), &linear, &edl).expect("GPU-Rendering");
        for (c, g) in cpu.iter().zip(gpu.iter()) {
            // Toleranz von 1, da CPU/GPU getrennt auf f32 runden, bevor
            // hier auf u8 quantisiert wird.
            assert!((*c as i16 - *g as i16).abs() <= 1, "CPU={c} GPU={g}");
        }
    }
}
