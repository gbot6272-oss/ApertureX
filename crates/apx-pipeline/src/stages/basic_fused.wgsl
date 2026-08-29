// Fusionierte Grundeinstellungen: dieselbe Mathematik wie
// white_balance.wgsl + exposure.wgsl + contrast.wgsl +
// highlights_shadows.wgsl + whites_blacks.wgsl, aber in einem einzigen
// Durchlauf/Dispatch statt fünf — siehe DECISIONS.md ADR-0017. Der
// interaktive Vorschau-Pfad nutzt ausschließlich diesen Shader; die
// Einzel-Shader bleiben für SPEC.md-Konformität ("jede Operation ein
// eigener Shader") und gezielte Tests bestehen. Ein Abgleichstest in
// basic_fused.rs stellt sicher, dass beide Wege dasselbe Ergebnis liefern.

struct Params {
    r_gain: f32,
    g_gain: f32,
    b_gain: f32,
    exposure_ev: f32,
    contrast: f32,
    highlights: f32,
    shadows: f32,
    whites: f32,
    blacks: f32,
    _pad0: f32,
    _pad1: f32,
    _pad2: f32,
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

    // 1. Weißabgleich
    let channel = i % 3u;
    var gain = params.r_gain;
    if (channel == 1u) {
        gain = params.g_gain;
    } else if (channel == 2u) {
        gain = params.b_gain;
    }
    var v = input_buf[i] * gain;

    // 2. Belichtung
    v = v * pow(2.0, params.exposure_ev);

    // 3. Kontrast
    let contrast_factor = 1.0 + params.contrast / 100.0;
    v = (v - 0.5) * contrast_factor + 0.5;

    // 4. Lichter/Tiefen
    let hl_weight = v * v;
    let sh_weight = (1.0 - v) * (1.0 - v);
    v = v + (params.highlights / 100.0) * hl_weight * 0.5 + (params.shadows / 100.0) * sh_weight * 0.5;

    // 5. Weiß/Schwarz
    let w_weight = v;
    let b_weight = 1.0 - v;
    v = v + (params.whites / 100.0) * w_weight * 0.3 + (params.blacks / 100.0) * b_weight * 0.3;

    output_buf[i] = v;
}
