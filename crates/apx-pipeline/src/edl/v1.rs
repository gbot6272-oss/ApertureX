//! Version 1 des EDL-Schemas: die sieben Grundeinstellungs-Regler aus
//! Phase 2 (siehe `SPEC.md` §5, `DECISIONS.md` ADR-0011).
//!
//! Typisierte Felder statt einer generischen Key-Value-Map — `SPEC.md`
//! §6 verlangt „jede Operation eigene Serialisierung", und typisierte
//! Felder lassen `serde` fehlende/falsche Werte strukturell ablehnen statt
//! sie stillschweigend zu ignorieren.

use serde::{Deserialize, Serialize};

/// Verschiebung des Weißabgleichs **relativ zum „as shot"-Wert** aus den
/// RAW-Metadaten (siehe `apx_raw::RawMetadata`), nicht als absoluter
/// Kelvin-/Tint-Wert. `0.0`/`0.0` bedeutet „keine Veränderung gegenüber
/// der Kamera-Einstellung" — das macht den neutralen Wert unabhängig von
/// der jeweiligen Kamera eindeutig, ohne dass das Datenmodell selbst
/// wissen muss, was der As-shot-Weißabgleich einer bestimmten Aufnahme
/// war (das rechnet Phase-2-Schritt 4 anhand von `apx_raw`s
/// Kamera-Metadaten aus).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct WhiteBalanceAdjustment {
    /// Verschiebung der Farbtemperatur in Kelvin.
    pub temp_shift_kelvin: f32,
    /// Verschiebung des Grün/Magenta-Tons.
    pub tint_shift: f32,
}

impl WhiteBalanceAdjustment {
    pub const NEUTRAL: Self = Self {
        temp_shift_kelvin: 0.0,
        tint_shift: 0.0,
    };
}

impl Default for WhiteBalanceAdjustment {
    fn default() -> Self {
        Self::NEUTRAL
    }
}

/// Die sechs Ton-Regler. Wertebereich `-100.0..=100.0` nach
/// Lightroom-Konvention (`0.0` = keine Veränderung), `exposure_ev` in
/// Blendenstufen.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct BasicAdjustments {
    pub white_balance: WhiteBalanceAdjustment,
    /// Belichtungskorrektur in Blendenstufen (EV).
    pub exposure_ev: f32,
    pub contrast: f32,
    pub highlights: f32,
    pub shadows: f32,
    pub whites: f32,
    pub blacks: f32,
}

impl BasicAdjustments {
    pub const NEUTRAL: Self = Self {
        white_balance: WhiteBalanceAdjustment::NEUTRAL,
        exposure_ev: 0.0,
        contrast: 0.0,
        highlights: 0.0,
        shadows: 0.0,
        whites: 0.0,
        blacks: 0.0,
    };
}

impl Default for BasicAdjustments {
    fn default() -> Self {
        Self::NEUTRAL
    }
}

/// Die konkrete EDL-Struktur für Schema-Version 1 — siehe
/// [`crate::edl::EDL_SCHEMA_VERSION`] und [`crate::edl::migrate`] für die
/// Umwandlung von/zu `apx_core::EdlEnvelope`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct EdlV1 {
    pub basic: BasicAdjustments,
}

impl EdlV1 {
    /// Die neutrale Bearbeitung: alle Regler unverändert (Ausgabe = Eingabe).
    pub const NEUTRAL: Self = Self {
        basic: BasicAdjustments::NEUTRAL,
    };
}

impl Default for EdlV1 {
    fn default() -> Self {
        Self::NEUTRAL
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn neutral_roundtrips_through_json() {
        let edl = EdlV1::NEUTRAL;
        let json = serde_json::to_string(&edl).expect("sollte serialisieren");
        let parsed: EdlV1 = serde_json::from_str(&json).expect("sollte parsen");
        assert_eq!(edl, parsed);
    }

    #[test]
    fn default_equals_neutral() {
        assert_eq!(EdlV1::default(), EdlV1::NEUTRAL);
    }
}
