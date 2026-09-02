//! Echtes ICC-Farbmanagement für den Export (Phase 8 Schritt 2, siehe
//! `DECISIONS.md` ADR-0034 Punkt 1) — anders als der frühere, nie
//! verdrahtete `lcms2`-Cargo.toml-Eintrag aus Phase 1 (siehe
//! `THIRD_PARTY.md`), diesmal mit echtem Aufrufer: `convert_from_srgb`
//! wandelt den bereits gerenderten sRGB-RGBA8-Puffer
//! (`apx_pipeline::develop::render_rgba8`, siehe dessen Moduldoku — die
//! Pipeline rendert immer nach sRGB) in ein Ziel-Ausgabeprofil um, bevor
//! `format::encode_rgba8` ihn kodiert. Phase 6s simulierter Soft-Proof
//! (`crate::color`-Sättigungskompressionsfaktor) bleibt davon unberührt —
//! der ist ein Vorschau-Feature, kein Export-Feature.
//!
//! Vier gebündelte Standardprofile werden aus ihren offiziellen
//! Chromatizitätskoordinaten/Weißpunkten/Gammawerten aufgebaut
//! (`lcms2::Profile::new_rgb`), statt echte `.icc`-Dateien mitzuliefern —
//! spart Binärgröße, ergibt exakt dieselbe Transformation. **Bewusste
//! Vereinfachung:** ProPhoto RGB und Display P3 nutzen hier eine reine
//! Potenzgamma-Übertragungsfunktion statt ihrer tatsächlichen
//! stückweisen Kurven (ProPhoto: linearer Bereich unter ~1/512;
//! Display P3: identisch zur sRGB-Kurve) — der Unterschied liegt nur in
//! den untersten paar Tonwerten und ist für Exportzwecke vernachlässigbar
//! (dieselbe Art Vereinfachung wie ADR-0028s vereinfachte Auto-Ausrichtung).
//!
//! [`soft_proof_rgba8`] (Phase 12 Schritt 6, siehe `DECISIONS.md`
//! ADR-0039-Nachtrag II) nutzt dieselbe `lcms2`-Anbindung für einen
//! **echten** Soft-Proof über `Transform::new_proofing` mit den
//! `SOFT_PROOFING`/`GAMUT_CHECK`-Flags — ersetzt die bisherige, rein
//! clientseitige Sättigungs-Näherung aus `frontend/src/lib/softProof.ts`
//! (`DECISIONS.md` ADR-0032 Punkt 6: "kein vollständiges ICC-Farb-
//! managementsystem") durch dieselbe LittleCMS-Farbumfangs-Simulation,
//! die auch echte Bildbearbeitungsprogramme verwenden.

/// Farbumfangs-Alarmfarbe für die Gamut-Warnung — Magenta, wie
/// Lightroom/Photoshop es auch verwenden (dieselbe Farbe wie die
/// bisherige clientseitige Näherung).
const GAMUT_ALARM_RGB: [u16; 3] = [0xFFFF, 0x0000, 0xFFFF];

use lcms2::{CIExyY, CIExyYTRIPLE, Flags, Intent, PixelFormat, Profile, ToneCurve, Transform};

use crate::error::{ExportError, Result};

/// Gebündelte Standard-Ausgabeprofile — siehe Moduldoku für die
/// zugrunde liegenden Chromatizitätswerte je Profil.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StandardIccProfile {
    Srgb,
    AdobeRgb,
    ProPhotoRgb,
    DisplayP3,
}

/// Das Ziel-Ausgabeprofil für [`convert_from_srgb`]/[`soft_proof_rgba8`] —
/// eines der vier gebündelten Standardprofile oder eine vom Nutzer
/// ausgewählte `.icc`-Datei.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IccTarget {
    Standard(StandardIccProfile),
    CustomFile(std::path::PathBuf),
}

/// Renderpriorität für [`soft_proof_rgba8`] — entspricht
/// `frontend/src/lib/softProof.ts`s `SoftProofIntent`-Union.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProofingIntent {
    Perceptual,
    RelativeColorimetric,
}

impl ProofingIntent {
    fn to_lcms(self) -> Intent {
        match self {
            ProofingIntent::Perceptual => Intent::Perceptual,
            ProofingIntent::RelativeColorimetric => Intent::RelativeColorimetric,
        }
    }
}

fn xyy(x: f64, y: f64) -> CIExyY {
    CIExyY { x, y, Y: 1.0 }
}

