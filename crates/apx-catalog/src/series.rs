//! Serien- und Belichtungsreihen-Erkennung (Phase 32 F7).
//!
//! **Was das über `auto_stack_by_time` (Phase 9 Schritt 1) hinaus kann.**
//! Jenes fasst alles zusammen, was zeitlich nah beieinander liegt — und
//! sagt nicht, *was* da zusammenliegt. Genau das ist aber der Unterschied
//! zwischen zwanzig Aufnahmen einer Vogelserie (davon will man eine) und
//! drei Aufnahmen einer Belichtungsreihe (die will man alle drei, und zwar
//! zusammen für HDR). Dieses Modul unterscheidet die Fälle anhand der
//! Belichtungswerte.
//!
//! **Reine Analyse, kein SQL.** Die Eingabe ist eine Liste von
//! [`Shot`]s; die Datenbankseite liefert sie
//! (`repository::stacks`/`commands`). So lässt sich die Klassifikation
//! ohne Katalog prüfen — und genau dort liegen die Entscheidungen.
//!
//! **Was dieses Modul bewusst NICHT erkennt:** Fokusreihen und
//! Panoramen. Beide sehen in den EXIF-Daten aus wie eine Serie mit
//! gleicher Belichtung; unterscheiden ließen sie sich nur über die
//! Fokusdistanz (steht selten und uneinheitlich im EXIF) bzw. über den
//! Bildinhalt. Sie werden deshalb als [`SeriesKind::Burst`] gemeldet,
//! statt eine Erkennung vorzutäuschen, die keine ist.

use apx_core::PhotoId;

