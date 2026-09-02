//! URL-Parsing für den `apx://`-Protokoll-Handler.
//!
//! Siehe `DECISIONS.md` ADR-0009: das Frontend baut Anfragen über Tauris
//! `convertFileSrc("preview/<id>/<level>", "apx")` bzw.
//! `convertFileSrc("image/<id>/<max_edge_oder_'full'>", "apx")` — das
//! gesamte Segment ist prozentkodiert und wird hier dekodiert und an `/`
//! aufgeteilt, statt einen echten Query-String zu erwarten.
//!
//! Ab Phase 2 (siehe `DECISIONS.md` ADR-0016, ADR-0019) kommt
//! `convertFileSrc("develop/<id>/<max_edge_oder_'full'>/<soft_proof>/<edl_json>", "apx")`
//! hinzu: `edl_json` ist die **vollständige, noch prozentkodierte
//! JSON-Serialisierung** des aktuell im Frontend aktiven (u. U. noch
//! nicht committeten) `EdlEnvelope` — bewusst keine reine Prüfsumme
//! ("Hash"), da die Route auch während des Ziehens eines Reglers live
//! rendern muss, bevor überhaupt committet wird (siehe `PLAN.md` Phase 2
//! Schritt 5, Abweichung von der ursprünglichen Plan-Notiz "edl_hash").
//! Derselbe String dient nebenbei als Unterscheidungsmerkmal für den
//! bestehenden Single-Flight-`ImageCache` (zwei verschiedene EDL-Zustände
//! erzeugen zwei verschiedene Cache-Schlüssel, siehe `cache`-Modul) — ein
//! separater Hash-Mechanismus ist dafür nicht nötig. `EdlV1`s Felder sind
//! ausschließlich Zahlen, enthalten also nie ein `/`-Zeichen — die
//! bestehende "erst dekodieren, dann an `/` aufteilen"-Reihenfolge bleibt
//! dadurch für alle drei Anfragearten sicher.
//!
//! `<soft_proof>` (Phase 12 Schritt 6, siehe `DECISIONS.md`
//! ADR-0039-Nachtrag II) ist entweder das literale `none` (kein
//! Soft-Proof, Normalfall) oder eine base64url-kodierte (kein Padding,
//! kein `/` im Alphabet) kleine JSON-Nutzlast
//! `{"target":"srgb"|"adobe_rgb"|"pro_photo_rgb"|"display_p3"|"custom",
//! "custom_path":string|null,"intent":"perceptual"|"relative_colorimetric",
//! "gamut_warning":bool}` — dieselbe Struktur wie `apx-app::commands::
//! parse_icc_target`s bestehende ICC-Zielprofil-Parameter für den Export,
//! hier wiederverwendet. Base64 statt Rohtext, weil `custom_path` (ein
//! Dateipfad zu einer vom Nutzer gewählten `.icc`-Datei) auf den meisten
//! Betriebssystemen `/`-Zeichen enthält — die bestehende
//! "erst dekodieren, dann an `/` aufteilen"-Reihenfolge würde daran sonst
//! zerbrechen (anders als bei `edl_json`, dessen Zahlenfelder das nie tun).

use std::path::PathBuf;

use apx_core::PhotoId;
use percent_encoding::percent_decode_str;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum ImageRequest {
    /// `preview/<id>/<level>` — 0=Thumbnail, 1=Standard, 2=Full.
    Preview { photo_id: PhotoId, level: u8 },
    /// `image/<id>/<max_edge>` bzw. `image/<id>/full`.
    Image {
        photo_id: PhotoId,
        max_edge: Option<u32>,
    },
    /// `develop/<id>/<max_edge_oder_'full'>/<soft_proof>/<edl_json>` — siehe Modul-Doku.
    Develop {
        photo_id: PhotoId,
        max_edge: Option<u32>,
        soft_proof: Option<SoftProofRequest>,
        edl_json: String,
    },
    /// `music/<absoluter_pfad>` (Phase 8 Schritt 4, Diashow-Musiksynchron-
    /// isation) — liefert eine vom Nutzer über den systemeigenen
    /// Datei-Auswahldialog gewählte lokale Audiodatei roh aus, damit sie
    /// im `<audio>`-Element abspielbar ist. Derselbe Vertrauensrahmen wie
    /// die bereits bestehenden ICC-Profil-/Wasserzeichen-Schriftdatei-Pfade
    /// (`apx-app::commands`): ein beliebiger lokaler Pfad, aber nur, weil
    /// der Nutzer ihn selbst über einen Dialog ausgewählt hat, kein
    /// generischer Dateisystemzugriff. Anders als bei den übrigen
    /// Anfragearten wird `<absoluter_pfad>` NICHT an `/` aufgeteilt (ein
    /// Dateipfad enthält so gut wie immer `/`) — alles nach dem ersten
    /// `/`-Segment `music` wird wieder zu einem Pfad zusammengefügt.
    Music { path: PathBuf },
}

/// Geparste `<soft_proof>`-Nutzlast — siehe Modul-Doku.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct SoftProofRequest {
    pub target: apx_export::icc::IccTarget,
    pub intent: apx_export::icc::ProofingIntent,
    pub gamut_warning: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RouteError(pub String);

