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

use bytemuck::Pod;
use wgpu::util::DeviceExt;

use super::GpuContext;
use crate::error::{PipelineError, Result};

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
    let device = &ctx.device;
    let queue = &ctx.queue;

    device.push_error_scope(wgpu::ErrorFilter::Validation);

    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("apx-pipeline-stage-shader"),
        source: wgpu::ShaderSource::Wgsl(shader_source.into()),
    });

    let byte_len = std::mem::size_of_val(input) as u64;

    let input_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("apx-pipeline-stage-input"),
        contents: bytemuck::cast_slice(input),
        usage: wgpu::BufferUsages::STORAGE,
    });
    let output_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("apx-pipeline-stage-output"),
        size: byte_len,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    });
    let params_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("apx-pipeline-stage-params"),
        contents: bytemuck::bytes_of(&params),
        usage: wgpu::BufferUsages::UNIFORM,
    });
    let staging_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("apx-pipeline-stage-staging"),
        size: byte_len,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("apx-pipeline-stage-pipeline"),
        layout: None,
        module: &shader,
        entry_point,
        compilation_options: wgpu::PipelineCompilationOptions::default(),
        cache: None,
    });

    let bind_group_layout = pipeline.get_bind_group_layout(0);
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("apx-pipeline-stage-bindgroup"),
        layout: &bind_group_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: params_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: input_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: output_buffer.as_entire_binding(),
            },
        ],
    });

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("apx-pipeline-stage-encoder"),
    });
    {
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("apx-pipeline-stage-pass"),
            timestamp_writes: None,
        });
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        let element_count = input.len() as u32;
        let workgroup_count = element_count.div_ceil(workgroup_size).max(1);
        pass.dispatch_workgroups(workgroup_count, 1, 1);
    }
    encoder.copy_buffer_to_buffer(&output_buffer, 0, &staging_buffer, 0, byte_len);
    queue.submit(Some(encoder.finish()));

    if let Some(error) = pollster::block_on(device.pop_error_scope()) {
        return Err(PipelineError::ShaderCompile {
            stage: stage_name,
            message: error.to_string(),
        });
    }

    let slice = staging_buffer.slice(..);
    let (tx, rx) = std::sync::mpsc::channel();
    slice.map_async(wgpu::MapMode::Read, move |result| {
        // Kanal kann bereits geschlossen sein, wenn der Aufrufer schon
        // aufgegeben hat (z. B. Timeout) — send()-Fehler hier sind kein
        // Programmfehler, nur nichts mehr zu tun.
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
    staging_buffer.unmap();

    Ok(result)
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
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let i = gid.x;
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
}
