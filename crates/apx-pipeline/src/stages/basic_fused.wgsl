// Fusionierte Grundeinstellungen: alle zwölf Regler (die sieben aus
// Phase 2 plus die fünf per ADR-0011/ADR-0028 nach Phase 4 verschobenen:
// Dunst entfernen, Dynamik, Sättigung — Textur/Klarheit laufen in
// `local_contrast.wgsl`, da sie echten Nachbarschafts-Zugriff brauchen,
// siehe dort) in einem einzigen Durchlauf/Dispatch — siehe DECISIONS.md
// ADR-0017. Der interaktive Vorschau-Pfad nutzt ausschließlich diesen
// Shader; die Einzel-Shader (`white_balance.wgsl` etc.) bleiben für
// SPEC.md-Konformität ("jede Operation ein eigener Shader") und gezielte
// Tests bestehen. Ein Abgleichstest in basic_fused.rs stellt sicher, dass
// beide Wege für die sieben Phase-2-Regler dasselbe Ergebnis liefern.
//
// Dynamik/Sättigung brauchen die Luminanz eines ganzen Pixels (alle drei
// Kanäle), nicht nur den eigenen Kanalwert — jede Invocation berechnet
// deshalb den tonwert-angepassten Zwischenwert aller drei Geschwister-
// kanäle selbst neu (etwas redundante, aber immer noch billige Arbeit;
// es gibt keine synchronisierte Zwischenablage zwischen Invocations
// innerhalb desselben Dispatch).

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
    dehaze: f32,
    vibrance: f32,
    saturation: f32,
};

@group(0) @binding(0) var<uniform> params: Params;
@group(0) @binding(1) var<storage, read> input_buf: array<f32>;
@group(0) @binding(2) var<storage, read_write> output_buf: array<f32>;

fn gain_for(channel: u32) -> f32 {
    if (channel == 0u) {
        return params.r_gain;
    }
    if (channel == 1u) {
        return params.g_gain;
    }
    return params.b_gain;
}

// Weißabgleich bis Weiß/Schwarz — dieselbe Mathematik wie in Phase 2,
// unverändert. Dunst entfernen (vereinfachtes Modell, kein echtes
// Dark-Channel-Prior-Verfahren, siehe DECISIONS.md ADR-0028) kommt danach:
// hebt/senkt einen konstanten "Schleier"-Betrag und dehnt den Kontrast
// wieder auf den vollen Bereich.
fn tonal(v0: f32, channel: u32) -> f32 {
    var v = v0 * gain_for(channel);
    v = v * pow(2.0, params.exposure_ev);

    let contrast_factor = 1.0 + params.contrast / 100.0;
    v = (v - 0.5) * contrast_factor + 0.5;

    let hl_weight = v * v;
    let sh_weight = (1.0 - v) * (1.0 - v);
    v = v + (params.highlights / 100.0) * hl_weight * 0.5 + (params.shadows / 100.0) * sh_weight * 0.5;

    let w_weight = v;
    let b_weight = 1.0 - v;
    v = v + (params.whites / 100.0) * w_weight * 0.3 + (params.blacks / 100.0) * b_weight * 0.3;

    let haze = params.dehaze / 100.0 * 0.2;
    v = (v - haze) / max(1.0 - haze, 0.0001);

    return v;
}

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let i = gid.x;
    if (i >= arrayLength(&input_buf)) {
        return;
    }

    let channel = i % 3u;
    let pixel_base = i - channel;

    let v_r = tonal(input_buf[pixel_base], 0u);
    let v_g = tonal(input_buf[pixel_base + 1u], 1u);
    let v_b = tonal(input_buf[pixel_base + 2u], 2u);

    var v = v_r;
    if (channel == 1u) {
        v = v_g;
    } else if (channel == 2u) {
        v = v_b;
    }

    // Dynamik/Sättigung: Skalierung des Abstands zum Luminanzwert.
    // Dynamik ("vibrance") wirkt umso schwächer, je gesättigter der Pixel
    // schon ist (chroma-gewichtet), Sättigung wirkt gleichmäßig auf alle.
    let luma = 0.299 * v_r + 0.587 * v_g + 0.114 * v_b;
    let max_c = max(v_r, max(v_g, v_b));
    let min_c = min(v_r, min(v_g, v_b));
    let chroma = clamp(max_c - min_c, 0.0, 1.0);
    let vibrance_factor = 1.0 + (params.vibrance / 100.0) * (1.0 - chroma);
    let saturation_factor = 1.0 + params.saturation / 100.0;
    let total_factor = vibrance_factor * saturation_factor;

    output_buf[i] = luma + (v - luma) * total_factor;
}
