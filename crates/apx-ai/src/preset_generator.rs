//! Preset-Generator (Phase 7 Schritt 4, `SPEC.md` §5, `DECISIONS.md`
//! ADR-0033) — vier unabhängige Erzeugungsarten für eine `PresetEdlSubset`
//! (`frontend/src/lib/presets.ts`), hier als `serde_json::Value`-Objekt
//! mit genau den zehn preset-fähigen Sektionsschlüsseln
//! ([`SECTION_KEYS`]) dargestellt — derselbe opake JSON-Umgang wie
//! `apx_catalog::PresetVersionDto::edl_subset_json`:
//!
//! - [`generate_from_llm`]: **echter** Anthropic-Messages-API-Aufruf
//!   (kein offizielles Rust-SDK vorhanden, deshalb rohes `reqwest`-JSON,
//!   siehe Modulkopf der Aufrufer-Seite). Bittet das Modell, aus einer
//!   Freitextbeschreibung eine EDL-Teilmenge zu entwerfen, und validiert
//!   das Ergebnis serverseitig (siehe [`validate_preset_subset`]).
//! - [`generate_from_reference`]: **kein LLM** — Koordinatenabstieg über
//!   die sieben tonwertbezogenen Grundeinstellungs-Parameter, der die
//!   simulierte Luminanzhistogramm-Distanz zu einem Referenzbild
//!   minimiert.
//! - [`generate_variations`]: deterministisch geseedete kleine Störungen
//!   eines Basis-Presets (Kontaktbogen-Vorschau im Frontend).
//! - [`average_subsets`]: Preset aus Bearbeitung lernen — Mittelwert
//!   mehrerer bereits committeter EDL-Teilmengen (der Aufrufer, ein
//!   Tauri-Command, sammelt die Teilmengen mehrerer ausgewählter Fotos).
//!
//! **Bewusste Vereinfachungen** (`DECISIONS.md` ADR-0033 Punkt 4):
//! Referenzbild-Modus vergleicht Histogramme im display-referred
//! sRGB-Raum statt im linearen Arbeitsraum der eigentlichen Pipeline
//! (dieselbe Näherung wie die Farbbereich-Maskenpipette, siehe
//! `MasksPanel.tsx`s Moduldoku) und deckt nur die sieben Tonwertregler ab,
//! nicht Farbe/HSL/Kurven; „Lernen" mittelt nur numerische Blattwerte,
//! strukturierte Listen (Kurvenpunkte, Farbmischer-Regionen,
//! Objektivkorrektur-Hilfslinien) werden unverändert vom ersten Foto
//! übernommen statt sinnvoll zusammengeführt — dieselbe Einschränkung,
//! die `frontend/src/lib/presets.ts::interpolateValue` für die
//! Preset-Stärke schon dokumentiert.

use serde_json::{Map, Value};

use apx_pipeline::edl::EdlV3;

use crate::error::{AiError, Result};

/// Die zehn preset-fähigen EDL-Sektionen — muss
/// `frontend/src/lib/presets.ts::PRESET_SECTION_KEYS` entsprechen
/// (Reparatur/Masken/Maskengruppen sind bewusst nie Teil eines Presets,
/// siehe dort).
pub const SECTION_KEYS: &[&str] = &[
    "basic",
    "curves",
    "hsl",
    "color_mixer",
    "color_grading",
    "details",
    "lens_corrections",
    "effects",
    "calibration",
    "geometry",
];

// ---- LLM-Anfrage (Anthropic Messages API) ----------------------------------

const ANTHROPIC_API_URL: &str = "https://api.anthropic.com/v1/messages";
const ANTHROPIC_VERSION: &str = "2023-06-01";
/// Bewusst das leistungsfähigste verfügbare Modell: ein Preset-Entwurf ist
/// ein seltener, per Knopfdruck ausgelöster Einzelaufruf, keine
/// Chat-Schleife — Ergebnisqualität zählt hier stärker als Tokenkosten.
const MODEL: &str = "claude-opus-5";
/// Reines JSON-Ergebnis, keine Bild-/Dokumenteneingabe — 8192 Token
/// Obergrenze ist für die zehn Sektionen komfortabel bemessen, ohne
/// unnötig zu übertreiben (siehe `claude-api`-Skill-Hinweis, `max_tokens`
/// nicht zu knapp zu bemessen).
const MAX_TOKENS: u32 = 8192;

