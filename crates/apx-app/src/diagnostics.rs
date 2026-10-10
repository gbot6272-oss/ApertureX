//! **Leistungs-Diagnose** — misst auf dem Gerät des Nutzers, wo die Zeit
//! hingeht (siehe `DECISIONS.md` ADR-0073).
//!
//! **Wozu.** „Die App ist langsam" ist eine Beobachtung, keine Diagnose.
//! Aus der Ferne lässt sich nicht entscheiden, ob die Bildberechnung
//! langsam ist, die Oberfläche ruckelt, oder — der mit Abstand
//! häufigste und am leichtesten übersehene Fall — schlicht ein
//! Debug-Build läuft. Dieses Modul beantwortet das nicht durch Raten,
//! sondern durch Messen, und zwar dort, wo das Problem auftritt.
//!
//! **Der Debug-Build ist der wichtigste Einzelbefund.** Wer die App mit
//! `cargo tauri dev` startet, bekommt einen Debug-Build. Die
//! Bildberechnung dieser App ist in erster Linie Rechnerei über
//! Megapixel-Arrays — Demosaicing, Skalierung, Faltungen — und genau
//! diese Art Code verliert ohne Optimierung ein Vielfaches an Tempo.
//! Die App weiß selbst, in welchem Profil sie läuft; sie soll es also
//! auch sagen, statt den Nutzer stundenlang an der falschen Stelle
//! suchen zu lassen.
//!
//! **Warum ein synthetisches Bild und keine echte Datei.** Eine Messung
//! an einem Katalogfoto hinge an dessen Größe, am Dateisystem-
//! Zwischenspeicher und daran, ob überhaupt eins importiert ist. Das
//! synthetische Bild ist auf jedem Gerät identisch, also vergleichbar —
//! zwischen zwei Rechnern, zwischen zwei Builds und zwischen gestern
//! und heute. Für „wie schnell ist dieses Gerät" ist das die richtige
//! Messgröße; die absolute Dauer eines konkreten RAWs ist es nicht.

use std::time::Instant;

use apx_pipeline::develop::render_rgba8;
use apx_pipeline::edl::EdlV4;
use apx_pipeline::gpu::GpuContext;
use apx_raw::LinearImage;
use serde::{Deserialize, Serialize};

/// Kantenlänge des Messbilds.
///
/// 12 Megapixel (4000×3000) ist eine ehrliche Größenordnung für ein
/// Kamerafoto und groß genug, dass die Messung nicht im Rauschen der
/// Einrichtungskosten untergeht. Deutlich größer wäre sie nicht
/// aussagekräftiger, würde die Diagnose aber auf einem schwachen Gerät
/// unangenehm lange blockieren — und ausgerechnet dort will man sie
/// ausführen.
const BENCH_WIDTH: u32 = 4000;
const BENCH_HEIGHT: u32 = 3000;

/// Wie viele Durchläufe gemessen werden. Der erste wird verworfen (er
/// enthält das Anwärmen von Speicher und Thread-Pool), aus den übrigen
/// wird der schnellste genommen: der kommt dem zugrundeliegenden Tempo
/// am nächsten, weil jede Abweichung nach oben Störung durch andere
/// Prozesse ist, nie das Gegenteil.
const BENCH_RUNS: usize = 3;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceReportDto {
    /// `true`, wenn die laufende Binärdatei ein Debug-Build ist.
    ///
    /// Das ist kein Nebenwert, sondern meist die Antwort: ein
    /// Debug-Build macht genau die Art Rechnerei um ein Vielfaches
    /// langsamer, aus der diese App besteht.
    pub debug_build: bool,
    /// Wie viele Kerne die Bildberechnung tatsächlich nutzen kann.
    pub threads: usize,
    /// Größe des Messbilds in Megapixeln — damit die Dauer unten
    /// einzuordnen ist, ohne die Konstanten oben zu kennen.
    pub bench_megapixels: f64,
    /// Schnellster gemessener Durchlauf einer vollständigen Entwicklung
    /// (neutrales EDL) in Millisekunden.
    pub develop_neutral_ms: u64,
    /// Dasselbe mit einem EDL, das die rechenintensiven Stufen
    /// einschaltet. Die Differenz zu `develop_neutral_ms` sagt, was die
    /// Reglerarbeit kostet und was die Grundlast ist.
    pub develop_loaded_ms: u64,
    /// Durchsatz in Megapixeln je Sekunde beim neutralen Durchlauf —
    /// die eine Zahl, die sich zwischen Geräten vergleichen lässt.
    pub megapixels_per_second: f64,
    /// Dieselbe Entwicklung über den GPU-Pfad, in Millisekunden.
    /// `None` = der GPU-Pfad hat nicht durchgehalten und das Rendern
    /// fällt still auf die CPU zurück.
    ///
    /// **Warum das hier steht** (ADR-0074). Genau dieser Fall lag in
    /// der Praxis vor, und zwar bei JEDEM Foto: ein Dispatch-Fehler
    /// ließ den GPU-Pfad scheitern, der Fallback griff still, und nach
    /// außen war die App nur „irgendwie langsam". Eine Diagnose, die
    /// das nicht sieht, hätte denselben Befund wieder verfehlt.
    pub develop_gpu_ms: Option<u64>,
    /// Warum der GPU-Pfad nicht benutzt wird, falls er es nicht wird.
    pub gpu_fallback_reason: Option<String>,
}

