//! Echter KI-Stiltransfer zwischen Fotos (Phase 14 Schritt 9, siehe
//! `DECISIONS.md` ADR-0041 Nachtrag IX, Recherche-Tabelle Punkt 7):
//! Lightroom hat dafür kein Äquivalent. Fünf feste, real lizenzierte
//! ONNX-Modelle (`onnx/models`, MIT, `fast_neural_style`:
//! candy/mosaic/rain-princess/udnie/pointilism) statt eines *beliebigen*
//! Referenzbilds als Stilvorlage ("arbitrary style transfer") — Schritt 0
//! hatte dafür real ein lizenzklares Modell gesucht (Googles Magenta,
//! Apache-2.0), aber nur als TFLite-Checkpoint ohne ONNX-Export
//! gefunden; der einzige gefundene ONNX-Nachbau lag ohne eigene
//! Lizenzangabe auf Google Drive — dieselbe Art unklarer Herkunft, die
//! `ADR-0040-Nachtrag VI`s SFace-Ablehnung schon begründet hat. Deshalb
//! bewusst auf die fünf sicher lizenzierten festen Stile beschränkt,
//! kein Kompromiss ohne Vorbild.
//!
//! **Real heruntergeladen und geprüft, nicht nur aus dem Gedächtnis
//! übernommen:** alle fünf `<stil>-9.onnx`-Dateien real per
//! `media.githubusercontent.com/media/onnx/models/main/validated/
//! vision/style_transfer/fast_neural_style/model/<stil>-9.onnx` geladen
//! (derselbe echte Git-LFS-Auslieferungs-Host wie schon in Schritt 0s
//! Spike identifiziert), jede Datei exakt `6 728 029` Byte (dieselbe
//! Netzarchitektur, nur andere trainierte Gewichte je Stil) mit real
//! berechneten, unterschiedlichen SHA-256-Hashes je Stil (siehe
//! `apx-app::commands::STYLE_TRANSFER_MODEL_SHA256`, das jeden Download
//! damit hart prüft — derselbe Ansatz wie MiDaS in Schritt 8, nicht
//! LaMas fehlender Hash in Phase 13).
//!
//! **Ein während dieser Sitzung real gefundener Korrektur-Fund, kein
//! Vorab-Design:** Schritt 0s Spike vermutete "dynamische NCHW-Eingabe"
//! (der damalige Testlauf probierte nur eine 224×224-Eingabe, keine
//! andere Größe). Ein echter `onnxruntime`-Introspektionslauf in dieser
//! Sitzung zeigt: von den vielen im ONNX-Graph aufgeführten "Inputs"
//! ist nur `input1` ein echter Laufzeit-Feed — alle anderen (jede
//! Gewichts-/Bias-Matrix des Netzes) sind tatsächlich per Initializer
//! belegte Konstanten, kein Nutzereingang. `input1` selbst hat eine FEST
//! codierte Form `[1,3,224,224]` (`dim_value`, kein `dim_param`); ein
//! echter Inferenzlauf mit `100×150` schlägt mit einer expliziten
//! ONNX-Runtime-Fehlermeldung fehl (kein stiller Fallback) — exakt
//! dieselbe Lehre wie MiDaS in Schritt 8 (ein Modell mit scheinbar
//! flexiblen Metadaten verlangt trotzdem eine feste Auflösung). Deshalb
//! dasselbe Muster: auf 224×224 herunterskalieren, inferieren, das
//! Ergebnis wieder auf die tatsächliche Zielauflösung hochskalieren.
//!
//! **Wertebereich:** anders als MiDaS' unbeschränkte Disparität gibt
//! `fast_neural_style` ein Bild in ungefähr demselben `0..255`-Bereich
//! wie die Eingabe zurück (kein `Sigmoid`/`Tanh` am Ausgang, echt per
//! Introspektion gegen ein Testbild geprüft) — das Netz selbst klemmt
//! den Wertebereich aber nicht hart, einzelne Pixel können knapp
//! außerhalb `0..255` landen. Deshalb wird hier wie bei jeder anderen
//! Pixel-Ausgabe dieses Projekts explizit auf `0.0..=255.0` geklemmt,
//! bevor auf `u8` gerundet wird.

use std::path::Path;

use crate::error::{AiError, Result};

const EDGE: u32 = 224;

/// Die fünf real lizenzierten, festen Stile (siehe Moduldoku) — kein
/// beliebiges Referenzbild.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StyleKind {
    Candy,
    Mosaic,
    RainPrincess,
    Udnie,
    Pointilism,
}

impl StyleKind {
    pub const ALL: [StyleKind; 5] = [
        StyleKind::Candy,
        StyleKind::Mosaic,
        StyleKind::RainPrincess,
        StyleKind::Udnie,
        StyleKind::Pointilism,
    ];

    /// Der Dateiname im `onnx/models`-Repo (auch als stabiler
    /// Bezeichner in Einstellungen/Frontend verwendet, siehe
    /// `apx-app::commands`).
    pub fn id(&self) -> &'static str {
        match self {
            StyleKind::Candy => "candy",
            StyleKind::Mosaic => "mosaic",
            StyleKind::RainPrincess => "rain-princess",
            StyleKind::Udnie => "udnie",
            StyleKind::Pointilism => "pointilism",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|s| s.id() == id)
    }
}

/// Ein geladenes `fast_neural_style`-Modell für genau einen Stil, bereit
/// für wiederholte Inferenz.
pub struct StyleTransferSession {
    session: ort::session::Session,
}

impl StyleTransferSession {
    /// Lädt `model_path` (die vom Nutzer für einen Stil heruntergeladene
    /// `.onnx`-Datei).
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

