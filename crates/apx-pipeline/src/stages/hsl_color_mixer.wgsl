// HSL (acht feste Farbbänder) + Farbmischer erweitert (offene Liste
// benutzerdefinierter Farbbereiche, siehe hsl_color_mixer.rs' Moduldoku
// für die Begründung des MAX_COLOR_MIXER_REGIONS-Limits). Beide
// verschieben Farbton/Sättigung/Luminanz gewichtet nach Farbton-Abstand
// zum jeweiligen Band-/Regionen-Zentrum (SPEC.md §3.2).
//
// Läuft im selben Ein-Pixel-pro-Invocation-Modell wie basic_fused.wgsl
// (Geschwisterkanal-Zugriff für die RGB<->HSL-Konvertierung, kein echter
// Nachbarschafts-Zugriff nötig).

struct BandParams {
    hue: f32,
    saturation: f32,
    luminance: f32,
    _pad: f32,
};

struct RegionParams {
    target_hue_degrees: f32,
    bandwidth_degrees: f32,
    feather: f32,
    hue_shift: f32,
    saturation_shift: f32,
    luminance_shift: f32,
    is_active: f32,
    _pad: f32,
};

struct Params {
    bands: array<BandParams, 8>,
    regions: array<RegionParams, 8>,
};

@group(0) @binding(0) var<uniform> params: Params;
@group(0) @binding(1) var<storage, read> input_buf: array<f32>;
@group(0) @binding(2) var<storage, read_write> output_buf: array<f32>;

const HSL_BAND_SIGMA: f32 = 25.0;
const MAX_HUE_SHIFT_DEGREES: f32 = 60.0;

fn band_center(index: u32) -> f32 {
    if (index == 0u) { return 0.0; }
    if (index == 1u) { return 30.0; }
    if (index == 2u) { return 60.0; }
    if (index == 3u) { return 120.0; }
    if (index == 4u) { return 180.0; }
    if (index == 5u) { return 240.0; }
    if (index == 6u) { return 270.0; }
    return 300.0;
}

fn circular_distance(a: f32, b: f32) -> f32 {
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
        h = (g - b) / d;
        h = h - 6.0 * floor(h / 6.0); // rem_euclid(6.0)
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
    var lum_sum = 0.0;
    var weight_sum = 0.0;

    for (var i = 0u; i < 8u; i = i + 1u) {
        let band = params.bands[i];
        let distance = circular_distance(h, band_center(i));
        let weight = gaussian_weight(distance, HSL_BAND_SIGMA);
        hue_sum = hue_sum + weight * band.hue;
        sat_sum = sat_sum + weight * band.saturation;
        lum_sum = lum_sum + weight * band.luminance;
        weight_sum = weight_sum + weight;
    }

    for (var i = 0u; i < 8u; i = i + 1u) {
        let region = params.regions[i];
        if (region.is_active <= 0.0) {
            continue;
        }
        let distance = circular_distance(h, region.target_hue_degrees);
        let sigma = max(region.bandwidth_degrees * (0.5 + clamp(region.feather, 0.0, 1.0)), 1.0);
        let weight = gaussian_weight(distance, sigma);
        hue_sum = hue_sum + weight * region.hue_shift;
        sat_sum = sat_sum + weight * region.saturation_shift;
        lum_sum = lum_sum + weight * region.luminance_shift;
        weight_sum = weight_sum + weight;
    }

    if (weight_sum < 1e-6) {
        return vec3<f32>(r, g, b);
    }

    let hue_shift = (hue_sum / weight_sum) / 100.0 * MAX_HUE_SHIFT_DEGREES;
    let sat_factor = 1.0 + (sat_sum / weight_sum) / 100.0;
    let lum_shift = (lum_sum / weight_sum) / 100.0 * 0.3;

    let new_h = h + hue_shift;
    let new_s = clamp(s * sat_factor, 0.0, 1.0);
    let new_l = clamp(l + lum_shift, 0.0, 1.0);

    return hsl_to_rgb(new_h, new_s, new_l);
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