#[derive(serde::Serialize)]
struct MessagesRequest<'a> {
    model: &'a str,
    max_tokens: u32,
    system: &'a str,
    messages: Vec<MessageParam>,
}

#[derive(serde::Serialize)]
struct MessageParam {
    role: &'static str,
    content: String,
}

#[derive(serde::Deserialize)]
struct MessagesResponse {
    #[serde(default)]
    content: Vec<ContentBlock>,
}

#[derive(serde::Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ContentBlock {
    Text {
        text: String,
    },
    #[serde(other)]
    Other,
}

#[derive(serde::Deserialize)]
struct AnthropicErrorResponse {
    error: AnthropicErrorDetail,
}

#[derive(serde::Deserialize)]
struct AnthropicErrorDetail {
    message: String,
}

/// Beschreibt dem Modell das Zielschema — die zehn Sektionen mit ihren
/// jeweiligen Feldern in Kurzform (vollständig genug, um plausible Werte
/// zu erzeugen, ohne den ganzen `EdlV3`-Rust-Quelltext einzubetten).
fn system_prompt() -> String {
    "Du bist ein Farbbearbeitungs-Assistent für den RAW-Foto-Editor \"Aperture X\". \
     Der Nutzer beschreibt in Freitext einen gewünschten Bildlook. Antworte AUSSCHLIESSLICH \
     mit einem einzigen JSON-Objekt, ohne Markdown-Codeblock, ohne Erklärtext davor oder danach. \
     Das JSON-Objekt darf nur Schlüssel aus dieser Liste enthalten, jeder Schlüssel ist optional \
     (nimm nur die Sektionen auf, die zum gewünschten Look beitragen): \
     \"basic\" { white_balance: { temp_shift_kelvin: number (-3000..3000), tint_shift: number (-150..150) }, \
     exposure_ev: number (-5..5), contrast: number (-100..100), highlights: number (-100..100), \
     shadows: number (-100..100), whites: number (-100..100), blacks: number (-100..100), \
     texture: number (-100..100), clarity: number (-100..100), dehaze: number (-100..100), \
     vibrance: number (-100..100), saturation: number (-100..100) }, \
     \"details\" { sharpen_amount: number (0..150), sharpen_radius: number (0.5..3), \
     sharpen_detail: number (0..100), sharpen_masking: number (0..100), \
     luminance_nr: number (0..100), luminance_detail: number (0..100), \
     color_nr: number (0..100), color_detail: number (0..100) }, \
     \"effects\" { vignette_amount: number (-100..100), vignette_midpoint: number (0..100), \
     vignette_feather: number (0..100), grain_amount: number (0..100), grain_size: number (0..100), \
     grain_roughness: number (0..100) }, \
     \"calibration\" { shadow_tint: number (-100..100), red_hue: number (-100..100), \
     red_saturation: number (-100..100), green_hue: number (-100..100), \
     green_saturation: number (-100..100), blue_hue: number (-100..100), \
     blue_saturation: number (-100..100) }. \
     Für \"hsl\", \"color_mixer\", \"color_grading\", \"curves\", \"lens_corrections\", \"geometry\" \
     gilt: nur setzen, wenn der Wunsch das eindeutig verlangt, sonst weglassen — ihre Struktur ist \
     komplexer (mehrere benannte Bänder/Regionen/Kanäle), verwende bei Unsicherheit lieber \"basic\". \
     Erzeuge keine Felder außerhalb dieser Namen. Antworte NUR mit dem JSON-Objekt.".to_string()
}

