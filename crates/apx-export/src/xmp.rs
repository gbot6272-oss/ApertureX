//! Adobe-XMP-Sidecar-Export/-Import (Phase 9 Schritt 2, siehe
//! `PLAN.md`/`DECISIONS.md` ADR-0031 Punkt 3/ADR-0035). Deckt zwei
//! Anwendungsfälle ab, die sich dasselbe XMP/RDF-Grundgerüst teilen:
//!
//! - **Sidecar-Export**: IPTC-artige Metadaten (`dc:title`/`description`/
//!   `creator`/`rights`, `dc:subject` für Schlagworte) als `.xmp`-Datei
//!   neben dem Original — reine Anzeigedaten, kein Rendering-Bezug.
//! - **Adobe-Entwickeln-Einstellungen** (`crs:`-Namensraum, „Camera Raw
//!   Settings"): **echt bidirektional** (Export *und* Import) für die
//!   Grundeinstellungen (Belichtung/Kontrast/Lichter/Tiefen/Weiß/Schwarz/
//!   Textur/Klarheit/Dunst/Dynamik/Sättigung) und die acht HSL-Bänder —
//!   möglich, weil deren Adobe-Eigenschaftsnamen und Wertebereiche seit
//!   Process Version 2012 öffentlich dokumentiert, stabil und (bis auf
//!   eine Ausnahme, siehe unten) numerisch identisch zu unseren eigenen
//!   Reglerbereichen sind (`-100..100` für die meisten, `-5..5` EV für
//!   die Belichtung — siehe `frontend/src/lib/edl.ts`s Slider-Definitionen).
//!   **Kurven/Farbmischer/Color-Grading/Objektivkorrekturen/Effekte/
//!   Masken bleiben bewusst unübersetzt** — Adobes interne Kodierung
//!   dieser Bereiche (z. B. Kurvenpunkt-Listen als gepackte Zeichenketten)
//!   ist nicht vollständig öffentlich dokumentiert, eine geratene Zuordnung
//!   wäre unehrlich gegenüber „Fertig (abweichend)".
//!
//! **Bewusste Vereinfachung Weißabgleich:** unser Modell speichert eine
//! *Verschiebung* (`temp_shift_kelvin`/`tint_shift` relativ zum
//! Kamera-Ausgangswert), Adobes `crs:Temperature`/`crs:Tint` sind dagegen
//! *absolute* Werte. Ohne den tatsächlichen Kamera-Ausgangswert (der beim
//! Export nicht mit hinreichender Zuverlässigkeit bekannt ist) kann keine
//! exakte Umrechnung erfolgen — der Weißabgleich wird deshalb **nicht**
//! mit exportiert/importiert, nur die übrigen Basic-Regler.

use quick_xml::events::Event;
use quick_xml::reader::Reader;

use apx_pipeline::edl::{BasicAdjustments, HslAdjustment, HslBand};

use crate::error::{ExportError, Result};

/// IPTC-artige Sidecar-Metadaten — unabhängig von den Entwickeln-
/// Einstellungen, kann auch ohne `develop` geschrieben werden.
#[derive(Debug, Clone, Default)]
pub struct XmpSidecarMetadata {
    pub title: Option<String>,
    pub caption: Option<String>,
    pub copyright: Option<String>,
    pub creator: Option<String>,
    pub keywords: Vec<String>,
}

