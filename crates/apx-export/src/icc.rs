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

use lcms2::{CIExyY, CIExyYTRIPLE, Intent, PixelFormat, Profile, ToneCurve, Transform};

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

/// Das Ziel-Ausgabeprofil für [`convert_from_srgb`] — eines der vier
/// gebündelten Standardprofile oder eine vom Nutzer ausgewählte `.icc`-Datei.
#[derive(Debug, Clone)]
pub enum IccTarget {
    Standard(StandardIccProfile),
    CustomFile(std::path::PathBuf),
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
}
