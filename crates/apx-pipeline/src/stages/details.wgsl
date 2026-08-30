// Details (Schärfung + Rauschreduzierung) — siehe details.rs' Moduldoku
// für die Herleitung der Schärfung-/Rauschreduzierungs-Formeln und ihre
// bewussten Vereinfachungen.

struct Params {
    width: u32,
    height: u32,
    sharpen_amount: f32,
    sharpen_radius: f32,
    sharpen_detail: f32,
    sharpen_masking: f32,
    use_deconvolution: f32,
    luminance_nr_amount: f32,
    luminance_nr_detail: f32,
    luminance_nr_contrast: f32,
    color_nr_amount: f32,
    color_nr_detail: f32,
    color_nr_smoothness: f32,
    _pad0: f32,
    _pad1: f32,
    _pad2: f32,
};

@group(0) @binding(0) var<uniform> params: Params;
@group(0) @binding(1) var<storage, read> input_buf: array<f32>;
@group(0) @binding(2) var<storage, read_write> output_buf: array<f32>;

const NR_BLUR_RADIUS: i32 = 1;
const MASKING_THRESHOLD_SCALE: f32 = 0.2;
const DETAIL_STRENGTH_BASE: f32 = 0.5;
const DETAIL_STRENGTH_RANGE: f32 = 0.5;
const DECONVOLUTION_EXPONENT: f32 = 0.6;
const EDGE_PRESERVE_SCALE: f32 = 0.15;
const CONTRAST_RESTORE_SCALE: f32 = 0.5;
const SMOOTHNESS_BOOST_RANGE: f32 = 0.5;

fn sample_at(x: i32, y: i32, channel: u32) -> f32 {
    let cx = clamp(x, 0, i32(params.width) - 1);
    let cy = clamp(y, 0, i32(params.height) - 1);
    let idx = (u32(cy) * params.width + u32(cx)) * 3u + channel;
    return input_buf[idx];
}

fn box_blur_radius(x: i32, y: i32, channel: u32, radius: i32) -> f32 {
    var sum = 0.0;
    var count = 0.0;
    for (var dy = -radius; dy <= radius; dy = dy + 1) {
        for (var dx = -radius; dx <= radius; dx = dx + 1) {
            sum = sum + sample_at(x + dx, y + dy, channel);
            count = count + 1.0;
        }
    }
    return sum / count;
}

fn smoothstep_custom(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = clamp((x - edge0) / (edge1 - edge0), 0.0, 1.0);
    return t * t * (3.0 - 2.0 * t);
}