/// Ruft die Anthropic Messages API auf und liefert eine validierte
/// [`SECTION_KEYS`]-Teilmenge zurück, die zu `description` passt.
pub async fn generate_from_llm(api_key: &str, description: &str) -> Result<Value> {
    if api_key.trim().is_empty() {
        return Err(AiError::MissingApiKey);
    }

    let client = reqwest::Client::new();
    let body = MessagesRequest {
        model: MODEL,
        max_tokens: MAX_TOKENS,
        system: &system_prompt(),
        messages: vec![MessageParam {
            role: "user",
            content: description.to_string(),
        }],
    };

    let response = client
        .post(ANTHROPIC_API_URL)
        .header("x-api-key", api_key)
        .header("anthropic-version", ANTHROPIC_VERSION)
        .header("content-type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|source| AiError::LlmRequest {
            message: format!("Netzwerkfehler: {source}"),
        })?;

    let status = response.status();
    let raw = response
        .text()
        .await
        .map_err(|source| AiError::LlmRequest {
            message: format!("Antwort konnte nicht gelesen werden: {source}"),
        })?;

    if !status.is_success() {
        let message = serde_json::from_str::<AnthropicErrorResponse>(&raw)
            .map(|err| err.error.message)
            .unwrap_or(raw);
        return Err(AiError::LlmRequest {
            message: format!("Anthropic-API antwortete mit {status}: {message}"),
        });
    }

    let parsed: MessagesResponse =
        serde_json::from_str(&raw).map_err(|source| AiError::LlmResponseUnparsable {
            message: format!("Antwort ist kein gültiges Messages-API-JSON: {source}"),
        })?;
    let text = parsed
        .content
        .iter()
        .find_map(|block| match block {
            ContentBlock::Text { text } => Some(text.as_str()),
            ContentBlock::Other => None,
        })
        .ok_or_else(|| AiError::LlmResponseUnparsable {
            message: "Antwort enthält keinen Textblock".to_string(),
        })?;

    let subset = extract_json_object(text)?;
    validate_preset_subset(&subset)?;
    Ok(subset)
}

/// Extrahiert ein JSON-Objekt aus der Modellantwort — toleriert das
/// häufige Ausweichverhalten, die Antwort trotz ausdrücklicher Anweisung
/// in einen Markdown-Codeblock (```json ... ```) einzupacken.
fn extract_json_object(text: &str) -> Result<Value> {
    let trimmed = text.trim();
    let without_prefix = trimmed
        .strip_prefix("```json")
        .or_else(|| trimmed.strip_prefix("```"))
        .unwrap_or(trimmed)
        .trim();
    let candidate = without_prefix
        .strip_suffix("```")
        .unwrap_or(without_prefix)
        .trim();
    serde_json::from_str(candidate).map_err(|source| AiError::LlmResponseUnparsable {
        message: format!("Modellantwort ist kein gültiges JSON: {source}"),
    })
}

/// Serverseitige Validierung (siehe Moduldoku): nur bekannte
/// Sektionsschlüssel erlaubt, und das Ergebnis muss — auf ein neutrales
/// `EdlV3` gemergt — vollständig als `EdlV3` deserialisierbar sein. Ein
/// halluziniertes/falsch geformtes Feld lässt den Aufruf fehlschlagen
/// statt eine kaputte Teilmenge durchzureichen.
pub fn validate_preset_subset(subset: &Value) -> Result<()> {
    let object = subset
        .as_object()
        .ok_or_else(|| AiError::LlmResponseUnparsable {
            message: "Modellantwort ist kein JSON-Objekt".to_string(),
        })?;
    for key in object.keys() {
        if !SECTION_KEYS.contains(&key.as_str()) {
            return Err(AiError::LlmResponseUnparsable {
                message: format!("Modellantwort enthält unbekannte Sektion '{key}'"),
            });
        }
    }

    let neutral = serde_json::to_value(EdlV3::neutral()).map_err(|source| {
        AiError::LlmResponseUnparsable {
            message: format!("Neutrales EDL konnte nicht serialisiert werden: {source}"),
        }
    })?;
    let Value::Object(mut merged) = neutral else {
        return Err(AiError::LlmResponseUnparsable {
            message: "Neutrales EDL ist kein JSON-Objekt".to_string(),
        });
    };
    for (key, value) in object {
        merged.insert(key.clone(), value.clone());
    }

    serde_json::from_value::<EdlV3>(Value::Object(merged)).map_err(|source| {
        AiError::LlmResponseUnparsable {
            message: format!("Modellantwort ergibt kein gültiges EDL: {source}"),
        }
    })?;
    Ok(())
}

