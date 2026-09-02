//! Echte ONNX-Modellinferenz (Phase 13, siehe `DECISIONS.md` ADR-0040 —
//! korrigiert ADR-0033 Punkt 1, das eine echte ONNX-Laufzeit noch für
//! unerreichbar hielt). Dieses Modul enthält bisher nur die
//! Umgebungs-Bootstrapping-Grundlage plus einen Spike-Test (Phase 13
//! Schritt 0); die eigentliche LaMa-Ausfüllfunktion kommt in Schritt 1.
//!
//! **Zwei Linking-Strategien, unterschiedlich für Sandbox vs. echte App:**
//! `ort`s Standard-Feature `download-binaries` holt beim Bauen ein
//! vorkompiliertes ONNX-Runtime-Binary von `cdn.pyke.io` — funktioniert
//! auf echten CI-Runnern/Nutzerrechnern normal, wird aber von dieser
//! Entwicklungs-Sandbox blockiert (Proxy-403, der Host steht nicht auf
//! der Freigabeliste dieser Sitzung — kein grundsätzliches Problem,
//! siehe `/root/.ccr/README.md`). Deshalb nutzt `apx-ai/Cargo.toml`
//! stattdessen `load-dynamic` (lädt eine vom System bereitgestellte
//! `.so`/`.dll`/`.dylib` zur Laufzeit) — das ist die einzige in dieser
//! Sandbox testbare Variante. Welche Strategie die ausgelieferte App
//! nutzt, entscheidet Schritt 1 (voraussichtlich `download-binaries`,
//! da auf echten Build-Maschinen unproblematisch).
//!
//! **Wichtig laut `ort`s eigener Dokumentation:** Bibliotheks-Crates
//! sollten die Umgebung nicht selbst initialisieren, das ist Aufgabe der
//! Anwendung — `init_environment` hier ist deshalb nur für den
//! Spike-Test gedacht; Schritt 1 verlagert den echten Aufruf nach
//! `apx-app`.

use crate::error::{AiError, Result};

/// Initialisiert die ONNX-Runtime-Umgebung dynamisch aus `dylib_path` —
/// `pub`, weil Schritt 1 diese Funktion von `apx-app` aus aufruft (siehe
/// Moduldoku, "Bibliotheks-Crates sollten die Umgebung nicht selbst
/// initialisieren"); hier schon vorhanden, weil der Spike-Test sie
/// braucht.
pub fn init_environment(dylib_path: &std::path::Path) -> Result<()> {
    // `commit()` liefert `bool` zurück (ob dieser Aufruf tatsächlich zur
    // aktiven Umgebung wurde) — `false` heißt "es gab schon eine", kein
    // Fehler (z. B. wenn ein anderer Test im selben Prozess bereits
    // initialisiert hat), siehe `ort`s eigene Dokumentation dazu.
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

#[cfg(test)]
mod tests {
    use super::*;
    use ort::session::Session;
    use ort::value::TensorRef;

    /// Findet die vom Spike-Setup per `pip install onnxruntime`
    /// installierte `libonnxruntime.so` — nur für diesen Test gedacht,
    /// die echte App (Schritt 1) bündelt ihre eigene Laufzeit.
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

    /// Spike-Test (Phase 13 Schritt 0): lädt ein winziges, echtes
    /// ONNX-Modell (`Y = X + 1`, per `onnx`-Python-Paket erzeugt, siehe
    /// `tests/fixtures/add_one.onnx`) über die echte `ort`-Laufzeit und
    /// führt eine echte Inferenz aus — kein trainiertes Modell nötig, um
    /// zu belegen, dass Laufzeit-Bindings, Graph-Laden und Tensor-Ein-/
    /// Ausgabe in dieser Umgebung tatsächlich funktionieren, bevor
    /// Schritt 1 darauf ein echtes LaMa-Modell aufsetzt. Übersprungen,
    /// wenn keine ONNX-Runtime-Bibliothek auffindbar ist (dasselbe
    /// „übersprungen statt fehlgeschlagen"-Muster wie die bestehenden
    /// GPU-Sandbox-Tests in `apx-pipeline`).
    #[test]
    fn onnx_runtime_loads_and_runs_a_real_tiny_model() {
        let Some(dylib) = find_test_dylib() else {
            eprintln!("übersprungen: keine ONNX-Runtime-Bibliothek in dieser Umgebung gefunden");
            return;
        };

        init_environment(&dylib).expect("ONNX-Umgebung sollte sich initialisieren lassen");

        let model_bytes = include_bytes!("../tests/fixtures/add_one.onnx");
        let mut session = Session::builder()
            .expect("Session-Builder sollte sich erzeugen lassen")
            .commit_from_memory(model_bytes)
            .expect("echtes Testmodell sollte sich laden lassen");

        let input =
            ndarray::Array2::<f32>::from_shape_vec((1, 4), vec![1.0, 2.0, 3.0, 4.0]).unwrap();
        let outputs = session
            .run(ort::inputs![TensorRef::from_array_view(&input).unwrap()])
            .expect("Inferenz sollte laufen");

        let (_, output_data) = outputs[0]
            .try_extract_tensor::<f32>()
            .expect("Ausgabe sollte ein f32-Tensor sein");
        assert_eq!(output_data, &[2.0, 3.0, 4.0, 5.0]);
    }
}
