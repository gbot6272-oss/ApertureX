// Reparatur (Klonen/Reparieren) — siehe repair.rs' Moduldoku für die
// Herleitung der Formeln und die bewussten Vereinfachungen (kein echtes
// Poisson-Blending, Pfad-Abstand als minimaler Stützpunkt-Abstand statt
// echter Punkt-zu-Liniensegment-Distanz).
//
// `path` ist auf 16 Byte je Stützpunkt aufgefüllt (siehe repair.rs'
// `PathPoint`-Kommentar) — WGSL verlangt für Arrays im `uniform`-
// Adressraum eine auf 16 Byte ausgerichtete Element-Schrittweite.

struct Params {
    width: u32,
    height: u32,
    mode: f32,
    radius: f32,
    feather: f32,
    opacity: f32,
    offset_x: f32,
    offset_y: f32,
    point_count: u32,
    _pad0: f32,
    _pad1: f32,
    _pad2: f32,
    path: array<vec4<f32>, 32>,
};

@group(0) @binding(0) var<uniform> params: Params;
@group(0) @binding(1) var<storage, read> input_buf: array<f32>;
@group(0) @binding(2) var<storage, read_write> output_buf: array<f32>;

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

fn box_blur3(x: i32, y: i32, channel: u32) -> f32 {
    var sum = 0.0;
    for (var dy = -1; dy <= 1; dy = dy + 1) {
        for (var dx = -1; dx <= 1; dx = dx + 1) {
            sum = sum + sample_at(x + dx, y + dy, channel);
        }
    }
    return sum / 9.0;
}

fn smoothstep_edge(edge0: f32, edge1: f32, x: f32) -> f32 {
    if (edge1 - edge0 < 1e-6) {
        if (x >= edge1) {
            return 1.0;
        }
        return 0.0;
    }
    let t = clamp((x - edge0) / (edge1 - edge0), 0.0, 1.0);
    return t * t * (3.0 - 2.0 * t);
}

fn distance_to_path(px: f32, py: f32) -> f32 {
    var min_dist = 3.4e38;
    for (var i = 0u; i < params.point_count; i = i + 1u) {
        let point = params.path[i];
        let dx = px - point.x;
        let dy = py - point.y;
        let dist = sqrt(dx * dx + dy * dy);
        if (dist < min_dist) {
            min_dist = dist;
        }
    }
    return min_dist;
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
    let x = f32(px);
    let y = f32(py);

    let dist = distance_to_path(x, y);
    let weight = (1.0 - smoothstep_edge(params.radius, params.radius + max(params.feather, 1e-3), dist)) * params.opacity;

    let original_r = input_buf[i];
    let original_g = input_buf[i + 1u];
    let original_b = input_buf[i + 2u];

    if (weight <= 0.0) {
        output_buf[i] = original_r;
        output_buf[i + 1u] = original_g;
        output_buf[i + 2u] = original_b;
        return;
    }

    let src_x = x - params.offset_x;
    let src_y = y - params.offset_y;
    let src_xi = i32(round(src_x));
    let src_yi = i32(round(src_y));
    let is_heal = params.mode > 0.5;

    let cloned_r = bilinear_sample(src_x, src_y, 0u);
    let cloned_g = bilinear_sample(src_x, src_y, 1u);
    let cloned_b = bilinear_sample(src_x, src_y, 2u);

    var value_r = cloned_r;
    var value_g = cloned_g;
    var value_b = cloned_b;
    if (is_heal) {
        // Reparieren: Tiefpass von der Quelle, Hochpass vom Ziel.
        value_r = box_blur3(src_xi, src_yi, 0u) + (original_r - box_blur3(i32(px), i32(py), 0u));
        value_g = box_blur3(src_xi, src_yi, 1u) + (original_g - box_blur3(i32(px), i32(py), 1u));
        value_b = box_blur3(src_xi, src_yi, 2u) + (original_b - box_blur3(i32(px), i32(py), 2u));
    }

    output_buf[i] = clamp(original_r + (value_r - original_r) * weight, 0.0, 1.0);
    output_buf[i + 1u] = clamp(original_g + (value_g - original_g) * weight, 0.0, 1.0);
    output_buf[i + 2u] = clamp(original_b + (value_b - original_b) * weight, 0.0, 1.0);
}
