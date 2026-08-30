// Color Grading (Farbräder) — siehe color_grading.rs' Moduldoku für die
// Herleitung der Zonen-Gewichtung und der additiven Rad-Tönung.

struct WheelParams {
    hue_degrees: f32,
    saturation: f32,
    luminance: f32,
    _pad: f32,
};

struct Params {
    shadows: WheelParams,
    midtones: WheelParams,
    highlights: WheelParams,
    global: WheelParams,
    balance: f32,
    blending: f32,
    _pad0: f32,
    _pad1: f32,
};

@group(0) @binding(0) var<uniform> params: Params;
@group(0) @binding(1) var<storage, read> input_buf: array<f32>;
@group(0) @binding(2) var<storage, read_write> output_buf: array<f32>;

const TINT_STRENGTH: f32 = 0.4;
const LUMINANCE_STRENGTH: f32 = 0.3;
const BASE_SIGMA: f32 = 0.2;
const BLENDING_SIGMA_RANGE: f32 = 0.3;

fn gaussian_weight(distance: f32, sigma: f32) -> f32 {
    return exp(-(distance * distance) / (2.0 * sigma * sigma));
}

fn hue_to_rgb_component(p: f32, q: f32, t_in: f32) -> f32 {
    var t = t_in;
    if (t < 0.0) { t = t + 1.0; }
    if (t > 1.0) { t = t - 1.0; }
    if (t < 1.0 / 6.0) { return p + (q - p) * 6.0 * t; }
    if (t < 1.0 / 2.0) { return q; }
    if (t < 2.0 / 3.0) { return p + (q - p) * (2.0 / 3.0 - t) * 6.0; }
    return p;
}

fn hsl_to_rgb(h_degrees: f32, s: f32, l: f32) -> vec3<f32> {
    if (s <= 0.0) {
        return vec3<f32>(l, l, l);
    }
    var h = h_degrees % 360.0;
    if (h < 0.0) {
        h = h + 360.0;
    }
    h = h / 360.0;
    var q: f32;
    if (l < 0.5) {
        q = l * (1.0 + s);
    } else {
        q = l + s - l * s;
    }
    let p = 2.0 * l - q;
    return vec3<f32>(
        hue_to_rgb_component(p, q, h + 1.0 / 3.0),
        hue_to_rgb_component(p, q, h),
        hue_to_rgb_component(p, q, h - 1.0 / 3.0),
    );
}

// Rückgabe: xyz = Farbkanal-Delta, w = Luminanz-Delta.
fn wheel_delta(wheel: WheelParams, weight: f32) -> vec4<f32> {
    let tint = hsl_to_rgb(wheel.hue_degrees, wheel.saturation, 0.5);
    let scale = TINT_STRENGTH * weight;
    return vec4<f32>(
        (tint.x - 0.5) * scale,
        (tint.y - 0.5) * scale,
        (tint.z - 0.5) * scale,
        (wheel.luminance / 100.0) * LUMINANCE_STRENGTH * weight,
    );
}

fn tonal_shift(r: f32, g: f32, b: f32) -> vec3<f32> {
    let luminance = 0.299 * r + 0.587 * g + 0.114 * b;
    let sigma = BASE_SIGMA + (params.blending / 100.0) * BLENDING_SIGMA_RANGE;

    let shadow_factor = max(1.0 - params.balance / 200.0, 0.0);
    let highlight_factor = max(1.0 + params.balance / 200.0, 0.0);

    let shadow_weight = gaussian_weight(luminance, sigma) * shadow_factor;
    let midtone_weight = gaussian_weight(luminance - 0.5, sigma);
    let highlight_weight = gaussian_weight(luminance - 1.0, sigma) * highlight_factor;

    var total = wheel_delta(params.shadows, shadow_weight);
    total = total + wheel_delta(params.midtones, midtone_weight);
    total = total + wheel_delta(params.highlights, highlight_weight);
    total = total + wheel_delta(params.global, 1.0);

    return vec3<f32>(
        clamp(r + total.x + total.w, 0.0, 1.0),
        clamp(g + total.y + total.w, 0.0, 1.0),
        clamp(b + total.z + total.w, 0.0, 1.0),
    );
}

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let i = gid.x;
    if (i >= arrayLength(&input_buf)) {
        return;
    }
    let channel = i % 3u;
    let pixel_base = i - channel;

    let result = tonal_shift(input_buf[pixel_base], input_buf[pixel_base + 1u], input_buf[pixel_base + 2u]);

    if (channel == 0u) {
        output_buf[i] = result.x;
    } else if (channel == 1u) {
        output_buf[i] = result.y;
    } else {
        output_buf[i] = result.z;
    }
}
