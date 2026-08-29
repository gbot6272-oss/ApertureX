//! Ein minimaler, versionsmarkierter Umschlag für eine Edit Decision List
//! (EDL) — wie eine Bearbeitung non-destruktiv beschrieben wird (siehe
//! `SPEC.md` §2.1). `apx-core` kennt nur diesen Umschlag, nicht die
//! konkrete Struktur einer Bearbeitung — die lebt in
//! `apx_pipeline::edl::EdlV1`. So kann `apx-catalog` ein EDL
//! speichern/lesen, ohne von `apx-pipeline` abhängen zu müssen (siehe
//! `ARCHITECTURE.md` Abschnitt 4/5, `DECISIONS.md` ADR-0013).

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::AppError;

/// Undurchsichtiger Umschlag: `apx-catalog` liest/schreibt nur
/// `schema_version` und reicht `payload` unverändert durch. Nur
/// `apx-pipeline` weiß, wie `payload` für eine gegebene `schema_version`
/// zu interpretieren ist.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EdlEnvelope {
    pub schema_version: u32,
    pub payload: Value,
}

impl EdlEnvelope {
    pub fn new(schema_version: u32, payload: Value) -> Self {
        Self {
            schema_version,
            payload,
        }
    }

    /// Parst einen Umschlag aus einem JSON-String, wie er aus der
    /// Katalog-Spalte `edit_history.edl_json` gelesen wird.
    pub fn from_json_str(json: &str) -> Result<Self, AppError> {
        serde_json::from_str(json)
            .map_err(|source| AppError::pipeline(format!("EDL-JSON ist ungültig: {source}")))
    }

    /// Serialisiert den Umschlag zu JSON, zum Schreiben in die Katalog-Spalte.
    pub fn to_json_string(&self) -> Result<String, AppError> {
        serde_json::to_string(self).map_err(|source| {
            AppError::pipeline(format!("EDL konnte nicht serialisiert werden: {source}"))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrips_through_json() {
        let envelope = EdlEnvelope::new(1, serde_json::json!({ "exposure_ev": 0.5 }));
        let json = envelope.to_json_string().expect("sollte serialisieren");
        let parsed = EdlEnvelope::from_json_str(&json).expect("sollte parsen");
        assert_eq!(envelope, parsed);
    }

    #[test]
    fn invalid_json_is_rejected() {
        let result = EdlEnvelope::from_json_str("das ist kein JSON");
        assert!(result.is_err());
    }
}