/// Parst den (noch prozentkodierten) Pfad aus `http::Uri::path()`.
pub(super) fn parse(raw_path: &str) -> Result<ImageRequest, RouteError> {
    let decoded = percent_decode_str(raw_path.trim_start_matches('/'))
        .decode_utf8()
        .map_err(|err| RouteError(format!("Pfad ist kein gültiges UTF-8: {err}")))?;

    let segments: Vec<&str> = decoded.split('/').collect();

    match segments.as_slice() {
        [kind @ ("preview" | "image"), id_str, param] => {
            parse_preview_or_image(kind, id_str, param)
        }
        ["develop", id_str, max_edge_str, soft_proof_str, edl_json] => {
            parse_develop(id_str, max_edge_str, soft_proof_str, edl_json)
        }
        ["music", rest @ ..] if !rest.is_empty() => parse_music(rest),
        _ => Err(RouteError(format!(
            "unbekannte oder falsch aufgebaute Anfrage (erwartet 'art/id/parameter', \
             'develop/id/max_edge/soft_proof/edl_json' oder 'music/absoluter_pfad'), erhalten: '{decoded}'"
        ))),
    }
}

fn parse_music(rest: &[&str]) -> Result<ImageRequest, RouteError> {
    let path = rest.join("/");
    if path.is_empty() {
        return Err(RouteError("leerer Musikdatei-Pfad".to_string()));
    }
    Ok(ImageRequest::Music {
        path: PathBuf::from(path),
    })
}

fn parse_photo_id(id_str: &str) -> Result<PhotoId, RouteError> {
    id_str
        .parse()
        .map_err(|err| RouteError(format!("ungültige Foto-ID '{id_str}': {err}")))
}

fn parse_max_edge(param: &str) -> Result<Option<u32>, RouteError> {
    if param == "full" {
        Ok(None)
    } else {
        param
            .parse::<u32>()
            .map(Some)
            .map_err(|err| RouteError(format!("ungültige Kantenlänge '{param}': {err}")))
    }
}

fn parse_preview_or_image(
    kind: &str,
    id_str: &str,
    param: &str,
) -> Result<ImageRequest, RouteError> {
    let photo_id = parse_photo_id(id_str)?;
    match kind {
        "preview" => {
            let level: u8 = param
                .parse()
                .map_err(|err| RouteError(format!("ungültige Vorschau-Stufe '{param}': {err}")))?;
            Ok(ImageRequest::Preview { photo_id, level })
        }
        "image" => {
            let max_edge = parse_max_edge(param)?;
            Ok(ImageRequest::Image { photo_id, max_edge })
        }
        other => Err(RouteError(format!(
            "unbekannte Anfrageart '{other}' (erwartet 'preview' oder 'image')"
        ))),
    }
}

fn parse_develop(
    id_str: &str,
    max_edge_str: &str,
    soft_proof_str: &str,
    edl_json: &str,
) -> Result<ImageRequest, RouteError> {
    let photo_id = parse_photo_id(id_str)?;
    let max_edge = parse_max_edge(max_edge_str)?;
    let soft_proof = parse_soft_proof(soft_proof_str)?;
    if edl_json.is_empty() {
        return Err(RouteError("leeres edl_json-Segment".to_string()));
    }
    Ok(ImageRequest::Develop {
        photo_id,
        max_edge,
        soft_proof,
        edl_json: edl_json.to_string(),
    })
}

