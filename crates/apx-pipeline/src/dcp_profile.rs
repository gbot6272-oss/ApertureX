//! Liest Adobe-`.dcp`-Kameraprofile (Phase 13 Schritt 3, siehe
//! `DECISIONS.md` ADR-0040-Nachtrag) — ersetzt `stages::calibration`s bisherige
//! `CAMERA_PROFILES`-Handliste durch echten Import beliebiger `.dcp`-
//! Dateien, genau wie Phase 12 Schritt 3 die Objektivprofile von einer
//! Handliste auf die echte LensFun-Datenbank gehoben hat.
//!
//! `.dcp`-Dateien sind reine TIFF/IFD-Container ohne Rohbild-Daten —
//! `gamut-dng`s `DngDecoder::decode()` scheitert daran (verlangt ein
//! echtes Rohbild, siehe dessen eigene Dokumentation). Dieser Parser
//! nutzt stattdessen `gamut-ifd` direkt (dieselbe IFD-Lese-Grundlage, die
//! `gamut-dng` selbst intern verwendet) und liest nur die Tags, die
//! `.dcp`-Dateien tatsächlich enthalten.
//!
//! **Tag-Konstanten:** die numerischen Werte sind gegen den
//! Referenz-Quelltext von `gamut-dng` (`crates.io`, dessen `tags`-Modul
//! dieselben Konstanten öffentlich macht) und unabhängig davon gegen
//! Adobes eigenes, quelloffenes DNG-SDK (`github.com/aizvorski/dng_sdk`,
//! `source/dng_tag_codes.h`) geprüft — nicht aus dem Gedächtnis
//! geschätzt. `gamut-dng` selbst wird hier bewusst NICHT als Abhängigkeit
//! wiederverwendet (nur `gamut-ifd`): `apx-pipeline` bräuchte sonst dessen
//! vollen Rohbild-/Encoder-Werkzeugkasten für ganze 15 Tag-Nummern.
//!
//! **HueSatMap-Anwendung (Tier B, tatsächlich in `stages::calibration`
//! angewendet):** Indexierung, Interpolationsformel und die HSV-
//! Parametrisierung (`0.0..6.0` statt Grad) sind eine direkte Portierung
//! von Adobes eigener Referenzimplementierung
//! (`dng_reference.cpp::RefBaselineHueSatMap`, `dng_hue_sat_map.h/.cpp`)
//! — gegen den echten Quelltext auf GitHub verifiziert (siehe
//! `stages::color_math`s `rgb_to_hsv6`/`hsv6_to_rgb`-Moduldoku), nicht
//! aus der Spezifikation nachgebaut (die PDF-Spezifikation selbst war von
//! dieser Sandbox aus nicht abrufbar — `docs.rs`/`huggingface.co`/
//! `helpx.adobe.com`/`paulbourke.net` sind alle blockiert,
//! `raw.githubusercontent.com` aber erreichbar). **Bewusst vereinfacht:**
//! das optionale `ProfileHueSatMapEncoding` (ein nichtlinearer
//! Kodierungs-Tisch für die Wert-Achse, nur bei wenigen Profilen gesetzt)
//! wird ignoriert — die Wert-Achse bleibt linear, derselbe Kompromiss wie
//! bei jeder anderen "seltener Sonderfall bewusst ausgelassen"-Stelle in
//! diesem Projekt.
//!
//! **Bewusst NICHT angewendet:** `ColorMatrix1`/`ColorMatrix2`/
//! `ForwardMatrix1/2`/`CameraCalibration1/2` — geparst (siehe
//! [`DcpProfile`]), aber nicht in [`edl::v2::DcpProfileData`] übernommen.
//! Eine echte Farbmatrix-Umrechnung (XYZ → Kamera-nativ) ist Aufgabe des
//! Rohdaten-Decoders (`apx-raw`s bereits vorhandene `cam_to_srgb`-Matrix,
//! einmalig beim Dekodieren angewendet), nicht dieser EDL-getriebenen
//! Entwickeln-Stufe — ein Matrixwechsel je Bearbeitung würde eine erneute
//! Rohdaten-Dekodierung bei jedem Kalibrierungs-Regler-Wechsel verlangen,
//! was der gesamten "einmal dekodieren, beliebig oft günstig entwickeln"-
//! Architektur dieses Projekts widerspräche (siehe `ARCHITECTURE.md`).