// ---- Referenzbild-Modus (kein LLM) -----------------------------------------

/// Anzahl Histogramm-Bins für die Luminanzverteilung.
const HISTOGRAM_BINS: usize = 64;
/// Feste Reihenfolge der sieben Tonwertregler, über die der
/// Koordinatenabstieg läuft — dieselben sieben Felder wie
/// `BasicAdjustments` abzüglich `white_balance`/`texture`/`clarity`/
/// `dehaze`/`vibrance`/`saturation` (Farbe/Struktur ändert die
/// *Luminanz*histogramm-Zielfunktion kaum bis gar nicht, siehe
/// Moduldoku).
const REFERENCE_MATCH_PARAM_COUNT: usize = 6;
const REFERENCE_MATCH_PARAM_RANGE: [(f32, f32); REFERENCE_MATCH_PARAM_COUNT] = [
    (-3.0, 3.0),     // exposure_ev
    (-100.0, 100.0), // contrast
    (-100.0, 100.0), // highlights
    (-100.0, 100.0), // shadows
    (-100.0, 100.0), // whites
    (-100.0, 100.0), // blacks
];
const REFERENCE_MATCH_PARAM_KEYS: [&str; REFERENCE_MATCH_PARAM_COUNT] = [
    "exposure_ev",
    "contrast",
    "highlights",
    "shadows",
    "whites",
    "blacks",
];

/// Sehr vereinfachte Tonwert-Simulation direkt auf einem Luminanzwert —
/// **kein** Ersatz für `apx_pipeline::stages::develop`s tatsächliche
/// Umsetzung, nur genau genug, um die Zielfunktion des Koordinatenabstiegs
/// auszuwerten (dieselbe Art Näherung wie `repair.rs`s Heal-Modus). Reihenfolge:
/// Belichtung (multiplikativ) → Kontrast (Pivot 0.5) → Lichter/Tiefen/Weiß/Schwarz
/// (geglättete Bereichsanhebung/-senkung nahe den jeweiligen Tonwertenden).
fn simulate_luma(luma: f32, params: &[f32; REFERENCE_MATCH_PARAM_COUNT]) -> f32 {
    let [exposure_ev, contrast, highlights, shadows, whites, blacks] = *params;

    let mut y = luma * 2f32.powf(exposure_ev);
    y = (y - 0.5) * (1.0 + contrast / 100.0) + 0.5;

    // Lichter/Tiefen: Gewichtung stärker zu den jeweiligen Tonwertenden hin
    // (glatte kubische Rampe), Weiß/Schwarz: schmalere Rampe direkt an den
    // Endpunkten — dieselbe grobe Charakteristik wie die eigentlichen
    // WGSL-Shader-Regler, nur ohne deren volle Kurvenform.
    let highlight_weight = y.clamp(0.0, 1.0).powf(2.0);
    let shadow_weight = (1.0 - y.clamp(0.0, 1.0)).powf(2.0);
    y += (highlights / 100.0) * 0.3 * highlight_weight;
    y += (shadows / 100.0) * 0.3 * shadow_weight;

    let white_weight = y.clamp(0.0, 1.0).powf(4.0);
    let black_weight = (1.0 - y.clamp(0.0, 1.0)).powf(4.0);
    y += (whites / 100.0) * 0.3 * white_weight;
    y += (blacks / 100.0) * 0.3 * black_weight;

    y.clamp(0.0, 1.0)
}

