//! Stapel-Umbenennung bereits importierter Fotos (Phase 32 F4).
//!
//! Der Import konnte Dateien seit Phase 9 nach einem Tokenmuster
//! benennen (`import::rename`), danach war der Name für immer fest. Wer
//! „IMG_0001.CR3" erst nach dem Sichten in „20240504_0001_Ostsee.CR3"
//! umbenennen wollte, musste den Katalog verlassen und im Dateimanager
//! arbeiten — womit der Katalog auf Dateien zeigte, die es nicht mehr
//! gab.
//!
//! Dieses Modul ist die **reine Planung**: aus Fotos + Muster wird eine
//! Liste geplanter Namen mit einem Status je Eintrag. Kein
//! Dateisystemzugriff, keine Katalogschreibung — beides macht
//! `commands::apply_batch_rename` auf Basis dieses Plans. Der Grund für
//! die Trennung ist die Vorschau: die Oberfläche zeigt exakt denselben
//! Plan an, den das Anwenden später ausführt, und nicht eine zweite,
//! nachgebaute Rechnung, die davon abweichen kann.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use apx_core::PhotoId;
use time::OffsetDateTime;

use crate::import::rename::{render_rename_pattern, RenameTokens};

/// Ein Foto, wie die Planung es braucht — bewusst nicht `apx_catalog::Photo`,
/// damit die Planung ohne Katalog testbar bleibt.
pub(crate) struct RenameCandidate {
    pub photo_id: PhotoId,
    pub current_filename: String,
    /// Aufnahmedatum, oder ersatzweise die Dateisystem-Änderungszeit —
    /// dieselbe Ersatzregel wie beim Import (`import::rename`s Moduldoku).
    pub date: OffsetDateTime,
    pub camera: Option<String>,
    /// Virtuelle Kopien (Phase 9 Schritt 1) teilen sich die Datei mit
    /// ihrem Quellfoto. Sie umzubenennen würde die Datei unter dem
    /// Quellfoto wegziehen — siehe `VirtualCopy` unten.
    pub is_virtual_copy: bool,
}

/// Warum ein Eintrag nicht umbenannt werden kann. Die Oberfläche zeigt
/// den Grund je Zeile an, statt nur „geht nicht" zu melden.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum RenameStatus {
    /// Wird umbenannt.
    Planned,
    /// Zielname ist der aktuelle Name — nichts zu tun. Kein Fehler: ein
    /// Muster auf einen bereits passend benannten Ordner anzuwenden soll
    /// nicht scheitern, sondern nichts bewirken.
    Unchanged,
    /// Das Muster ergab einen leeren Namen (z. B. nur unbekannte Tokens,
    /// die auf leere Werte fielen).
    EmptyName,
    /// Zwei Fotos des Stapels bekämen denselben Namen — typischer Fall:
    /// ein Muster ohne `{seq}` und ohne `{original}`.
    DuplicateInBatch,
    /// Im selben Ordner liegt bereits ein anderes Foto mit diesem Namen.
    CollidesWithExisting,
    /// Virtuelle Kopie — teilt sich die Datei mit dem Quellfoto. Ein
    /// Umbenennen würde die Datei unter dem Quellfoto wegziehen, dessen
    /// Katalogzeile dann auf einen Namen zeigt, den es nicht mehr gibt.
    /// Deshalb übersprungen statt heimlich mitbenannt: der Dateiname
    /// gehört dem Original, nicht der Kopie.
    VirtualCopy,
}

pub(crate) struct RenamePlanEntry {
    pub photo_id: PhotoId,
    pub current_filename: String,
    pub new_filename: String,
    pub status: RenameStatus,
}

impl RenamePlanEntry {
    pub fn is_blocked(&self) -> bool {
        !matches!(self.status, RenameStatus::Planned | RenameStatus::Unchanged)
    }
}