use gamut_ifd::Value;

use crate::edl::v2::DcpProfileData;
use crate::error::{PipelineError, Result};

// ---- DNG-1.7.1-Tag-Nummern (siehe Moduldoku) --------------------------------

const UNIQUE_CAMERA_MODEL: u16 = 50708;
const COLOR_MATRIX1: u16 = 50721;
const COLOR_MATRIX2: u16 = 50722;
const CAMERA_CALIBRATION1: u16 = 50723;
const CAMERA_CALIBRATION2: u16 = 50724;
const CALIBRATION_ILLUMINANT1: u16 = 50778;
const CALIBRATION_ILLUMINANT2: u16 = 50779;
const PROFILE_NAME: u16 = 50936;
const PROFILE_HUE_SAT_MAP_DIMS: u16 = 50937;
const PROFILE_HUE_SAT_MAP_DATA1: u16 = 50938;
const PROFILE_TONE_CURVE: u16 = 50940;
const FORWARD_MATRIX1: u16 = 50964;
const FORWARD_MATRIX2: u16 = 50965;

/// Vollständig aus einer `.dcp`-Datei gelesene Profildaten — mehr als
/// [`crate::edl::v2::DcpProfileData`] enthält (auch die Farbmatrizen),
/// da Letzteres nur speichert, was `stages::calibration` tatsächlich
/// anwendet (siehe Moduldoku).
#[derive(Debug, Clone, PartialEq)]
pub struct DcpProfile {
    pub name: String,
    pub color_matrix1: [f32; 9],
    pub color_matrix2: Option<[f32; 9]>,
    pub camera_calibration1: Option<[f32; 9]>,
    pub camera_calibration2: Option<[f32; 9]>,
    pub forward_matrix1: Option<[f32; 9]>,
    pub forward_matrix2: Option<[f32; 9]>,
    /// DNG-`LightSource`-Code (siehe DNG-Spezifikation, dieselbe Tabelle
    /// wie EXIF `LightSource`, z. B. `21` = D65, `17` = Standardlicht A).
    pub calibration_illuminant1: Option<u16>,
    pub calibration_illuminant2: Option<u16>,
    /// Nur gesetzt, wenn die Datei ein gültiges HueSatMap-Gitter enthält
    /// (`hue_divisions >= 1`, `sat_divisions >= 2`, `val_divisions >= 1`
    /// — dieselbe Gültigkeitsbedingung wie Adobes eigenes
    /// `dng_hue_sat_map::IsValid()`).
    pub hue_sat_map: Option<DcpProfileData>,
}

fn srational_matrix9(value: &Value) -> Option<[f32; 9]> {
    let pairs = value.as_srationals()?;
    if pairs.len() != 9 {
        return None;
    }
    let mut out = [0.0f32; 9];
    for (slot, &(num, den)) in out.iter_mut().zip(pairs.iter()) {
        *slot = if den == 0 {
            0.0
        } else {
            num as f32 / den as f32
        };
    }
    Some(out)
}

fn ascii_string(value: &Value) -> Option<String> {
    value
        .as_str()
        // DNG-ASCII-Felder können mehrere NUL-getrennte Strings enthalten
        // (siehe `gamut_ifd::Value::Ascii`s Moduldoku) — nur der erste
        // (der einzige, den `.dcp`-Profilnamen tatsächlich nutzen) zählt.
        .map(|s| s.split('\0').next().unwrap_or("").to_string())
        .filter(|s| !s.is_empty())
}

fn calibration_illuminant(value: Option<&Value>) -> Option<u16> {
    match value {
        Some(Value::Short(shorts)) => shorts.first().copied(),
        Some(Value::Long(longs)) => longs.first().and_then(|&v| u16::try_from(v).ok()),
        _ => None,
    }
}

