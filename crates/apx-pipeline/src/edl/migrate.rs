//! Umwandlung zwischen `apx_core::EdlEnvelope` (dem für `apx-catalog`
//! undurchsichtigen Umschlag) und der jeweils aktuellen konkreten
//! EDL-Struktur (`EdlV2`).
//!
//! Es gibt jetzt zwei bekannte Schema-Versionen: die aktuelle
//! ([`EDL_SCHEMA_VERSION`], `EdlV2`) und die historische Version 1
//! (`EdlV1`, aus Phase 2 — noch in alten `edit_history`-Zeilen
//! gespeichert). [`from_envelope`] liest beide, zieht Version 1 aber
//! sofort auf `EdlV2` hoch (`EdlV2::from_v1`) — der Aufrufer (`apx-app`)
//! sieht davon nichts, er ruft immer nur `from_envelope` auf und bekommt
//! die jeweils aktuelle Struktur zurück. Sobald eine Schema-Version 3
//! hinzukommt, wird hier ein weiterer Umwandlungsschritt v2→v3 ergänzt,
//! nach demselben Muster.

use apx_core::EdlEnvelope;

use super::{v1::EdlV1, v2::EdlV2, EDL_SCHEMA_VERSION};
use crate::error::{PipelineError, Result};

/// Die historische Schema-Version 1 (Phase 2, sieben Grundregler) — nur
/// noch zum *Lesen* alter Einträge relevant, [`to_envelope`] schreibt nie
/// mehr in dieser Version.
const V1_SCHEMA_VERSION: u32 = 1;

/// Liest die jeweils aktuelle EDL-Struktur (`EdlV2`) aus einem Umschlag.
/// Alte Version-1-Umschläge werden automatisch hochgezogen
/// (`EdlV2::from_v1`); eine unbekannte Version oder eine kaputte Nutzlast
/// für die jeweilige Version wird abgelehnt statt stillschweigend
/// repariert (siehe `SPEC.md` §2.1: „Versionierte EDL: Schema-Migration
/// muss alte Kataloge öffnen können" — die Prüfung hier ist die Kehrseite
/// davon: eine *neuere*, noch unbekannte Version wird explizit abgelehnt
/// statt stillschweigend falsch interpretiert).
pub fn from_envelope(envelope: &EdlEnvelope) -> Result<EdlV2> {
    if envelope.schema_version == EDL_SCHEMA_VERSION {
        return serde_json::from_value(envelope.payload.clone()).map_err(|source| {
            PipelineError::InvalidEdl {
                message: format!(
                    "EDL-Nutzlast entspricht nicht Schema-Version {EDL_SCHEMA_VERSION}: {source}"
                ),
            }
        });
    }

    if envelope.schema_version == V1_SCHEMA_VERSION {
        let old: EdlV1 = serde_json::from_value(envelope.payload.clone()).map_err(|source| {
            PipelineError::InvalidEdl {
                message: format!(
                    "EDL-Nutzlast entspricht nicht Schema-Version {V1_SCHEMA_VERSION}: {source}"
                ),
            }
        })?;
        return Ok(EdlV2::from_v1(old));
    }

    Err(PipelineError::InvalidEdl {
        message: format!(
            "unbekannte EDL-Schema-Version {} (diese Aperture-X-Version kennt Version \
             {V1_SCHEMA_VERSION} und {EDL_SCHEMA_VERSION})",
            envelope.schema_version
        ),
    })
}

/// Verpackt ein `EdlV2` in einen Umschlag zum Speichern — immer als
/// aktuelle Schema-Version. Abwärtskompatibilität ist nur fürs *Lesen*
/// nötig (siehe [`from_envelope`]), nicht fürs Schreiben: jede neu
/// committete Bearbeitung landet als Version 2 in `edit_history`.
pub fn to_envelope(edl: &EdlV2) -> Result<EdlEnvelope> {
    let payload = serde_json::to_value(edl).map_err(|source| PipelineError::InvalidEdl {
        message: format!("EDL konnte nicht serialisiert werden: {source}"),
    })?;
    Ok(EdlEnvelope::new(EDL_SCHEMA_VERSION, payload))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::edl::v1::BasicAdjustments as V1BasicAdjustments;
    use crate::edl::v2::BasicAdjustments as V2BasicAdjustments;

    #[test]
    fn roundtrips_through_envelope() {
        let edl = EdlV2 {
            basic: V2BasicAdjustments {
                exposure_ev: 0.5,
                ..V2BasicAdjustments::NEUTRAL
            },
            ..EdlV2::neutral()
        };
        let envelope = to_envelope(&edl).expect("sollte verpacken");
        assert_eq!(envelope.schema_version, EDL_SCHEMA_VERSION);
        let parsed = from_envelope(&envelope).expect("sollte entpacken");
        assert_eq!(edl, parsed);
    }

    #[test]
    fn old_v1_envelope_is_upgraded_to_v2() {
        let old = EdlV1 {
            basic: V1BasicAdjustments {
                exposure_ev: 0.7,
                contrast: -10.0,
                ..V1BasicAdjustments::NEUTRAL
            },
        };
        let payload = serde_json::to_value(old).expect("sollte serialisieren");
        let envelope = EdlEnvelope::new(1, payload);

        let upgraded = from_envelope(&envelope).expect("v1 sollte lesbar bleiben");
        assert_eq!(upgraded.basic.exposure_ev, 0.7);
        assert_eq!(upgraded.basic.contrast, -10.0);
        assert_eq!(upgraded.basic.texture, 0.0, "neue Felder bleiben neutral");
        assert_eq!(upgraded.repair, Vec::new());
    }

    #[test]
    fn unknown_schema_version_is_rejected() {
        let envelope = EdlEnvelope::new(9999, serde_json::json!({}));
        let result = from_envelope(&envelope);
        assert!(matches!(result, Err(PipelineError::InvalidEdl { .. })));
    }

    #[test]
    fn malformed_v2_payload_is_rejected() {
        // Richtige Schema-Version, aber Nutzlast passt nicht zu EdlV2
        // (fehlende Pflichtfelder) — muss als Fehler erkannt werden, nicht
        // stillschweigend mit Default-Werten aufgefüllt werden.
        let envelope = EdlEnvelope::new(
            EDL_SCHEMA_VERSION,
            serde_json::json!({ "unbekanntes_feld": 1 }),
        );
        let result = from_envelope(&envelope);
        assert!(result.is_err());
    }

    #[test]
    fn malformed_v1_payload_is_rejected() {
        let envelope = EdlEnvelope::new(1, serde_json::json!({ "unbekanntes_feld": 1 }));
        let result = from_envelope(&envelope);
        assert!(result.is_err());
    }
}
