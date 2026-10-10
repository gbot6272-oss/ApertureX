//! Gemeinsamer Hoch-/Ausführen-/Runterladen-Helfer für alle Regler-Module
//! in `crate::stages`: einen WGSL-Compute-Shader auf einem `f32`-Puffer
//! ausführen und das Ergebnis zurücklesen.
//!
//! Bind-Group-Layout, das jeder Shader, der über [`run_compute_f32`]
//! läuft, einhalten muss (Gruppe 0):
//! - `binding(0)`: `var<uniform> params: Params` — die Regler-Parameter
//! - `binding(1)`: `var<storage, read> input: array<f32>`
//! - `binding(2)`: `var<storage, read_write> output: array<f32>`
//!
//! Alle Phase-2-Regler sind 1:1 (Ausgabegröße = Eingabegröße) — daher
//! keine gesonderte `output_len`.
//!
//! **Vertrag für jeden Shader, der hier durchläuft** (ADR-0074): der
//! Dispatch ist ZWEIdimensional, weil eine Dimension höchstens
//! `max_compute_workgroups_per_dimension` Gruppen aufnimmt (in aller
//! Regel 65535). `gid.x` allein genügt deshalb nicht. Jeder Shader
//! nimmt `@builtin(num_workgroups)` entgegen und bildet den Index so:
//!
//! ```wgsl
//! @compute @workgroup_size(64)
//! fn main(
//!     @builtin(global_invocation_id) gid: vec3<u32>,
//!     @builtin(num_workgroups) groups: vec3<u32>,
//! ) {
//!     let i = gid.x + gid.y * groups.x * 64u;
//!     if (i >= arrayLength(&input_buf)) { return; }
//!     // ...
//! }
//! ```
//!
//! `shader_contract_tests` unten prüft das für jede `.wgsl`-Datei in
//! `stages/` — ein neuer Shader kann den Fehler also nicht still
//! wieder einführen.

use super::GpuContext;
use crate::error::{PipelineError, Result};
use bytemuck::Pod;

/// Verteilt die nötigen Workgroups auf X und Y.
///
/// **Der Fehler, den das behebt** (ADR-0074). Bis hierher ging alles in
/// die X-Dimension: `dispatch_workgroups(count, 1, 1)`. WebGPU begrenzt
/// jede Dimension aber auf `max_compute_workgroups_per_dimension`, in
/// aller Regel 65535. Bei 64 Elementen je Gruppe und drei Kanälen je
/// Pixel liegt die Grenze damit bei
///
/// > 65535 × 64 ÷ 3 = 1.398.080 Pixel, also knapp **1,4 Megapixel**.
///
/// Darüber scheiterte die Validierung, der Aufrufer fiel still auf den
/// CPU-Pfad zurück, und die GPU-Beschleunigung dieser App hat deshalb
/// für kein einziges realistisches Foto je gearbeitet — nicht einmal
/// für die verkleinerten Vorschauen, die schon bei 1,6 MP anfangen.
///
/// Die Aufteilung füllt X bis zum Limit und legt den Rest in Y. Die
/// letzte Zeile ist dann meist nur teilweise belegt; das fängt die
/// `arrayLength`-Schranke ab, die ohnehin in jedem Shader steht.
pub(super) fn workgroup_grid(
    element_count: u32,
    workgroup_size: u32,
    max_per_dim: u32,
) -> (u32, u32) {
    // Ein Gerät, das 0 meldet, gäbe es nicht; eine Division durch null
    // hier wäre aber ein Absturz statt eines Fallbacks, und das ist den
    // einen `max(1)` wert.
    let max_per_dim = max_per_dim.max(1);
    let workgroup_size = workgroup_size.max(1);
    let total = element_count.div_ceil(workgroup_size).max(1);
    if total <= max_per_dim {
        (total, 1)
    } else {
        (max_per_dim, total.div_ceil(max_per_dim))
    }
}