fn luma_histogram(luma_values: &[f32]) -> [f32; HISTOGRAM_BINS] {
    let mut hist = [0f32; HISTOGRAM_BINS];
    if luma_values.is_empty() {
        return hist;
    }
    for &v in luma_values {
        let bin = ((v.clamp(0.0, 1.0) * (HISTOGRAM_BINS - 1) as f32).round() as usize)
            .min(HISTOGRAM_BINS - 1);
        hist[bin] += 1.0;
    }
    let total = luma_values.len() as f32;
    for bin in &mut hist {
        *bin /= total;
    }
    hist
}

/// Distanz zweier normierter Histogramme als L1-Abstand ihrer
/// **Kumulativsummen** (die diskrete Earth-Mover's-/Wasserstein-1-Distanz
/// zwischen den beiden Verteilungen) statt eines rohen Bin-für-Bin-
/// Vergleichs. Wichtig für den Koordinatenabstieg: ein roher
/// Bin-Vergleich ist für schmale/spitze Verteilungen (z. B. ein fast
/// einfarbiges Bild) nicht monoton in der Verschiebung — zwei
/// nicht überlappende Spitzen ergäben unabhängig von ihrem Abstand
/// stets denselben Wert, sodass kein Kandidat je eine Verbesserung
/// zeigen würde. Die Kumulativsummen-Distanz sinkt dagegen stetig, je
/// näher sich die Verteilungen kommen.
fn histogram_distance(a: &[f32; HISTOGRAM_BINS], b: &[f32; HISTOGRAM_BINS]) -> f32 {
    let mut cumulative_a = 0.0;
    let mut cumulative_b = 0.0;
    let mut distance = 0.0;
    for i in 0..HISTOGRAM_BINS {
        cumulative_a += a[i];
        cumulative_b += b[i];
        distance += (cumulative_a - cumulative_b).abs();
    }
    distance
}

fn pixels_to_luma(pixels: &[f32]) -> Vec<f32> {
    pixels
        .chunks_exact(3)
        .map(|p| crate::color::luminance(p[0], p[1], p[2]))
        .collect()
}

/// Koordinatenabstieg: für jeden der sechs Parameter abwechselnd ein paar
/// Schrittweiten probieren (schrumpfender Schritt je Runde), den
/// verbessernden Wert übernehmen — deterministisch, keine
/// Zufallskomponente. `iterations` volle Durchläufe über alle sechs
/// Parameter.
fn coordinate_descent(
    source_luma: &[f32],
    reference_hist: &[f32; HISTOGRAM_BINS],
) -> [f32; REFERENCE_MATCH_PARAM_COUNT] {
    let mut params = [0f32; REFERENCE_MATCH_PARAM_COUNT];
    let mut best_distance = histogram_distance(
        &luma_histogram(
            &source_luma
                .iter()
                .map(|&l| simulate_luma(l, &params))
                .collect::<Vec<_>>(),
        ),
        reference_hist,
    );

    const ITERATIONS: usize = 4;
    for iteration in 0..ITERATIONS {
        for (index, &(min, max)) in REFERENCE_MATCH_PARAM_RANGE.iter().enumerate() {
            let span = (max - min) * 0.5f32.powi(iteration as i32 + 1);
            for &direction in &[1.0f32, -1.0] {
                let mut candidate = params;
                candidate[index] = (candidate[index] + direction * span).clamp(min, max);
                let simulated: Vec<f32> = source_luma
                    .iter()
                    .map(|&l| simulate_luma(l, &candidate))
                    .collect();
                let distance = histogram_distance(&luma_histogram(&simulated), reference_hist);
                if distance < best_distance {
                    best_distance = distance;
                    params = candidate;
                }
            }
        }
    }
    params
}