fn escape_xml(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

type HslBandGetter = fn(&HslAdjustment) -> &HslBand;

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

/// Baut den vollständigen Inhalt einer `.xmp`-Sidecar-Datei — `develop`
/// ist optional (nur Metadaten ohne Entwickeln-Einstellungen ist ein
/// gültiger, häufiger Fall).
pub fn generate_xmp(
    metadata: &XmpSidecarMetadata,
    develop: Option<(&BasicAdjustments, &HslAdjustment)>,
) -> String {
    let mut crs_attrs = String::new();
    if let Some((basic, hsl)) = develop {
        // Alle Basic-Werte außer Weißabgleich (siehe Moduldoku) passen als
        // einfache RDF-Attribute auf das `crs:`-Beschreibungselement —
        // dieselbe kompakte Form, die echte Adobe-Sidecars verwenden.
        crs_attrs.push_str(&format!(
            "\n   crs:Exposure2012=\"{:.6}\"\n   crs:Contrast2012=\"{}\"\n   crs:Highlights2012=\"{}\"\n   crs:Shadows2012=\"{}\"\n   crs:Whites2012=\"{}\"\n   crs:Blacks2012=\"{}\"\n   crs:Texture=\"{}\"\n   crs:Clarity2012=\"{}\"\n   crs:Dehaze=\"{}\"\n   crs:Vibrance=\"{}\"\n   crs:Saturation=\"{}\"",
            basic.exposure_ev,
            basic.contrast as i32,
            basic.highlights as i32,
            basic.shadows as i32,
            basic.whites as i32,
            basic.blacks as i32,
            basic.texture as i32,
            basic.clarity as i32,
            basic.dehaze as i32,
            basic.vibrance as i32,
            basic.saturation as i32,
        ));
        for (name, get) in HSL_BANDS {
            let band = get(hsl);
            crs_attrs.push_str(&format!(
                "\n   crs:HueAdjustment{name}=\"{}\"\n   crs:SaturationAdjustment{name}=\"{}\"\n   crs:LuminanceAdjustment{name}=\"{}\"",
                band.hue as i32, band.saturation as i32, band.luminance as i32
            ));
        }
    }

    let dc_title = metadata
        .title
        .as_ref()
        .map(|t| format!("\n     <dc:title><rdf:Alt><rdf:li xml:lang=\"x-default\">{}</rdf:li></rdf:Alt></dc:title>", escape_xml(t)))
        .unwrap_or_default();
    let dc_description = metadata
        .caption
        .as_ref()
        .map(|c| format!("\n     <dc:description><rdf:Alt><rdf:li xml:lang=\"x-default\">{}</rdf:li></rdf:Alt></dc:description>", escape_xml(c)))
        .unwrap_or_default();
    let dc_rights = metadata
        .copyright
        .as_ref()
        .map(|c| format!("\n     <dc:rights><rdf:Alt><rdf:li xml:lang=\"x-default\">{}</rdf:li></rdf:Alt></dc:rights>", escape_xml(c)))
        .unwrap_or_default();
    let dc_creator = metadata
        .creator
        .as_ref()
        .map(|c| {
            format!(
                "\n     <dc:creator><rdf:Seq><rdf:li>{}</rdf:li></rdf:Seq></dc:creator>",
                escape_xml(c)
            )
        })
        .unwrap_or_default();
    let dc_subject = if metadata.keywords.is_empty() {
        String::new()
    } else {
        let items: String = metadata
            .keywords
            .iter()
            .map(|k| format!("<rdf:li>{}</rdf:li>", escape_xml(k)))
            .collect();
        format!("\n     <dc:subject><rdf:Bag>{items}</rdf:Bag></dc:subject>")
    };

    format!(
        "<?xpacket begin=\"\u{feff}\" id=\"W5M0MpCehiHzreSzNTczkc9d\"?>\n\
<x:xmpmeta xmlns:x=\"adobe:ns:meta/\">\n\
 <rdf:RDF xmlns:rdf=\"http://www.w3.org/1999/02/22-rdf-syntax-ns#\">\n\
  <rdf:Description rdf:about=\"\"\n\
   xmlns:dc=\"http://purl.org/dc/elements/1.1/\"\n\
   xmlns:crs=\"http://ns.adobe.com/camera-raw-settings/1.0/\"{crs_attrs}>{dc_title}{dc_description}{dc_rights}{dc_creator}{dc_subject}\n\
  </rdf:Description>\n\
 </rdf:RDF>\n\
</x:xmpmeta>\n\
<?xpacket end=\"w\"?>\n"
    )
}

/// Schreibt eine `.xmp`-Sidecar-Datei neben `photo_path` (gleicher
/// Dateiname, `.xmp`-Endung statt der ursprünglichen).
pub fn write_sidecar(
    photo_path: &std::path::Path,
    metadata: &XmpSidecarMetadata,
    develop: Option<(&BasicAdjustments, &HslAdjustment)>,
) -> Result<std::path::PathBuf> {
    let sidecar_path = photo_path.with_extension("xmp");
    let content = generate_xmp(metadata, develop);
    std::fs::write(&sidecar_path, content).map_err(|err| ExportError::Io {
        path: sidecar_path.display().to_string(),
        message: err.to_string(),
    })?;
    Ok(sidecar_path)
}

/// Aus einer `.xmp`-Datei (egal ob von Aperture X oder einem echten
/// Adobe-Produkt exportiert) geparste Entwickeln-Einstellungen — nur die
/// Felder, die tatsächlich im XMP vorkamen, sind `Some`. Der Aufrufer
/// mischt sie auf den aktuellen Bearbeitungsstand (dasselbe Muster wie
/// eine Preset-EDL-Teilmenge, siehe `lib/presets.ts::mergeEdlSubset`).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ParsedDevelopSettings {
    pub basic: Option<BasicAdjustments>,
    pub hsl: Option<HslAdjustment>,
}

