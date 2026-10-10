//! **Wiederverwendeter GPU-Arbeitsbereich** — Pipelines und Puffer, die
//! den einzelnen Stufenaufruf überleben (siehe `DECISIONS.md` ADR-0075).
//!
//! **Was vorher passierte.** [`super::dispatch::run_compute_f32`] legte
//! bei **jedem** Aufruf alles neu an: das WGSL wurde neu übersetzt, die
//! Compute-Pipeline neu gebaut und vier Puffer frisch belegt, drei davon
//! in voller Bildgröße. Bei einem zwei Megapixel grossen Vorschaubild
//! sind das dreimal 22 MiB, die der Treiber je Regler-Tick und je Stufe
//! neu beschafft, befüllt und wieder freigibt.
//!
//! **Gemessen** (2 Megapixel, eine Stufe, derselbe Adapter, jeweils der
//! beste von drei Durchläufen):
//!
//! | Weg | Zeit |
//! | --- | --- |
//! | alles pro Aufruf neu (vorher) | 228 ms |
//! | Pipeline + Puffer wiederverwendet | **33 ms** |
//! | zusätzlich das Bild GPU-resident | 13 ms |
//!
//! Dieses Modul setzt die mittlere Zeile um. Die untere verlangt, dass
//! die Zwischenergebnisse zwischen den Stufen gar nicht erst zur CPU
//! zurückwandern — das ist ein Umbau des Renderpfads und steht noch aus
//! (ADR-0075, „Was offen bleibt").
//!
//! **Warum ein Mutex und keine Puffer je Aufruf.** Ein gemeinsamer
//! Arbeitsbereich heisst, dass zwei gleichzeitige Renderläufe sich
//! dieselben Puffer teilen würden — und das wäre ein Datenfehler, kein
//! Tempoproblem. Der Mutex serialisiert sie. Das kostet nichts, was
//! nicht ohnehin verloren wäre: es gibt genau ein Gerät, und zwei
//! parallele Läufe würden sich dort ohnehin gegenseitig ausbremsen.

use std::collections::HashMap;

use crate::error::{PipelineError, Result};

/// Eine einmal übersetzte und gebaute Compute-Pipeline.
struct CachedPipeline {
    /// Der Quelltext, aus dem sie gebaut wurde.
    ///
    /// Mitgeführt, damit ein Stufenname, der später mit einem anderen
    /// Shader aufgerufen wird, nicht stillschweigend die alte Pipeline
    /// bekommt. Der Vergleich einer wenige Kilobyte grossen Zeichenkette
    /// ist gegenüber allem anderen hier kostenlos — eine falsche
    /// Pipeline wäre es nicht.
    source: String,
    entry_point: String,
    pipeline: wgpu::ComputePipeline,
    /// Die Bind-Group muss neu gebaut werden, sobald die Puffer
    /// gewechselt haben (Grössenänderung) — deshalb die Generation.
    bind_group: Option<(u64, wgpu::BindGroup)>,
}

/// Die drei grossen Puffer plus der kleine Parameterpuffer.
struct PooledBuffers {
    /// Zählt hoch, sobald neu belegt wurde. Zwischengespeicherte
    /// Bind-Groups aus einer älteren Generation sind dann ungültig.
    generation: u64,
    bytes: u64,
    params_bytes: u64,
    input: wgpu::Buffer,
    output: wgpu::Buffer,
    staging: wgpu::Buffer,
    params: wgpu::Buffer,
}

#[derive(Default)]
pub(crate) struct Workspace {
    pipelines: HashMap<&'static str, CachedPipeline>,
    buffers: Option<PooledBuffers>,
}