/// **Referenzbild-Modus**: passt die sieben Tonwertregler von `source`
/// (das aktuell bearbeitete Foto, lineare oder display-referred RGB-`f32`-
/// Pixel) so an, dass ihre simulierte Luminanzverteilung der von
/// `reference` (ein beliebiges Referenzbild, z. B. eine importierte JPEG)
/// möglichst nahekommt. Gibt eine `{"basic": {...}}`-Teilmenge zurück.
pub fn generate_from_reference(
    source_pixels: &[f32],
    source_width: u32,
    source_height: u32,
    reference_pixels: &[f32],
    reference_width: u32,
    reference_height: u32,
) -> Result<Value> {
    let expected_source = (source_width as usize) * (source_height as usize) * 3;
    let expected_reference = (reference_width as usize) * (reference_height as usize) * 3;
    if source_width == 0 || source_height == 0 || reference_width == 0 || reference_height == 0 {
        return Err(AiError::Analysis {
            message: "Quell- oder Referenzbild ist 0×0".to_string(),
        });
    }
    if source_pixels.len() != expected_source || reference_pixels.len() != expected_reference {
        return Err(AiError::Analysis {
            message: "Pufferlänge passt nicht zu den angegebenen Bildmaßen".to_string(),
        });
    }

    let source_luma = pixels_to_luma(source_pixels);
    let reference_luma = pixels_to_luma(reference_pixels);
    let reference_hist = luma_histogram(&reference_luma);
    let params = coordinate_descent(&source_luma, &reference_hist);

    let mut basic =
        serde_json::to_value(apx_pipeline::edl::BasicAdjustments::NEUTRAL).map_err(|source| {
            AiError::Analysis {
                message: format!("Grundeinstellungen konnten nicht serialisiert werden: {source}"),
            }
        })?;
    if let Value::Object(ref mut map) = basic {
        for (key, value) in REFERENCE_MATCH_PARAM_KEYS.iter().zip(params.iter()) {
            map.insert((*key).to_string(), serde_json::json!(value));
        }
    }

    let mut subset = Map::new();
    subset.insert("basic".to_string(), basic);
    Ok(Value::Object(subset))
}

// ---- Variationen-Generator --------------------------------------------------

/// Deterministischer xorshift32-PRNG — dieselbe einfache, aber gut
/// gemischte Konstruktion wie `apx_pipeline::stages::repair`s
/// PatchMatch-Zufallszahlen (siehe dort für die Begründung: keine
/// externe `rand`-Abhängigkeit nötig für eine kleine, reproduzierbare
/// Störung).
fn xorshift32(state: &mut u32) -> u32 {
    let mut x = *state;
    x ^= x << 13;
    x ^= x >> 17;
    x ^= x << 5;
    *state = x;
    x
}

/// Zufallswert in `-1.0..=1.0`, aus den oberen Bits von `xorshift32`
/// (die unteren Bits eines xorshift-Zustands sind schwächer gemischt).
fn signed_unit_random(state: &mut u32) -> f32 {
    let raw = xorshift32(state) >> 8; // 24 gemischte Bits
    (raw as f32 / 0x00FF_FFFF as f32) * 2.0 - 1.0
}

/// Stört jeden numerischen Blattwert um bis zu `strength` (z. B. `0.15`
/// = ±15 % relativ zum Betrag des Werts, mit einer kleinen additiven
/// Mindeststörung für Werte nahe Null, damit ein neutraler `0.0`-Regler
/// nicht für immer unverändert bleibt). Arrays/Strings/Booleans bleiben
/// unverändert (dieselbe Einschränkung wie
/// `frontend/src/lib/presets.ts::interpolateValue`, siehe Moduldoku).
fn perturb_value(value: &Value, strength: f32, state: &mut u32) -> Value {
    match value {
        Value::Number(n) => {
            if let Some(f) = n.as_f64() {
                let f = f as f32;
                let magnitude = f.abs().max(1.0);
                let jitter = magnitude * strength * signed_unit_random(state);
                Value::from(f as f64 + jitter as f64)
            } else {
                value.clone()
            }
        }
        Value::Object(map) => {
            let mut result = Map::new();
            for (key, v) in map {
                result.insert(key.clone(), perturb_value(v, strength, state));
            }
            Value::Object(result)
        }
        // Arrays (Kurvenpunkte, Farbmischer-Regionen, …), Strings, Booleans,
        // Null: unverändert übernehmen.
        other => other.clone(),
    }
}