/// Parst die `crs:`-Attribute einer `.xmp`-Datei — liest nur das erste
/// `rdf:Description`-Element mit `crs:`-Attributen, wie es echte Adobe-
/// Sidecars und unser eigener [`generate_xmp`] erzeugen.
pub fn parse_xmp_develop_settings(xml: &str) -> Result<ParsedDevelopSettings> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    loop {
        match reader.read_event() {
            Ok(Event::Start(tag)) | Ok(Event::Empty(tag)) => {
                let local: &str = tag.name().into_inner();
                if local != "rdf:Description" && local != "Description" {
                    continue;
                }
                let mut basic = BasicAdjustments::NEUTRAL;
                let mut hsl = HslAdjustment::NEUTRAL;
                let mut found_basic = false;
                let mut found_hsl = false;

                for attr in tag.attributes().flatten() {
                    let key: &str = attr.key.into_inner();
                    let key = key.strip_prefix("crs:").unwrap_or(key);
                    let Ok(value) = attr.value.parse::<f32>() else {
                        continue;
                    };
                    match key {
                        "Exposure2012" => {
                            basic.exposure_ev = value;
                            found_basic = true;
                        }
                        "Contrast2012" => {
                            basic.contrast = value;
                            found_basic = true;
                        }
                        "Highlights2012" => {
                            basic.highlights = value;
                            found_basic = true;
                        }
                        "Shadows2012" => {
                            basic.shadows = value;
                            found_basic = true;
                        }
                        "Whites2012" => {
                            basic.whites = value;
                            found_basic = true;
                        }
                        "Blacks2012" => {
                            basic.blacks = value;
                            found_basic = true;
                        }
                        "Texture" => {
                            basic.texture = value;
                            found_basic = true;
                        }
                        "Clarity2012" => {
                            basic.clarity = value;
                            found_basic = true;
                        }
                        "Dehaze" => {
                            basic.dehaze = value;
                            found_basic = true;
                        }
                        "Vibrance" => {
                            basic.vibrance = value;
                            found_basic = true;
                        }
                        "Saturation" => {
                            basic.saturation = value;
                            found_basic = true;
                        }
                        _ => {
                            for (name, _) in HSL_BANDS {
                                if key == format!("HueAdjustment{name}") {
                                    set_hsl_field(&mut hsl, name, value, HslField::Hue);
                                    found_hsl = true;
                                } else if key == format!("SaturationAdjustment{name}") {
                                    set_hsl_field(&mut hsl, name, value, HslField::Saturation);
                                    found_hsl = true;
                                } else if key == format!("LuminanceAdjustment{name}") {
                                    set_hsl_field(&mut hsl, name, value, HslField::Luminance);
                                    found_hsl = true;
                                }
                            }
                        }
                    }
                }

                return Ok(ParsedDevelopSettings {
                    basic: found_basic.then_some(basic),
                    hsl: found_hsl.then_some(hsl),
                });
            }
            Ok(Event::Eof) => break,
            Err(err) => {
                return Err(ExportError::Unsupported(format!(
                    "XMP-Datei nicht lesbar: {err}"
                )))
            }
            _ => {}
        }
    }

    Ok(ParsedDevelopSettings::default())
}

enum HslField {
    Hue,
    Saturation,
    Luminance,
}

