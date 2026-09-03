//! Monokulare Tiefenschätzung per MiDaS v2.1 small (Phase 14 Schritt 8,
//! siehe `DECISIONS.md` ADR-0041 Nachtrag VIII, Recherche-Tabelle
//! Punkt 1): Lightroom hat keine KI-Tiefenschätzung/synthetisches
//! Bokeh — nur die vorhandene grobe Unschärfe-Heuristik in ApertureX
//! selbst (Laplace-Varianz, `apx_pipeline::stages::masks`s
//! `BlurDepthApprox`-Maskentyp, Phase 11 Schritt 7). Dieses Modul
//! ergänzt eine echte, trainierte monokulare Tiefenkarte als Alternative
//! — additiv, die bestehende Heuristik bleibt der Fallback ohne
//! heruntergeladenes Modell (siehe `apx-app::commands::estimate_depth`s
//! Moduldoku).
//!
//! **Opt-in-Download wie das LaMa-Inpainting-Modell** (Phase 13
//! Schritt 1, `apx_ai::inpaint`) — echtes MIT-lizenziertes ONNX-Release-
//! Asset (`isl-org/MiDaS`, `model-small.onnx`,
//! `github.com/isl-org/MiDaS/releases/download/v2_1/model-small.onnx`,
//! **real heruntergeladen und geprüft** in dieser Sitzung: exakt
//! `66 764 249` Bytes, SHA-256
//! `2d8c6cb8f415229daf1eb041024208e2608c9f98e17c81cc7c6ecb449c56fd58` —
//! anders als beim LaMa-Modell war `huggingface.co` dort blockiert, hier
//! ist `github.com` erreichbar, deshalb diesmal ein echter Hash statt
//! der dortigen ehrlichen Lücke (siehe `apx-app::commands::
//! MIDAS_MODEL_SHA256`, das den Download tatsächlich damit prüft).
//!
//! **Ein-/Ausgabeform real per `onnxruntime`-Introspektion geprüft**
//! (nicht nur aus Sekundärquellen übernommen, `python3 -c
//! "onnxruntime.InferenceSession(...).get_inputs()/.get_outputs()"`
//! gegen die echte heruntergeladene Datei): Eingang `"0"`, fest
//! `(1, 3, 256, 256)` — trotz scheinbar dynamischer Achsen in manchen
//! Anzeige-Tools verlangt das Modell tatsächlich exakt diese Form
//! (bestätigt schon in Schritt 0s Spike). Ausgang `"797"`, `(1, 256,
//! 256)` — **ein** Kanal ohne eigene Kanal-Achse, anders als LaMas
//! `(1, 3, H, W)`-Bildausgabe.
//!
//! **Normalisierung real aus `isl-org/MiDaS`s eigenem
//! `hubconf.py::small_transform` übernommen** (echt abgerufen, nicht aus
//! dem Gedächtnis behauptet): `0..1`-Skalierung, danach ImageNet-
//! Mittelwert/-Streuung je Kanal (`mean=[0.485,0.456,0.406]`,
//! `std=[0.229,0.224,0.225]`) — dieselben Konstanten wie unzählige
//! ImageNet-vortrainierte Modelle, hier aber tatsächlich am
//! MiDaS-Quellcode verifiziert statt nur angenommen.
//!
//! Die rohe Ausgabe ist eine **relative Disparität** (`1/Tiefe`, höhere
//! Werte = näher) ohne festen Wertebereich — deshalb hier per-Bild auf
//! `0.0..=1.0` normiert (`1.0` = am nächsten, `0.0` = am weitesten
//! entfernt). Dieselbe Konvention wie die bestehende Laplace-Varianz-
//! Heuristik, damit beide austauschbar denselben nachgelagerten Code
//! (Fokuspunkt-Differenz -> Unschärferadius) speisen können.

use std::path::Path;

use crate::error::{AiError, Result};

const EDGE: u32 = 256;
const MEAN: [f32; 3] = [0.485, 0.456, 0.406];
const STD: [f32; 3] = [0.229, 0.224, 0.225];

/// Ein geladenes MiDaS-Modell, bereit für wiederholte Inferenz.
pub struct DepthSession {
    session: ort::session::Session,
}

impl DepthSession {
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