/// Plant die Umbenennung.
///
/// `start_seq` ist die Nummer des ersten Fotos; `{seq}` zählt in der
/// übergebenen Reihenfolge weiter — also in der Reihenfolge, die der
/// Nutzer in der Oberfläche sieht. Die Zählung läuft über den ganzen
/// Stapel, nicht je Ordner: wer 300 Fotos aus drei Ordnern auswählt,
/// erwartet 0001…0300, nicht dreimal 0001.
///
/// `occupied` sind die Dateinamen, die im jeweiligen Ordner schon
/// vergeben sind, **einschließlich** der Fotos des Stapels selbst (die
/// werden hier herausgerechnet — sonst meldete jedes Foto eine Kollision
/// mit sich selbst).
///
/// Die Dateiendung stammt immer aus dem bisherigen Namen und ist nicht
/// Teil des Musters — genau wie beim Import. Ein Muster kann die Endung
/// eines RAW also nicht versehentlich auf `.jpg` ändern.
pub(crate) fn plan_batch_rename(
    candidates: &[RenameCandidate],
    pattern: &str,
    start_seq: usize,
    occupied: &HashSet<String>,
) -> Vec<RenamePlanEntry> {
    let own_names: HashSet<&str> = candidates
        .iter()
        .map(|c| c.current_filename.as_str())
        .collect();

    let mut taken: HashSet<String> = HashSet::new();
    let mut entries = Vec::with_capacity(candidates.len());

    for (index, candidate) in candidates.iter().enumerate() {
        let extension = extension_of(&candidate.current_filename);
        let stem = stem_of(&candidate.current_filename);
        let tokens = RenameTokens {
            date: candidate.date,
            seq: start_seq + index,
            camera: candidate.camera.as_deref(),
            original_stem: stem,
        };
        let rendered = render_rename_pattern(pattern, &tokens);
        let rendered = rendered.trim().to_string();

        let new_filename = match &extension {
            Some(ext) if !rendered.is_empty() => format!("{rendered}.{ext}"),
            _ => rendered.clone(),
        };

        let status = if candidate.is_virtual_copy {
            RenameStatus::VirtualCopy
        } else if rendered.is_empty() {
            RenameStatus::EmptyName
        } else if new_filename == candidate.current_filename {
            RenameStatus::Unchanged
        } else if taken.contains(&new_filename) {
            RenameStatus::DuplicateInBatch
        } else if occupied.contains(&new_filename) && !own_names.contains(new_filename.as_str()) {
            RenameStatus::CollidesWithExisting
        } else {
            RenameStatus::Planned
        };

        if matches!(status, RenameStatus::Planned | RenameStatus::Unchanged) {
            taken.insert(new_filename.clone());
        }

        entries.push(RenamePlanEntry {
            photo_id: candidate.photo_id,
            current_filename: candidate.current_filename.clone(),
            new_filename,
            status,
        });
    }

    entries
}

/// Endung ohne Punkt, oder `None` bei einem Namen ohne Endung.
///
/// `rsplit_once('.')` statt `Path::extension`, weil ein Name wie
/// `.gitignore` (führender Punkt, keine Endung) hier sonst „gitignore"
/// als Endung bekäme; ein leerer Stamm gilt als endungslos.
fn extension_of(filename: &str) -> Option<String> {
    match filename.rsplit_once('.') {
        Some((stem, ext)) if !stem.is_empty() && !ext.is_empty() => Some(ext.to_string()),
        _ => None,
    }
}

fn stem_of(filename: &str) -> &str {
    match filename.rsplit_once('.') {
        Some((stem, _)) if !stem.is_empty() => stem,
        _ => filename,
    }
}

/// Benennt die Dateien eines fertigen Plans um — in **zwei Phasen**.
///
/// Erst bekommt jede Datei einen eindeutigen Zwischennamen, dann ihren
/// Zielnamen. Ohne diesen Umweg scheitert jeder Ringtausch: bei
/// `a → b, b → a` überschriebe der erste direkte Schritt bereits `b`.
/// Derselbe Umweg löst auch den häufigeren Fall einer reinen
/// Umnummerierung, bei der Ziel- und Quellnamen sich überlappen
/// (`0002 → 0001`, `0003 → 0002`, …).
///
/// Scheitert Phase 1, werden alle bereits verschobenen Dateien
/// zurückgeholt und `Err` gemeldet — auf der Platte ist dann nichts
/// verändert. Scheitert Phase 2 (deutlich unwahrscheinlicher: das Ziel
/// wurde in Phase 1 bereits als frei erkannt), bleibt der erreichte
/// Stand stehen und die Meldung sagt das ausdrücklich, statt einen
/// zweiten Rückbau zu versuchen, der genauso scheitern kann.
///
/// Gibt bei Erfolg die tatsächlich gesetzten Zielpfade in der
/// Eingabereihenfolge zurück — der Aufrufer schreibt danach den Katalog
/// nach. Reihenfolge Datei-vor-Katalog ist Absicht, siehe
/// `apx_catalog::repository::photos::set_filename`.
pub(crate) fn rename_files(moves: &[(PathBuf, PathBuf)]) -> Result<Vec<PathBuf>, String> {
    let mut staged: Vec<(PathBuf, PathBuf, PathBuf)> = Vec::with_capacity(moves.len());

    for (index, (source, target)) in moves.iter().enumerate() {
        let folder = source
            .parent()
            .ok_or_else(|| format!("Kein Ordner zu {}", source.display()))?;
        let temp = folder.join(format!(".apx-rename-{index}"));
        if temp.exists() {
            rollback(&staged);
            return Err(format!(
                "Zwischenname {} ist bereits belegt — Umbenennen abgebrochen, nichts verändert.",
                temp.display()
            ));
        }
        if let Err(err) = std::fs::rename(source, &temp) {
            rollback(&staged);
            return Err(format!(
                "{} konnte nicht umbenannt werden ({err}) — nichts verändert.",
                source.display()
            ));
        }
        move_sidecar(source, &temp);
        staged.push((source.clone(), temp, target.clone()));
    }

    let mut done = Vec::with_capacity(staged.len());
    for (_, temp, target) in &staged {
        std::fs::rename(temp, target).map_err(|err| {
            format!(
                "{} konnte nicht auf den Zielnamen gesetzt werden ({err}) — bereits umbenannte Dateien bleiben umbenannt.",
                temp.display()
            )
        })?;
        move_sidecar(temp, target);
        done.push(target.clone());
    }
    Ok(done)
}