fn sharpen_radius_px() -> i32 {
    let clamped = clamp(params.sharpen_radius, 0.5, 3.0);
    return i32(max(round(clamped), 1.0));
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
    let x = i32(pixel_index % params.width);
    let y = i32(pixel_index / params.width);

    let r0 = input_buf[i];
    let g0 = input_buf[i + 1u];
    let b0 = input_buf[i + 2u];
    let luminance0 = 0.299 * r0 + 0.587 * g0 + 0.114 * b0;

    // --- Schärfung ---
    // Manuell entrollt statt einer Schleife über den Kanal-Index: naga
    // erlaubt nur konstante Indizes in ein lokales `array<f32, N>` (siehe
    // details.rs' Moduldoku-Nachbarschaft — derselbe Fehlerklasse wie die
    // frühere `active`-Falle, hier aber ein Indizierungs-, kein
    // Namenskonflikt).
    let sharpen_radius = sharpen_radius_px();
    let mask_threshold = (params.sharpen_masking / 100.0) * MASKING_THRESHOLD_SCALE;
    let detail_factor = DETAIL_STRENGTH_BASE + (params.sharpen_detail / 100.0) * DETAIL_STRENGTH_RANGE;

    var deltas = vec3<f32>(0.0, 0.0, 0.0);
    {
        let blur = box_blur_radius(x, y, 0u, sharpen_radius);
        var high_pass = r0 - blur;
        if (params.use_deconvolution > 0.5) {
            high_pass = sign(high_pass) * pow(abs(high_pass), DECONVOLUTION_EXPONENT);
        }
        var mask_weight = 1.0;
        if (mask_threshold >= 1e-6) {
            mask_weight = smoothstep_custom(0.0, mask_threshold, abs(high_pass));
        }
        deltas.x = high_pass * (params.sharpen_amount / 100.0) * detail_factor * mask_weight;
    }
    {
        let blur = box_blur_radius(x, y, 1u, sharpen_radius);
        var high_pass = g0 - blur;
        if (params.use_deconvolution > 0.5) {
            high_pass = sign(high_pass) * pow(abs(high_pass), DECONVOLUTION_EXPONENT);
        }
        var mask_weight = 1.0;
        if (mask_threshold >= 1e-6) {
            mask_weight = smoothstep_custom(0.0, mask_threshold, abs(high_pass));
        }
        deltas.y = high_pass * (params.sharpen_amount / 100.0) * detail_factor * mask_weight;
    }
    {
        let blur = box_blur_radius(x, y, 2u, sharpen_radius);
        var high_pass = b0 - blur;
        if (params.use_deconvolution > 0.5) {
            high_pass = sign(high_pass) * pow(abs(high_pass), DECONVOLUTION_EXPONENT);
        }
        var mask_weight = 1.0;
        if (mask_threshold >= 1e-6) {
            mask_weight = smoothstep_custom(0.0, mask_threshold, abs(high_pass));
        }
        deltas.z = high_pass * (params.sharpen_amount / 100.0) * detail_factor * mask_weight;
    }

    // --- Rauschreduzierung ---
    let nr_blur_r = box_blur_radius(x, y, 0u, NR_BLUR_RADIUS);
    let nr_blur_g = box_blur_radius(x, y, 1u, NR_BLUR_RADIUS);
    let nr_blur_b = box_blur_radius(x, y, 2u, NR_BLUR_RADIUS);
    let luminance_blur = 0.299 * nr_blur_r + 0.587 * nr_blur_g + 0.114 * nr_blur_b;
    let luminance_edge = abs(luminance0 - luminance_blur);

    let luminance_detail_threshold = (params.luminance_nr_detail / 100.0) * EDGE_PRESERVE_SCALE;
    var luminance_preserve = 0.0;
    if (luminance_detail_threshold >= 1e-6) {
        luminance_preserve = smoothstep_custom(0.0, luminance_detail_threshold, luminance_edge);
    }
    let luminance_blend = (params.luminance_nr_amount / 100.0) * (1.0 - luminance_preserve);
    let luminance_denoised = luminance0 + (luminance_blur - luminance0) * luminance_blend;
    let luminance_final = luminance_denoised + (luminance0 - luminance_denoised) * (params.luminance_nr_contrast / 100.0) * CONTRAST_RESTORE_SCALE;

    let color_detail_threshold = (params.color_nr_detail / 100.0) * EDGE_PRESERVE_SCALE;
    var color_preserve = 0.0;
    if (color_detail_threshold >= 1e-6) {
        color_preserve = smoothstep_custom(0.0, color_detail_threshold, luminance_edge);
    }
    let smoothness_boost = (params.color_nr_smoothness / 100.0) * SMOOTHNESS_BOOST_RANGE;
    let color_blend = (params.color_nr_amount / 100.0) * clamp(1.0 - color_preserve + smoothness_boost, 0.0, 1.0);

    let chroma_r = (r0 - luminance0) + ((nr_blur_r - luminance_blur) - (r0 - luminance0)) * color_blend;
    let chroma_g = (g0 - luminance0) + ((nr_blur_g - luminance_blur) - (g0 - luminance0)) * color_blend;
    let chroma_b = (b0 - luminance0) + ((nr_blur_b - luminance_blur) - (b0 - luminance0)) * color_blend;

    output_buf[i] = clamp(luminance_final + chroma_r + deltas.x, 0.0, 1.0);
    output_buf[i + 1u] = clamp(luminance_final + chroma_g + deltas.y, 0.0, 1.0);
    output_buf[i + 2u] = clamp(luminance_final + chroma_b + deltas.z, 0.0, 1.0);
}