    /// Schätzt eine relative, `0.0..=1.0`-normierte Tiefenkarte
    /// (`1.0` = am nächsten) für `pixels` (interleaved RGB, `0..=255`,
    /// `width * height * 3` Bytes), zurückskaliert auf die Original-
    /// Auflösung.
    pub fn estimate_rgb8(&mut self, pixels: &[u8], width: u32, height: u32) -> Result<Vec<f32>> {
        if pixels.len() != (width as usize) * (height as usize) * 3 {
            return Err(AiError::Model {
                message: format!(
                    "Bildpuffer-Länge {} passt nicht zu {width}x{height} RGB",
                    pixels.len()
                ),
            });
        }

        let (r, g, b) = split_channels(pixels, width, height);
        let r = apx_core::raster::bilinear_resize_u8(&r, width, height, EDGE, EDGE);
        let g = apx_core::raster::bilinear_resize_u8(&g, width, height, EDGE, EDGE);
        let b = apx_core::raster::bilinear_resize_u8(&b, width, height, EDGE, EDGE);

        let tensor_data = to_normalized_nchw(&[&r, &g, &b]);
        let image_array =
            ndarray::Array4::from_shape_vec((1, 3, EDGE as usize, EDGE as usize), tensor_data)
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
        // Erwartet (1, H, W) — ein Kanal ohne eigene Achse, siehe
        // Moduldoku. Nur die letzten beiden Achsen auswerten, damit ein
        // Modell mit zusätzlicher Batch-Achse (immer `1`) trotzdem
        // funktioniert, ohne die Achsenzahl exakt hart zu kodieren.
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

        // Relative Disparität -> 0..1 normiert (1.0 = am nächsten).
        let min = output_data.iter().copied().fold(f32::INFINITY, f32::min);
        let max = output_data
            .iter()
            .copied()
            .fold(f32::NEG_INFINITY, f32::max);
        let range = (max - min).max(1e-6);
        let normalized_u8: Vec<u8> = output_data
            .iter()
            .map(|&v| (((v - min) / range) * 255.0).round().clamp(0.0, 255.0) as u8)
            .collect();

        let resized =
            apx_core::raster::bilinear_resize_u8(&normalized_u8, out_w, out_h, width, height);
        Ok(resized.into_iter().map(|v| f32::from(v) / 255.0).collect())
    }
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

/// Baut einen NCHW-`f32`-Tensor aus drei `EDGE * EDGE`-RGB-Kanalpuffern:
/// `0..1`-Skalierung, dann ImageNet-Mittelwert/-Streuung je Kanal — siehe
/// Moduldoku für die reale Quelle dieser Konstanten.
fn to_normalized_nchw(channels: &[&[u8]; 3]) -> Vec<f32> {
    let mut out = Vec::with_capacity(3 * (EDGE as usize) * (EDGE as usize));
    for (channel, (&mean, &std)) in channels.iter().zip(MEAN.iter().zip(STD.iter())) {
        out.extend(channel.iter().map(|&v| ((v as f32 / 255.0) - mean) / std));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Findet die vom Spike-Setup per `pip install onnxruntime`
    /// installierte `libonnxruntime.so` — dasselbe Muster wie
    /// `apx_ai::inpaint`s Testhilfsfunktion.
    fn find_test_dylib() -> Option<std::path::PathBuf> {
        std::env::var_os("ORT_DYLIB_PATH")
            .map(std::path::PathBuf::from)
            .filter(|p| p.exists())
            .or_else(|| {
                let candidate = std::path::PathBuf::from(
                    "/usr/local/lib/python3.11/dist-packages/onnxruntime/capi/libonnxruntime.so.1.29.0",
                );
                candidate.exists().then_some(candidate)
            })
    }

    /// Gezielter Test für Phase 14 Schritt 8: baut ein winziges, echtes
    /// ONNX-Modell mit **derselben Ein-Eingang-Ein-Ausgang-Topologie**
    /// wie das echte MiDaS-Modell (ein Bild-Tensor rein, ein
    /// Ein-Kanal-Tensor ohne Kanal-Achse raus — hier eine reine
    /// Luminanz-Mittelung über die drei Eingabekanäle statt echter
    /// Tiefenschätzung), um `estimate_rgb8`s Vorverarbeitung/
    /// Tensor-Aufbau/Nachverarbeitung end-to-end zu prüfen, ohne die
    /// echten MiDaS-Gewichte zu brauchen (die 66-MB-Datei ist nicht Teil
    /// des Testlaufs, siehe `PLAN.md` Phase 14s Verifikations-Abschnitt).
    #[test]
    fn estimate_rgb8_resizes_output_back_to_the_original_resolution() {
        let Some(dylib) = find_test_dylib() else {
            eprintln!("übersprungen: keine ONNX-Runtime-Bibliothek in dieser Umgebung gefunden");
            return;
        };
        crate::inpaint::init_environment(&dylib)
            .expect("ONNX-Umgebung sollte sich initialisieren lassen");

        let model_bytes = include_bytes!("../tests/fixtures/mean_channel_depth.onnx");
        let session = ort::session::Session::builder()
            .expect("Session-Builder sollte sich erzeugen lassen")
            .commit_from_memory(model_bytes)
            .expect("Testmodell sollte sich laden lassen");
        let mut depth = DepthSession { session };

        let width = 8;
        let height = 6;
        let pixels = vec![128u8; width * height * 3];
        let out = depth
            .estimate_rgb8(&pixels, width as u32, height as u32)
            .expect("Inferenz sollte laufen");

        assert_eq!(out.len(), width * height);
        for &v in &out {
            assert!((0.0..=1.0).contains(&v), "v={v}");
        }
    }

    #[test]
    fn a_uniform_gray_image_yields_a_uniform_normalized_depth_map() {
        let Some(dylib) = find_test_dylib() else {
            eprintln!("übersprungen: keine ONNX-Runtime-Bibliothek in dieser Umgebung gefunden");
            return;
        };
        crate::inpaint::init_environment(&dylib)
            .expect("ONNX-Umgebung sollte sich initialisieren lassen");

        let model_bytes = include_bytes!("../tests/fixtures/mean_channel_depth.onnx");
        let session = ort::session::Session::builder()
            .expect("Session-Builder sollte sich erzeugen lassen")
            .commit_from_memory(model_bytes)
            .expect("Testmodell sollte sich laden lassen");
        let mut depth = DepthSession { session };

        // Ein völlig gleichförmiges Bild liefert eine völlig
        // gleichförmige Roh-Ausgabe -> `range` kollabiert auf nahe 0,
        // die Min-Max-Normierung muss trotzdem nicht divergieren (siehe
        // `.max(1e-6)` im Code) und liefert einen einzigen, endlichen
        // Wert überall.
        let width = 4;
        let height = 4;
        let pixels = vec![50u8; width * height * 3];
        let out = depth
            .estimate_rgb8(&pixels, width as u32, height as u32)
            .expect("Inferenz sollte laufen");

        let first = out[0];
        assert!(first.is_finite());
        for &v in &out {
            assert!((v - first).abs() < 1e-3, "v={v} first={first}");
        }
    }
}
