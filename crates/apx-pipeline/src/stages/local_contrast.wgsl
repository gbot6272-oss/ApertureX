// Textur/Klarheit: lokaler Kontrast bei mittleren (Textur) bzw. niedrigen
// (Klarheit) Ortsfrequenzen — ein echter Nachbarschafts-Zugriff, anders
// als `basic_fused.wgsl`s Ein-Pixel-Modell (siehe `PLAN.md` Phase 4
// Schritt 2, `gpu/dispatch.rs`s Moduldoku: derselbe generische
// `run_compute_f32`-Helfer trägt auch diesen Fall, da der Shader selbst
// per `width`/`height`-Uniform beliebige Nachbarpixel aus `input_buf`
// lesen kann — keine Änderung an `dispatch.rs` nötig).
//
// Vereinfachtes Unsharp-Masking: 3×3-Box-Unschärfe je Kanal als
// Tiefpass-Referenz, Differenz zum Original ist der Hochpass-Anteil.
// Textur wirkt gleichmäßig auf den Hochpass-Anteil; Klarheit wirkt
// zusätzlich tonwertzonen-gewichtet (`4*v*(1-v)`, Maximum bei v=0.5,
// Null bei v=0/1) — schont so Lichter/Tiefen stärker als Textur, analog
// zur Lightroom-Beschreibung ("Klarheit schont Hauttöne").

struct Params {
    width: u32,
    height: u32,
    texture: f32,
    clarity: f32,
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

fn box_blur3(x: i32, y: i32, channel: u32) -> f32 {
    var sum = 0.0;
    for (var dy = -1; dy <= 1; dy = dy + 1) {
        for (var dx = -1; dx <= 1; dx = dx + 1) {
            sum = sum + sample_at(x + dx, y + dy, channel);
        }
    }
    return sum / 9.0;
}

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let i = gid.x;
    if (i >= arrayLength(&input_buf)) {
        return;
    }

    let channel = i % 3u;
    let pixel_index = i / 3u;
    let x = i32(pixel_index % params.width);
    let y = i32(pixel_index / params.width);

    let original = input_buf[i];
    let blur = box_blur3(x, y, channel);
    let high_pass = original - blur;
    let strength = params.texture / 100.0 + (params.clarity / 100.0) * 4.0 * original * (1.0 - original);
    output_buf[i] = clamp(original + high_pass * strength, 0.0, 1.0);
}
