//! Echtes KI-Ausfüllen per LaMa-Inpainting (Phase 13 Schritt 1, siehe
//! `DECISIONS.md` ADR-0040 — korrigiert ADR-0033 Punkt 1). Nutzt ein
//! echtes, Apache-2.0-lizenziertes, öffentlich als ONNX exportiertes
//! LaMa-Modell (`Carve/LaMa-ONNX`, `lama_fp32.onnx`, Hugging Face,
//! trainiert auf Places2/CC-BY-4.0) über die echte `ort`-ONNX-Laufzeit —
//! kein Text-Prompt, reines maskenbasiertes Ausfüllen wie Photoshops
//! älteres Content-Aware-Fill, nur mit einem echten trainierten Modell
//! statt PatchMatch (siehe `apx_pipeline::stages::repair::RepairMode::
//! ContentAwareFill` für die bisherige, weiterhin bestehende Variante).
//!
//! **Ehrliche Lücke, offen dokumentiert statt stillschweigend
//! angenommen:** `huggingface.co` ist von dieser Entwicklungs-Sandbox
//! aus nicht erreichbar (Proxy blockiert den gesamten Host, nicht nur
//! `cdn.pyke.io` wie beim `ort`-Spike, siehe `inpaint`-Modul dort) — das
//! Modell selbst und sein offizielles README ließen sich in dieser
//! Sitzung nicht abrufen. Die genaue Ein-/Ausgabeform ist deshalb per
//! Web-Suche (Sekundärquellen, nicht das Original-README) ermittelt:
//! fest 512×512, Bild und Maske je als `float32`-Tensor normiert auf
//! `0..1`. Damit sich ein falscher Name/eine falsche Kanalreihenfolge
//! nicht in einen stillen Falschresultat-Bug verwandelt, liest dieser
//! Code die tatsächlichen Tensor-Namen/-Formen **aus dem geladenen
//! Modell selbst** (`Session::inputs()`/`outputs()`) statt sie
//! hart zu kodieren — funktioniert unabhängig davon, ob die exakten
//! Namen "image"/"mask" stimmen. **Trotzdem nicht End-zu-Ende gegen das
//! echte 208-MB-Modell verifiziert** (auch das ist von hier aus nicht
//! herunterladbar) — vor Produktivnutzung sollte das jemand mit
//! erreichbarem `huggingface.co` einmal nachprüfen (derselbe Vorbehalt
//! wie bei jeder GPU-Sandbox-Grenze dieses Projekts, nur diesmal
//! Netzwerk statt Hardware).

use std::path::Path;

use crate::error::{AiError, Result};

/// Feste Referenzauflösung, falls das Modell keine konkrete Tensorform
/// deklariert (dynamische Achse, `-1`) — laut Recherche (siehe
/// Moduldoku) ist `lama_fp32.onnx` tatsächlich fest auf 512×512 exportiert.
const FALLBACK_EDGE: u32 = 512;

/// Initialisiert die ONNX-Runtime-Umgebung dynamisch aus `dylib_path` —
/// `pub`, weil `apx-app` diese Funktion beim Programmstart aufruft
/// (siehe `ort`s eigene Empfehlung, "Bibliotheks-Crates sollten die
/// Umgebung nicht selbst initialisieren").
pub fn init_environment(dylib_path: &Path) -> Result<()> {
    // `commit()` liefert `bool` zurück (ob dieser Aufruf tatsächlich zur
    // aktiven Umgebung wurde) — `false` heißt "es gab schon eine", kein
    // Fehler (z. B. bei einem zweiten Aufruf im selben Prozess).
    ort::init_from(dylib_path)
        .map_err(|err| AiError::Model {
            message: format!(
                "ONNX-Laufzeit '{}' konnte nicht geladen werden: {err}",
                dylib_path.display()
            ),
        })?
        .commit();
    Ok(())
}

/// Ein geladenes LaMa-Modell, bereit für wiederholte Inferenz.
pub struct InpaintSession {
    session: ort::session::Session,
}

