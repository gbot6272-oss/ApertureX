// Kalibrierung — siehe calibration.rs' Moduldoku für die Herleitung der
// Primärfarben-Bandgewichtung, der Schattentönung und des
// Kameraprofil-Bias.

struct PrimaryParams {
    hue: f32,
    saturation: f32,
    _pad0: f32,
    _pad1: f32,
};

struct Params {
    red_primary: PrimaryParams,
    green_primary: PrimaryParams,
    blue_primary: PrimaryParams,
    shadow_tint: f32,
    camera_profile_saturation: f32,
    camera_profile_contrast: f32,
    _pad: f32,
};

@group(0) @binding(0) var<uniform> params: Params;
@group(0) @binding(1) var<storage, read> input_buf: array<f32>;
@group(0) @binding(2) var<storage, read_write> output_buf: array<f32>;

const PRIMARY_SIGMA_DEGREES: f32 = 45.0;
const MAX_PRIMARY_HUE_SHIFT_DEGREES: f32 = 30.0;
const SHADOW_TINT_SIGMA: f32 = 0.25;
const SHADOW_TINT_STRENGTH: f32 = 0.3;

fn circular_distance_degrees(a: f32, b: f32) -> f32 {
    let diff = abs(a - b) % 360.0;
    return min(diff, 360.0 - diff);
}

fn gaussian_weight(distance: f32, sigma: f32) -> f32 {
    return exp(-(distance * distance) / (2.0 * sigma * sigma));
}

fn rgb_to_hsl(r: f32, g: f32, b: f32) -> vec3<f32> {
    let max_c = max(r, max(g, b));
    let min_c = min(r, min(g, b));
    let l = (max_c + min_c) / 2.0;
    let d = max_c - min_c;
    if (d < 1e-6) {
        return vec3<f32>(0.0, 0.0, l);
    }
    var s: f32;
    if (l > 0.5) {
        s = d / (2.0 - max_c - min_c);
    } else {
        s = d / (max_c + min_c);
    }
    var h: f32;
    if (max_c == r) {
        h = (((g - b) / d) % 6.0 + 6.0) % 6.0;
    } else if (max_c == g) {
        h = (b - r) / d + 2.0;
    } else {
        h = (r - g) / d + 4.0;
    }
    return vec3<f32>(h * 60.0, s, l);
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

fn tonal_shift(r: f32, g: f32, b: f32) -> vec3<f32> {
    let hsl = rgb_to_hsl(r, g, b);
    let h = hsl.x;
    let s = hsl.y;
    let l = hsl.z;

    var hue_sum = 0.0;
    var sat_sum = 0.0;
    var weight_sum = 0.0;

    let red_distance = circular_distance_degrees(h, 0.0);
    let red_weight = gaussian_weight(red_distance, PRIMARY_SIGMA_DEGREES);
    hue_sum = hue_sum + red_weight * params.red_primary.hue;
    sat_sum = sat_sum + red_weight * params.red_primary.saturation;
    weight_sum = weight_sum + red_weight;

    let green_distance = circular_distance_degrees(h, 120.0);
    let green_weight = gaussian_weight(green_distance, PRIMARY_SIGMA_DEGREES);
    hue_sum = hue_sum + green_weight * params.green_primary.hue;
    sat_sum = sat_sum + green_weight * params.green_primary.saturation;
    weight_sum = weight_sum + green_weight;

    let blue_distance = circular_distance_degrees(h, 240.0);
    let blue_weight = gaussian_weight(blue_distance, PRIMARY_SIGMA_DEGREES);
    hue_sum = hue_sum + blue_weight * params.blue_primary.hue;
    sat_sum = sat_sum + blue_weight * params.blue_primary.saturation;
    weight_sum = weight_sum + blue_weight;

    if (weight_sum < 1e-6) {
        return vec3<f32>(r, g, b);
    }

    let hue_shift = (hue_sum / weight_sum) / 100.0 * MAX_PRIMARY_HUE_SHIFT_DEGREES;
    let sat_factor = 1.0 + (sat_sum / weight_sum) / 100.0;
    var shifted = hsl_to_rgb(h + hue_shift, clamp(s * sat_factor, 0.0, 1.0), l);

    let shadow_weight = gaussian_weight(l, SHADOW_TINT_SIGMA);
    shifted.y = clamp(shifted.y - (params.shadow_tint / 100.0) * SHADOW_TINT_STRENGTH * shadow_weight, 0.0, 1.0);

    let hsl2 = rgb_to_hsl(shifted.x, shifted.y, shifted.z);
    let profile_sat_factor = 1.0 + params.camera_profile_saturation / 100.0;
    let profiled = hsl_to_rgb(hsl2.x, clamp(hsl2.y * profile_sat_factor, 0.0, 1.0), hsl2.z);

    let contrast_factor = 1.0 + params.camera_profile_contrast / 100.0;
    return vec3<f32>(
        clamp((profiled.x - 0.5) * contrast_factor + 0.5, 0.0, 1.0),
        clamp((profiled.y - 0.5) * contrast_factor + 0.5, 0.0, 1.0),
        clamp((profiled.z - 0.5) * contrast_factor + 0.5, 0.0, 1.0),
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