/// Erzeugt `count` Varianten von `base` — jede mit demselben `seed` und
/// ihrem eigenen Index deterministisch geseedet, sodass ein wiederholter
/// Aufruf mit identischem `seed` exakt dieselben Varianten liefert (die
/// Kontaktbogen-Vorschau im Frontend darf nicht bei jedem Neu-Rendern
/// flackern).
pub fn generate_variations(base: &Value, count: usize, seed: u64) -> Result<Vec<Value>> {
    if !base.is_object() {
        return Err(AiError::Analysis {
            message: "Basis-Preset ist kein JSON-Objekt".to_string(),
        });
    }
    const STRENGTH: f32 = 0.2;
    Ok((0..count)
        .map(|index| {
            let mut state = (seed ^ ((index as u64 + 1).wrapping_mul(0x9E37_79B9_7F4A_7C15)))
                .wrapping_add(1) as u32
                | 1;
            perturb_value(base, STRENGTH, &mut state)
        })
        .collect())
}

// ---- Preset aus Bearbeitung lernen ------------------------------------------

/// Mittelt numerische Blattwerte über `subsets` hinweg (arithmetisches
/// Mittel je Pfad) — strukturierte Listen werden unverändert vom ersten
/// Element übernommen (siehe Moduldoku). Ein leeres `subsets` ist ein
/// Fehler (nichts zu lernen); ein einzelnes Element liefert es
/// unverändert zurück.
pub fn average_subsets(subsets: &[Value]) -> Result<Value> {
    let Some(first) = subsets.first() else {
        return Err(AiError::Analysis {
            message: "Keine Fotos zum Lernen ausgewählt".to_string(),
        });
    };
    if subsets.len() == 1 {
        return Ok(first.clone());
    }
    for subset in subsets {
        if !subset.is_object() {
            return Err(AiError::Analysis {
                message: "Eine der EDL-Teilmengen ist kein JSON-Objekt".to_string(),
            });
        }
    }
    Ok(average_value(first, &subsets[1..]))
}

