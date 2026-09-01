// Kontrast: symmetrische Spreizung/Stauchung um den Mittelpunkt 0.5 —
// siehe contrast.rs für die Begründung dieser vereinfachten Formel.

struct Params {
    contrast: f32,
    _pad0: f32,
    _pad1: f32,
    _pad2: f32,
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
    let factor = 1.0 + params.contrast / 100.0;
    output_buf[i] = (input_buf[i] - 0.5) * factor + 0.5;
}