/// Führt `entry_point` aus `shader_source` auf `input` aus. `stage_name`
/// dient nur der Fehlerzuordnung (siehe [`PipelineError::ShaderCompile`]).
/// `workgroup_size` muss zur `@workgroup_size(...)`-Deklaration im Shader
/// passen.
pub fn run_compute_f32<Params: Pod>(
    ctx: &GpuContext,
    stage_name: &'static str,
    shader_source: &str,
    entry_point: &str,
    params: Params,
    input: &[f32],
    workgroup_size: u32,
) -> Result<Vec<f32>> {
    // Der eigentliche Ablauf steht in `super::workspace`: dort leben
    // die Pipelines und die Puffer, die den einzelnen Aufruf
    // überleben. Bis ADR-0075 legte DIESE Funktion bei jedem Aufruf
    // alles neu an — Shader übersetzen, Pipeline bauen, vier Puffer
    // belegen —, was bei zwei Megapixeln gemessene 228 statt 33
    // Millisekunden je Stufe kostete.
    let mut workspace = ctx
        .workspace
        .lock()
        .map_err(|_| PipelineError::GpuUnavailable {
            message: "GPU-Arbeitsbereich ist nicht mehr benutzbar (vergifteter Mutex)".to_string(),
        })?;
    workspace.run(
        &ctx.device,
        &ctx.queue,
        stage_name,
        shader_source,
        entry_point,
        bytemuck::bytes_of(&params),
        input,
        workgroup_size,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[repr(C)]
    #[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
    struct AddOffsetParams {
        offset: f32,
        _padding: [f32; 3],
    }

    const ADD_OFFSET_SHADER: &str = r#"
struct Params {
    offset: f32,
};

@group(0) @binding(0) var<uniform> params: Params;
@group(0) @binding(1) var<storage, read> input_buf: array<f32>;
@group(0) @binding(2) var<storage, read_write> output_buf: array<f32>;

@compute @workgroup_size(64)
fn main(
    @builtin(global_invocation_id) gid: vec3<u32>,
    @builtin(num_workgroups) groups: vec3<u32>,
) {
    let i = gid.x + gid.y * groups.x * 64u;
    if (i >= arrayLength(&input_buf)) {
        return;
    }
    output_buf[i] = input_buf[i] + params.offset;
}
"#;

    /// Beweist die Hoch-/Ausführen-/Runterladen-Mechanik unabhängig von
    /// echter Bildmathematik (siehe `PLAN.md`, Phase 2, Schritt 3): ein
    /// trivialer Shader, der einen konstanten Wert addiert.
    #[test]
    fn add_offset_roundtrip_matches_cpu_computation() {
        let ctx = match GpuContext::new_blocking() {
            Ok(ctx) => ctx,
            Err(_) => {
                // Siehe gpu::tests::context_construction_never_panics —
                // ohne verfügbaren Adapter kann dieser Test die eigentliche
                // GPU-Ausführung nicht prüfen. Schritt 8 verifiziert die
                // CI-Umgebung dafür gezielt (Mesa-Software-Adapter).
                eprintln!("übersprungen: kein GPU-Adapter in dieser Umgebung verfügbar");
                return;
            }
        };

        let input: Vec<f32> = (0..256).map(|i| i as f32).collect();
        let params = AddOffsetParams {
            offset: 10.0,
            _padding: [0.0; 3],
        };

        let output = run_compute_f32(
            &ctx,
            "test-add-offset",
            ADD_OFFSET_SHADER,
            "main",
            params,
            &input,
            64,
        )
        .expect("Shader-Ausführung sollte gelingen");

        let expected: Vec<f32> = input.iter().map(|&v| v + 10.0).collect();
        assert_eq!(output, expected);
    }

    /// Der Regressionstest zum Fund aus dem Nutzer-Log (ADR-0074).
    ///
    /// Die Zahlen sind keine erfundenen Beispiele, sondern genau die
    /// Gruppenzahlen, die das Log gemeldet hat — jede davon lag über
    /// dem Limit, jede führte zum stillen CPU-Fallback.
    #[test]
    fn workgroup_grid_stays_within_the_per_dimension_limit() {
        const MAX: u32 = 65535;
        const WG: u32 = 64;

        // (Bildbreite, Bildhöhe) aus dem Log, mal drei Kanäle.
        for (width, height, groups_in_log) in [
            (1911u32, 860u32, 77_038u32),
            (2256, 1015, 107_337),
            (3456, 1555, 251_910),
            (1361, 3024, 192_922),
            (1872, 842, 73_886),
        ] {
            let elements = width * height * 3;
            // Erst bestätigen, dass die Rechnung dieselbe ist wie die,
            // die das Gerät angestellt hat — sonst prüfte dieser Test
            // einen anderen Fall als den gemeldeten.
            assert_eq!(
                elements.div_ceil(WG),
                groups_in_log,
                "{width}×{height} ergab im Log {groups_in_log} Gruppen"
            );

            let (x, y) = workgroup_grid(elements, WG, MAX);
            assert!(x <= MAX, "X-Dimension {x} überschreitet das Limit");
            assert!(y <= MAX, "Y-Dimension {y} überschreitet das Limit");
            // Und die Aufteilung muss weiterhin jedes Element abdecken.
            assert!(
                (x as u64) * (y as u64) * (WG as u64) >= elements as u64,
                "{width}×{height}: {x}×{y} Gruppen decken {elements} Elemente nicht ab"
            );
        }
    }

    #[test]
    fn workgroup_grid_leaves_small_inputs_one_dimensional() {
        // Unterhalb des Limits soll nichts aufgeteilt werden: eine
        // zweite Dimension wäre hier nur unnötige Leerlauf-Arbeit.
        assert_eq!(workgroup_grid(64 * 100, 64, 65535), (100, 1));
        assert_eq!(workgroup_grid(1, 64, 65535), (1, 1));
        // Genau auf der Grenze noch eindimensional.
        assert_eq!(workgroup_grid(65535 * 64, 64, 65535), (65535, 1));
        // Ein Element darüber kippt es in die zweite Dimension.
        assert_eq!(workgroup_grid(65535 * 64 + 1, 64, 65535), (65535, 2));
    }

    #[test]
    fn workgroup_grid_covers_every_element_without_gaps() {
        // Vollständige Abdeckung ist die eigentliche Zusage: ein Loch
        // in der Aufteilung würde Pixel unbearbeitet lassen, und das
        // sähe man einem Foto nicht unbedingt an.
        for elements in [1u32, 63, 64, 65, 4_194_240, 4_194_241, 50_000_000] {
            let (x, y) = workgroup_grid(elements, 64, 65535);
            let covered = (x as u64) * (y as u64) * 64;
            assert!(
                covered >= elements as u64,
                "{elements} Elemente nicht abgedeckt"
            );
            // Und nicht masslos überdecken: höchstens eine volle Zeile
            // Verschnitt, sonst rechnet die GPU sinnlos.
            let row = (x as u64) * 64;
            assert!(
                covered - elements as u64 <= row,
                "{elements} Elemente: {covered} abgedeckt ist mehr als eine Zeile Verschnitt"
            );
        }
    }

    #[test]
    fn workgroup_grid_survives_absurd_limits() {
        // Ein Gerät, das 0 meldet, gibt es nicht — ein Absturz an
        // dieser Stelle wäre aber schlimmer als eine sinnlose Zahl.
        let (x, y) = workgroup_grid(1000, 64, 0);
        assert!(x >= 1 && y >= 1);
        let (x, y) = workgroup_grid(1000, 0, 65535);
        assert!(x >= 1 && y >= 1);
    }

    /// Führt einen Puffer über der alten 1,4-Megapixel-Grenze wirklich
    /// durch die GPU.
    ///
    /// Die reinen `workgroup_grid`-Tests oben prüfen die Arithmetik;
    /// nur dieser hier beweist, dass die Shader den zweidimensionalen
    /// Index auch richtig auflösen. Hätte ich nur die Arithmetik
    /// geprüft, wäre ein Shader, der weiter nur `gid.x` liest, grün
    /// durchgelaufen und hätte ab der zweiten Zeile Nullen geliefert.
    #[test]
    fn a_buffer_past_the_old_limit_round_trips_through_the_gpu() {
        let ctx = match GpuContext::new_blocking() {
            Ok(ctx) => ctx,
            Err(_) => {
                eprintln!("übersprungen: kein GPU-Adapter in dieser Umgebung verfügbar");
                return;
            }
        };

        let max_per_dim = ctx.device.limits().max_compute_workgroups_per_dimension;
        // Gerade so viele Elemente, dass eine zweite Dispatch-Zeile
        // nötig wird — aus dem ECHTEN Gerätelimit abgeleitet, nicht aus
        // einer angenommenen 65535, damit der Test auf jedem Adapter
        // das prüft, was er prüfen soll.
        let elements = (max_per_dim as u64 + 1) * 64;
        if elements > 32_000_000 {
            eprintln!(
                "übersprungen: Gerätelimit {max_per_dim} würde einen Puffer von                  {elements} Elementen erfordern"
            );
            return;
        }
        let elements = elements as usize;

        let (x, y) = workgroup_grid(elements as u32, 64, max_per_dim);
        assert!(
            y > 1,
            "der Test muss die zweite Dimension wirklich benutzen"
        );
        assert_eq!(x, max_per_dim);

        let input: Vec<f32> = (0..elements).map(|i| (i % 997) as f32).collect();
        let params = AddOffsetParams {
            offset: 10.0,
            _padding: [0.0; 3],
        };

        let output = run_compute_f32(
            &ctx,
            "test-large-buffer",
            ADD_OFFSET_SHADER,
            "main",
            params,
            &input,
            64,
        )
        .expect("ein Puffer über dem alten Limit muss auf der GPU durchlaufen");

        assert_eq!(output.len(), input.len());
        // Nicht nur die Länge: das letzte Element liegt in der zweiten
        // Dispatch-Zeile und ist damit genau der Wert, der vor dieser
        // Korrektur nie berechnet worden wäre.
        for index in [0usize, 1, elements / 2, elements - 2, elements - 1] {
            assert_eq!(
                output[index],
                input[index] + 10.0,
                "Element {index} von {elements} wurde nicht berechnet"
            );
        }
    }
}

/// Hält den Shader-Vertrag aus der Moduldoku maschinell fest.
///
/// Der Dispatch-Fehler (ADR-0074) war ein Vertragsbruch zwischen einer
/// Rust-Zeile und vierzehn Shadern, und nichts hat ihn bemerkt. Diese
/// Tests lesen die `.wgsl`-Dateien als Text — unelegant, aber es ist
/// die einzige Stelle, an der sich der Vertrag prüfen lässt, ohne für
/// jeden Shader einen GPU-Lauf zu brauchen.
#[cfg(test)]
mod shader_contract_tests {
    use std::path::{Path, PathBuf};

    fn stage_shaders() -> Vec<PathBuf> {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/stages");
        let mut out: Vec<PathBuf> = std::fs::read_dir(&dir)
            .expect("stages-Verzeichnis muss lesbar sein")
            .filter_map(|entry| entry.ok().map(|e| e.path()))
            .filter(|path| path.extension().is_some_and(|ext| ext == "wgsl"))
            .collect();
        out.sort();
        out
    }

    #[test]
    fn every_stage_shader_exists_and_is_found() {
        // Ohne diese Schranke wäre ein leeres Verzeichnis (falscher
        // Pfad, verschobene Dateien) ein grüner Test, der nichts
        // behauptet.
        assert!(
            stage_shaders().len() >= 14,
            "es wurden nur {} Shader gefunden",
            stage_shaders().len()
        );
    }

    #[test]
    fn no_stage_shader_derives_its_index_from_gid_x_alone() {
        let mut offenders = Vec::new();
        for path in stage_shaders() {
            let source = std::fs::read_to_string(&path).expect("Shader muss lesbar sein");
            let name = path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            if source.contains("let i = gid.x;") {
                offenders.push(format!("{name}: bildet den Index nur aus gid.x"));
            }
        }
        assert_eq!(
            offenders,
            Vec::<String>::new(),
            "diese Shader würden ab der zweiten Dispatch-Zeile nichts mehr berechnen"
        );
    }

    #[test]
    fn every_stage_shader_takes_num_workgroups_and_uses_both_dimensions() {
        let mut offenders = Vec::new();
        for path in stage_shaders() {
            let source = std::fs::read_to_string(&path).expect("Shader muss lesbar sein");
            let name = path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            if !source.contains("@builtin(num_workgroups)") {
                offenders.push(format!("{name}: nimmt `num_workgroups` nicht entgegen"));
            }
            if !source.contains("gid.y * groups.x") {
                offenders.push(format!("{name}: rechnet die zweite Dimension nicht ein"));
            }
        }
        assert_eq!(offenders, Vec::<String>::new());
    }

    /// Die Zeilenbreite im Shader (`groups.x * 64u`) muss zur
    /// `@workgroup_size`-Deklaration passen. Weichen sie voneinander ab,
    /// rechnet der Shader mit einem falschen Versatz und überspringt
    /// oder doppelt ganze Blöcke — ein Fehler, den man dem Ergebnis
    /// ansieht, aber erst bei grossen Bildern.
    #[test]
    fn the_row_stride_matches_the_declared_workgroup_size() {
        let mut offenders = Vec::new();
        for path in stage_shaders() {
            let source = std::fs::read_to_string(&path).expect("Shader muss lesbar sein");
            let name = path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            let Some(start) = source.find("@workgroup_size(") else {
                offenders.push(format!("{name}: keine @workgroup_size-Deklaration"));
                continue;
            };
            let rest = &source[start + "@workgroup_size(".len()..];
            let Some(end) = rest.find(')') else {
                offenders.push(format!("{name}: @workgroup_size nicht geschlossen"));
                continue;
            };
            let declared = rest[..end]
                .split(',')
                .next()
                .unwrap_or("")
                .trim()
                .to_string();
            let expected = format!("gid.y * groups.x * {declared}u");
            if !source.contains(&expected) {
                offenders.push(format!(
                    "{name}: @workgroup_size({declared}), aber die Zeilenbreite lautet nicht `{expected}`"
                ));
            }
        }
        assert_eq!(offenders, Vec::<String>::new());
    }
}