fn average_value(first: &Value, rest: &[Value]) -> Value {
    match first {
        Value::Number(n) => {
            let Some(base) = n.as_f64() else {
                return first.clone();
            };
            let mut sum = base;
            let mut count = 1u32;
            for other in rest {
                if let Some(v) = other.as_f64() {
                    sum += v;
                    count += 1;
                }
            }
            Value::from(sum / count as f64)
        }
        Value::Object(map) => {
            let mut result = Map::new();
            for (key, value) in map {
                let others: Vec<Value> = rest
                    .iter()
                    .filter_map(|other| other.as_object().and_then(|m| m.get(key)).cloned())
                    .collect();
                result.insert(key.clone(), average_value(value, &others));
            }
            Value::Object(result)
        }
        other => other.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_preset_subset_accepts_a_well_formed_basic_section() {
        let subset = serde_json::json!({
            "basic": {
                "white_balance": { "temp_shift_kelvin": 0.0, "tint_shift": 0.0 },
                "exposure_ev": 0.5,
                "contrast": 10.0,
                "highlights": 0.0,
                "shadows": 0.0,
                "whites": 0.0,
                "blacks": 0.0,
                "texture": 0.0,
                "clarity": 0.0,
                "dehaze": 0.0,
                "vibrance": 0.0,
                "saturation": 0.0,
            }
        });
        assert!(validate_preset_subset(&subset).is_ok());
    }

    #[test]
    fn validate_preset_subset_rejects_unknown_section() {
        let subset = serde_json::json!({ "unbekannt": {} });
        assert!(validate_preset_subset(&subset).is_err());
    }

    #[test]
    fn validate_preset_subset_rejects_wrong_shape() {
        let subset = serde_json::json!({ "basic": { "exposure_ev": "nicht eine Zahl" } });
        assert!(validate_preset_subset(&subset).is_err());
    }

    #[test]
    fn validate_preset_subset_rejects_non_object() {
        let subset = serde_json::json!([1, 2, 3]);
        assert!(validate_preset_subset(&subset).is_err());
    }

    #[test]
    fn extract_json_object_strips_markdown_fence() {
        let text = "```json\n{\"basic\": {}}\n```";
        let value = extract_json_object(text).expect("sollte parsen");
        assert_eq!(value, serde_json::json!({"basic": {}}));
    }

    #[test]
    fn extract_json_object_accepts_plain_json() {
        let value = extract_json_object("{\"basic\": {}}").expect("sollte parsen");
        assert_eq!(value, serde_json::json!({"basic": {}}));
    }

    fn flat_pixels(width: u32, height: u32, value: f32) -> Vec<f32> {
        vec![value; (width * height * 3) as usize]
    }

    #[test]
    fn reference_match_brightens_a_dark_source_toward_a_bright_reference() {
        let size = 16;
        let source = flat_pixels(size, size, 0.2);
        let reference = flat_pixels(size, size, 0.8);
        let subset = generate_from_reference(&source, size, size, &reference, size, size)
            .expect("sollte gelingen");
        let exposure = subset["basic"]["exposure_ev"]
            .as_f64()
            .expect("exposure_ev sollte eine Zahl sein");
        assert!(
            exposure > 0.0,
            "Belichtung sollte für ein dunkleres Quellbild angehoben werden, war {exposure}"
        );
    }

    #[test]
    fn reference_match_rejects_mismatched_buffer_length() {
        let result = generate_from_reference(&[0.0; 10], 4, 4, &[0.0; 3], 1, 1);
        assert!(result.is_err());
    }

    #[test]
    fn generate_variations_is_deterministic_for_the_same_seed() {
        let base = serde_json::json!({ "basic": { "exposure_ev": 0.5, "contrast": 10.0 } });
        let first = generate_variations(&base, 3, 42).expect("sollte gelingen");
        let second = generate_variations(&base, 3, 42).expect("sollte gelingen");
        assert_eq!(first, second);
    }

    #[test]
    fn generate_variations_differ_between_indices() {
        let base = serde_json::json!({ "basic": { "exposure_ev": 0.5 } });
        let variations = generate_variations(&base, 4, 7).expect("sollte gelingen");
        let unique: std::collections::HashSet<String> =
            variations.iter().map(|v| v.to_string()).collect();
        assert_eq!(
            unique.len(),
            variations.len(),
            "Varianten sollten sich voneinander unterscheiden"
        );
    }

    #[test]
    fn generate_variations_leaves_arrays_untouched() {
        let base = serde_json::json!({ "curves": { "rgb": [[0.0, 0.0], [1.0, 1.0]] } });
        let variations = generate_variations(&base, 1, 1).expect("sollte gelingen");
        assert_eq!(variations[0]["curves"]["rgb"], base["curves"]["rgb"]);
    }

    #[test]
    fn average_subsets_computes_the_arithmetic_mean_of_numeric_leaves() {
        let a = serde_json::json!({ "basic": { "exposure_ev": 0.0, "contrast": 20.0 } });
        let b = serde_json::json!({ "basic": { "exposure_ev": 1.0, "contrast": 40.0 } });
        let averaged = average_subsets(&[a, b]).expect("sollte gelingen");
        assert!((averaged["basic"]["exposure_ev"].as_f64().unwrap() - 0.5).abs() < 1e-9);
        assert!((averaged["basic"]["contrast"].as_f64().unwrap() - 30.0).abs() < 1e-9);
    }

    #[test]
    fn average_subsets_rejects_empty_input() {
        assert!(average_subsets(&[]).is_err());
    }

    #[test]
    fn average_subsets_single_input_is_identity() {
        let a = serde_json::json!({ "basic": { "exposure_ev": 0.3 } });
        let averaged = average_subsets(std::slice::from_ref(&a)).expect("sollte gelingen");
        assert_eq!(averaged, a);
    }
}
