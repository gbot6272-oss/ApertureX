// Weiß/Schwarz: lineare Anhebung/Absenkung der Clipping-Punkte — siehe
// whites_blacks.rs für die Begründung dieser vereinfachten Formel.

struct Params {
    whites: f32,
    blacks: f32,
    _pad0: f32,
    _pad1: f32,
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
    let v = input_buf[i];
    let w_weight = v;
    let b_weight = 1.0 - v;
    let adjustment = (params.whites / 100.0) * w_weight * 0.3 + (params.blacks / 100.0) * b_weight * 0.3;
    output_buf[i] = v + adjustment;
}
