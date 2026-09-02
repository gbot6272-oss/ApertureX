//! Adobe `.lrtemplate`-Export (Phase 11 Schritt 8, siehe `DECISIONS.md`
//! ADR-0038) — Lightrooms alte, vor Version 4/2018 verwendete
//! Entwickeln-Vorlagen-Serialisierung: eine Lua-Tabellenzuweisung an die
//! globale Variable `s`. Kein offiziell dokumentiertes Format (Adobe hat
//! Anfang 2018 auf `.xmp`-basierte Vorlagen umgestellt, siehe
//! `xmp.rs`s Moduldoku) — dieses Modul rekonstruiert die Struktur
//! best-effort anhand einer real vorliegenden Beispiel-Vorlagendatei
//! (öffentlich unter github.com/pforret/Lightroom, Datei „Wes Anderson
//! 1.lrtemplate"), analog zu `xmp.rs`s ebenfalls best-effort ermittelten
//! `crs:`-Feldnamen.
//!
//! **Nur Export, kein Import** (siehe `PLAN.md` Phase 11 Schritt 8):
//! Import bräuchte einen robusten Lua-Tabellen-Parser für einen nicht
//! spezifizierten Dialekt — höheres Risiko für stillen Datenverlust bei
//! Abweichungen. Export ist die risikoärmere Richtung, hier kontrollieren
//! wir die Ausgabe vollständig selbst.
//!
//! **Deckt dieselben Felder ab wie `xmp.rs`s `crs:`-Export** (Basic ohne
//! Weißabgleich, siehe dessen Moduldoku zur Begründung, + die acht
//! HSL-Bänder) — aus demselben Grund: nur diese Adobe-Eigenschaftsnamen/
//! -Wertebereiche sind seit Process Version 2012 stabil und öffentlich
//! genug dokumentiert, um ohne Ratewerk zuzuordnen. Kurven/Farbmischer/
//! Color-Grading/Objektivkorrekturen/Effekte/Masken bleiben unübersetzt.
//!
//! **Reale Struktur** (aus der oben genannten Beispieldatei
//! rekonstruiert):
//! ```lua
//! s = {
//!     id = "...",
//!     internalName = "...",
//!     title = "...",
//!     type = "Develop",
//!     value = {
//!         settings = {
//!             <Feld> = <Wert>,
//!             ...
//!         },
//!         uuid = "...",
//!     },
//!     version = 0,
//! }
//! ```
//! Die Felder innerhalb von `settings` erscheinen in der real geprüften
//! Beispieldatei alphabetisch sortiert (`Blacks2012`, `BlueHue`,
//! `BlueSaturation`, `CameraProfile`, `Clarity2012`, …) — dieselbe
//! Reihenfolge übernimmt [`generate_lrtemplate`] für seine Teilmenge.
//!
//! **Bewusste Vereinfachung:** die Beispieldatei trägt zwei
//! *unterschiedliche* UUIDs (`id` und `value.uuid`) — diese Funktion
//! nimmt nur eine einzige `id` entgegen und trägt sie an beiden Stellen
//! ein, statt intern eine zweite zufällig zu erzeugen (das würde die
//! Funktion unrein/nicht-deterministisch machen, siehe Testmodul:
//! Byte-für-Byte-Vergleich braucht reproduzierbare Ausgabe).

use apx_pipeline::edl::{BasicAdjustments, HslAdjustment, HslBand};

type HslBandGetter = fn(&HslAdjustment) -> &HslBand;

/// Dieselben acht Bänder/Namen wie `xmp.rs`s `HSL_BANDS` — bewusst nicht
/// wiederverwendet (kein `pub(crate)` dort), aber inhaltlich identisch,
/// damit `.xmp`- und `.lrtemplate`-Export exakt dieselben Adobe-
/// Farbtonnamen verwenden.
const HSL_BANDS: [(&str, HslBandGetter); 8] = [
    ("Red", |h| &h.red),
    ("Orange", |h| &h.orange),
    ("Yellow", |h| &h.yellow),
    ("Green", |h| &h.green),
    ("Aqua", |h| &h.aqua),
    ("Blue", |h| &h.blue),
    ("Purple", |h| &h.purple),
    ("Magenta", |h| &h.magenta),
];

/// Lua-Zeichenkette maskieren — für unsere eigenen Preset-Namen reicht
/// Rückstrich/Anführungszeichen (Lightroom selbst dürfte kaum mehr
/// maskieren, echte Vorlagennamen enthalten praktisch nie Steuerzeichen).
fn escape_lua_string(text: &str) -> String {
    text.replace('\\', "\\\\").replace('"', "\\\"")
}

