// Effekte (Vignettierung + Körnung) — siehe effects.rs' Moduldoku für
// die Herleitung der Formeln und ihre bewussten Vereinfachungen.

struct Params {
    width: u32,
    height: u32,
    post_vignette_amount: f32,
    post_vignette_midpoint: f32,
    post_vignette_roundness: f32,
    post_vignette_feather: f32,
    post_vignette_highlights: f32,
    grain_amount: f32,
    grain_size: f32,
    grain_roughness: f32,
    _pad0: f32,
    _pad1: f32,
};

@group(0) @binding(0) var<uniform> params: Params;
@group(0) @binding(1) var<storage, read> input_buf: array<f32>;
@group(0) @binding(2) var<storage, read_write> output_buf: array<f32>;

const VIGNETTE_STRENGTH: f32 = 0.6;
const FEATHER_MIN: f32 = 0.05;
const FEATHER_RANGE: f32 = 0.6;
const GRAIN_STRENGTH: f32 = 0.25;
const ROUGHNESS_RANGE: f32 = 0.6;

fn smoothstep_custom(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = clamp((x - edge0) / (edge1 - edge0), 0.0, 1.0);
    return t * t * (3.0 - 2.0 * t);
}

fn hash_u32(x_in: u32) -> u32 {
    var h = x_in;
    h = h ^ (h >> 16u);
    h = h * 0x7feb352du;
    h = h ^ (h >> 15u);
    h = h * 0x846ca68bu;
    h = h ^ (h >> 16u);
    return h;
}

fn noise_at(x: i32, y: i32) -> f32 {
    let combined = (u32(x) * 374761393u) ^ (u32(y) * 668265263u);
    let h = hash_u32(combined);
    return (f32(h) / 4294967295.0) * 2.0 - 1.0;
}

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let i = gid.x;
    if (i >= arrayLength(&input_buf)) {
        return;
    }
    let channel = i % 3u;
    if (channel != 0u) {
        return;
    }
    let pixel_index = i / 3u;
    let px = pixel_index % params.width;
    let py = pixel_index / params.width;

    let r_in = input_buf[i];
    let g_in = input_buf[i + 1u];
    let b_in = input_buf[i + 2u];

    let half_w = f32(params.width) / 2.0;
    let half_h = f32(params.height) / 2.0;
    let dx = f32(px) - half_w;
    let dy = f32(py) - half_h;

    let r2_aspect = pow(dx / half_w, 2.0) + pow(dy / half_h, 2.0);
    let max_half = max(half_w, half_h);
    let r2_circle = (dx * dx + dy * dy) / (max_half * max_half);
    let roundness_blend = clamp(params.post_vignette_roundness / 100.0, 0.0, 1.0);
    let r2 = r2_aspect + (r2_circle - r2_aspect) * roundness_blend;
    let radius = sqrt(max(r2, 0.0));

    let edge0 = params.post_vignette_midpoint / 100.0;
    let edge1 = edge0 + FEATHER_MIN + (params.post_vignette_feather / 100.0) * FEATHER_RANGE;
    let weight = smoothstep_custom(edge0, edge1, radius);

    let luminance = 0.299 * r_in + 0.587 * g_in + 0.114 * b_in;
    let protection = clamp(1.0 - (params.post_vignette_highlights / 100.0) * luminance, 0.0, 1.0);
    let vignette_delta = (params.post_vignette_amount / 100.0) * weight * protection * VIGNETTE_STRENGTH;

    let block = i32(max(round(params.grain_size / 10.0), 1.0));
    let bx = i32(px) / block;
    let by = i32(py) / block;
    let raw_noise = noise_at(bx, by);
    let exponent = max(1.0 - (params.grain_roughness - 50.0) / 50.0 * ROUGHNESS_RANGE, 0.05);
    let shaped_noise = sign(raw_noise) * pow(abs(raw_noise), exponent);
    let grain_delta = shaped_noise * (params.grain_amount / 100.0) * GRAIN_STRENGTH;

    let total_delta = vignette_delta + grain_delta;

    output_buf[i] = clamp(r_in + total_delta, 0.0, 1.0);
    output_buf[i + 1u] = clamp(g_in + total_delta, 0.0, 1.0);
    output_buf[i + 2u] = clamp(b_in + total_delta, 0.0, 1.0);
}
