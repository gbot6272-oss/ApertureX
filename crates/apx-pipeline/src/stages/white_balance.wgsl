// Weißabgleich: reiner Kanal-Gain, siehe white_balance.rs für die
// Herleitung der drei Gains aus As-shot-Koeffizienten + Nutzer-Shift.

struct Params {
    r_gain: f32,
    g_gain: f32,
    b_gain: f32,
    _pad: f32,
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
    let channel = i % 3u;
    var gain = params.r_gain;
    if (channel == 1u) {
        gain = params.g_gain;
    } else if (channel == 2u) {
        gain = params.b_gain;
    }
    output_buf[i] = input_buf[i] * gain;
}