    /// Stilisiert `pixels` (interleaved RGB, `0..=255`, `width * height *
    /// 3` Bytes) und liefert ein ebenso großes Ergebnis zurück — die
    /// tatsächliche Netz-Inferenz läuft intern immer auf `224×224` und
    /// wird zurückskaliert, siehe Moduldoku.
    pub fn stylize_rgb8(&mut self, pixels: &[u8], width: u32, height: u32) -> Result<Vec<u8>> {
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

        let tensor_data = to_nchw_0_255(&[&r, &g, &b]);
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
        // Erwartet (1, 3, H, W) — dieselbe NCHW-Form wie die Eingabe.
        if output_shape.len() != 4 || output_shape[1] != 3 {
            return Err(AiError::Model {
                message: format!("unerwartete Ausgabeform {output_shape:?}, erwartet (1, 3, H, W)"),
            });
        }
        let out_h = output_shape[2] as u32;
        let out_w = output_shape[3] as u32;

        let out_r = from_nchw_channel_clamped(output_data, out_w, out_h, 0);
        let out_g = from_nchw_channel_clamped(output_data, out_w, out_h, 1);
        let out_b = from_nchw_channel_clamped(output_data, out_w, out_h, 2);

        let out_r = apx_core::raster::bilinear_resize_u8(&out_r, out_w, out_h, width, height);
        let out_g = apx_core::raster::bilinear_resize_u8(&out_g, out_w, out_h, width, height);
        let out_b = apx_core::raster::bilinear_resize_u8(&out_b, out_w, out_h, width, height);

        let n = (width as usize) * (height as usize);
        let mut out = vec![0u8; n * 3];
        for i in 0..n {
            out[i * 3] = out_r[i];
            out[i * 3 + 1] = out_g[i];
            out[i * 3 + 2] = out_b[i];
        }
        Ok(out)
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

/// Baut einen NCHW-`f32`-Tensor aus drei `EDGE * EDGE`-RGB-Kanalpuffern
/// — anders als `depth.rs::to_normalized_nchw` keine ImageNet-
/// Normalisierung, `fast_neural_style` erwartet rohe `0..255`-Werte
/// (siehe Moduldoku).
fn to_nchw_0_255(channels: &[&[u8]; 3]) -> Vec<f32> {
    let mut out = Vec::with_capacity(3 * (EDGE as usize) * (EDGE as usize));
    for channel in channels {
        out.extend(channel.iter().map(|&v| v as f32));
    }
    out
}

/// Liest einen einzelnen Kanal aus einem NCHW-Ausgabetensor, geklemmt auf
/// `0.0..=255.0` vor der Rundung auf `u8` (siehe Moduldoku).
fn from_nchw_channel_clamped(data: &[f32], width: u32, height: u32, channel: usize) -> Vec<u8> {
    let plane = (width as usize) * (height as usize);
    let start = channel * plane;
    data[start..start + plane]
        .iter()
        .map(|&v| v.round().clamp(0.0, 255.0) as u8)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Dasselbe Testhilfsfunktion-Muster wie `depth.rs`/`inpaint.rs`.
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

    fn load_stub_session() -> Option<StyleTransferSession> {
        let dylib = find_test_dylib()?;
        crate::inpaint::init_environment(&dylib)
            .expect("ONNX-Umgebung sollte sich initialisieren lassen");
        let model_bytes = include_bytes!("../tests/fixtures/add_300_style_stub.onnx");
        let session = ort::session::Session::builder()
            .expect("Session-Builder sollte sich erzeugen lassen")
            .commit_from_memory(model_bytes)
            .expect("Testmodell sollte sich laden lassen");
        Some(StyleTransferSession { session })
    }

    /// Gezielter Test für Phase 14 Schritt 9: baut ein winziges, echtes
    /// ONNX-Modell mit **derselben Ein-Eingang-Ein-Ausgang-NCHW-
    /// Topologie** wie die echten `fast_neural_style`-Modelle (`input1`
    /// [1,3,224,224] -> `output1` [1,3,224,224], hier per `Add(+300)`
    /// statt echter Stil-Gewichte), um `stylize_rgb8`s Vorverarbeitung/
    /// Tensor-Aufbau/Rundskalierung end-to-end zu prüfen, ohne die
    /// 6,7-MB-Gewichtsdatei zu brauchen (Modell-Download nicht Teil des
    /// CI-Testlaufs, siehe `PLAN.md` Phase 14s Verifikations-Abschnitt).
    /// `+300` auf einen mittelgrauen Eingangswert testet zugleich das
    /// Klemmen auf `255` (siehe Moduldoku).
    #[test]
    fn stylize_rgb8_resizes_output_back_and_clamps_out_of_range_values() {
        let Some(mut style) = load_stub_session() else {
            eprintln!("übersprungen: keine ONNX-Runtime-Bibliothek in dieser Umgebung gefunden");
            return;
        };

        let width = 8;
        let height = 6;
        let pixels = vec![100u8; width * height * 3];
        let out = style
            .stylize_rgb8(&pixels, width as u32, height as u32)
            .expect("Inferenz sollte laufen");

        assert_eq!(out.len(), width * height * 3);
        // 100 + 300 = 400, weit über 255 -> muss auf 255 geklemmt sein.
        for &v in &out {
            assert_eq!(v, 255, "erwartete durchgehend geklemmte 255, war {v}");
        }
    }

    #[test]
    fn style_kind_round_trips_through_its_stable_id() {
        for kind in StyleKind::ALL {
            assert_eq!(StyleKind::from_id(kind.id()), Some(kind));
        }
        assert_eq!(StyleKind::from_id("unbekannt"), None);
    }
}