/// Liest [`DcpProfileData::hue_sat_map`] aus `ifd`, falls vorhanden und
/// strukturell gültig — `None` (nicht Fehler), wenn eines der drei Tags
/// fehlt oder die Tabellengröße nicht zur Formel `hueDiv * satDiv *
/// valDiv * 3` passt (Format-Fehler, nicht Programmierfehler: eine echte
/// `.dcp`-Datei, die dieser Formel widerspricht, deutet auf eine defekte/
/// fremde Datei hin — lieber ohne HueSatMap weiterlaufen als abstürzen).
fn parse_hue_sat_map(ifd: &gamut_ifd::Ifd, name: &str) -> Option<DcpProfileData> {
    let Value::Short(dims) = ifd.get(PROFILE_HUE_SAT_MAP_DIMS)? else {
        return None;
    };
    if dims.len() != 3 {
        return None;
    }
    let (hue_divisions, sat_divisions, val_divisions) =
        (dims[0] as u32, dims[1] as u32, dims[2] as u32);
    if hue_divisions == 0 || sat_divisions < 2 || val_divisions == 0 {
        return None;
    }

    let Value::Float(data) = ifd.get(PROFILE_HUE_SAT_MAP_DATA1)? else {
        return None;
    };
    let expected_len = (hue_divisions * sat_divisions * val_divisions * 3) as usize;
    if data.len() != expected_len {
        return None;
    }
    let hue_sat_map: Vec<[f32; 3]> = data
        .chunks_exact(3)
        .map(|chunk| [chunk[0], chunk[1], chunk[2]])
        .collect();

    let tone_curve = match ifd.get(PROFILE_TONE_CURVE) {
        Some(Value::Float(points)) if points.len() >= 4 && points.len() % 2 == 0 => points
            .chunks_exact(2)
            .map(|chunk| [chunk[0], chunk[1]])
            .collect(),
        _ => Vec::new(),
    };

    Some(DcpProfileData {
        name: name.to_string(),
        hue_divisions,
        sat_divisions,
        val_divisions,
        hue_sat_map,
        tone_curve,
    })
}

