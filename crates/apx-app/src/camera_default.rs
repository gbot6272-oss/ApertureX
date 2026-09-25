//! Standardentwicklung je Kamera (Phase 34 F7, siehe `DECISIONS.md`
//! ADR-0070).
//!
//! **Wozu.** Jede Kamera hat ihren eigenen Charakter: die eine
//! unterbelichtet systematisch, die andere braucht immer etwas
//! Entrauschung, die dritte einen anderen Weißabgleich. Wer mit zwei
//! Gehäusen arbeitet, stellt nach jedem Import dieselben Regler wieder
//! neu ein. Eine Vorgabe je Kameramodell nimmt das ab: neu importierte
//! Fotos dieser Kamera starten nicht neutral, sondern bei dem Stand,
//! den der Nutzer für diese Kamera festgelegt hat.
//!
//! **Warum nur beim Import und nur für neue Fotos.** Die Vorgabe ist
//! ein Startpunkt, keine laufende Regel. Sie nachträglich auf schon
//! vorhandene Fotos anzuwenden würde deren Bearbeitung überschreiben —
//! und eine Vorgabe, die später geändert wird, würde rückwirkend
//! fremde Arbeit verändern. Der Vorgang bleibt deshalb der erste
//! Verlaufseintrag eines frisch importierten Fotos und ist wie jeder
//! andere Schritt rückgängig zu machen.
//!
//! **Warum die Vorlagen-Tabelle statt einer neuen.** Dieselbe
//! Entscheidung wie bei den Metadaten-Vorgaben (Phase 33 F4) und den
//! Wasserzeichen-Vorlagen (F5): `templates` trägt bereits eine freie
//! `kind`-Zeichenkette und eine JSON-Nutzlast. Eine eigene Tabelle für
//! zwei Spalten wäre Schema-Aufwand ohne Gegenwert.

/// `kind` in der `templates`-Tabelle.
pub const CAMERA_DEFAULT_KIND: &str = "camera_default";

/// Schlüssel, unter dem ein Kameramodell verglichen wird.
///
/// Kameras schreiben ihr Modell nicht einheitlich: mal mit
/// Herstellerpräfix, mal mit anderer Groß-/Kleinschreibung, oft mit
/// angehängten Leerzeichen. Ohne Normalisierung bekäme dasselbe Gehäuse
/// zwei Vorgaben, von denen jeweils nur eine greift — und der Nutzer
/// sähe nicht, warum.
pub fn model_key(model: &str) -> String {
    model.trim().to_lowercase()
}

/// Sucht die Vorgabe zu einem Kameramodell.
///
/// `None` für Fotos ohne Modellangabe: ohne Kamera gibt es keine
/// kameraspezifische Vorgabe, und auf irgendeine zu raten wäre
/// schlimmer als keine.
pub fn find_for<'a, T>(defaults: &'a [(String, T)], camera_model: Option<&str>) -> Option<&'a T> {
    let key = model_key(camera_model?);
    if key.is_empty() {
        return None;
    }
    defaults
        .iter()
        .find(|(model, _)| model_key(model) == key)
        .map(|(_, value)| value)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn defaults() -> Vec<(String, &'static str)> {
        vec![
            ("Canon EOS R5".to_string(), "edl-r5"),
            ("NIKON Z 6".to_string(), "edl-z6"),
        ]
    }

    #[test]
    fn finds_the_default_for_an_exact_model() {
        assert_eq!(find_for(&defaults(), Some("Canon EOS R5")), Some(&"edl-r5"));
    }

    #[test]
    fn ignores_case_and_surrounding_spaces() {
        // Genau so schreiben verschiedene Kameras und Konverter
        // dasselbe Modell.
        assert_eq!(
            find_for(&defaults(), Some("  canon eos r5 ")),
            Some(&"edl-r5")
        );
        assert_eq!(find_for(&defaults(), Some("nikon z 6")), Some(&"edl-z6"));
    }

    #[test]
    fn an_unknown_model_gets_nothing() {
        assert_eq!(find_for(&defaults(), Some("Fujifilm X-T5")), None);
    }

    #[test]
    fn a_photo_without_a_model_gets_nothing() {
        assert_eq!(find_for(&defaults(), None), None);
        assert_eq!(find_for(&defaults(), Some("   ")), None);
    }

    #[test]
    fn the_key_collapses_the_usual_spelling_differences() {
        assert_eq!(model_key(" Canon EOS R5 "), model_key("canon eos r5"));
        assert_ne!(model_key("Canon EOS R5"), model_key("Canon EOS R6"));
    }

    #[test]
    fn an_empty_list_matches_nothing() {
        let empty: Vec<(String, &str)> = Vec::new();
        assert_eq!(find_for(&empty, Some("Canon EOS R5")), None);
    }
}