impl Workspace {
    /// Legt die Puffer an oder vergrössert sie.
    ///
    /// Verkleinert wird nie: Vorschaugrössen wechseln im Betrieb hin und
    /// her (Ziehen 1280, Loslassen volle Anzeigegrösse), und bei jedem
    /// Wechsel neu zu belegen wäre genau der Aufwand, den dieses Modul
    /// vermeiden soll.
    fn ensure_buffers(&mut self, device: &wgpu::Device, bytes: u64, params_bytes: u64) {
        let params_bytes = params_bytes.max(16);
        let fits = self
            .buffers
            .as_ref()
            .is_some_and(|b| b.bytes >= bytes && b.params_bytes >= params_bytes);
        if fits {
            return;
        }

        let bytes = bytes.max(self.buffers.as_ref().map_or(0, |b| b.bytes));
        let params_bytes = params_bytes.max(self.buffers.as_ref().map_or(0, |b| b.params_bytes));
        let generation = self.buffers.as_ref().map_or(0, |b| b.generation) + 1;

        self.buffers = Some(PooledBuffers {
            generation,
            bytes,
            params_bytes,
            input: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("apx-pipeline-stage-input"),
                size: bytes,
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
            output: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("apx-pipeline-stage-output"),
                size: bytes,
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
                mapped_at_creation: false,
            }),
            staging: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("apx-pipeline-stage-staging"),
                size: bytes,
                usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
            params: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("apx-pipeline-stage-params"),
                size: params_bytes,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
        });
    }

    /// Führt eine Stufe aus. Alles Wiederverwendbare wird
    /// wiederverwendet; neu gebaut wird nur, was sich geändert hat.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn run(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        stage_name: &'static str,
        shader_source: &str,
        entry_point: &str,
        params_bytes: &[u8],
        input: &[f32],
        workgroup_size: u32,
    ) -> Result<Vec<f32>> {
        let bytes = std::mem::size_of_val(input) as u64;
        let limit = u64::from(device.limits().max_storage_buffer_binding_size);
        // Vorher prüfen statt in einen Validierungsfehler zu laufen: so
        // bekommt der Aufrufer einen Satz, mit dem sich etwas anfangen
        // lässt, und fällt sauber auf die CPU zurück (ADR-0074).
        if bytes > limit {
            return Err(PipelineError::GpuExecution {
                stage: stage_name,
                message: format!(
                    "Bild braucht {} MiB je Puffer, die Grafikkarte erlaubt {} MiB",
                    bytes / (1024 * 1024),
                    limit / (1024 * 1024)
                ),
            });
        }

        device.push_error_scope(wgpu::ErrorFilter::Validation);
        self.ensure_buffers(device, bytes, params_bytes.len() as u64);

        // Ab hier steht fest, dass `self.buffers` belegt ist.
        let Some(buffers) = self.buffers.as_ref() else {
            return Err(PipelineError::GpuUnavailable {
                message: "Arbeitspuffer konnten nicht angelegt werden".to_string(),
            });
        };

        let rebuild = match self.pipelines.get(stage_name) {
            Some(cached) => cached.source != shader_source || cached.entry_point != entry_point,
            None => true,
        };
        if rebuild {
            let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some(stage_name),
                source: wgpu::ShaderSource::Wgsl(shader_source.into()),
            });
            let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some(stage_name),
                layout: None,
                module: &module,
                entry_point,
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                cache: None,
            });
            self.pipelines.insert(
                stage_name,
                CachedPipeline {
                    source: shader_source.to_string(),
                    entry_point: entry_point.to_string(),
                    pipeline,
                    bind_group: None,
                },
            );
        }

        let Some(cached) = self.pipelines.get_mut(stage_name) else {
            return Err(PipelineError::GpuUnavailable {
                message: "Pipeline konnte nicht gebaut werden".to_string(),
            });
        };

        let needs_bind_group =
            !matches!(&cached.bind_group, Some((gen, _)) if *gen == buffers.generation);
        if needs_bind_group {
            let layout = cached.pipeline.get_bind_group_layout(0);
            let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some(stage_name),
                layout: &layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: buffers.params.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: buffers.input.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: buffers.output.as_entire_binding(),
                    },
                ],
            });
            cached.bind_group = Some((buffers.generation, bind_group));
        }
        let Some((_, bind_group)) = cached.bind_group.as_ref() else {
            return Err(PipelineError::GpuUnavailable {
                message: "Bind-Group konnte nicht gebaut werden".to_string(),
            });
        };

        queue.write_buffer(&buffers.input, 0, bytemuck::cast_slice(input));
        queue.write_buffer(&buffers.params, 0, params_bytes);

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("apx-pipeline-stage-encoder"),
        });
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("apx-pipeline-stage-pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&cached.pipeline);
            pass.set_bind_group(0, bind_group, &[]);
            let (groups_x, groups_y) = super::dispatch::workgroup_grid(
                input.len() as u32,
                workgroup_size,
                device.limits().max_compute_workgroups_per_dimension,
            );
            pass.dispatch_workgroups(groups_x, groups_y, 1);
        }
        encoder.copy_buffer_to_buffer(&buffers.output, 0, &buffers.staging, 0, bytes);
        queue.submit(Some(encoder.finish()));

        if let Some(error) = pollster::block_on(device.pop_error_scope()) {
            return Err(PipelineError::GpuExecution {
                stage: stage_name,
                message: error.to_string(),
            });
        }

        // Nur den tatsächlich belegten Anfang zurücklesen: der Puffer ist
        // womöglich grösser als dieses Bild, weil er von einem früheren,
        // grösseren Lauf stammt.
        let slice = buffers.staging.slice(..bytes);
        let (tx, rx) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |result| {
            // Kanal kann bereits geschlossen sein, wenn der Aufrufer
            // schon aufgegeben hat — kein Programmfehler.
            let _ = tx.send(result);
        });
        device.poll(wgpu::Maintain::Wait);
        rx.recv()
            .map_err(|_| PipelineError::GpuUnavailable {
                message: "GPU-Antwortkanal wurde geschlossen, bevor eine Antwort ankam".to_string(),
            })?
            .map_err(|source| PipelineError::GpuUnavailable {
                message: format!("Rücklesen des Ergebnispuffers fehlgeschlagen: {source}"),
            })?;

        let mapped = slice.get_mapped_range();
        let result = bytemuck::cast_slice(&mapped).to_vec();
        drop(mapped);
        buffers.staging.unmap();

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn buffers_grow_but_never_shrink() {
        // Vorschaugrössen wechseln im Betrieb hin und her (Ziehen 1280,
        // Loslassen volle Anzeigegrösse). Würde bei jedem Wechsel neu
        // belegt, wäre genau der Aufwand zurück, den dieses Modul
        // vermeiden soll.
        let Ok(ctx) = crate::GpuContext::new_blocking() else {
            eprintln!("übersprungen: kein GPU-Adapter");
            return;
        };
        let mut ws = Workspace::default();
        ws.ensure_buffers(&ctx.device, 1024, 16);
        let first = ws.buffers.as_ref().map(|b| (b.generation, b.bytes));
        assert_eq!(first, Some((1, 1024)));

        // Kleiner: nichts passiert, dieselbe Generation.
        ws.ensure_buffers(&ctx.device, 512, 16);
        assert_eq!(
            ws.buffers.as_ref().map(|b| (b.generation, b.bytes)),
            Some((1, 1024))
        );

        // Grösser: neu belegt, neue Generation.
        ws.ensure_buffers(&ctx.device, 4096, 16);
        assert_eq!(
            ws.buffers.as_ref().map(|b| (b.generation, b.bytes)),
            Some((2, 4096))
        );

        // Und wieder kleiner: bleibt bei der grossen Belegung.
        ws.ensure_buffers(&ctx.device, 1024, 16);
        assert_eq!(
            ws.buffers.as_ref().map(|b| (b.generation, b.bytes)),
            Some((2, 4096))
        );
    }

    #[test]
    fn a_stage_name_reused_with_another_shader_gets_a_new_pipeline() {
        // Der Zwischenspeicher ist nach dem Stufennamen geschlüsselt.
        // Bekäme ein Name mit geändertem Shader die alte Pipeline,
        // rechnete die Stufe stillschweigend etwas anderes als ihr
        // Quelltext sagt — der unangenehmste Fehler überhaupt, weil das
        // Ergebnis plausibel aussieht.
        let Ok(ctx) = crate::GpuContext::new_blocking() else {
            eprintln!("übersprungen: kein GPU-Adapter");
            return;
        };
        let mut ws = Workspace::default();
        let input = vec![1.0f32; 64];
        let params = [0u8; 16];

        // `params` muss in beiden Shadern wirklich vorkommen: bei
        // `layout: None` leitet wgpu das Bind-Group-Layout aus dem
        // Quelltext ab und streicht eine Bindung, die der Shader nie
        // liest. Die Bind-Group hätte dann drei Einträge für ein
        // Layout mit zweien — ein Validierungsfehler, der nichts über
        // den Zwischenspeicher aussagt. Alle echten Stufen lesen ihre
        // Parameter, deshalb tun diese beiden es hier auch.
        const PLUS_ONE: &str = r#"
struct Params { offset: f32 };
@group(0) @binding(0) var<uniform> params: Params;
@group(0) @binding(1) var<storage, read> input_buf: array<f32>;
@group(0) @binding(2) var<storage, read_write> output_buf: array<f32>;
@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) gid: vec3<u32>, @builtin(num_workgroups) g: vec3<u32>) {
    let i = gid.x + gid.y * g.x * 64u;
    if (i >= arrayLength(&input_buf)) { return; }
    output_buf[i] = input_buf[i] + 1.0 + params.offset * 0.0;
}"#;
        const TIMES_TEN: &str = r#"