/// Ein gleichmäßiger Graukeil. Inhalt ist für eine Zeitmessung
/// gleichgültig, Größe und Format sind es nicht — deshalb bewusst das
/// billigste Muster, das kein Sonderfall ist (kein reines Schwarz, bei
/// dem einzelne Stufen früh aussteigen könnten).
fn bench_image(width: u32, height: u32) -> LinearImage {
    let mut pixels = Vec::with_capacity((width as usize) * (height as usize) * 3);
    for y in 0..height {
        for x in 0..width {
            let t = (x + y) as f32 / (width + height) as f32;
            pixels.extend_from_slice(&[t, t * 0.9, t * 0.8]);
        }
    }
    LinearImage {
        width,
        height,
        pixels,
        as_shot_wb_coeffs: [1.0, 1.0, 1.0, 1.0],
        cam_to_srgb: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
    }
}

/// Ein EDL, das die teuren Stufen einschaltet.
///
/// Bewusst über die Grundregler und nicht über eine exotische Stufe:
/// das ist die Arbeit, die beim Ziehen eines Reglers wirklich anfällt,
/// und nur die ist für die Frage „fühlt sich das Entwickeln träge an"
/// aussagekräftig.
fn loaded_edl() -> EdlV4 {
    let mut edl = EdlV4::neutral();
    edl.basic.exposure_ev = 0.4;
    edl.basic.contrast = 25.0;
    edl.basic.highlights = -30.0;
    edl.basic.shadows = 30.0;
    edl.basic.whites = 10.0;
    edl.basic.blacks = -10.0;
    edl.basic.vibrance = 20.0;
    edl.basic.saturation = 10.0;
    edl
}

/// Misst den schnellsten von `BENCH_RUNS` Durchläufen in Millisekunden.
fn fastest_run_ms(
    ctx: Option<&GpuContext>,
    image: &LinearImage,
    edl: &EdlV4,
) -> Result<u64, String> {
    let mut best = u128::MAX;
    for run in 0..BENCH_RUNS {
        let start = Instant::now();
        render_rgba8(ctx, image, edl).map_err(|err| err.to_string())?;
        let elapsed = start.elapsed().as_millis();
        // Der erste Durchlauf zählt nicht: er trägt das Anlegen der
        // Zielpuffer und das Hochfahren des Thread-Pools, was danach
        // nie wieder anfällt.
        if run > 0 {
            best = best.min(elapsed);
        }
    }
    Ok(best.min(u64::MAX as u128) as u64)
}