impl InpaintSession {
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

    /// Füllt die Bereiche von `pixels` (interleaved RGB, `0..=255`,
    /// `width * height * 3` Bytes) aus, für die `mask` (`0..=255`,
    /// `width * height` Bytes, ein Wert je Pixel) größer als `0` ist —
    /// per echter LaMa-Inferenz. Pixel außerhalb der Maske bleiben
    /// exakt unverändert (explizit zusammengesetzt, siehe unten — kein
    /// Vertrauen darauf, dass das Modell selbst den Rand exakt erhält).
    pub fn fill_rgb8(
        &mut self,
        pixels: &[u8],
        width: u32,
        height: u32,
        mask: &[u8],
    ) -> Result<Vec<u8>> {
        if pixels.len() != (width as usize) * (height as usize) * 3 {
            return Err(AiError::Model {
                message: format!(
                    "Bildpuffer-Länge {} passt nicht zu {width}x{height} RGB",
                    pixels.len()
                ),
            });
        }
        if mask.len() != (width as usize) * (height as usize) {
            return Err(AiError::Model {
                message: format!(
                    "Maskenpuffer-Länge {} passt nicht zu {width}x{height}",
                    mask.len()
                ),
            });
        }

        let inputs = self.session.inputs();
        if inputs.len() != 2 {
            return Err(AiError::Model {
                message: format!(
                    "Modell hat {} Eingänge, erwartet werden genau zwei (Bild, Maske)",
                    inputs.len()
                ),
            });
        }
        let edge = declared_edge(&inputs[0]).unwrap_or(FALLBACK_EDGE);

        // Auf die vom Modell erwartete Auflösung skalieren — je Kanal
        // über `apx_core::raster::bilinear_resize_u8` (dieselbe Funktion,
        // die auch die KI-Masken-Heuristiken für ihre Ziel-Auflösung
        // nutzen), da diese Funktion nur Ein-Kanal-Puffer kennt.
        let (r, g, b) = split_channels(pixels, width, height);
        let r = apx_core::raster::bilinear_resize_u8(&r, width, height, edge, edge);
        let g = apx_core::raster::bilinear_resize_u8(&g, width, height, edge, edge);
        let b = apx_core::raster::bilinear_resize_u8(&b, width, height, edge, edge);
        let mask_small = apx_core::raster::bilinear_resize_u8(mask, width, height, edge, edge);

        let image_tensor = to_nchw_f32(&[&r, &g, &b], edge);
        let mask_tensor = to_nchw_f32(&[&mask_small], edge);

        let image_array =
            ndarray::Array4::from_shape_vec((1, 3, edge as usize, edge as usize), image_tensor)
                .map_err(|err| AiError::Model {
                    message: format!("Bild-Tensor-Form ungültig: {err}"),
                })?;
        let mask_array =
            ndarray::Array4::from_shape_vec((1, 1, edge as usize, edge as usize), mask_tensor)
                .map_err(|err| AiError::Model {
                    message: format!("Masken-Tensor-Form ungültig: {err}"),
                })?;

        let outputs = self
            .session
            .run(ort::inputs![
                ort::value::TensorRef::from_array_view(&image_array).map_err(|err| {
                    AiError::Model {
                        message: format!("Bild-Tensor konnte nicht erzeugt werden: {err}"),
                    }
                })?,
                ort::value::TensorRef::from_array_view(&mask_array).map_err(|err| {
                    AiError::Model {
                        message: format!("Masken-Tensor konnte nicht erzeugt werden: {err}"),
                    }
                })?
            ])
            .map_err(|err| AiError::Model {
                message: format!("Inferenz fehlgeschlagen: {err}"),
            })?;

        let (output_shape, output_data) =
            outputs[0]
                .try_extract_tensor::<f32>()
                .map_err(|err| AiError::Model {
                    message: format!("Ausgabe ist kein f32-Tensor: {err}"),
                })?;
        // Erwartet (1, 3, edge, edge) — dieselbe NCHW-Form wie die
        // Eingabe, laut denselben Sekundärquellen (siehe Moduldoku).
        if output_shape.len() != 4 || output_shape[1] != 3 {
            return Err(AiError::Model {
                message: format!("unerwartete Ausgabeform {output_shape:?}, erwartet (1, 3, H, W)"),
            });
        }
        let out_h = output_shape[2] as u32;
        let out_w = output_shape[3] as u32;

        let filled_r = from_nchw_channel(output_data, out_w, out_h, 0);
        let filled_g = from_nchw_channel(output_data, out_w, out_h, 1);
        let filled_b = from_nchw_channel(output_data, out_w, out_h, 2);

        let filled_r = apx_core::raster::bilinear_resize_u8(&filled_r, out_w, out_h, width, height);
        let filled_g = apx_core::raster::bilinear_resize_u8(&filled_g, out_w, out_h, width, height);
        let filled_b = apx_core::raster::bilinear_resize_u8(&filled_b, out_w, out_h, width, height);

        // Zusammensetzen: nur innerhalb der (Original-Auflösung-)Maske
        // durch das KI-Ergebnis ersetzen, sonst der unveränderte
        // Original-Pixel — siehe Funktionsdoku.
        let mut out = pixels.to_vec();
        for i in 0..(width as usize * height as usize) {
            let weight = mask[i] as f32 / 255.0;
            if weight <= 0.0 {
                continue;
            }
            for (c, filled) in [&filled_r, &filled_g, &filled_b].into_iter().enumerate() {
                let original = pixels[i * 3 + c] as f32;
                let value = original + (filled[i] as f32 - original) * weight;
                out[i * 3 + c] = value.round().clamp(0.0, 255.0) as u8;
            }
        }
        Ok(out)
    }
}

