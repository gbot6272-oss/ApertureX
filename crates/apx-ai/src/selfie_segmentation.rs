//! Echte Personen-Hintergrundtrennung per MediaPipe Selfie Segmentation
//! (Phase 17 Schritt 8, siehe `DECISIONS.md` ADR-0045) — Grundlage für
//! "Greenscreen/Hintergrund entfernen": ersetzt den Hintergrund eines
//! Videos framegenau durch eine einfarbige Fläche, ohne dass ein echter
//! Greenscreen im Bild gestanden haben muss.
//!
//! **Lizenz (real geprüft, siehe ADR-0045s Recherche-Tabelle):**
//! MediaPipe Selfie Segmentation ist Apache-2.0 (Google) — bewusst
//! **nicht** `RobustVideoMatting` (GPL-3.0, abgelehnt, siehe ADR-0045).
//!
//! **Ehrliche Lücke, offen dokumentiert statt stillschweigend
//! angenommen (dasselbe Muster wie `apx_ai::inpaint`s LaMa-Modell):**
//! `huggingface.co` ist von dieser Entwicklungs-Sandbox aus blockiert —
//! weder das offizielle Modell noch eine ONNX-Community-Konvertierung
//! ließen sich in dieser Sitzung tatsächlich herunterladen oder gegen
//! echte Gewichte verifizieren (siehe `apx_app::commands::
//! SELFIE_SEGMENTATION_MODEL_URL`s Moduldoku für den genauen
//! Rechercheweg). Ein-/Ausgabeform (`256×256×3` HWC RGB rein, `256×256×1`
//! Segmentierungsmaske raus) und die `0..1`-Normierung ohne Kanal-
//! Mittelwert/-Streuung stammen aus MediaPipes öffentlicher
//! Modellbeschreibung (Sekundärquellen-Recherche, nicht das Original-
//! README, das hinter dem blockierten Host liegt) — **nicht End-zu-Ende
//! gegen die echten Gewichte getestet**. Wie bei `inpaint` liest dieser
//! Code die tatsächliche Eingabeform **aus dem geladenen Modell selbst**
//! (`Session::inputs()`), damit ein falscher Name/eine falsche Form
//! nicht zu einem stillen Falschresultat wird, sondern zu einem klaren
//! Fehler.

use std::path::Path;

use crate::error::{AiError, Result};

const FALLBACK_EDGE: u32 = 256;

/// Ein geladenes Selfie-Segmentation-Modell, bereit für wiederholte
/// Inferenz.
pub struct SelfieSegmentationSession {
    session: ort::session::Session,
}

impl SelfieSegmentationSession {
    /// Lädt `model_path` (die vom Nutzer heruntergeladene `.onnx`-Datei).
    pub fn load(model_path: &Path) -> Result<Self> {
        let session = ort::session::Session::builder()
            .map_err(|err| AiError::Model {
                message: format!("ONNX-Session-Builder konnte nicht erzeugt werden: {err}"),
            })?
            .commit_from_file(model_path)
            .map_err(|err| AiError::Model {
                message: format!(
                    "Modell '{}' konnte nicht geladen werden: {err}",
                    model_path.display()
                ),
            })?;
        Ok(Self { session })
    }

