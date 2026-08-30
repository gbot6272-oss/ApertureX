//! Umwandlung zwischen `apx_core::EdlEnvelope` (dem für `apx-catalog`
//! undurchsichtigen Umschlag) und der jeweils aktuellen konkreten
//! EDL-Struktur (`EdlV3`).
//!
//! Es gibt jetzt drei bekannte Schema-Versionen: die aktuelle
//! ([`EDL_SCHEMA_VERSION`], `EdlV3`) sowie die historischen Versionen 1
//! (`EdlV1`, aus Phase 2) und 2 (`EdlV2`, aus Phase 4) — beide noch in
//! alten `edit_history`-Zeilen gespeichert. [`from_envelope`] liest alle
//! drei, zieht ältere Versionen aber sofort auf `EdlV3` hoch (`EdlV2` über
//! `EdlV3::from_v2`, `EdlV1` über den bereits bestehenden `EdlV2::from_v1`
//! plus denselben v2→v3-Schritt) — der Aufrufer (`apx-app`) sieht davon
//! nichts, er ruft immer nur `from_envelope` auf und bekommt die jeweils
//! aktuelle Struktur zurück. Sobald eine Schema-Version 4 hinzukommt,
//! wird hier ein weiterer Umwandlungsschritt v3→v4 ergänzt, nach
//! demselben Muster.

use apx_core::EdlEnvelope;

use super::{v1::EdlV1, v2::EdlV2, v3::EdlV3, EDL_SCHEMA_VERSION};
use crate::error::{PipelineError, Result};

/// Die historische Schema-Version 1 (Phase 2, sieben Grundregler) — nur
/// noch zum *Lesen* alter Einträge relevant, [`to_envelope`] schreibt nie
/// mehr in dieser Version.
const V1_SCHEMA_VERSION: u32 = 1;

/// Die historische Schema-Version 2 (Phase 4, zehn Werkzeugkategorien
/// ohne Masken) — dieselbe Lese-only-Rolle wie [`V1_SCHEMA_VERSION`].
const V2_SCHEMA_VERSION: u32 = 2;

/// Liest die jeweils aktuelle EDL-Struktur (`EdlV3`) aus einem Umschlag.
/// Ältere Umschläge werden automatisch hochgezogen; eine unbekannte
/// Version oder eine kaputte Nutzlast für die jeweilige Version wird
/// abgelehnt statt stillschweigend repariert (siehe `SPEC.md` §2.1:
/// „Versionierte EDL: Schema-Migration muss alte Kataloge öffnen können"
/// — die Prüfung hier ist die Kehrseite davon: eine *neuere*, noch
/// unbekannte Version wird explizit abgelehnt statt stillschweigend
/// falsch interpretiert).
pub fn from_envelope(envelope: &EdlEnvelope) -> Result<EdlV3> {
    if envelope.schema_version == EDL_SCHEMA_VERSION {
        return serde_json::from_value(envelope.payload.clone()).map_err(|source| {
            PipelineError::InvalidEdl {
                message: format!(
                    "EDL-Nutzlast entspricht nicht Schema-Version {EDL_SCHEMA_VERSION}: {source}"
                ),
            }
        });
    }

    if envelope.schema_version == V2_SCHEMA_VERSION {
        let old: EdlV2 = serde_json::from_value(envelope.payload.clone()).map_err(|source| {
            PipelineError::InvalidEdl {
                message: format!(
                    "EDL-Nutzlast entspricht nicht Schema-Version {V2_SCHEMA_VERSION}: {source}"
                ),
            }
        })?;
        return Ok(EdlV3::from_v2(old));
    }

    if envelope.schema_version == V1_SCHEMA_VERSION {
        let old: EdlV1 = serde_json::from_value(envelope.payload.clone()).map_err(|source| {
            PipelineError::InvalidEdl {
                message: format!(
                    "EDL-Nutzlast entspricht nicht Schema-Version {V1_SCHEMA_VERSION}: {source}"
                ),
            }
        })?;
        return Ok(EdlV3::from_v2(EdlV2::from_v1(old)));
    }

    Err(PipelineError::InvalidEdl {
        message: format!(
            "unbekannte EDL-Schema-Version {} (diese Aperture-X-Version kennt Version \
             {V1_SCHEMA_VERSION}, {V2_SCHEMA_VERSION} und {EDL_SCHEMA_VERSION})",
            envelope.schema_version
        ),
    })
}

/// Verpackt ein `EdlV3` in einen Umschlag zum Speichern — immer als
/// aktuelle Schema-Version. Abwärtskompatibilität ist nur fürs *Lesen*
/// nötig (siehe [`from_envelope`]), nicht fürs Schreiben: jede neu
/// committete Bearbeitung landet als Version 3 in `edit_history`.
pub fn to_envelope(edl: &EdlV3) -> Result<EdlEnvelope> {
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
        let edl = EdlV3 {
            basic: V2BasicAdjustments {
                exposure_ev: 0.5,
                ..V2BasicAdjustments::NEUTRAL
            },
            ..EdlV3::neutral()
        };
        let envelope = to_envelope(&edl).expect("sollte verpacken");
        assert_eq!(envelope.schema_version, EDL_SCHEMA_VERSION);
        let parsed = from_envelope(&envelope).expect("sollte entpacken");
        assert_eq!(edl, parsed);
    }

    #[test]
    fn old_v1_envelope_is_upgraded_to_v3() {
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
        assert_eq!(upgraded.masks, Vec::new());
    }

    #[test]
    fn old_v2_envelope_is_upgraded_to_v3() {
        let old = EdlV2 {
            basic: V2BasicAdjustments {
                exposure_ev: -0.3,
                ..V2BasicAdjustments::NEUTRAL
            },
            ..EdlV2::neutral()
        };
        let payload = serde_json::to_value(old).expect("sollte serialisieren");
        let envelope = EdlEnvelope::new(2, payload);

        let upgraded = from_envelope(&envelope).expect("v2 sollte lesbar bleiben");
        assert_eq!(upgraded.basic.exposure_ev, -0.3);
        assert_eq!(upgraded.masks, Vec::new(), "v2 kannte noch keine Masken");
    }

    #[test]
    fn unknown_schema_version_is_rejected() {
        let envelope = EdlEnvelope::new(9999, serde_json::json!({}));
        let result = from_envelope(&envelope);
        assert!(matches!(result, Err(PipelineError::InvalidEdl { .. })));
    }

    #[test]
    fn malformed_v3_payload_is_rejected() {
        // Richtige Schema-Version, aber Nutzlast passt nicht zu EdlV3
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
    fn malformed_v2_payload_is_rejected() {
        let envelope = EdlEnvelope::new(2, serde_json::json!({ "unbekanntes_feld": 1 }));
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
