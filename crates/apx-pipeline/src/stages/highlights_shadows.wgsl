// Lichter/Tiefen: tonwertzonen-gewichtete Anhebung/Absenkung — siehe
// highlights_shadows.rs für die Begründung dieser vereinfachten Formel.

struct Params {
    highlights: f32,
    shadows: f32,
    _pad0: f32,
    _pad1: f32,
};

@group(0) @binding(0) var<uniform> params: Params;
@group(0) @binding(1) var<storage, read> input_buf: array<f32>;
@group(0) @binding(2) var<storage, read_write> output_buf: array<f32>;

@compute @workgroup_size(64)
fn main(
    @builtin(global_invocation_id) gid: vec3<u32>,
    @builtin(num_workgroups) groups: vec3<u32>,
) {
    // Zweidimensionaler Dispatch (ADR-0074): `gid.x` allein deckte
    // nur die erste Zeile ab. `gid.y` zaehlt die Zeilen,
    // `groups.x * 64u` ist die Zeilenbreite in Elementen.
    let i = gid.x + gid.y * groups.x * 64u;
    if (i >= arrayLength(&input_buf)) {
        return;
    }
    let v = input_buf[i];
    let hl_weight = v * v;
    let sh_weight = (1.0 - v) * (1.0 - v);
    let adjustment = (params.highlights / 100.0) * hl_weight * 0.5 + (params.shadows / 100.0) * sh_weight * 0.5;
    output_buf[i] = v + adjustment;
}