    /// Schätzt eine `0..=255`-Personen-Maske (`255` = Person, `0` =
    /// Hintergrund) für `pixels` (interleaved RGB, `0..=255`,
    /// `width * height * 3` Bytes), zurückskaliert auf die Original-
    /// Auflösung.
    pub fn person_mask_rgb8(&mut self, pixels: &[u8], width: u32, height: u32) -> Result<Vec<u8>> {
        if pixels.len() != (width as usize) * (height as usize) * 3 {
            return Err(AiError::Model {
                message: format!(
                    "Bildpuffer-Länge {} passt nicht zu {width}x{height} RGB",
                    pixels.len()
                ),
            });
        }

        let inputs = self.session.inputs();
        if inputs.len() != 1 {
            return Err(AiError::Model {
                message: format!(
                    "Modell hat {} Eingänge, erwartet wird genau einer (Bild)",
                    inputs.len()
                ),
            });
        }
        let edge = declared_edge(&inputs[0]).unwrap_or(FALLBACK_EDGE);

        let (r, g, b) = split_channels(pixels, width, height);
        let r = apx_core::raster::bilinear_resize_u8(&r, width, height, edge, edge);
        let g = apx_core::raster::bilinear_resize_u8(&g, width, height, edge, edge);
        let b = apx_core::raster::bilinear_resize_u8(&b, width, height, edge, edge);

        let tensor_data = to_normalized_nchw(&[&r, &g, &b]);
        let image_array =
            ndarray::Array4::from_shape_vec((1, 3, edge as usize, edge as usize), tensor_data)
                .map_err(|err| AiError::Model {
                    message: format!("Bild-Tensor-Form ungültig: {err}"),
                })?;

        let outputs = self
            .session
            .run(ort::inputs![ort::value::TensorRef::from_array_view(
                &image_array
            )
            .map_err(|err| AiError::Model {
                message: format!("Bild-Tensor konnte nicht erzeugt werden: {err}"),
            })?])
            .map_err(|err| AiError::Model {
                message: format!("Inferenz fehlgeschlagen: {err}"),
            })?;

        let (output_shape, output_data) =
            outputs[0]
                .try_extract_tensor::<f32>()
                .map_err(|err| AiError::Model {
                    message: format!("Ausgabe ist kein f32-Tensor: {err}"),
                })?;
        if output_shape.len() < 2 {
            return Err(AiError::Model {
                message: format!(
                    "unerwartete Ausgabeform {output_shape:?}, erwartet mind. 2 Achsen (H, W)"
                ),
            });
        }
        let out_h = output_shape[output_shape.len() - 2] as u32;
        let out_w = output_shape[output_shape.len() - 1] as u32;
        if (out_h as usize) * (out_w as usize) != output_data.len() {
            return Err(AiError::Model {
                message: format!(
                    "Ausgabeform {output_shape:?} passt nicht zur tatsächlichen Datenlänge {}",
                    output_data.len()
                ),
            });
        }

        let mask_u8: Vec<u8> = output_data
            .iter()
            .map(|&v| (v.clamp(0.0, 1.0) * 255.0).round() as u8)
            .collect();
        Ok(apx_core::raster::bilinear_resize_u8(
            &mask_u8, out_w, out_h, width, height,
        ))
    }
}

/// Liest die deklarierte quadratische Kantenlänge aus der Eingabeform
/// eines ONNX-Tensors, falls fest (nicht `-1`/dynamisch) — dasselbe
/// Muster wie `apx_ai::inpaint::declared_edge`, hier für einen NCHW-
/// Bild-Eingang mit `[1, 3, H, W]`.
fn declared_edge(input: &ort::value::Outlet) -> Option<u32> {
    let ort::value::ValueType::Tensor { shape, .. } = input.dtype() else {
        return None;
    };
    // NCHW: die letzten beiden Achsen sind Höhe/Breite.
    if shape.len() < 2 {
        return None;
    }
    let h = shape[shape.len() - 2];
    let w = shape[shape.len() - 1];
    (h > 0 && h == w).then_some(h as u32)
}

fn split_channels(pixels: &[u8], width: u32, height: u32) -> (Vec<u8>, Vec<u8>, Vec<u8>) {
    let n = (width as usize) * (height as usize);
    let mut r = vec![0u8; n];
    let mut g = vec![0u8; n];
    let mut b = vec![0u8; n];
    for i in 0..n {
        r[i] = pixels[i * 3];
        g[i] = pixels[i * 3 + 1];
        b[i] = pixels[i * 3 + 2];
    }
    (r, g, b)
}

/// Baut einen NCHW-`f32`-Tensor aus drei `edge * edge`-RGB-Kanalpuffern
/// — reine `0..1`-Skalierung, keine Kanal-Mittelwert-/Streuungs-
/// Normierung (siehe Moduldoku zur Quelle dieser Annahme).
fn to_normalized_nchw(channels: &[&[u8]; 3]) -> Vec<f32> {
    let edge_sq = channels[0].len();
    let mut out = Vec::with_capacity(3 * edge_sq);
    for channel in channels {
        out.extend(channel.iter().map(|&v| v as f32 / 255.0));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn to_normalized_nchw_scales_bytes_into_the_unit_interval() {
        let r = [0u8, 255, 128];
        let g = [255u8, 0, 64];
        let b = [10u8, 20, 30];
        let out = to_normalized_nchw(&[&r, &g, &b]);
        assert_eq!(out.len(), 9);
        assert!((out[0] - 0.0).abs() < 1e-6);
        assert!((out[1] - 1.0).abs() < 1e-6);
        for &v in &out {
            assert!((0.0..=1.0).contains(&v));
        }
    }
}