/// Parst eine `.dcp`-Datei aus ihren rohen Bytes (Phase 13 Schritt 3).
/// `ColorMatrix1` ist laut DNG-Spezifikation für jedes gültige Profil
/// Pflicht — ihr Fehlen (oder eine strukturell ungültige `.dcp`-Datei)
/// ist ein `Err`, alle anderen Tags sind optional.
pub fn parse_dcp_bytes(bytes: &[u8]) -> Result<DcpProfile> {
    let file = gamut_ifd::read(bytes).map_err(|err| PipelineError::DcpProfile {
        message: format!("Datei ist kein gültiges TIFF/IFD-Format: {err}"),
    })?;
    let ifd = file.ifds.first().ok_or_else(|| PipelineError::DcpProfile {
        message: "Datei enthält kein Bildverzeichnis (IFD)".to_string(),
    })?;

    let color_matrix1 = ifd
        .get(COLOR_MATRIX1)
        .and_then(srational_matrix9)
        .ok_or_else(|| PipelineError::DcpProfile {
            message: "Pflichtfeld ColorMatrix1 fehlt oder ist ungültig — keine echte .dcp-Datei?"
                .to_string(),
        })?;

    let name = ifd
        .get(PROFILE_NAME)
        .and_then(ascii_string)
        .or_else(|| ifd.get(UNIQUE_CAMERA_MODEL).and_then(ascii_string))
        .unwrap_or_else(|| "Unbenanntes Profil".to_string());

    Ok(DcpProfile {
        hue_sat_map: parse_hue_sat_map(ifd, &name),
        name,
        color_matrix1,
        color_matrix2: ifd.get(COLOR_MATRIX2).and_then(srational_matrix9),
        camera_calibration1: ifd.get(CAMERA_CALIBRATION1).and_then(srational_matrix9),
        camera_calibration2: ifd.get(CAMERA_CALIBRATION2).and_then(srational_matrix9),
        forward_matrix1: ifd.get(FORWARD_MATRIX1).and_then(srational_matrix9),
        forward_matrix2: ifd.get(FORWARD_MATRIX2).and_then(srational_matrix9),
        calibration_illuminant1: calibration_illuminant(ifd.get(CALIBRATION_ILLUMINANT1)),
        calibration_illuminant2: calibration_illuminant(ifd.get(CALIBRATION_ILLUMINANT2)),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Baut eine winzige, aber strukturell echte `.dcp`-Bytefolge über
    /// `gamut_ifd::write` — derselbe "über die reale Bibliothek erzeugen,
    /// nicht von Hand Bytes basteln"-Ansatz wie die ONNX-Testfixturen in
    /// Phase 13 Schritt 1.
    fn build_test_dcp(hue_sat_map: bool) -> Vec<u8> {
        use gamut_ifd::{write, ByteOrder, Ifd, TiffFile, Value, Variant};

        let mut ifd = Ifd::new();
        ifd.set(
            COLOR_MATRIX1,
            Value::SRational(vec![
                (1, 1),
                (0, 1),
                (0, 1),
                (0, 1),
                (1, 1),
                (0, 1),
                (0, 1),
                (0, 1),
                (2, 1),
            ]),
        );
        ifd.set(PROFILE_NAME, Value::Ascii("Testprofil".to_string()));
        ifd.set(CALIBRATION_ILLUMINANT1, Value::Short(vec![21]));

        if hue_sat_map {
            // 2×2×1-Gitter (kleinstmögliches gültiges 2.5D-Gitter:
            // sat_divisions >= 2), jeder Eintrag ein anderer, gut
            // unterscheidbarer Wert.
            ifd.set(PROFILE_HUE_SAT_MAP_DIMS, Value::Short(vec![2, 2, 1]));
            ifd.set(
                PROFILE_HUE_SAT_MAP_DATA1,
                Value::Float(vec![
                    10.0, 1.0, 1.0, // hue=0, sat=0
                    20.0, 1.1, 1.0, // hue=0, sat=1
                    30.0, 1.2, 1.0, // hue=1, sat=0
                    40.0, 1.3, 1.0, // hue=1, sat=1
                ]),
            );
            ifd.set(
                PROFILE_TONE_CURVE,
                Value::Float(vec![0.0, 0.0, 0.5, 0.6, 1.0, 1.0]),
            );
        }

        write(&TiffFile {
            order: ByteOrder::LittleEndian,
            variant: Variant::Classic,
            ifds: vec![ifd],
        })
        .expect("Test-.dcp sollte sich schreiben lassen")
    }

    #[test]
    fn parses_color_matrix_and_name_from_a_real_tiff_structure() {
        let bytes = build_test_dcp(false);
        let profile = parse_dcp_bytes(&bytes).expect("sollte sich parsen lassen");
        assert_eq!(profile.name, "Testprofil");
        assert_eq!(
            profile.color_matrix1,
            [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 2.0]
        );
        assert_eq!(profile.calibration_illuminant1, Some(21));
        assert!(profile.hue_sat_map.is_none());
    }

    #[test]
    fn parses_a_real_hue_sat_map_and_tone_curve() {
        let bytes = build_test_dcp(true);
        let profile = parse_dcp_bytes(&bytes).expect("sollte sich parsen lassen");
        let map = profile
            .hue_sat_map
            .expect("HueSatMap sollte vorhanden sein");
        assert_eq!(map.hue_divisions, 2);
        assert_eq!(map.sat_divisions, 2);
        assert_eq!(map.val_divisions, 1);
        assert_eq!(map.hue_sat_map.len(), 4);
        assert_eq!(map.hue_sat_map[0], [10.0, 1.0, 1.0]);
        assert_eq!(map.hue_sat_map[3], [40.0, 1.3, 1.0]);
        assert_eq!(map.tone_curve, vec![[0.0, 0.0], [0.5, 0.6], [1.0, 1.0]]);
    }

    #[test]
    fn missing_color_matrix1_is_a_clear_error_not_a_panic() {
        use gamut_ifd::{write, ByteOrder, Ifd, TiffFile, Variant};
        let bytes = write(&TiffFile {
            order: ByteOrder::LittleEndian,
            variant: Variant::Classic,
            ifds: vec![Ifd::new()],
        })
        .expect("leere Test-Datei sollte sich schreiben lassen");
        let result = parse_dcp_bytes(&bytes);
        assert!(result.is_err());
    }
}