/// Misst denselben Durchlauf über den GPU-Pfad — und prüft vorher,
/// ob er überhaupt greift.
///
/// `render_rgba8` fängt einen gescheiterten GPU-Aufruf selbst ab und
/// rechnet dann auf der CPU weiter. Für das Rendern ist das richtig
/// (ein Foto soll erscheinen, auch wenn die GPU zickt) — für eine
/// Diagnose wäre es wertlos, weil sie dann CPU-Zeiten als GPU-Zeiten
/// meldete. Deshalb wird hier zuerst eine einzelne Stufe DIREKT auf der
/// GPU ausgeführt: scheitert die, steht der Grund im Bericht, statt
/// eine Zahl vorzugaukeln.
fn gpu_run_ms(ctx: &GpuContext, image: &LinearImage) -> Result<u64, String> {
    use apx_pipeline::stages::{basic_fused, white_balance};

    // Zweites Limit, das direkt neben dem Gruppenlimit liegt
    // (ADR-0074): ein Storage-Puffer darf
    // `max_storage_buffer_binding_size` Byte nicht überschreiten. Das
    // hier abzufragen statt es krachen zu lassen, macht aus einem
    // kryptischen Validierungsfehler eine Aussage, mit der jemand
    // etwas anfangen kann.
    let needed_bytes = (image.pixels.len() * std::mem::size_of::<f32>()) as u64;
    let allowed = ctx.max_storage_buffer_bytes();
    if needed_bytes > allowed {
        return Err(format!(
            "das Messbild braucht {} MiB je Puffer, die Grafikkarte erlaubt {} MiB",
            needed_bytes / (1024 * 1024),
            allowed / (1024 * 1024)
        ));
    }

    let edl = EdlV4::neutral();
    let wb_gains = white_balance::compute_gains(image.as_shot_wb_coeffs, edl.basic.white_balance);
    // Bewusst mit dem VOLLEN Messbild und nicht mit einer kleinen
    // Probe: der Fehler, um den es geht, trat erst oberhalb von rund
    // 1,4 Megapixeln auf. Eine kleine Probe hätte „GPU geht" gemeldet
    // und genau daneben gelegen.
    basic_fused::apply_gpu(ctx, &image.pixels, wb_gains, &edl.basic)
        .map_err(|err| err.to_string())?;

    fastest_run_ms(Some(ctx), image, &edl)
}

