// Objektivkorrekturen — siehe lens_corrections.rs' Moduldoku für die
// Herleitung der Verzeichnungs-/Perspektiv-/CA-/Vignette-Formeln und
// ihre bewussten Vereinfachungen.

struct Params {
    width: u32,
    height: u32,
    distortion_k1: f32,
    vignette_amount: f32,
    ca_red_cyan: f32,
    ca_blue_yellow: f32,
    rotate_degrees: f32,
    vertical: f32,
    horizontal: f32,
    aspect: f32,
    scale: f32,
    offset_x: f32,
    offset_y: f32,
    _pad0: f32,
    _pad1: f32,
    _pad2: f32,
};

@group(0) @binding(0) var<uniform> params: Params;
@group(0) @binding(1) var<storage, read> input_buf: array<f32>;
@group(0) @binding(2) var<storage, read_write> output_buf: array<f32>;

const OFFSET_STRENGTH: f32 = 0.4;
const ASPECT_STRENGTH: f32 = 0.3;
const SHEAR_STRENGTH: f32 = 0.5;
const CA_STRENGTH: f32 = 0.02;
const VIGNETTE_STRENGTH: f32 = 0.01;
const PI: f32 = 3.14159265358979;

fn sample_at(x: i32, y: i32, channel: u32) -> f32 {
    let cx = clamp(x, 0, i32(params.width) - 1);
    let cy = clamp(y, 0, i32(params.height) - 1);
    let idx = (u32(cy) * params.width + u32(cx)) * 3u + channel;
    return input_buf[idx];
}

fn bilinear_sample(x: f32, y: f32, channel: u32) -> f32 {
    let x0 = floor(x);
    let y0 = floor(y);
    let fx = x - x0;
    let fy = y - y0;
    let x0i = i32(x0);
    let y0i = i32(y0);
    let v00 = sample_at(x0i, y0i, channel);
    let v10 = sample_at(x0i + 1, y0i, channel);
    let v01 = sample_at(x0i, y0i + 1, channel);
    let v11 = sample_at(x0i + 1, y0i + 1, channel);
    let top = v00 + (v10 - v00) * fx;
    let bottom = v01 + (v11 - v01) * fx;
    return top + (bottom - top) * fy;
}

fn undo_manual_transform(nx_in: f32, ny_in: f32) -> vec2<f32> {
    var x = nx_in - (params.offset_x / 100.0) * OFFSET_STRENGTH;
    var y = ny_in - (params.offset_y / 100.0) * OFFSET_STRENGTH;

    let scale_factor = max(params.scale / 100.0, 0.01);
    x = x / scale_factor;
    y = y / scale_factor;

    let aspect_factor_x = 1.0 + (params.aspect / 100.0) * ASPECT_STRENGTH;
    let aspect_factor_y = 1.0 - (params.aspect / 100.0) * ASPECT_STRENGTH;
    x = x / aspect_factor_x;
    y = y / aspect_factor_y;

    let angle = -params.rotate_degrees * (PI / 180.0);
    let sin_a = sin(angle);
    let cos_a = cos(angle);
    let rx = x * cos_a - y * sin_a;
    let ry = x * sin_a + y * cos_a;

    let sheared_x = rx - (params.horizontal / 100.0) * SHEAR_STRENGTH * ry;
    let sheared_y = ry - (params.vertical / 100.0) * SHEAR_STRENGTH * rx;

    return vec2<f32>(sheared_x, sheared_y);
}

fn apply_distortion(x: f32, y: f32) -> vec2<f32> {
    let factor = 1.0 + params.distortion_k1 * (x * x + y * y);
    return vec2<f32>(x * factor, y * factor);
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

    let half_w = f32(params.width) / 2.0;
    let half_h = f32(params.height) / 2.0;
    // Siehe lens_corrections.rs' Kommentar: bewusst ohne
    // Pixelmitten-Versatz, damit die Identitätsabbildung exakt bleibt.
    let nx = (f32(px) - half_w) / half_w;
    let ny = (f32(py) - half_h) / half_h;

    let undone = undo_manual_transform(nx, ny);
    let distorted = apply_distortion(undone.x, undone.y);

    let ca_r = 1.0 + CA_STRENGTH * (params.ca_red_cyan / 100.0);
    let ca_b = 1.0 + CA_STRENGTH * (params.ca_blue_yellow / 100.0);

    let src_r = vec2<f32>(half_w + distorted.x * ca_r * half_w, half_h + distorted.y * ca_r * half_h);
    let src_g = vec2<f32>(half_w + distorted.x * half_w, half_h + distorted.y * half_h);
    let src_b = vec2<f32>(half_w + distorted.x * ca_b * half_w, half_h + distorted.y * ca_b * half_h);

    let r = bilinear_sample(src_r.x, src_r.y, 0u);
    let g = bilinear_sample(src_g.x, src_g.y, 1u);
    let b = bilinear_sample(src_b.x, src_b.y, 2u);

    let vignette_factor = 1.0 + (params.vignette_amount / 100.0) * VIGNETTE_STRENGTH * 100.0 * (nx * nx + ny * ny);

    output_buf[i] = clamp(r * vignette_factor, 0.0, 1.0);
    output_buf[i + 1u] = clamp(g * vignette_factor, 0.0, 1.0);
    output_buf[i + 2u] = clamp(b * vignette_factor, 0.0, 1.0);
}