fn set_hsl_field(hsl: &mut HslAdjustment, band_name: &str, value: f32, field: HslField) {
    let band = match band_name {
        "Red" => &mut hsl.red,
        "Orange" => &mut hsl.orange,
        "Yellow" => &mut hsl.yellow,
        "Green" => &mut hsl.green,
        "Aqua" => &mut hsl.aqua,
        "Blue" => &mut hsl.blue,
        "Purple" => &mut hsl.purple,
        "Magenta" => &mut hsl.magenta,
        _ => return,
    };
    match field {
        HslField::Hue => band.hue = value,
        HslField::Saturation => band.saturation = value,
        HslField::Luminance => band.luminance = value,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_basic() -> BasicAdjustments {
        BasicAdjustments {
            exposure_ev: 0.75,
            contrast: 15.0,
            highlights: -30.0,
            shadows: 20.0,
            ..BasicAdjustments::NEUTRAL
        }
    }

    fn sample_hsl() -> HslAdjustment {
        let mut hsl = HslAdjustment::NEUTRAL;
        hsl.red.saturation = 25.0;
        hsl.blue.luminance = -10.0;
        hsl
    }

    #[test]
    fn generate_xmp_embeds_metadata_and_develop_settings() {
        let metadata = XmpSidecarMetadata {
            title: Some("Sonnenuntergang".to_string()),
            caption: None,
            copyright: Some("© Test & Co".to_string()),
            creator: Some("Max Mustermann".to_string()),
            keywords: vec!["Natur".to_string(), "Abend".to_string()],
        };
        let basic = sample_basic();
        let hsl = sample_hsl();
        let xml = generate_xmp(&metadata, Some((&basic, &hsl)));

        assert!(xml.contains("Sonnenuntergang"));
        assert!(xml.contains("&amp;"), "muss XML-Sonderzeichen escapen");
        assert!(xml.contains("crs:Exposure2012=\"0.75"));
        assert!(xml.contains("crs:HueAdjustmentRed=\"0\""));
        assert!(xml.contains("crs:SaturationAdjustmentRed=\"25\""));
        assert!(xml.contains("crs:LuminanceAdjustmentBlue=\"-10\""));
    }

    #[test]
    fn generate_xmp_without_develop_omits_crs_namespace_attributes() {
        let metadata = XmpSidecarMetadata {
            title: Some("Nur Metadaten".to_string()),
            ..Default::default()
        };
        let xml = generate_xmp(&metadata, None);
        assert!(!xml.contains("crs:Exposure2012"));
        assert!(xml.contains("Nur Metadaten"));
    }

    #[test]
    fn roundtrip_basic_and_hsl_through_generate_and_parse() {
        let basic = sample_basic();
        let hsl = sample_hsl();
        let xml = generate_xmp(&XmpSidecarMetadata::default(), Some((&basic, &hsl)));

        let parsed = parse_xmp_develop_settings(&xml).expect("parsen");
        assert_eq!(parsed.basic, Some(basic));
        assert_eq!(parsed.hsl, Some(hsl));
    }

    #[test]
    fn parse_xmp_without_crs_attributes_returns_none_for_both() {
        let xml = generate_xmp(
            &XmpSidecarMetadata {
                title: Some("x".to_string()),
                ..Default::default()
            },
            None,
        );
        let parsed = parse_xmp_develop_settings(&xml).expect("parsen");
        assert_eq!(parsed.basic, None);
        assert_eq!(parsed.hsl, None);
    }

    #[test]
    fn parse_real_looking_adobe_xmp_fragment() {
        // Ein handgekürzter, aber strukturell echter Adobe-Camera-Raw-XMP-Ausschnitt.
        let xml = r#"<x:xmpmeta xmlns:x="adobe:ns:meta/">
 <rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#">
  <rdf:Description rdf:about=""
   xmlns:crs="http://ns.adobe.com/camera-raw-settings/1.0/"
   crs:Exposure2012="+0.50"
   crs:Contrast2012="10"
   crs:HueAdjustmentAqua="-5">
  </rdf:Description>
 </rdf:RDF>
</x:xmpmeta>"#;
        let parsed = parse_xmp_develop_settings(xml).expect("parsen");
        let basic = parsed.basic.expect("basic vorhanden");
        assert_eq!(basic.exposure_ev, 0.5);
        assert_eq!(basic.contrast, 10.0);
        let hsl = parsed.hsl.expect("hsl vorhanden");
        assert_eq!(hsl.aqua.hue, -5.0);
    }
}