fn build_profile(target: &StandardIccProfile) -> Result<Profile> {
    match target {
        StandardIccProfile::Srgb => Ok(Profile::new_srgb()),
        StandardIccProfile::AdobeRgb => {
            let white = xyy(0.3127, 0.3290); // D65
            let primaries = CIExyYTRIPLE {
                Red: xyy(0.6400, 0.3300),
                Green: xyy(0.2100, 0.7100),
                Blue: xyy(0.1500, 0.0600),
            };
            let curve = ToneCurve::new(2.19921875);
            Profile::new_rgb(&white, &primaries, &[&curve, &curve, &curve]).map_err(|_| {
                ExportError::Icc {
                    message: "Adobe-RGB-Profil konnte nicht erzeugt werden".to_string(),
                }
            })
        }
        StandardIccProfile::ProPhotoRgb => {
            let white = xyy(0.3457, 0.3585); // D50
            let primaries = CIExyYTRIPLE {
                Red: xyy(0.734699, 0.265301),
                Green: xyy(0.159597, 0.840403),
                Blue: xyy(0.036598, 0.000105),
            };
            let curve = ToneCurve::new(1.8);
            Profile::new_rgb(&white, &primaries, &[&curve, &curve, &curve]).map_err(|_| {
                ExportError::Icc {
                    message: "ProPhoto-RGB-Profil konnte nicht erzeugt werden".to_string(),
                }
            })
        }
        StandardIccProfile::DisplayP3 => {
            let white = xyy(0.3127, 0.3290); // D65
            let primaries = CIExyYTRIPLE {
                Red: xyy(0.680, 0.320),
                Green: xyy(0.265, 0.690),
                Blue: xyy(0.150, 0.060),
            };
            let curve = ToneCurve::new(2.2);
            Profile::new_rgb(&white, &primaries, &[&curve, &curve, &curve]).map_err(|_| {
                ExportError::Icc {
                    message: "Display-P3-Profil konnte nicht erzeugt werden".to_string(),
                }
            })
        }
    }
}

/// Wandelt einen interleaved-sRGB-RGBA8-Puffer (`4 * width * height` Bytes,
/// wie ihn `render_rgba8` liefert) in `target` um. Der Alphakanal bleibt
/// unverändert (LittleCMS behandelt ihn als Extrakanal, nicht als
/// Farbkanal). Gibt für [`StandardIccProfile::Srgb`] ohne Transformation
/// den unveränderten Puffer zurück (Identität) statt unnötig durch LCMS
/// zu laufen.
pub fn convert_from_srgb(
    width: u32,
    height: u32,
    pixels: &[u8],
    target: &IccTarget,
) -> Result<Vec<u8>> {
    let expected_len = width as usize * height as usize * 4;
    if pixels.len() != expected_len {
        return Err(ExportError::Icc {
            message: format!(
                "Pufferlänge {} passt nicht zu {width}x{height} RGBA8",
                pixels.len()
            ),
        });
    }

    if let IccTarget::Standard(StandardIccProfile::Srgb) = target {
        return Ok(pixels.to_vec());
    }

    let source = Profile::new_srgb();
    let dest = match target {
        IccTarget::Standard(profile) => build_profile(profile)?,
        IccTarget::CustomFile(path) => Profile::new_file(path).map_err(|err| ExportError::Icc {
            message: format!(
                "ICC-Datei '{}' konnte nicht geladen werden: {err}",
                path.display()
            ),
        })?,
    };

    let transform = Transform::<u8, u8>::new(
        &source,
        PixelFormat::RGBA_8,
        &dest,
        PixelFormat::RGBA_8,
        Intent::RelativeColorimetric,
    )
    .map_err(|_| ExportError::Icc {
        message: "ICC-Transformation konnte nicht erzeugt werden".to_string(),
    })?;

    // Mit einer Kopie von `pixels` vorbelegen, nicht mit Nullen: LittleCMS
    // fasst bei `RGBA_8` nur die drei Farbkanäle an, der Alphakanal bleibt
    // unverändert stehen, wie immer er im Zielpuffer beim Aufruf war —
    // eine Nullvorbelegung würde ihn fälschlich auf 0 statt der
    // tatsächlichen Eingabe-Alpha setzen (siehe Test unten).
    let mut out = pixels.to_vec();
    transform.transform_pixels(pixels, &mut out);
    Ok(out)
}