/// Eine Aufnahme, so wie die Erkennung sie braucht.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Shot {
    pub photo_id: PhotoId,
    /// Aufnahmezeit als Unix-Sekunden.
    pub captured_at: i64,
    pub aperture: Option<f32>,
    /// Belichtungszeit in Sekunden.
    pub shutter: Option<f32>,
    pub iso: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SeriesKind {
    /// Schnelle Folge mit praktisch gleicher Belichtung — Reihenaufnahme.
    /// Auch Fokusreihen und Panoramen landen hier, siehe Moduldoku.
    Burst,
    /// Schnelle Folge mit gestuften Belichtungswerten — Belichtungsreihe.
    ExposureBracket,
    /// Zeitlich zusammengehörig, aber weder gleich belichtet noch sauber
    /// gestuft (z. B. hektisches Nachregeln von Hand). Bewusst eine
    /// eigene Antwort statt einer geratenen.
    Mixed,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DetectedSeries {
    pub kind: SeriesKind,
    /// Chronologisch, älteste zuerst.
    pub photo_ids: Vec<PhotoId>,
    /// Sekunden zwischen erster und letzter Aufnahme.
    pub span_seconds: i64,
    /// Belichtungswerte (EV bei ISO 100) der Aufnahmen, soweit
    /// berechenbar — dieselbe Reihenfolge wie `photo_ids`.
    pub ev_values: Vec<Option<f32>>,
    /// Spannweite der EV-Werte, `None` wenn zu wenige bekannt sind.
    pub ev_spread: Option<f32>,
}

/// Größter Zeitabstand innerhalb einer Serie, in Sekunden.
///
/// Zwei Sekunden: eine Reihenaufnahme liegt weit darunter, eine
/// Belichtungsreihe per Selbstauslöser-Automatik knapp darunter. Größere
/// Fenster fassen zwei getrennte Motive derselben Minute zusammen, was
/// beim Sichten mehr stört als es hilft.
pub const DEFAULT_MAX_GAP_SECONDS: i64 = 2;

/// Ab dieser EV-Spannweite gilt eine Gruppe als belichtungsgestuft.
///
/// Eine halbe Blendenstufe: darunter liegt das normale Zittern einer
/// Automatik von Bild zu Bild, darüber beginnt Absicht. Die kleinste
/// Stufe, die Kameras für Belichtungsreihen anbieten, ist 1/3 EV — drei
/// Aufnahmen mit ±1/3 EV ergeben 0,67 EV Spannweite und werden damit
/// sicher erkannt.
const BRACKET_MIN_SPREAD_EV: f32 = 0.5;

/// Darunter gilt eine Gruppe als gleich belichtet.
const BURST_MAX_SPREAD_EV: f32 = 0.2;

/// Belichtungswert bei ISO 100: `EV100 = log2(N² / t) − log2(ISO/100)`.
///
/// Auf ISO 100 normiert, weil eine Belichtungsreihe auch über die
/// Empfindlichkeit gestuft sein kann (ISO-Bracketing) — ohne die
/// Normierung sähen drei Aufnahmen mit gleicher Blende/Zeit und
/// ISO 100/200/400 wie eine Reihenaufnahme aus.
pub fn ev100(aperture: f32, shutter: f32, iso: u32) -> Option<f32> {
    if aperture <= 0.0 || shutter <= 0.0 || iso == 0 {
        return None;
    }
    let ev = ((aperture * aperture) / shutter).log2() - ((iso as f32) / 100.0).log2();
    ev.is_finite().then_some(ev)
}

fn shot_ev(shot: &Shot) -> Option<f32> {
    ev100(shot.aperture?, shot.shutter?, shot.iso?)
}

/// Findet Serien in einer Liste von Aufnahmen.
///
/// Gruppiert zuerst chronologisch nach `max_gap_seconds` und
/// klassifiziert dann jede Gruppe ab zwei Aufnahmen. Aufnahmen ohne
/// Zeitstempel kann der Aufrufer gar nicht erst übergeben — ohne Zeit
/// gibt es keine Serie.
pub fn detect_series(shots: &[Shot], max_gap_seconds: i64) -> Vec<DetectedSeries> {
    let mut sorted: Vec<Shot> = shots.to_vec();
    sorted.sort_by_key(|shot| shot.captured_at);

    let mut result = Vec::new();
    let mut group: Vec<Shot> = Vec::new();

    for shot in sorted {
        match group.last() {
            Some(previous) if shot.captured_at - previous.captured_at <= max_gap_seconds => {
                group.push(shot)
            }
            Some(_) => {
                if let Some(series) = classify(&group) {
                    result.push(series);
                }
                group = vec![shot];
            }
            None => group.push(shot),
        }
    }
    if let Some(series) = classify(&group) {
        result.push(series);
    }
    result
}

/// Klassifiziert eine bereits zeitlich zusammenhängende Gruppe.
///
/// `None` für weniger als zwei Aufnahmen — ein Einzelbild ist keine
/// Serie.
pub fn classify(group: &[Shot]) -> Option<DetectedSeries> {
    if group.len() < 2 {
        return None;
    }
    let ev_values: Vec<Option<f32>> = group.iter().map(shot_ev).collect();
    let known: Vec<f32> = ev_values.iter().filter_map(|ev| *ev).collect();

    // Die Spannweite braucht mindestens zwei bekannte Werte; mit nur
    // einem ließe sich nichts vergleichen.
    let ev_spread = if known.len() >= 2 {
        let min = known.iter().copied().fold(f32::INFINITY, f32::min);
        let max = known.iter().copied().fold(f32::NEG_INFINITY, f32::max);
        Some(max - min)
    } else {
        None
    };

    let kind = match ev_spread {
        // Ohne Belichtungswerte bleibt nur die Zeit — das ist eine
        // Serie, aber welche, lässt sich nicht sagen.
        None => SeriesKind::Mixed,
        Some(spread) if spread >= BRACKET_MIN_SPREAD_EV => {
            // Eine Belichtungsreihe hat so viele verschiedene Stufen wie
            // Aufnahmen. Wiederholt sich ein Wert, war es eher eine
            // Serie mit zwischendurch geänderter Einstellung.
            if distinct_levels(&known) >= 3 && distinct_levels(&known) == known.len() {
                SeriesKind::ExposureBracket
            } else {
                SeriesKind::Mixed
            }
        }
        Some(spread) if spread <= BURST_MAX_SPREAD_EV => SeriesKind::Burst,
        Some(_) => SeriesKind::Mixed,
    };

    let first = group.first()?.captured_at;
    let last = group.last()?.captured_at;
    Some(DetectedSeries {
        kind,
        photo_ids: group.iter().map(|shot| shot.photo_id).collect(),
        span_seconds: last - first,
        ev_values,
        ev_spread,
    })
}

/// Zählt verschiedene EV-Stufen — Werte innerhalb von 1/6 EV gelten als
/// dieselbe Stufe (Rundungsrauschen aus den EXIF-Brüchen, z. B. 1/250 s
/// gegen 0,004 s).
fn distinct_levels(values: &[f32]) -> usize {
    let mut sorted: Vec<f32> = values.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let mut count = 0;
    let mut last: Option<f32> = None;
    for value in sorted {
        if last.is_none_or(|previous| (value - previous).abs() > 1.0 / 6.0) {
            count += 1;
            last = Some(value);
        }
    }
    count
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shot(seconds: i64, aperture: f32, shutter: f32, iso: u32) -> Shot {
        Shot {
            photo_id: PhotoId::new(),
            captured_at: seconds,
            aperture: Some(aperture),
            shutter: Some(shutter),
            iso: Some(iso),
        }
    }

    #[test]
    fn ev100_matches_the_textbook_values() {
        // f/1.0, 1 s, ISO 100 ist per Definition EV 0.
        assert!((ev100(1.0, 1.0, 100).expect("EV") - 0.0).abs() < 1e-5);
        // Sonnenschein-Regel: f/16 bei 1/ISO Sekunde, hier also
        // 1/125 s bei ISO 100 — das ist EV 15 (log2(256·125) = 14,97).
        let ev = ev100(16.0, 1.0 / 125.0, 100).expect("EV");
        assert!((ev - 15.0).abs() < 0.05, "EV war {ev}");
        // ISO verdoppeln senkt den auf ISO 100 bezogenen Wert um 1 EV.
        let base = ev100(4.0, 1.0 / 60.0, 100).expect("EV");
        let doubled = ev100(4.0, 1.0 / 60.0, 200).expect("EV");
        assert!((base - doubled - 1.0).abs() < 1e-5);
    }

    #[test]
    fn ev100_refuses_impossible_values_instead_of_returning_infinity() {
        assert!(ev100(0.0, 1.0 / 60.0, 100).is_none());
        assert!(ev100(4.0, 0.0, 100).is_none());
        assert!(ev100(4.0, 1.0 / 60.0, 0).is_none());
    }

    #[test]
    fn a_fast_sequence_with_the_same_exposure_is_a_burst() {
        let series = detect_series(
            &[
                shot(1000, 4.0, 1.0 / 500.0, 400),
                shot(1000, 4.0, 1.0 / 500.0, 400),
                shot(1001, 4.0, 1.0 / 500.0, 400),
            ],
            DEFAULT_MAX_GAP_SECONDS,
        );
        assert_eq!(series.len(), 1);
        assert_eq!(series[0].kind, SeriesKind::Burst);
        assert_eq!(series[0].photo_ids.len(), 3);
    }

    #[test]
    fn three_shots_two_ev_apart_are_an_exposure_bracket() {
        // −2 EV / 0 / +2 EV über die Belichtungszeit gestuft.
        let series = detect_series(
            &[
                shot(2000, 8.0, 1.0 / 250.0, 100),
                shot(2000, 8.0, 1.0 / 1000.0, 100),
                shot(2001, 8.0, 1.0 / 60.0, 100),
            ],
            DEFAULT_MAX_GAP_SECONDS,
        );
        assert_eq!(series.len(), 1);
        assert_eq!(series[0].kind, SeriesKind::ExposureBracket);
        assert!(series[0].ev_spread.expect("Spannweite") > 3.9);
    }

    #[test]
    fn an_iso_bracket_is_recognised_too() {
        // Gleiche Blende und Zeit, gestufte Empfindlichkeit — ohne die
        // ISO-Normierung in `ev100` sähe das wie eine Reihenaufnahme aus.
        let series = detect_series(
            &[
                shot(3000, 4.0, 1.0 / 125.0, 100),
                shot(3000, 4.0, 1.0 / 125.0, 400),
                shot(3001, 4.0, 1.0 / 125.0, 1600),
            ],
            DEFAULT_MAX_GAP_SECONDS,
        );
        assert_eq!(series[0].kind, SeriesKind::ExposureBracket);
    }

    #[test]
    fn a_repeated_exposure_level_is_not_a_bracket() {
        // Zwei gleiche und eine abweichende Aufnahme: jemand hat
        // zwischendurch nachgeregelt, keine Reihe.
        let series = detect_series(
            &[
                shot(4000, 4.0, 1.0 / 125.0, 100),
                shot(4000, 4.0, 1.0 / 125.0, 100),
                shot(4001, 4.0, 1.0 / 15.0, 100),
            ],
            DEFAULT_MAX_GAP_SECONDS,
        );
        assert_eq!(series[0].kind, SeriesKind::Mixed);
    }

    #[test]
    fn a_gap_larger_than_the_window_starts_a_new_series() {
        let series = detect_series(
            &[
                shot(5000, 4.0, 1.0 / 500.0, 400),
                shot(5000, 4.0, 1.0 / 500.0, 400),
                // 30 s später: anderes Motiv.
                shot(5030, 4.0, 1.0 / 500.0, 400),
                shot(5030, 4.0, 1.0 / 500.0, 400),
            ],
            DEFAULT_MAX_GAP_SECONDS,
        );
        assert_eq!(series.len(), 2);
        assert_eq!(series[0].photo_ids.len(), 2);
        assert_eq!(series[1].photo_ids.len(), 2);
    }

    #[test]
    fn a_single_photo_is_no_series() {
        let series = detect_series(
            &[shot(6000, 4.0, 1.0 / 500.0, 400)],
            DEFAULT_MAX_GAP_SECONDS,
        );
        assert!(series.is_empty());
    }

    #[test]
    fn shots_without_exposure_data_are_reported_as_mixed_not_guessed() {
        let bare = |seconds: i64| Shot {
            photo_id: PhotoId::new(),
            captured_at: seconds,
            aperture: None,
            shutter: None,
            iso: None,
        };
        let series = detect_series(&[bare(7000), bare(7001)], DEFAULT_MAX_GAP_SECONDS);
        assert_eq!(series[0].kind, SeriesKind::Mixed);
        assert!(series[0].ev_spread.is_none());
        assert_eq!(series[0].ev_values, vec![None, None]);
    }

    #[test]
    fn the_result_is_chronological_even_if_the_input_is_not() {
        let later = shot(8005, 4.0, 1.0 / 500.0, 400);
        let earlier = shot(8004, 4.0, 1.0 / 500.0, 400);
        let series = detect_series(&[later, earlier], DEFAULT_MAX_GAP_SECONDS);
        assert_eq!(series[0].photo_ids, vec![earlier.photo_id, later.photo_id]);
        assert_eq!(series[0].span_seconds, 1);
    }
}