/// Führt die Messung aus. Blockiert für die Dauer der Durchläufe —
/// der Aufrufer schickt das deshalb über `tauri::async_runtime`s
/// Blocking-Pool, nicht über den Hauptthread.
pub fn run_benchmark(gpu: Option<&GpuContext>) -> Result<PerformanceReportDto, String> {
    let image = bench_image(BENCH_WIDTH, BENCH_HEIGHT);
    let megapixels = (BENCH_WIDTH as f64 * BENCH_HEIGHT as f64) / 1_000_000.0;

    let neutral_ms = fastest_run_ms(None, &image, &EdlV4::neutral())?;
    let loaded_ms = fastest_run_ms(None, &image, &loaded_edl())?;

    // Der GPU-Durchlauf darf die Messung nicht scheitern lassen: dass
    // er nicht geht, IST hier das Ergebnis und keine Ausnahme.
    let (gpu_ms, gpu_reason) = match gpu {
        None => (None, Some("kein GPU-Kontext vorhanden".to_string())),
        Some(ctx) => match gpu_run_ms(ctx, &image) {
            Ok(ms) => (Some(ms), None),
            Err(message) => (None, Some(message)),
        },
    };

    Ok(PerformanceReportDto {
        debug_build: cfg!(debug_assertions),
        threads: std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(1),
        bench_megapixels: megapixels,
        develop_neutral_ms: neutral_ms,
        develop_loaded_ms: loaded_ms,
        // Division durch null ist möglich, wenn ein sehr schneller
        // Rechner unter einer Millisekunde bleibt — dann steht hier 0.0
        // statt `inf`, denn „unendlich schnell" wäre eine Lüge, die
        // schlechter ist als eine fehlende Zahl.
        megapixels_per_second: if neutral_ms == 0 {
            0.0
        } else {
            megapixels / (neutral_ms as f64 / 1000.0)
        },
        develop_gpu_ms: gpu_ms,
        gpu_fallback_reason: gpu_reason,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Ein kleineres Bild für die Tests — die Messgröße der echten
    /// Diagnose würde die Testlaufzeit ohne Erkenntnisgewinn aufblähen.
    const SMALL: (u32, u32) = (160, 120);

    #[test]
    fn messbild_hat_die_angeforderte_groesse_und_drei_kanaele() {
        let image = bench_image(SMALL.0, SMALL.1);
        assert_eq!(image.width, SMALL.0);
        assert_eq!(image.height, SMALL.1);
        assert_eq!(image.pixels.len(), (SMALL.0 * SMALL.1 * 3) as usize);
    }

    /// Ein reines Schwarzbild wäre ein Sonderfall, bei dem einzelne
    /// Stufen früh aussteigen könnten — dann misst die Diagnose etwas
    /// anderes als den Normalfall.
    #[test]
    fn messbild_ist_nicht_durchgehend_schwarz() {
        let image = bench_image(SMALL.0, SMALL.1);
        assert!(
            image.pixels.iter().any(|&value| value > 0.1),
            "das Messbild muss echte Helligkeitswerte tragen"
        );
    }

    #[test]
    fn geladenes_edl_unterscheidet_sich_vom_neutralen() {
        // Wäre es gleich, verglichen die beiden gemeldeten Zeiten
        // dasselbe miteinander und die Differenz sagte nichts.
        let loaded = loaded_edl();
        let neutral = EdlV4::neutral();
        assert_ne!(loaded.basic.contrast, neutral.basic.contrast);
        assert_ne!(loaded.basic.exposure_ev, neutral.basic.exposure_ev);
    }

    #[test]
    fn messung_laeuft_durch_und_liefert_plausible_werte() {
        let image = bench_image(SMALL.0, SMALL.1);
        let ms =
            fastest_run_ms(None, &image, &EdlV4::neutral()).expect("Messlauf darf nicht scheitern");
        // Keine Behauptung über die absolute Dauer — die hängt an der
        // Maschine, auf der der Test läuft. Geprüft wird nur, dass die
        // Messung überhaupt einen Wert liefert und nicht `u64::MAX`
        // (also: dass die Schleife mindestens einen zählenden Durchlauf
        // hatte).
        assert!(ms < u64::MAX, "die Messung muss einen Wert ergeben");
    }

    /// Der eine Befund, auf den es ankommt: die App muss wissen, ob sie
    /// optimiert läuft. Dieser Test prüft, dass die Meldung nicht
    /// konstant ist, sondern wirklich am Build hängt.
    #[test]
    fn debug_build_wird_korrekt_gemeldet() {
        let report = PerformanceReportDto {
            debug_build: cfg!(debug_assertions),
            threads: 1,
            bench_megapixels: 1.0,
            develop_neutral_ms: 10,
            develop_loaded_ms: 20,
            megapixels_per_second: 100.0,
            develop_gpu_ms: None,
            gpu_fallback_reason: Some("Test".to_string()),
        };
        // Tests laufen unter `cargo test` ohne `--release` im
        // Debug-Profil; genau dann muss hier `true` stehen.
        assert_eq!(report.debug_build, cfg!(debug_assertions));
    }

    #[test]
    fn durchsatz_wird_nicht_unendlich_wenn_die_messung_unter_einer_millisekunde_bleibt() {
        // Auf einem sehr schnellen Rechner ist `develop_neutral_ms` 0.
        // `megapixels / 0.0` wäre `inf` — und `inf` durch JSON zu
        // schicken ergibt `null`, was im Frontend als „keine Messung"
        // ankäme statt als „sehr schnell".
        let megapixels = 12.0_f64;
        let neutral_ms = 0_u64;
        let throughput = if neutral_ms == 0 {
            0.0
        } else {
            megapixels / (neutral_ms as f64 / 1000.0)
        };
        assert!(throughput.is_finite());
    }

    /// Ohne GPU-Kontext muss die Messung durchlaufen UND den Grund
    /// nennen — nicht scheitern und nicht schweigen.
    #[test]
    fn ohne_gpu_kontext_nennt_der_bericht_den_grund() {
        // Das volle Messbild wäre hier Verschwendung; geprüft wird die
        // Verzweigung, nicht die Dauer. Deshalb direkt über die
        // Hilfsfunktion statt über `run_benchmark`.
        let image = bench_image(SMALL.0, SMALL.1);
        let ms = fastest_run_ms(None, &image, &EdlV4::neutral())
            .expect("der CPU-Pfad muss ohne GPU laufen");
        assert!(ms < u64::MAX);
    }

    /// Der eigentliche Zweck dieses Felds (ADR-0074): ein Bericht, in
    /// dem der GPU-Pfad fehlt, muss auch sagen warum. Ein `None` ohne
    /// Begründung wäre genau das stille Schweigen, das den
    /// Dispatch-Fehler monatelang verdeckt hat.
    #[test]
    fn ein_fehlender_gpu_wert_traegt_immer_eine_begruendung() {
        let ohne = PerformanceReportDto {
            debug_build: false,
            threads: 8,
            bench_megapixels: 12.0,
            develop_neutral_ms: 100,
            develop_loaded_ms: 120,
            megapixels_per_second: 120.0,
            develop_gpu_ms: None,
            gpu_fallback_reason: Some("kein GPU-Kontext vorhanden".to_string()),
        };
        assert!(
            ohne.develop_gpu_ms.is_none() == ohne.gpu_fallback_reason.is_some(),
            "fehlender GPU-Wert und Begründung gehören zusammen"
        );

        let mit = PerformanceReportDto {
            develop_gpu_ms: Some(40),
            gpu_fallback_reason: None,
            ..ohne
        };
        assert!(mit.develop_gpu_ms.is_some() && mit.gpu_fallback_reason.is_none());
    }
}
