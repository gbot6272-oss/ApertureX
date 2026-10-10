//! Fotos nach Aufnahmedatum in Ordner einsortieren (Phase 34 F8, siehe
//! `DECISIONS.md` ADR-0070).
//!
//! Wer über Jahre importiert hat, hat irgendwann einen Ordner mit ein paar
//! tausend Dateien darin — der Import legt sie dort ab, wo sie herkamen,
//! und das war auf der Speicherkarte nun mal ein einziges Verzeichnis. Das
//! hier baut daraus einen Datumsbaum (`2024/2024-05-17/…`).
//!
//! Dieses Modul rechnet nur: es liest keine Datei, es verschiebt keine, es
//! fasst den Katalog nicht an. Es bekommt die Kandidaten samt Datum und
//! sagt, wohin jedes Foto gehörte und was dabei im Weg steht. Genau wie
//! beim Ordner-Abgleich (Phase 33 F2) ist das der Punkt: der Plan ist
//! vorher sichtbar, das Verschieben erst der zweite Schritt.
//!
//! **Die Kollisionsprüfung ist der eigentliche Inhalt.** Zwei Fotos, die
//! am selben Tag aufgenommen wurden und denselben Dateinamen tragen
//! (`IMG_0001.CR3` aus zwei verschiedenen Quellordnern — bei zwei Kameras
//! oder nach einem Zählerüberlauf keine Seltenheit), landen im selben
//! Zielverzeichnis. Ohne Prüfung überschriebe das zweite das erste, und
//! zwar unwiederbringlich. Deshalb bekommt der erste Anspruch auf einen
//! Zielpfad den Zuschlag, jeder weitere wird als [`SortOutcome::Collision`]
//! gemeldet und bleibt liegen.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use apx_core::PhotoId;
use time::OffsetDateTime;

/// Vorgabemuster: Jahr als Oberordner, darunter ein Ordner je Tag.
///
/// Der Tagesordner trägt das volle Datum, nicht nur den Tag — sonst
/// stünden unter `2024/` die Ordner `01` bis `31` und man müsste erst den
/// Monat dazudenken.
pub(crate) const DEFAULT_DATE_PATTERN: &str = "{year}/{year}-{month}-{day}";

/// Ein Foto, das einsortiert werden könnte.
pub(crate) struct SortCandidate {
    pub photo_id: PhotoId,
    pub filename: String,
    /// Verzeichnis, in dem die Datei heute liegt.
    pub current_dir: PathBuf,
    /// Aufnahmedatum aus EXIF, falls vorhanden.
    pub captured_at: Option<OffsetDateTime>,
    /// Änderungszeit der Datei — immer vorhanden, aber nur ein Notnagel:
    /// sie sagt, wann die Datei zuletzt geschrieben wurde, nicht wann das
    /// Foto entstand. Ein Kopiervorgang setzt sie auf heute.
    pub file_mtime: OffsetDateTime,
}

/// Was mit einem Kandidaten passieren würde.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SortOutcome {
    /// Wird verschoben.
    Move,
    /// Liegt bereits im richtigen Ordner.
    AlreadyInPlace,
    /// Kein Aufnahmedatum, und der Rückfall auf die Dateizeit ist aus.
    NoDate,
    /// Der Zielpfad ist innerhalb dieses Plans schon vergeben.
    Collision,
}

pub(crate) struct SortEntry {
    pub photo_id: PhotoId,
    pub filename: String,
    pub current_dir: PathBuf,
    /// Zielverzeichnis. Bei [`SortOutcome::NoDate`] leer.
    pub target_dir: PathBuf,
    pub outcome: SortOutcome,
    /// Ob das Datum aus der Dateizeit statt aus EXIF kam — der Nutzer
    /// soll sehen, welche Einsortierung auf dem schwächeren Datum beruht.
    pub used_mtime: bool,
}

pub(crate) struct DateSortPlan {
    pub entries: Vec<SortEntry>,
}

impl DateSortPlan {
    pub fn count(&self, outcome: SortOutcome) -> usize {
        self.entries
            .iter()
            .filter(|entry| entry.outcome == outcome)
            .count()
    }

    /// Ob es überhaupt etwas zu tun gibt.
    pub fn is_clean(&self) -> bool {
        self.count(SortOutcome::Move) == 0
    }
}

/// Baut aus `pattern` die Ordnerkette für ein Datum.
///
/// Erkannte Tokens: `{year}` (vierstellig), `{month}` und `{day}`
/// (jeweils zweistellig nullgepolstert). Getrennt wird an `/` — jeder
/// Abschnitt wird eine Ordnerebene. Leere Abschnitte (führender,
/// doppelter oder abschließender Schrägstrich) fallen weg, unbekannte
/// `{…}`-Platzhalter bleiben wie in [`crate::import::rename`] stehen:
/// ein Tippfehler im Muster führt zu einem auffälligen Ordnernamen, nicht
/// zu einem Abbruch.
pub(crate) fn render_date_pattern(pattern: &str, date: OffsetDateTime) -> Vec<String> {
    let year = format!("{:04}", date.year());
    let month = format!("{:02}", u8::from(date.month()));
    let day = format!("{:02}", date.day());

    pattern
        .split('/')
        .map(|segment| {
            segment
                .replace("{year}", &year)
                .replace("{month}", &month)
                .replace("{day}", &day)
        })
        .map(|segment| sanitize_segment(&segment))
        .filter(|segment| !segment.is_empty())
        .collect()
}