/// Deserialisiert `<soft_proof>` — siehe Modul-Doku für das Format.
/// `"none"` (Normalfall) ergibt `Ok(None)`, alles andere wird als
/// base64url-kodiertes JSON interpretiert.
fn parse_soft_proof(segment: &str) -> Result<Option<SoftProofRequest>, RouteError> {
    if segment == "none" {
        return Ok(None);
    }

    use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};

    #[derive(serde::Deserialize)]
    struct RawSoftProof {
        target: String,
        custom_path: Option<String>,
        intent: String,
        gamut_warning: bool,
    }

    let json_bytes = URL_SAFE_NO_PAD
        .decode(segment)
        .map_err(|err| RouteError(format!("ungültige Soft-Proof-Kodierung: {err}")))?;
    let json_str = String::from_utf8(json_bytes)
        .map_err(|err| RouteError(format!("Soft-Proof-Segment ist kein gültiges UTF-8: {err}")))?;
    let raw: RawSoftProof = serde_json::from_str(&json_str)
        .map_err(|err| RouteError(format!("Soft-Proof-JSON ungültig: {err}")))?;

    let target = crate::commands::parse_icc_target(&raw.target, raw.custom_path.as_deref())
        .map_err(RouteError)?;
    let intent = match raw.intent.as_str() {
        "perceptual" => apx_export::icc::ProofingIntent::Perceptual,
        "relative_colorimetric" => apx_export::icc::ProofingIntent::RelativeColorimetric,
        other => {
            return Err(RouteError(format!(
                "unbekannte Soft-Proof-Renderpriorität '{other}'"
            )))
        }
    };

    Ok(Some(SoftProofRequest {
        target,
        intent,
        gamut_warning: raw.gamut_warning,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use percent_encoding::{utf8_percent_encode, NON_ALPHANUMERIC};

    fn encode(path: &str) -> String {
        format!("/{}", utf8_percent_encode(path, NON_ALPHANUMERIC))
    }

    #[test]
    fn parses_preview_request() {
        let id = PhotoId::new();
        let raw = encode(&format!("preview/{id}/0"));
        let parsed = parse(&raw).expect("sollte parsen");
        assert_eq!(
            parsed,
            ImageRequest::Preview {
                photo_id: id,
                level: 0
            }
        );
    }

    #[test]
    fn parses_image_request_with_max_edge() {
        let id = PhotoId::new();
        let raw = encode(&format!("image/{id}/2560"));
        let parsed = parse(&raw).expect("sollte parsen");
        assert_eq!(
            parsed,
            ImageRequest::Image {
                photo_id: id,
                max_edge: Some(2560)
            }
        );
    }

    #[test]
    fn parses_image_request_full_resolution() {
        let id = PhotoId::new();
        let raw = encode(&format!("image/{id}/full"));
        let parsed = parse(&raw).expect("sollte parsen");
        assert_eq!(
            parsed,
            ImageRequest::Image {
                photo_id: id,
                max_edge: None
            }
        );
    }

    #[test]
    fn rejects_unknown_kind() {
        let id = PhotoId::new();
        let raw = encode(&format!("thumbnail/{id}/0"));
        assert!(parse(&raw).is_err());
    }

    #[test]
    fn rejects_wrong_segment_count() {
        assert!(parse(&encode("preview/only-one-segment")).is_err());
        assert!(parse(&encode("preview/too/many/segments/here")).is_err());
    }

    #[test]
    fn rejects_invalid_photo_id() {
        assert!(parse(&encode("preview/nicht-valide/0")).is_err());
    }

    #[test]
    fn parses_develop_request_with_max_edge() {
        let id = PhotoId::new();
        let edl_json = r#"{"schema_version":1,"payload":{}}"#;
        let raw = encode(&format!("develop/{id}/2048/none/{edl_json}"));
        let parsed = parse(&raw).expect("sollte parsen");
        assert_eq!(
            parsed,
            ImageRequest::Develop {
                photo_id: id,
                max_edge: Some(2048),
                soft_proof: None,
                edl_json: edl_json.to_string(),
            }
        );
    }

    #[test]
    fn parses_develop_request_full_resolution() {
        let id = PhotoId::new();
        let edl_json = "{}";
        let raw = encode(&format!("develop/{id}/full/none/{edl_json}"));
        let parsed = parse(&raw).expect("sollte parsen");
        assert_eq!(
            parsed,
            ImageRequest::Develop {
                photo_id: id,
                max_edge: None,
                soft_proof: None,
                edl_json: edl_json.to_string(),
            }
        );
    }

    #[test]
    fn parses_develop_request_with_soft_proof() {
        use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};

        let id = PhotoId::new();
        let edl_json = "{}";
        let soft_proof_json = r#"{"target":"adobe_rgb","custom_path":null,"intent":"perceptual","gamut_warning":true}"#;
        let soft_proof_segment = URL_SAFE_NO_PAD.encode(soft_proof_json);
        let raw = encode(&format!(
            "develop/{id}/full/{soft_proof_segment}/{edl_json}"
        ));
        let parsed = parse(&raw).expect("sollte parsen");
        assert_eq!(
            parsed,
            ImageRequest::Develop {
                photo_id: id,
                max_edge: None,
                soft_proof: Some(SoftProofRequest {
                    target: apx_export::icc::IccTarget::Standard(
                        apx_export::icc::StandardIccProfile::AdobeRgb
                    ),
                    intent: apx_export::icc::ProofingIntent::Perceptual,
                    gamut_warning: true,
                }),
                edl_json: edl_json.to_string(),
            }
        );
    }

    #[test]
    fn rejects_develop_request_with_empty_edl_json() {
        let id = PhotoId::new();
        assert!(parse(&encode(&format!("develop/{id}/full/none/"))).is_err());
    }

    #[test]
    fn rejects_develop_request_with_wrong_segment_count() {
        let id = PhotoId::new();
        assert!(parse(&encode(&format!("develop/{id}/full"))).is_err());
        assert!(parse(&encode(&format!("develop/{id}/full/none/{{}}/extra"))).is_err());
    }

    #[test]
    fn rejects_develop_request_with_invalid_max_edge() {
        let id = PhotoId::new();
        assert!(parse(&encode(&format!("develop/{id}/not-a-number/none/{{}}"))).is_err());
    }

    #[test]
    fn parses_music_request_with_absolute_path_preserving_slashes() {
        let raw = encode("music//home/user/Musik/song.mp3");
        let parsed = parse(&raw).expect("sollte parsen");
        assert_eq!(
            parsed,
            ImageRequest::Music {
                path: PathBuf::from("/home/user/Musik/song.mp3")
            }
        );
    }

    #[test]
    fn rejects_music_request_without_a_path() {
        assert!(parse(&encode("music")).is_err());
    }
}
