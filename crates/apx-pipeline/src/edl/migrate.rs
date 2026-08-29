//! Umwandlung zwischen `apx_core::EdlEnvelope` (dem für `apx-catalog`
//! undurchsichtigen Umschlag) und der konkreten `EdlV1`-Struktur.
//!
//! Aktuell gibt es nur Schema-Version 1, daher ist die „Upgrade-Kette"
//! ein einzelner Vergleich. Sobald eine Schema-Version 2 hinzukommt, wird
//! [`from_envelope`] hier um einen Umwandlungsschritt v1→v2 erweitert —
//! der Aufrufer (`apx-app`) ändert sich nicht, er ruft immer nur
//! `from_envelope` auf und bekommt die jeweils aktuelle Struktur zurück.

use apx_core::EdlEnvelope;

use super::{EdlV1, EDL_SCHEMA_VERSION};
use crate::error::{PipelineError, Result};

/// Liest ein `EdlV1` aus einem Umschlag. Schlägt fehl, wenn die
/// `schema_version` nicht bekannt ist (siehe `SPEC.md` §2.1: „Versionierte
/// EDL: Schema-Migration muss alte Kataloge öffnen können" — die Prüfung
/// hier ist die Kehrseite davon: eine *neuere*, noch unbekannte Version
/// wird explizit abgelehnt statt stillschweigend falsch interpretiert).
pub fn from_envelope(envelope: &EdlEnvelope) -> Result<EdlV1> {
    if envelope.schema_version != EDL_SCHEMA_VERSION {
        return Err(PipelineError::InvalidEdl {
            message: format!(
                "unbekannte EDL-Schema-Version {} (diese Aperture-X-Version kennt nur Version {EDL_SCHEMA_VERSION})",
                envelope.schema_version
            ),
        });
    }
    serde_json::from_value(envelope.payload.clone()).map_err(|source| PipelineError::InvalidEdl {
        message: format!(
            "EDL-Nutzlast entspricht nicht Schema-Version {EDL_SCHEMA_VERSION}: {source}"
        ),
    })
}

/// Verpackt ein `EdlV1` in einen Umschlag zum Speichern.
pub fn to_envelope(edl: &EdlV1) -> Result<EdlEnvelope> {
    let payload = serde_json::to_value(edl).map_err(|source| PipelineError::InvalidEdl {
        message: format!("EDL konnte nicht serialisiert werden: {source}"),
    })?;
    Ok(EdlEnvelope::new(EDL_SCHEMA_VERSION, payload))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::edl::v1::BasicAdjustments;

    #[test]
    fn roundtrips_through_envelope() {
        let edl = EdlV1 {
            basic: BasicAdjustments {
                exposure_ev: 0.5,
                ..BasicAdjustments::NEUTRAL
            },
        };
        let envelope = to_envelope(&edl).expect("sollte verpacken");
        let parsed = from_envelope(&envelope).expect("sollte entpacken");
        assert_eq!(edl, parsed);
    }

    #[test]
    fn unknown_schema_version_is_rejected() {
        let envelope = EdlEnvelope::new(9999, serde_json::json!({}));
        let result = from_envelope(&envelope);
        assert!(matches!(result, Err(PipelineError::InvalidEdl { .. })));
    }

    #[test]
    fn malformed_payload_for_known_version_is_rejected() {
        // Richtige Schema-Version, aber Nutzlast passt nicht zu EdlV1
        // (fehlende Pflichtfelder) — muss als Fehler erkannt werden, nicht
        // stillschweigend mit Default-Werten aufgefüllt werden.
        let envelope = EdlEnvelope::new(
            EDL_SCHEMA_VERSION,
            serde_json::json!({ "unbekanntes_feld": 1 }),
        );
        let result = from_envelope(&envelope);
        assert!(result.is_err());
    }
}