fn rollback(staged: &[(PathBuf, PathBuf, PathBuf)]) {
    for (source, temp, _) in staged {
        let _ = std::fs::rename(temp, source);
        move_sidecar(temp, source);
    }
}

/// Zieht eine vorhandene `.xmp`-Sidecar-Datei mit um (siehe
/// `apx_export::xmp::write_sidecar`, das sie als `<name>.xmp` neben das
/// Foto legt). Fehlt sie, passiert nichts — die meisten Fotos haben
/// keine. Ein Fehler beim Sidecar bricht die Umbenennung bewusst nicht
/// ab: das Foto ist die Hauptsache, eine zurückgebliebene
/// Metadatendatei ist ärgerlich, aber kein Datenverlust.
pub(crate) fn move_sidecar(from: &Path, to: &Path) {
    let from_sidecar = from.with_extension("xmp");
    if from_sidecar.exists() {
        let _ = std::fs::rename(&from_sidecar, to.with_extension("xmp"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::Date;

    fn date(year: i32, month: u8, day: u8) -> OffsetDateTime {
        Date::from_calendar_date(year, month.try_into().expect("Monat"), day)
            .expect("Datum")
            .midnight()
            .assume_utc()
    }

    fn candidate(filename: &str) -> RenameCandidate {
        RenameCandidate {
            photo_id: PhotoId::new(),
            current_filename: filename.to_string(),
            date: date(2024, 5, 4),
            camera: Some("Canon EOS R5".to_string()),
            is_virtual_copy: false,
        }
    }

    fn write(dir: &Path, name: &str, content: &str) {
        std::fs::write(dir.join(name), content).expect("schreiben");
    }

    fn read(dir: &Path, name: &str) -> String {
        std::fs::read_to_string(dir.join(name)).expect("lesen")
    }

    #[test]
    fn rename_files_survives_a_swap_of_two_names() {
        // Der Fall, an dem ein einphasiges Umbenennen scheitert: a → b
        // überschriebe b, bevor b umbenannt wäre.
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path();
        write(path, "a.CR3", "INHALT-A");
        write(path, "b.CR3", "INHALT-B");

        rename_files(&[
            (path.join("a.CR3"), path.join("b.CR3")),
            (path.join("b.CR3"), path.join("a.CR3")),
        ])
        .expect("Ringtausch");

        assert_eq!(read(path, "b.CR3"), "INHALT-A");
        assert_eq!(read(path, "a.CR3"), "INHALT-B");
    }

    #[test]
    fn rename_files_moves_the_xmp_sidecar_along() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path();
        write(path, "IMG_0001.CR3", "FOTO");
        write(path, "IMG_0001.xmp", "SIDECAR");

        rename_files(&[(path.join("IMG_0001.CR3"), path.join("Ostsee_0001.CR3"))]).expect("ok");

        assert_eq!(read(path, "Ostsee_0001.CR3"), "FOTO");
        assert_eq!(read(path, "Ostsee_0001.xmp"), "SIDECAR");
        assert!(!path.join("IMG_0001.xmp").exists());
    }

    #[test]
    fn a_failure_in_the_first_phase_leaves_every_file_under_its_old_name() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path();
        write(path, "a.CR3", "INHALT-A");

        // Zweiter Eintrag zeigt auf eine Datei, die es nicht gibt — Phase 1
        // scheitert dort und muss den ersten Eintrag zurückholen.
        let result = rename_files(&[
            (path.join("a.CR3"), path.join("neu_a.CR3")),
            (path.join("fehlt.CR3"), path.join("neu_b.CR3")),
        ]);

        assert!(result.is_err());
        assert_eq!(read(path, "a.CR3"), "INHALT-A");
        assert!(!path.join("neu_a.CR3").exists());
        // Kein Zwischenname bleibt liegen.
        let leftovers: Vec<_> = std::fs::read_dir(path)
            .expect("lesen")
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.file_name().to_string_lossy().to_string())
            .filter(|name| name.starts_with(".apx-rename-"))
            .collect();
        assert!(leftovers.is_empty(), "übrig: {leftovers:?}");
    }

    #[test]
    fn renders_pattern_and_keeps_the_original_extension() {
        let plan = plan_batch_rename(
            &[candidate("IMG_0001.CR3")],
            "{date}_{seq}",
            1,
            &HashSet::new(),
        );
        assert_eq!(plan[0].new_filename, "20240504_0001.CR3");
        assert_eq!(plan[0].status, RenameStatus::Planned);
    }

    #[test]
    fn seq_counts_across_the_whole_batch_starting_at_start_seq() {
        let plan = plan_batch_rename(
            &[candidate("a.CR3"), candidate("b.CR3"), candidate("c.CR3")],
            "Bild_{seq}",
            10,
            &HashSet::new(),
        );
        let names: Vec<&str> = plan.iter().map(|e| e.new_filename.as_str()).collect();
        assert_eq!(names, ["Bild_0010.CR3", "Bild_0011.CR3", "Bild_0012.CR3"]);
    }

    #[test]
    fn a_pattern_without_seq_flags_every_photo_after_the_first_as_duplicate() {
        // Der klassische Fehler: „{date}" allein gibt allen Fotos eines
        // Tages denselben Namen. Ohne diese Prüfung bliebe nach dem
        // Umbenennen genau eine Datei übrig.
        let plan = plan_batch_rename(
            &[candidate("a.CR3"), candidate("b.CR3")],
            "{date}",
            1,
            &HashSet::new(),
        );
        assert_eq!(plan[0].status, RenameStatus::Planned);
        assert_eq!(plan[1].status, RenameStatus::DuplicateInBatch);
    }

    #[test]
    fn target_that_already_exists_in_the_folder_is_a_collision() {
        let mut occupied = HashSet::new();
        occupied.insert("Bild_0001.CR3".to_string());
        let plan = plan_batch_rename(&[candidate("a.CR3")], "Bild_{seq}", 1, &occupied);
        assert_eq!(plan[0].status, RenameStatus::CollidesWithExisting);
    }

    #[test]
    fn a_name_currently_held_by_another_photo_of_the_batch_is_not_a_collision() {
        // Ringtausch: a → b, b → a. Beide Zielnamen sind „belegt", aber
        // eben von Fotos, die selbst mitwandern. Das muss erlaubt sein,
        // sonst scheitert jede Umnummerierung innerhalb desselben
        // Ordners.
        let mut occupied = HashSet::new();
        occupied.insert("a.CR3".to_string());
        occupied.insert("b.CR3".to_string());
        let plan = plan_batch_rename(
            &[candidate("b.CR3"), candidate("a.CR3")],
            "{seq}",
            1,
            &occupied,
        );
        assert_eq!(plan[0].new_filename, "0001.CR3");
        assert_eq!(plan[1].new_filename, "0002.CR3");
        assert!(plan.iter().all(|e| e.status == RenameStatus::Planned));
    }

    #[test]
    fn unchanged_name_is_not_an_error() {
        let plan = plan_batch_rename(&[candidate("0001.CR3")], "{seq}", 1, &HashSet::new());
        assert_eq!(plan[0].status, RenameStatus::Unchanged);
        assert!(!plan[0].is_blocked());
    }

    #[test]
    fn empty_pattern_is_rejected_per_entry_instead_of_producing_a_dotfile() {
        let plan = plan_batch_rename(&[candidate("a.CR3")], "   ", 1, &HashSet::new());
        assert_eq!(plan[0].status, RenameStatus::EmptyName);
        assert!(plan[0].is_blocked());
    }

    #[test]
    fn original_token_uses_the_stem_without_the_extension() {
        let plan = plan_batch_rename(
            &[candidate("IMG_0042.CR3")],
            "{original}_bearbeitet",
            1,
            &HashSet::new(),
        );
        assert_eq!(plan[0].new_filename, "IMG_0042_bearbeitet.CR3");
    }

    #[test]
    fn a_virtual_copy_is_skipped_because_the_file_belongs_to_its_source() {
        let mut copy = candidate("IMG_0001.CR3");
        copy.is_virtual_copy = true;
        let plan = plan_batch_rename(&[copy], "Bild_{seq}", 1, &HashSet::new());
        assert_eq!(plan[0].status, RenameStatus::VirtualCopy);
        assert!(plan[0].is_blocked());
    }

    #[test]
    fn a_file_without_extension_stays_without_one() {
        let plan = plan_batch_rename(
            &[candidate("ROHDATEN")],
            "{original}_neu",
            1,
            &HashSet::new(),
        );
        assert_eq!(plan[0].new_filename, "ROHDATEN_neu");
    }
}