struct Params { offset: f32 };
@group(0) @binding(0) var<uniform> params: Params;
@group(0) @binding(1) var<storage, read> input_buf: array<f32>;
@group(0) @binding(2) var<storage, read_write> output_buf: array<f32>;
@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) gid: vec3<u32>, @builtin(num_workgroups) g: vec3<u32>) {
    let i = gid.x + gid.y * g.x * 64u;
    if (i >= arrayLength(&input_buf)) { return; }
    output_buf[i] = input_buf[i] * 10.0 + params.offset * 0.0;
}"#;

        let a = ws
            .run(
                &ctx.device,
                &ctx.queue,
                "derselbe-name",
                PLUS_ONE,
                "main",
                &params,
                &input,
                64,
            )
            .expect("erster Lauf");
        assert_eq!(a[0], 2.0);

        let b = ws
            .run(
                &ctx.device,
                &ctx.queue,
                "derselbe-name",
                TIMES_TEN,
                "main",
                &params,
                &input,
                64,
            )
            .expect("zweiter Lauf mit anderem Shader");
        assert_eq!(
            b[0], 10.0,
            "der geänderte Shader muss wirken, nicht der zwischengespeicherte"
        );
    }

    /// Der eigentliche Zweck dieses Moduls, maschinell festgehalten.
    ///
    /// Eine Zeitmessung wäre hier der naheliegende, aber falsche Test:
    /// sie hängt an der Maschine und würde mal grün, mal rot. Geprüft
    /// wird deshalb die Tatsache dahinter — dass wiederholte Aufräufe
    /// derselben Stufe WEDER eine neue Pipeline NOCH neue Puffer
    /// anlegen. Genau das kostete vor ADR-0075 bei zwei Megapixeln
    /// gemessene 228 statt 33 Millisekunden je Stufe.
    #[test]
    fn repeated_calls_reuse_pipeline_and_buffers() {
        let Ok(ctx) = crate::GpuContext::new_blocking() else {
            eprintln!("übersprungen: kein GPU-Adapter");
            return;
        };
        let mut ws = Workspace::default();
        const SHADER: &str = r#"
struct Params { offset: f32 };
@group(0) @binding(0) var<uniform> params: Params;
@group(0) @binding(1) var<storage, read> input_buf: array<f32>;
@group(0) @binding(2) var<storage, read_write> output_buf: array<f32>;
@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) gid: vec3<u32>, @builtin(num_workgroups) g: vec3<u32>) {
    let i = gid.x + gid.y * g.x * 64u;
    if (i >= arrayLength(&input_buf)) { return; }
    output_buf[i] = input_buf[i] + params.offset;
}"#;
        let input = vec![1.0f32; 4096];
        let params = [0u8; 16];

        for _ in 0..5 {
            ws.run(
                &ctx.device,
                &ctx.queue,
                "wiederverwendet",
                SHADER,
                "main",
                &params,
                &input,
                64,
            )
            .expect("Lauf");
        }

        assert_eq!(
            ws.pipelines.len(),
            1,
            "fünf Aufrufe derselben Stufe dürfen genau eine Pipeline erzeugen"
        );
        assert_eq!(
            ws.buffers.as_ref().map(|b| b.generation),
            Some(1),
            "die Puffer dürfen bei gleichbleibender Größe nur EINMAL angelegt werden"
        );
        assert!(
            ws.pipelines
                .get("wiederverwendet")
                .and_then(|p| p.bind_group.as_ref())
                .is_some(),
            "auch die Bind-Group soll den Aufruf überleben"
        );
    }

    #[test]
    fn repeated_runs_of_the_same_stage_stay_correct() {
        // Wiederverwendete Puffer sind nur dann richtig, wenn jeder Lauf
        // sie vollständig neu beschreibt. Bliebe ein Rest des vorigen
        // Laufs stehen, sähe man das erst an einem Bild, das kleiner ist
        // als das vorige.
        let Ok(ctx) = crate::GpuContext::new_blocking() else {
            eprintln!("übersprungen: kein GPU-Adapter");
            return;
        };
        let mut ws = Workspace::default();
        const SHADER: &str = r#"
struct Params { offset: f32 };
@group(0) @binding(0) var<uniform> params: Params;
@group(0) @binding(1) var<storage, read> input_buf: array<f32>;
@group(0) @binding(2) var<storage, read_write> output_buf: array<f32>;
@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) gid: vec3<u32>, @builtin(num_workgroups) g: vec3<u32>) {
    let i = gid.x + gid.y * g.x * 64u;
    if (i >= arrayLength(&input_buf)) { return; }
    output_buf[i] = input_buf[i] + params.offset;
}"#;

        // Erst gross, dann klein — der Puffer bleibt auf der grossen
        // Länge, das Ergebnis muss trotzdem die kleine Länge haben.
        for (len, value, offset) in [
            (4096usize, 1.0f32, 5.0f32),
            (64, 2.0, 0.5),
            (256, 3.0, -1.0),
        ] {
            let input = vec![value; len];
            let mut params = [0u8; 16];
            params[..4].copy_from_slice(&offset.to_le_bytes());
            let out = ws
                .run(
                    &ctx.device,
                    &ctx.queue,
                    "wiederholt",
                    SHADER,
                    "main",
                    &params,
                    &input,
                    64,
                )
                .expect("Lauf");
            assert_eq!(out.len(), len, "Ergebnislänge muss zur Eingabe passen");
            assert!(
                out.iter().all(|&v| (v - (value + offset)).abs() < 1e-5),
                "Länge {len}: Reste eines früheren Laufs im Puffer"
            );
        }
    }
}