/// Baut den vollständigen Inhalt einer `.lrtemplate`-Datei für ein
/// benanntes Preset. `id` wird vom Aufrufer übergeben (die eigene
/// Preset-ID aus dem Katalog) statt hier zufällig erzeugt — siehe
/// Moduldoku zur bewussten Vereinfachung.
pub fn generate_lrtemplate(name: &str, id: &str, basic: &BasicAdjustments, hsl: &HslAdjustment) -> String {
    let mut fields: Vec<(String, String)> = vec![
        ("Blacks2012".to_string(), (basic.blacks as i32).to_string()),
        ("Clarity2012".to_string(), (basic.clarity as i32).to_string()),
        ("Contrast2012".to_string(), (basic.contrast as i32).to_string()),
        ("Dehaze".to_string(), (basic.dehaze as i32).to_string()),
        ("Exposure2012".to_string(), format!("{:.6}", basic.exposure_ev)),
        ("Highlights2012".to_string(), (basic.highlights as i32).to_string()),
        ("Saturation".to_string(), (basic.saturation as i32).to_string()),
        ("Shadows2012".to_string(), (basic.shadows as i32).to_string()),
        ("Texture".to_string(), (basic.texture as i32).to_string()),
        ("Vibrance".to_string(), (basic.vibrance as i32).to_string()),
        ("Whites2012".to_string(), (basic.whites as i32).to_string()),
    ];
    for (band_name, get) in HSL_BANDS {
        let band = get(hsl);
        fields.push((format!("HueAdjustment{band_name}"), (band.hue as i32).to_string()));
        fields.push((
            format!("LuminanceAdjustment{band_name}"),
            (band.luminance as i32).to_string(),
        ));
        fields.push((
            format!("SaturationAdjustment{band_name}"),
            (band.saturation as i32).to_string(),
        ));
    }
    fields.sort_by(|a, b| a.0.cmp(&b.0));

    let mut settings = String::new();
    for (key, value) in &fields {
        settings.push_str(&format!("\t\t\t{key} = {value},\n"));
    }

    let escaped_name = escape_lua_string(name);
    let escaped_id = escape_lua_string(id);
    format!(
        "s = {{\n\tid = \"{escaped_id}\",\n\tinternalName = \"{escaped_name}\",\n\ttitle = \"{escaped_name}\",\n\ttype = \"Develop\",\n\tvalue = {{\n\t\tsettings = {{\n{settings}\t\t}},\n\t\tuuid = \"{escaped_id}\",\n\t}},\n\tversion = 0,\n}}\n"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Byte-für-Byte-Vergleich gegen eine handgepflegte erwartete
    /// Ausgabe für ein festes Test-EDL (echter Rundreise-Test ist nicht
    /// möglich, siehe Moduldoku: kein verlässlicher `.lrtemplate`-Parser
    /// vorhanden) — die Feld-*Struktur* selbst ist gegen die real
    /// abgerufene Beispieldatei verifiziert (siehe Moduldoku).
    #[test]
    fn generate_lrtemplate_matches_the_expected_lua_table_byte_for_byte() {
        let mut basic = BasicAdjustments::NEUTRAL;
        basic.exposure_ev = 0.75;
        basic.contrast = 10.0;
        basic.highlights = -20.0;
        basic.shadows = 15.0;
        basic.whites = 5.0;
        basic.blacks = -5.0;
        basic.texture = 0.0;
        basic.clarity = 12.0;
        basic.dehaze = 0.0;
        basic.vibrance = 8.0;
        basic.saturation = 0.0;

        let mut hsl = HslAdjustment::NEUTRAL;
        hsl.red.hue = 0.0;
        hsl.red.saturation = 25.0;
        hsl.red.luminance = 0.0;
        hsl.aqua.hue = -5.0;
        hsl.aqua.saturation = 0.0;
        hsl.aqua.luminance = -10.0;

        let output = generate_lrtemplate("Testvorlage", "TEST-ID-0001", &basic, &hsl);

        let expected = "s = {\n\
\tid = \"TEST-ID-0001\",\n\
\tinternalName = \"Testvorlage\",\n\
\ttitle = \"Testvorlage\",\n\
\ttype = \"Develop\",\n\
\tvalue = {\n\
\t\tsettings = {\n\
\t\t\tBlacks2012 = -5,\n\
\t\t\tClarity2012 = 12,\n\
\t\t\tContrast2012 = 10,\n\
\t\t\tDehaze = 0,\n\
\t\t\tExposure2012 = 0.750000,\n\
\t\t\tHighlights2012 = -20,\n\
\t\t\tHueAdjustmentAqua = -5,\n\
\t\t\tHueAdjustmentBlue = 0,\n\
\t\t\tHueAdjustmentGreen = 0,\n\
\t\t\tHueAdjustmentMagenta = 0,\n\
\t\t\tHueAdjustmentOrange = 0,\n\
\t\t\tHueAdjustmentPurple = 0,\n\
\t\t\tHueAdjustmentRed = 0,\n\
\t\t\tHueAdjustmentYellow = 0,\n\
\t\t\tLuminanceAdjustmentAqua = -10,\n\
\t\t\tLuminanceAdjustmentBlue = 0,\n\
\t\t\tLuminanceAdjustmentGreen = 0,\n\
\t\t\tLuminanceAdjustmentMagenta = 0,\n\
\t\t\tLuminanceAdjustmentOrange = 0,\n\
\t\t\tLuminanceAdjustmentPurple = 0,\n\
\t\t\tLuminanceAdjustmentRed = 0,\n\
\t\t\tLuminanceAdjustmentYellow = 0,\n\
\t\t\tSaturation = 0,\n\
\t\t\tSaturationAdjustmentAqua = 0,\n\
\t\t\tSaturationAdjustmentBlue = 0,\n\
\t\t\tSaturationAdjustmentGreen = 0,\n\
\t\t\tSaturationAdjustmentMagenta = 0,\n\
\t\t\tSaturationAdjustmentOrange = 0,\n\
\t\t\tSaturationAdjustmentPurple = 0,\n\
\t\t\tSaturationAdjustmentRed = 25,\n\
\t\t\tSaturationAdjustmentYellow = 0,\n\
\t\t\tShadows2012 = 15,\n\
\t\t\tTexture = 0,\n\
\t\t\tVibrance = 8,\n\
\t\t\tWhites2012 = 5,\n\
\t\t},\n\
\t\tuuid = \"TEST-ID-0001\",\n\
\t},\n\
\tversion = 0,\n\
}\n";

        assert_eq!(output, expected);
    }
}