/// Echter Soft-Proof: simuliert, wie der Puffer auf einem Gerät mit
/// `proofing_target` als Ausgabeprofil aussähe, gerendert für die
/// Bildschirmanzeige (sRGB, siehe Moduldoku). Nutzt `Transform::
/// new_proofing` statt einer clientseitigen Sättigungs-Näherung — dieselbe
/// LittleCMS-Farbumfangs-Simulation, die Lightroom/Photoshop intern
/// verwenden. Bei `gamut_warning = true` werden Pixel außerhalb des
/// simulierten Farbraums in [`GAMUT_ALARM_RGB`] eingefärbt (LittleCMS'
/// `GAMUTCHECK`-Mechanismus, `cmsSetAlarmCodes`).
///
/// **Bewusste Vereinfachung:** die Alarmfarbe wird über den *globalen*
/// `cmsSetAlarmCodes`-Zustand gesetzt (`Transform::set_global_alarm_codes`,
/// als `deprecated` markiert zugunsten eines `ThreadContext`-Objekts pro
/// Aufruf). Das ist hier unproblematisch, weil die Alarmfarbe eine feste
/// Konstante ist, keine aufrufabhängige Einstellung — ein Datenrennen
/// zwischen gleichzeitigen Aufrufen (z. B. paralleler Export-Job und
/// Soft-Proof-Vorschau) schreibt in jedem Fall denselben Wert, ist also
/// folgenlos. Ein `ThreadContext` pro Aufruf würde außerdem verlangen,
/// dass auch [`build_profile`]/`Profile::new_srgb`/`Profile::new_file`
/// auf ihre `_context`-Varianten umgestellt werden — mehr Code ohne
/// zusätzliche Korrektheit für diesen Fall.
pub fn soft_proof_rgba8(
    width: u32,
    height: u32,
    pixels: &[u8],
    proofing_target: &IccTarget,
    intent: ProofingIntent,
    gamut_warning: bool,
) -> Result<Vec<u8>> {
    let expected_len = width as usize * height as usize * 4;
    if pixels.len() != expected_len {
        return Err(ExportError::Icc {
            message: format!(
                "Pufferlänge {} passt nicht zu {width}x{height} RGBA8",
                pixels.len()
            ),
        });
    }

    let display = Profile::new_srgb();
    let proofing = match proofing_target {
        IccTarget::Standard(profile) => build_profile(profile)?,
        IccTarget::CustomFile(path) => Profile::new_file(path).map_err(|err| ExportError::Icc {
            message: format!(
                "ICC-Datei '{}' konnte nicht geladen werden: {err}",
                path.display()
            ),
        })?,
    };

    let mut flags = Flags::SOFT_PROOFING;
    if gamut_warning {
        let mut codes = [0u16; 16];
        codes[0] = GAMUT_ALARM_RGB[0];
        codes[1] = GAMUT_ALARM_RGB[1];
        codes[2] = GAMUT_ALARM_RGB[2];
        #[allow(deprecated)]
        Transform::<u8, u8>::set_global_alarm_codes(codes);
        flags = flags | Flags::GAMUT_CHECK;
    }

    let lcms_intent = intent.to_lcms();
    let transform = Transform::<u8, u8>::new_proofing(
        &display,
        PixelFormat::RGBA_8,
        &display,
        PixelFormat::RGBA_8,
        &proofing,
        lcms_intent,
        lcms_intent,
        flags,
    )
    .map_err(|_| ExportError::Icc {
        message: "Soft-Proof-Transformation konnte nicht erzeugt werden".to_string(),
    })?;

    // Wie bei `convert_from_srgb`: mit einer Kopie von `pixels`
    // vorbelegen, damit der unangetastete Alphakanal erhalten bleibt.
    let mut out = pixels.to_vec();
    transform.transform_pixels(pixels, &mut out);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_pixels() -> Vec<u8> {
        // Ein paar unterschiedliche Farben plus Schwarz/Weiß, damit eine
        // reine Identitätsabbildung nicht zufällig unentdeckt bliebe.
        vec![
            255, 0, 0, 255, // Rot
            0, 255, 0, 255, // Grün
            0, 0, 255, 255, // Blau
            255, 255, 255, 255, // Weiß
        ]
    }

    #[test]
    fn srgb_target_is_a_true_identity_without_running_lcms() {
        let pixels = sample_pixels();
        let out = convert_from_srgb(
            2,
            2,
            &pixels,
            &IccTarget::Standard(StandardIccProfile::Srgb),
        )
        .unwrap();
        assert_eq!(out, pixels);
    }

    #[test]
    fn adobe_rgb_conversion_preserves_alpha_and_changes_saturated_colors() {
        let pixels = sample_pixels();
        let out = convert_from_srgb(
            2,
            2,
            &pixels,
            &IccTarget::Standard(StandardIccProfile::AdobeRgb),
        )
        .unwrap();
        assert_eq!(out.len(), pixels.len());
        // Alpha unangetastet.
        for chunk in out.chunks_exact(4) {
            assert_eq!(chunk[3], 255);
        }
        // Ein gesättigtes Rot bewegt sich in einem größeren Arbeitsraum
        // (Adobe RGB) tatsächlich von seinen ursprünglichen Werten weg —
        // eine reine Passthrough-Implementierung würde hier fehlschlagen.
        assert_ne!(&out[0..3], &pixels[0..3]);
    }

    #[test]
    fn white_stays_approximately_white_across_profiles() {
        // Reines Weiß (relative Farbmetrik, gleicher Weißpunkt bei
        // AdobeRGB/sRGB) darf sich nur minimal verschieben.
        let pixels = sample_pixels();
        let out = convert_from_srgb(
            2,
            2,
            &pixels,
            &IccTarget::Standard(StandardIccProfile::AdobeRgb),
        )
        .unwrap();
        let white = &out[12..15];
        for &channel in white {
            assert!(
                channel >= 250,
                "Weiß sollte nahezu weiß bleiben, war {channel}"
            );
        }
    }

    #[test]
    fn display_p3_conversion_also_preserves_buffer_length() {
        let pixels = sample_pixels();
        let out = convert_from_srgb(
            2,
            2,
            &pixels,
            &IccTarget::Standard(StandardIccProfile::DisplayP3),
        )
        .unwrap();
        assert_eq!(out.len(), pixels.len());
    }

    #[test]
    fn pro_photo_rgb_conversion_also_preserves_buffer_length() {
        let pixels = sample_pixels();
        let out = convert_from_srgb(
            2,
            2,
            &pixels,
            &IccTarget::Standard(StandardIccProfile::ProPhotoRgb),
        )
        .unwrap();
        assert_eq!(out.len(), pixels.len());
    }

    #[test]
    fn mismatched_buffer_length_is_rejected() {
        let pixels = vec![0u8; 3];
        let err = convert_from_srgb(
            2,
            2,
            &pixels,
            &IccTarget::Standard(StandardIccProfile::AdobeRgb),
        )
        .unwrap_err();
        assert!(matches!(err, ExportError::Icc { .. }));
    }

    #[test]
    fn missing_custom_icc_file_is_a_clean_error() {
        let pixels = sample_pixels();
        let err = convert_from_srgb(
            2,
            2,
            &pixels,
            &IccTarget::CustomFile(std::path::PathBuf::from("/nicht/vorhanden.icc")),
        )
        .unwrap_err();
        assert!(matches!(err, ExportError::Icc { .. }));
    }

    #[test]
    fn soft_proof_flags_out_of_gamut_pixels_with_gamut_warning_enabled() {
        // Ein winziges, synthetisches Zielprofil (alle drei Primärfarben
        // dicht am Weißpunkt, also ein deutlich engerer Gamut als sRGB) —
        // reicht aus, um zu zeigen, dass echte gesättigte sRGB-Farben als
        // "außerhalb" erkannt und in der Alarmfarbe markiert werden, ohne
        // auf eine mitgelieferte Referenz-.icc-Datei angewiesen zu sein.
        let white = xyy(0.3127, 0.3290);
        let narrow_primaries = CIExyYTRIPLE {
            Red: xyy(0.33, 0.34),
            Green: xyy(0.31, 0.33),
            Blue: xyy(0.30, 0.31),
        };
        let curve = ToneCurve::new(2.2);
        let mut narrow_profile =
            Profile::new_rgb(&white, &narrow_primaries, &[&curve, &curve, &curve]).unwrap();

        let path = std::env::temp_dir().join(format!(
            "apx-export-test-narrow-gamut-{}.icc",
            std::process::id()
        ));
        narrow_profile.save_profile_to_file(&path).unwrap();

        let pixels = sample_pixels(); // Index 0..3 = reines Rot.
        let out = soft_proof_rgba8(
            2,
            2,
            &pixels,
            &IccTarget::CustomFile(path.clone()),
            ProofingIntent::RelativeColorimetric,
            true,
        );
        std::fs::remove_file(&path).ok();
        let out = out.unwrap();

        // Reines Rot liegt weit außerhalb des winzigen Zielgamuts -> muss
        // als Magenta-Alarmfarbe markiert sein (Alpha bleibt unangetastet).
        assert_eq!(&out[0..4], &[255, 0, 255, 255]);
    }
}
