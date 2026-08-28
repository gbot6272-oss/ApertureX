//! URL-Parsing für den `apx://`-Protokoll-Handler.
//!
//! Siehe `DECISIONS.md` ADR-0009: das Frontend baut Anfragen über Tauris
//! `convertFileSrc("preview/<id>/<level>", "apx")` bzw.
//! `convertFileSrc("image/<id>/<max_edge_oder_'full'>", "apx")` — das
//! gesamte Segment ist prozentkodiert und wird hier dekodiert und an `/`
//! aufgeteilt, statt einen echten Query-String zu erwarten.

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
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RouteError(pub String);

/// Parst den (noch prozentkodierten) Pfad aus `http::Uri::path()`.
pub(super) fn parse(raw_path: &str) -> Result<ImageRequest, RouteError> {
    let decoded = percent_decode_str(raw_path.trim_start_matches('/'))
        .decode_utf8()
        .map_err(|err| RouteError(format!("Pfad ist kein gültiges UTF-8: {err}")))?;

    let segments: Vec<&str> = decoded.split('/').collect();
    let [kind, id_str, param] = segments[..] else {
        return Err(RouteError(format!(
            "Erwarte genau drei Pfadsegmente (art/id/parameter), erhalten: '{decoded}'"
        )));
    };

    let photo_id: PhotoId = id_str
        .parse()
        .map_err(|err| RouteError(format!("ungültige Foto-ID '{id_str}': {err}")))?;

    match kind {
        "preview" => {
            let level: u8 = param
                .parse()
                .map_err(|err| RouteError(format!("ungültige Vorschau-Stufe '{param}': {err}")))?;
            Ok(ImageRequest::Preview { photo_id, level })
        }
        "image" => {
            let max_edge =
                if param == "full" {
                    None
                } else {
                    Some(param.parse::<u32>().map_err(|err| {
                        RouteError(format!("ungültige Kantenlänge '{param}': {err}"))
                    })?)
                };
            Ok(ImageRequest::Image { photo_id, max_edge })
        }
        other => Err(RouteError(format!(
            "unbekannte Anfrageart '{other}' (erwartet 'preview' oder 'image')"
        ))),
    }
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
}
