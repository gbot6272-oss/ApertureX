//! Konfliktabgleich für den Kollaborationsmodus (Phase 9 Schritt 10, siehe
//! `PLAN.md`, `DECISIONS.md` ADR-0035 Punkt 4): reine Vergleichslogik ohne
//! eigene SQL-Abfrage — der aufrufende Code (`apx-app`) hat den lokalen
//! aktuellen Bearbeitungsstand bereits über [`crate::Catalog::current_edit`]
//! und den importierten Stand aus der `.apxs`-Datei zur Hand, diese
//! Funktion entscheidet nur, was mit dem Paar zu tun ist.

use apx_core::EdlEnvelope;
use time::OffsetDateTime;

/// Ergebnis eines Abgleichs zwischen dem lokalen aktuellen Bearbeitungsstand
/// eines Fotos und einem importierten Stand für dasselbe Foto (per
/// `content_hash` gematcht).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShareDiff {
    /// Identischer EDL-Inhalt — keine Aktion nötig.
    Identical,
    /// Unterschiedlicher Inhalt — Konflikt. `prefer_incoming` ist der
    /// Vorschlag nach der Standardregel „zuletzt geändert gewinnt"
    /// (`edits.created_at`), keine automatische Übernahme — die Oberfläche
    /// zeigt den Konflikt immer explizit an und lässt eine manuelle
    /// Entscheidung zu (meins behalten/übernehmen/als virtuelle Kopie
    /// behalten).
    Conflict { prefer_incoming: bool },
}

/// Vergleicht zwei EDL-Umschläge (Gleichheit ist rein strukturell — gleiche
/// `schema_version` und gleiche `payload`-Werte, siehe `EdlEnvelope`s
/// `PartialEq`-Ableitung) und schlägt bei Abweichung anhand der
/// Zeitstempel eine Richtung vor.
pub fn diff_edit(
    local_edl: &EdlEnvelope,
    local_created_at: OffsetDateTime,
    incoming_edl: &EdlEnvelope,
    incoming_created_at: OffsetDateTime,
) -> ShareDiff {
    if local_edl == incoming_edl {
        ShareDiff::Identical
    } else {
        ShareDiff::Conflict {
            prefer_incoming: incoming_created_at > local_created_at,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use time::Duration;

    fn envelope(exposure: f64) -> EdlEnvelope {
        EdlEnvelope::new(4, json!({ "basic": { "exposure_ev": exposure } }))
    }

    #[test]
    fn identical_edl_content_is_not_a_conflict() {
        let now = OffsetDateTime::now_utc();
        let outcome = diff_edit(
            &envelope(0.5),
            now,
            &envelope(0.5),
            now + Duration::hours(1),
        );
        assert_eq!(outcome, ShareDiff::Identical);
    }

    #[test]
    fn differing_edl_content_is_a_conflict_and_prefers_the_newer_side() {
        let now = OffsetDateTime::now_utc();
        let older_local = diff_edit(
            &envelope(0.0),
            now,
            &envelope(0.5),
            now + Duration::hours(1),
        );
        assert_eq!(
            older_local,
            ShareDiff::Conflict {
                prefer_incoming: true
            }
        );

        let newer_local = diff_edit(
            &envelope(0.0),
            now + Duration::hours(1),
            &envelope(0.5),
            now,
        );
        assert_eq!(
            newer_local,
            ShareDiff::Conflict {
                prefer_incoming: false
            }
        );
    }

    #[test]
    fn identical_timestamps_prefer_the_local_side_on_conflict() {
        let now = OffsetDateTime::now_utc();
        let outcome = diff_edit(&envelope(0.0), now, &envelope(0.5), now);
        assert_eq!(
            outcome,
            ShareDiff::Conflict {
                prefer_incoming: false
            }
        );
    }
}