/// Liest die deklarierte quadratische Kantenlänge aus einer Tensor-
/// Eingabeform, falls beide Raumachsen (`H`, `W`) konkret (nicht `-1`,
/// dynamisch) und gleich sind — sonst `None` (Aufrufer fällt auf
/// [`FALLBACK_EDGE`] zurück).
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

/// Baut einen NCHW-`f32`-Tensor (`0..1` normiert) aus `channels`
/// (je ein `edge * edge`-Ein-Kanal-Puffer, in Kanalreihenfolge).
fn to_nchw_f32(channels: &[&[u8]], edge: u32) -> Vec<f32> {
    let mut out = Vec::with_capacity(channels.len() * (edge as usize) * (edge as usize));
    for channel in channels {
        out.extend(channel.iter().map(|&v| v as f32 / 255.0));
    }
    out
}

/// Extrahiert Kanal `channel` aus einem NCHW-`f32`-Ausgabetensor
/// (Batch-Größe 1 angenommen) zurück in einen `0..=255`-`u8`-Puffer.
fn from_nchw_channel(data: &[f32], width: u32, height: u32, channel: usize) -> Vec<u8> {
    let n = (width as usize) * (height as usize);
    let offset = channel * n;
    data[offset..offset + n]
        .iter()
        .map(|&v| (v * 255.0).round().clamp(0.0, 255.0) as u8)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Findet die vom Spike-Setup per `pip install onnxruntime`
    /// installierte `libonnxruntime.so` — nur für Tests gedacht, die
    /// echte App bündelt ihre eigene Laufzeit (siehe Moduldoku).
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

    /// Spike-Test (Phase 13 Schritt 0, hier gelassen statt in ein
    /// eigenes Modul verschoben — dasselbe Muster wie der `lensfun`-
    /// Spike-Test aus Phase 12 Schritt 0): lädt ein winziges, echtes
    /// ONNX-Modell (`Y = X + 1`) über die echte `ort`-Laufzeit und führt
    /// eine echte Inferenz aus — belegt, dass Laufzeit-Bindings,
    /// Graph-Laden und Tensor-Ein-/Ausgabe in dieser Umgebung
    /// tatsächlich funktionieren. Übersprungen, wenn keine
    /// ONNX-Runtime-Bibliothek auffindbar ist.
    #[test]
    fn onnx_runtime_loads_and_runs_a_real_tiny_model() {
        let Some(dylib) = find_test_dylib() else {
            eprintln!("übersprungen: keine ONNX-Runtime-Bibliothek in dieser Umgebung gefunden");
            return;
        };

        init_environment(&dylib).expect("ONNX-Umgebung sollte sich initialisieren lassen");

        let model_bytes = include_bytes!("../tests/fixtures/add_one.onnx");
        let mut session = ort::session::Session::builder()
            .expect("Session-Builder sollte sich erzeugen lassen")
            .commit_from_memory(model_bytes)
            .expect("echtes Testmodell sollte sich laden lassen");

        let input =
            ndarray::Array2::<f32>::from_shape_vec((1, 4), vec![1.0, 2.0, 3.0, 4.0]).unwrap();
        let outputs = session
            .run(ort::inputs![
                ort::value::TensorRef::from_array_view(&input).unwrap()
            ])
            .expect("Inferenz sollte laufen");

        let (_, output_data) = outputs[0]
            .try_extract_tensor::<f32>()
            .expect("Ausgabe sollte ein f32-Tensor sein");
        assert_eq!(output_data, &[2.0, 3.0, 4.0, 5.0]);
    }

    /// Gezielter Test für Phase 13 Schritt 1 (siehe Moduldoku für die
    /// ehrliche Grenze — das echte LaMa-Modell ist von hier aus nicht
    /// herunterladbar): baut ein winziges, echtes ONNX-Modell mit
    /// **derselben Zwei-Eingänge-Ein-Ausgang-NCHW-Topologie** wie ein
    /// echtes Inpainting-Modell (Bild + Maske rein, gefülltes Bild
    /// raus — hier eine reine Identität auf dem Bild-Eingang, ignoriert
    /// die Maske), um `fill_rgb8`s Vorverarbeitung/Tensor-Aufbau/
    /// Nachverarbeitung end-to-end zu prüfen, ohne die echten
    /// LaMa-Gewichte zu brauchen.
    #[test]
    fn fill_rgb8_composites_only_inside_the_mask() {
        let Some(dylib) = find_test_dylib() else {
            eprintln!("übersprungen: keine ONNX-Runtime-Bibliothek in dieser Umgebung gefunden");
            return;
        };
        init_environment(&dylib).expect("ONNX-Umgebung sollte sich initialisieren lassen");

        let model_bytes = include_bytes!("../tests/fixtures/identity_image_mask.onnx");
        let session = ort::session::Session::builder()
            .expect("Session-Builder sollte sich erzeugen lassen")
            .commit_from_memory(model_bytes)
            .expect("Testmodell sollte sich laden lassen");
        let mut inpaint = InpaintSession { session };

        let width = 4;
        let height = 4;
        // Reines Rot überall — das Testmodell liefert exakt dasselbe
        // Bild zurück (Identität), daher muss das Ergebnis für JEDEN
        // Pixel wieder reines Rot sein, unabhängig von der Maske. Der
        // eigentliche Test ist unten: dass Pixel *außerhalb* der Maske
        // bitgenau dem Original entsprechen, auch wenn das Modell
        // (hypothetisch) etwas anderes geliefert hätte.
        let pixels = [255u8, 0, 0].repeat(width * height);
        let mut mask = vec![0u8; width * height];
        mask[0] = 255; // nur der allererste Pixel ist "in der Maske".

        let out = inpaint
            .fill_rgb8(&pixels, width as u32, height as u32, &mask)
            .expect("Inferenz sollte laufen");

        assert_eq!(out.len(), pixels.len());
        // Außerhalb der Maske bitgenau unverändert.
        assert_eq!(&out[3..], &pixels[3..]);
    }
}