/// Entfernt aus einem Ordnernamen, was auf mindestens einer Zielplattform
/// verboten ist — gleiche Liste wie [`crate::import::rename`], plus die
/// beiden Sonderfälle `.` und `..`, die sonst aus dem Zielbaum
/// herausführen würden.
fn sanitize_segment(raw: &str) -> String {
    let cleaned: String = raw
        .trim()
        .chars()
        .map(|c| if "\\/:*?\"<>|".contains(c) { '_' } else { c })
        .collect();
    if cleaned == "." || cleaned == ".." {
        return String::new();
    }
    cleaned
}

/// Rechnet den Plan.
///
/// `use_mtime_fallback` entscheidet über Fotos ohne EXIF-Aufnahmedatum:
/// aus ist die vorsichtige Vorgabe (lieber liegen lassen als nach einem
/// Datum einsortieren, das vom letzten Kopiervorgang stammt).
pub(crate) fn plan_date_sort(
    root: &Path,
    pattern: &str,
    candidates: &[SortCandidate],
    use_mtime_fallback: bool,
) -> DateSortPlan {
    // Zielpfad -> wer ihn zuerst beansprucht hat. Auch bereits richtig
    // liegende Fotos tragen sich ein: sonst schöbe der Plan ein zweites
    // Foto auf eine Datei, die schon dort liegt.
    let mut claimed: HashMap<PathBuf, PhotoId> = HashMap::new();
    let mut entries = Vec::with_capacity(candidates.len());

    for candidate in candidates {
        let (date, used_mtime) = match candidate.captured_at {
            Some(date) => (Some(date), false),
            None if use_mtime_fallback => (Some(candidate.file_mtime), true),
            None => (None, false),
        };

        let Some(date) = date else {
            entries.push(SortEntry {
                photo_id: candidate.photo_id,
                filename: candidate.filename.clone(),
                current_dir: candidate.current_dir.clone(),
                target_dir: PathBuf::new(),
                outcome: SortOutcome::NoDate,
                used_mtime: false,
            });
            continue;
        };

        let mut target_dir = root.to_path_buf();
        for segment in render_date_pattern(pattern, date) {
            target_dir.push(segment);
        }
        let target_path = target_dir.join(&candidate.filename);

        let outcome = match claimed.entry(target_path) {
            std::collections::hash_map::Entry::Occupied(_) => SortOutcome::Collision,
            std::collections::hash_map::Entry::Vacant(slot) => {
                slot.insert(candidate.photo_id);
                if target_dir == candidate.current_dir {
                    SortOutcome::AlreadyInPlace
                } else {
                    SortOutcome::Move
                }
            }
        };

        entries.push(SortEntry {
            photo_id: candidate.photo_id,
            filename: candidate.filename.clone(),
            current_dir: candidate.current_dir.clone(),
            target_dir,
            outcome,
            used_mtime: used_mtime && outcome != SortOutcome::NoDate,
        });
    }

    DateSortPlan { entries }
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::{Date, Month, Time, UtcOffset};

    fn dt(year: i32, month: Month, day: u8) -> OffsetDateTime {
        Date::from_calendar_date(year, month, day)
            .expect("gültiges Datum")
            .with_time(Time::from_hms(12, 0, 0).expect("gültige Zeit"))
            .assume_offset(UtcOffset::UTC)
    }

    fn candidate(filename: &str, dir: &str, captured: Option<OffsetDateTime>) -> SortCandidate {
        SortCandidate {
            photo_id: PhotoId::new(),
            filename: filename.to_string(),
            current_dir: PathBuf::from(dir),
            captured_at: captured,
            file_mtime: dt(2030, Month::January, 1),
        }
    }

    #[test]
    fn rendert_das_vorgabemuster_als_zwei_ebenen() {
        let segments = render_date_pattern(DEFAULT_DATE_PATTERN, dt(2024, Month::May, 7));
        assert_eq!(segments, vec!["2024".to_string(), "2024-05-07".to_string()]);
    }

    #[test]
    fn laesst_unbekannte_tokens_stehen_und_wirft_leere_abschnitte_weg() {
        let segments = render_date_pattern("/{year}//{unbekannt}/", dt(2024, Month::May, 7));
        assert_eq!(
            segments,
            vec!["2024".to_string(), "{unbekannt}".to_string()]
        );
    }

    #[test]
    fn faengt_punkt_abschnitte_ab_die_aus_dem_zielbaum_herausfuehren() {
        let segments = render_date_pattern("../{year}", dt(2024, Month::May, 7));
        assert_eq!(segments, vec!["2024".to_string()]);
    }

    #[test]
    fn plant_ein_foto_mit_aufnahmedatum_in_den_datumsordner() {
        let plan = plan_date_sort(
            Path::new("/fotos"),
            DEFAULT_DATE_PATTERN,
            &[candidate(
                "IMG_0001.CR3",
                "/fotos/Karte",
                Some(dt(2024, Month::May, 7)),
            )],
            false,
        );
        assert_eq!(plan.count(SortOutcome::Move), 1);
        assert_eq!(
            plan.entries[0].target_dir,
            PathBuf::from("/fotos/2024/2024-05-07")
        );
        assert!(!plan.entries[0].used_mtime);
    }

    #[test]
    fn meldet_bereits_richtig_liegende_fotos_statt_sie_zu_verschieben() {
        let plan = plan_date_sort(
            Path::new("/fotos"),
            DEFAULT_DATE_PATTERN,
            &[candidate(
                "IMG_0001.CR3",
                "/fotos/2024/2024-05-07",
                Some(dt(2024, Month::May, 7)),
            )],
            false,
        );
        assert_eq!(plan.count(SortOutcome::AlreadyInPlace), 1);
        assert!(plan.is_clean());
    }

    #[test]
    fn laesst_fotos_ohne_datum_liegen_solange_der_rueckfall_aus_ist() {
        let plan = plan_date_sort(
            Path::new("/fotos"),
            DEFAULT_DATE_PATTERN,
            &[candidate("SCAN.TIF", "/fotos/Karte", None)],
            false,
        );
        assert_eq!(plan.count(SortOutcome::NoDate), 1);
        assert_eq!(plan.entries[0].target_dir, PathBuf::new());
    }

    #[test]
    fn nutzt_die_dateizeit_nur_wenn_der_rueckfall_an_ist_und_sagt_es_dazu() {
        let plan = plan_date_sort(
            Path::new("/fotos"),
            DEFAULT_DATE_PATTERN,
            &[candidate("SCAN.TIF", "/fotos/Karte", None)],
            true,
        );
        assert_eq!(plan.count(SortOutcome::Move), 1);
        assert_eq!(
            plan.entries[0].target_dir,
            PathBuf::from("/fotos/2030/2030-01-01")
        );
        assert!(plan.entries[0].used_mtime);
    }

    /// Der Kern des Moduls: gleicher Tag, gleicher Dateiname, zwei
    /// verschiedene Quellordner. Ohne die Prüfung überschriebe das zweite
    /// Foto das erste.
    #[test]
    fn verschiebt_niemals_zwei_fotos_auf_denselben_zielpfad() {
        let plan = plan_date_sort(
            Path::new("/fotos"),
            DEFAULT_DATE_PATTERN,
            &[
                candidate(
                    "IMG_0001.CR3",
                    "/fotos/KarteA",
                    Some(dt(2024, Month::May, 7)),
                ),
                candidate(
                    "IMG_0001.CR3",
                    "/fotos/KarteB",
                    Some(dt(2024, Month::May, 7)),
                ),
            ],
            false,
        );
        assert_eq!(plan.count(SortOutcome::Move), 1);
        assert_eq!(plan.count(SortOutcome::Collision), 1);
        assert_eq!(plan.entries[1].outcome, SortOutcome::Collision);
    }

    /// Ein bereits richtig liegendes Foto belegt seinen Platz ebenfalls —
    /// sonst schöbe der Plan ein gleichnamiges Foto darauf.
    #[test]
    fn ein_bereits_richtig_liegendes_foto_belegt_seinen_zielpfad() {
        let plan = plan_date_sort(
            Path::new("/fotos"),
            DEFAULT_DATE_PATTERN,
            &[
                candidate(
                    "IMG_0001.CR3",
                    "/fotos/2024/2024-05-07",
                    Some(dt(2024, Month::May, 7)),
                ),
                candidate(
                    "IMG_0001.CR3",
                    "/fotos/Karte",
                    Some(dt(2024, Month::May, 7)),
                ),
            ],
            false,
        );
        assert_eq!(plan.count(SortOutcome::AlreadyInPlace), 1);
        assert_eq!(plan.count(SortOutcome::Collision), 1);
        assert_eq!(plan.count(SortOutcome::Move), 0);
    }

    /// Verschiedene Dateinamen am selben Tag sind kein Konflikt.
    #[test]
    fn zwei_fotos_desselben_tages_mit_verschiedenen_namen_wandern_beide() {
        let plan = plan_date_sort(
            Path::new("/fotos"),
            DEFAULT_DATE_PATTERN,
            &[
                candidate(
                    "IMG_0001.CR3",
                    "/fotos/Karte",
                    Some(dt(2024, Month::May, 7)),
                ),
                candidate(
                    "IMG_0002.CR3",
                    "/fotos/Karte",
                    Some(dt(2024, Month::May, 7)),
                ),
            ],
            false,
        );
        assert_eq!(plan.count(SortOutcome::Move), 2);
        assert_eq!(plan.count(SortOutcome::Collision), 0);
    }
}
