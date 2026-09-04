//! Automatische Untertitel per OpenAI Whisper (Phase 17 Schritt 5, siehe
//! `DECISIONS.md` ADR-0045) — vollständig lokal, keine Cloud-Spracherkennung
//! (dieselbe Linie wie jede andere KI-Funktion dieses Projekts).
//!
//! **Lizenzen (real geprüft, siehe ADR-0045s Recherche-Tabelle):** OpenAI
//! Whisper selbst (Modellgewichte/Code) MIT; `whisper.cpp` (die C/C++-
//! Inferenz-Engine, die dieses Modul über `whisper-rs` einbindet) MIT;
//! `whisper-rs` selbst Unlicense.
//!
//! **Modell:** `ggml-base.en.bin` (englisches Basismodell, ~142 MiB) —
//! Download-URL und SHA1-Prüfsumme real aus `whisper.cpp`s eigenem
//! `models/download-ggml-model.sh` bzw. `models/README.md` übernommen
//! (`github.com` erreichbar in dieser Sitzung, siehe
//! `apx_app::commands::WHISPER_MODEL_SHA1`s Moduldoku) — anders als beim
//! LaMa-Inpainting-Modell (Phase 13, `huggingface.co` dort blockiert) also
//! keine offene Lücke, sondern eine echte, quellenbelegte Prüfsumme.
//! **Nur SHA1** (40 Hex-Zeichen), weil `whisper.cpp` selbst nur SHA1
//! veröffentlicht — bewusst nicht durch eine erfundene SHA-256-Prüfsumme
//! ersetzt.
//!
//! **Hinter dem Cargo-Feature `subtitles`** (siehe `Cargo.toml`, dieselbe
//! Konvention wie `people`/`apx-tether`s `tethering`) — `whisper-rs` baut
//! `whisper.cpp`s gebündelten C/C++-Quellcode lokal per `cmake`, ein
//! spürbarer zusätzlicher Compile-Schritt, den der normale
//! `cargo check --workspace` nicht mittragen soll.
//!
//! **Bewusste Vereinfachung:** nur ein einziges, fest gewähltes Modell
//! (`base.en`, Englisch) statt einer Modellauswahl — deckt den
//! Hauptanwendungsfall ab, ohne die Oberfläche mit einer Modellgrößen-/
//! Sprachenwahl zu überladen (derselbe "nicht so krass wie CapCut"-Rahmen
//! wie der Rest von Phase 17).

use std::path::Path;

use crate::error::{AiError, Result};

/// Ein transkribierter Zeitabschnitt — `start_ms`/`end_ms` relativ zum
/// Beginn des übergebenen Audios. Direkt kompatibel mit
/// `apx_export::timeline::TimelineTextOverlay`s Zeitfeldern (Phase 17
/// Schritt 4) — die aufrufende Seite (`apx-app`) baut daraus Overlay-
/// Einträge, ohne eine zweite Zeitrepräsentation zu brauchen.
#[derive(Debug, Clone, PartialEq)]
pub struct SubtitleSegment {
    pub start_ms: i64,
    pub end_ms: i64,
    pub text: String,
}

/// Ein geladenes Whisper-Modell, bereit für wiederholte Transkription.
pub struct WhisperSession {
    context: whisper_rs::WhisperContext,
}

impl WhisperSession {
    /// Lädt `model_path` (die vom Nutzer heruntergeladene `ggml-*.bin`-Datei).
    pub fn load(model_path: &Path) -> Result<Self> {
        let path_str = model_path.to_str().ok_or_else(|| AiError::Model {
            message: "Modellpfad enthält ungültige UTF-8-Zeichen".to_string(),
        })?;
        let context = whisper_rs::WhisperContext::new_with_params(
            path_str,
            whisper_rs::WhisperContextParameters::default(),
        )
        .map_err(|err| AiError::Model {
            message: format!(
                "Whisper-Modell '{}' konnte nicht geladen werden: {err}",
                model_path.display()
            ),
        })?;
        Ok(Self { context })
    }

    /// Transkribiert `samples` (f32-PCM, **16 kHz, mono** — Whisper
    /// verlangt exakt dieses Format, siehe `apx_app::commands::
    /// extract_audio_pcm_f32`s Moduldoku für die `ffmpeg`-
    /// Aufbereitung) in Zeitabschnitte mit Text. `language`: ISO-639-1-
    /// Kürzel (z. B. `"de"`) oder `None` für Auto-Erkennung.
    pub fn transcribe(
        &self,
        samples: &[f32],
        language: Option<&str>,
    ) -> Result<Vec<SubtitleSegment>> {
        let mut state = self.context.create_state().map_err(|err| AiError::Model {
            message: format!("Whisper-Sitzung konnte nicht erzeugt werden: {err}"),
        })?;

        let mut params =
            whisper_rs::FullParams::new(whisper_rs::SamplingStrategy::Greedy { best_of: 1 });
        params.set_language(language);
        params.set_print_progress(false);
        params.set_print_special(false);
        params.set_print_realtime(false);
        params.set_print_timestamps(false);

        state.full(params, samples).map_err(|err| AiError::Model {
            message: format!("Transkription fehlgeschlagen: {err}"),
        })?;

        // `full_n_segments` gibt die Anzahl direkt zurück (kein
        // `Result`) — anders als die übrigen `whisper-rs`-Aufrufe hier,
        // die alle einen `WhisperError` zurückgeben können.
        let segment_count = state.full_n_segments();

        let mut segments = Vec::with_capacity(segment_count.max(0) as usize);
        for index in 0..segment_count {
            let Some(segment) = state.get_segment(index) else {
                continue;
            };
            let text = segment.to_str_lossy().map_err(|err| AiError::Model {
                message: format!("Segmenttext {index} konnte nicht gelesen werden: {err}"),
            })?;
            let text = text.trim();
            if text.is_empty() {
                continue;
            }
            // `whisper.cpp` liefert Zeitstempel in Zehntel-Sekunden
            // (Centisekunden) — mal 10 für Millisekunden.
            segments.push(SubtitleSegment {
                start_ms: segment.start_timestamp() * 10,
                end_ms: segment.end_timestamp() * 10,
                text: text.to_string(),
            });
        }
        Ok(segments)
    }
}
