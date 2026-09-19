//! Ordner-Abgleich (Phase 33 F2): sagt für einen bereits importierten
//! Ordner, was sich auf der Platte seit dem Import geändert hat.
//!
//! **Abgrenzung zu dem, was es schon gab.** `reconcile.rs` prüft nur die
//! eine Frage „ist die Datei noch da?" und setzt `missing`. Der Import
//! (`import/mod.rs`) geht den umgekehrten Weg: er liest alles vom
//! Dateisystem und legt an, was fehlt. Keiner von beiden beantwortet die
//! Frage, die man nach einem Zwischenstand tatsächlich stellt: *was hat
//! sich geändert?* — und keiner erlaubt, die Antwort anzusehen, bevor
//! etwas passiert.
//!
//! Deshalb ist die Planung hier eine reine Funktion über zwei Listen
//! ([`plan_folder_sync`]) und nicht in den Dateisystem-Durchlauf
//! verwoben: dieselbe Trennung wie bei der Stapel-Umbenennung (Phase 32
//! F4). Die Vorschau im Dialog und das Anwenden rechnen damit garantiert
//! dasselbe Ergebnis aus, statt zwei Implementierungen zu haben, die
//! auseinanderlaufen können.
//!
//! **Warum die Änderungserkennung Größe UND Zeitstempel nimmt.** Nur die
//! Größe übersieht eine Bearbeitung, die zufällig gleich groß bleibt
//! (bei einem verlustfrei gedrehten JPEG passiert genau das). Nur der
//! Zeitstempel schlägt fälschlich an, wenn eine Datei kopiert oder von
//! einem Backup zurückgespielt wurde, ohne dass sich der Inhalt
//! geändert hat. Beides zusammen ist immer noch keine Garantie — die
//! gäbe nur ein Hash, und den für jede Datei jedes Mal neu zu lesen
//! macht aus einem Abgleich einen Neu-Import. Der Abgleich *meldet*
//! deshalb, er entscheidet nicht: das Wiedereinlesen der Metadaten ist
//! ein eigener, bestätigter Schritt.

use std::collections::BTreeMap;

use apx_core::PhotoId;

/// Eine Datei, wie sie im Ordner liegt.
#[derive(Debug, Clone, PartialEq)]
pub struct DiskFile {
    pub filename: String,
    pub file_size: u64,
    /// Unix-Sekunden der letzten Änderung.
    pub file_mtime: i64,
}

/// Ein Foto, wie es im Katalog steht.
#[derive(Debug, Clone, PartialEq)]
pub struct CatalogFile {
    pub photo_id: PhotoId,
    pub filename: String,
    pub file_size: u64,
    pub file_mtime: i64,
    /// Virtuelle Kopien teilen sich die Datei mit ihrem Quellfoto und
    /// tauchen im Abgleich deshalb nie auf — sonst würde jede Kopie die
    /// Datei ihres Originals ein zweites Mal melden.
    pub is_virtual_copy: bool,
    /// Bereits als fehlend markiert. Solche Fotos werden weiterhin
    /// gemeldet (sie sind ja immer noch weg), aber der Zähler „neu
    /// verschwunden" unterscheidet sie.
    pub already_missing: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SyncChange {
    /// Liegt im Ordner, steht nicht im Katalog.
    New,
    /// Steht im Katalog, liegt nicht mehr im Ordner.
    Vanished,
    /// Steht im Katalog und liegt im Ordner, aber Größe oder Zeitstempel
    /// weichen ab.
    Modified,
    /// War als fehlend markiert und ist wieder da.
    Returned,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SyncEntry {
    pub filename: String,
    pub change: SyncChange,
    /// Nur bei `Vanished`/`Modified`/`Returned` gesetzt.
    pub photo_id: Option<PhotoId>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct FolderSyncPlan {
    pub entries: Vec<SyncEntry>,
}

impl FolderSyncPlan {
    pub fn count(&self, change: &SyncChange) -> usize {
        self.entries.iter().filter(|e| &e.change == change).count()
    }

    /// `true`, wenn der Ordner und der Katalog übereinstimmen.
    pub fn is_clean(&self) -> bool {
        self.entries.is_empty()
    }
}

/// Vergleicht Katalog und Ordner und meldet jede Abweichung.
///
/// Die Zuordnung läuft über den Dateinamen, nicht über den Inhalt: eine
/// umbenannte Datei erscheint deshalb als ein Verschwundenes plus ein
/// Neues, nicht als Umbenennung. Das ist Absicht — den Unterschied
/// könnte nur ein Inhaltsvergleich sicher machen, und ein falsch
/// geratenes „das ist dieselbe Datei" würde die Bearbeitungen des einen
/// Fotos einem anderen zuschlagen. Zwei Einträge, die man beide sieht,
/// sind der ehrlichere Bericht.
///
/// Das Ergebnis ist nach Dateiname sortiert und damit stabil — die
/// Vorschau springt zwischen zwei Durchläufen nicht um.
pub fn plan_folder_sync(catalog: &[CatalogFile], disk: &[DiskFile]) -> FolderSyncPlan {
    let on_disk: BTreeMap<&str, &DiskFile> = disk
        .iter()
        .map(|file| (file.filename.as_str(), file))
        .collect();

    let mut entries: Vec<SyncEntry> = Vec::new();
    let mut known: BTreeMap<&str, ()> = BTreeMap::new();

    for photo in catalog.iter().filter(|photo| !photo.is_virtual_copy) {
        known.insert(photo.filename.as_str(), ());
        match on_disk.get(photo.filename.as_str()) {
            None => entries.push(SyncEntry {
                filename: photo.filename.clone(),
                change: SyncChange::Vanished,
                photo_id: Some(photo.photo_id),
            }),
            Some(file) => {
                if photo.already_missing {
                    entries.push(SyncEntry {
                        filename: photo.filename.clone(),
                        change: SyncChange::Returned,
                        photo_id: Some(photo.photo_id),
                    });
                } else if file.file_size != photo.file_size || file.file_mtime != photo.file_mtime {
                    entries.push(SyncEntry {
                        filename: photo.filename.clone(),
                        change: SyncChange::Modified,
                        photo_id: Some(photo.photo_id),
                    });
                }
            }
        }
    }

    for file in disk {
        if !known.contains_key(file.filename.as_str()) {
            entries.push(SyncEntry {
                filename: file.filename.clone(),
                change: SyncChange::New,
                photo_id: None,
            });
        }
    }

    entries.sort_by(|a, b| a.filename.cmp(&b.filename));
    FolderSyncPlan { entries }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn catalog_file(name: &str, size: u64, mtime: i64) -> CatalogFile {
        CatalogFile {
            photo_id: PhotoId::new(),
            filename: name.to_string(),
            file_size: size,
            file_mtime: mtime,
            is_virtual_copy: false,
            already_missing: false,
        }
    }

    fn disk_file(name: &str, size: u64, mtime: i64) -> DiskFile {
        DiskFile {
            filename: name.to_string(),
            file_size: size,
            file_mtime: mtime,
        }
    }

    #[test]
    fn ein_unveraenderter_ordner_meldet_nichts() {
        let catalog = vec![
            catalog_file("a.jpg", 100, 10),
            catalog_file("b.jpg", 200, 20),
        ];
        let disk = vec![disk_file("a.jpg", 100, 10), disk_file("b.jpg", 200, 20)];
        let plan = plan_folder_sync(&catalog, &disk);
        assert!(plan.is_clean(), "{plan:?}");
    }

    #[test]
    fn eine_neue_datei_wird_gemeldet() {
        let catalog = vec![catalog_file("a.jpg", 100, 10)];
        let disk = vec![disk_file("a.jpg", 100, 10), disk_file("neu.jpg", 50, 30)];
        let plan = plan_folder_sync(&catalog, &disk);
        assert_eq!(plan.entries.len(), 1);
        assert_eq!(plan.entries[0].change, SyncChange::New);
        assert_eq!(plan.entries[0].filename, "neu.jpg");
        assert!(plan.entries[0].photo_id.is_none());
    }

    #[test]
    fn eine_verschwundene_datei_wird_mit_ihrer_foto_id_gemeldet() {
        let catalog = vec![catalog_file("weg.jpg", 100, 10)];
        let plan = plan_folder_sync(&catalog, &[]);
        assert_eq!(plan.entries.len(), 1);
        assert_eq!(plan.entries[0].change, SyncChange::Vanished);
        assert_eq!(plan.entries[0].photo_id, Some(catalog[0].photo_id));
    }

    #[test]
    fn eine_geaenderte_groesse_zaehlt_als_aenderung() {
        let catalog = vec![catalog_file("a.jpg", 100, 10)];
        let disk = vec![disk_file("a.jpg", 111, 10)];
        assert_eq!(
            plan_folder_sync(&catalog, &disk).entries[0].change,
            SyncChange::Modified
        );
    }

    #[test]
    fn ein_geaenderter_zeitstempel_zaehlt_ebenfalls_als_aenderung() {
        // Genau der Fall, den eine reine Größenprüfung übersieht: ein
        // verlustfrei gedrehtes JPEG bleibt oft gleich groß.
        let catalog = vec![catalog_file("a.jpg", 100, 10)];
        let disk = vec![disk_file("a.jpg", 100, 99)];
        assert_eq!(
            plan_folder_sync(&catalog, &disk).entries[0].change,
            SyncChange::Modified
        );
    }

    #[test]
    fn eine_zurueckgekehrte_datei_wird_als_solche_gemeldet_nicht_als_aenderung() {
        let mut photo = catalog_file("a.jpg", 100, 10);
        photo.already_missing = true;
        // Bewusst mit abweichender Größe: „wieder da" gewinnt gegen
        // „geändert", sonst müsste man dieselbe Datei zweimal bestätigen.
        let disk = vec![disk_file("a.jpg", 777, 10)];
        let plan = plan_folder_sync(&[photo], &disk);
        assert_eq!(plan.entries.len(), 1);
        assert_eq!(plan.entries[0].change, SyncChange::Returned);
    }

    #[test]
    fn virtuelle_kopien_tauchen_im_abgleich_nicht_auf() {
        let mut copy = catalog_file("a.jpg", 100, 10);
        copy.is_virtual_copy = true;
        let catalog = vec![catalog_file("a.jpg", 100, 10), copy];
        let disk = vec![disk_file("a.jpg", 100, 10)];
        assert!(plan_folder_sync(&catalog, &disk).is_clean());
    }

    #[test]
    fn eine_virtuelle_kopie_haelt_kein_verschwundenes_original_am_leben() {
        // Umgekehrte Richtung derselben Regel: die Kopie darf das
        // Original nicht verdecken, wenn dessen Datei wirklich weg ist.
        let mut copy = catalog_file("a.jpg", 100, 10);
        copy.is_virtual_copy = true;
        let catalog = vec![catalog_file("a.jpg", 100, 10), copy];
        let plan = plan_folder_sync(&catalog, &[]);
        assert_eq!(plan.entries.len(), 1);
        assert_eq!(plan.entries[0].change, SyncChange::Vanished);
    }

    #[test]
    fn eine_umbenennung_erscheint_als_zwei_eintraege() {
        let catalog = vec![catalog_file("alt.jpg", 100, 10)];
        let disk = vec![disk_file("neu.jpg", 100, 10)];
        let plan = plan_folder_sync(&catalog, &disk);
        assert_eq!(plan.entries.len(), 2);
        assert_eq!(plan.entries[0].filename, "alt.jpg");
        assert_eq!(plan.entries[0].change, SyncChange::Vanished);
        assert_eq!(plan.entries[1].filename, "neu.jpg");
        assert_eq!(plan.entries[1].change, SyncChange::New);
    }

    #[test]
    fn das_ergebnis_ist_nach_dateiname_sortiert() {
        let catalog = vec![catalog_file("z.jpg", 1, 1)];
        let disk = vec![disk_file("a.jpg", 1, 1), disk_file("m.jpg", 1, 1)];
        let plan = plan_folder_sync(&catalog, &disk);
        let names: Vec<&str> = plan.entries.iter().map(|e| e.filename.as_str()).collect();
        assert_eq!(names, vec!["a.jpg", "m.jpg", "z.jpg"]);
    }

    #[test]
    fn die_zaehler_stimmen() {
        let catalog = vec![
            catalog_file("weg.jpg", 1, 1),
            catalog_file("anders.jpg", 1, 1),
        ];
        let disk = vec![disk_file("anders.jpg", 2, 1), disk_file("neu.jpg", 1, 1)];
        let plan = plan_folder_sync(&catalog, &disk);
        assert_eq!(plan.count(&SyncChange::New), 1);
        assert_eq!(plan.count(&SyncChange::Vanished), 1);
        assert_eq!(plan.count(&SyncChange::Modified), 1);
        assert_eq!(plan.count(&SyncChange::Returned), 0);
    }

    #[test]
    fn ein_leerer_ordner_meldet_jedes_katalogfoto_als_verschwunden() {
        let catalog = vec![catalog_file("a.jpg", 1, 1), catalog_file("b.jpg", 1, 1)];
        let plan = plan_folder_sync(&catalog, &[]);
        assert_eq!(plan.count(&SyncChange::Vanished), 2);
    }

    #[test]
    fn ein_leerer_katalog_meldet_jede_datei_als_neu() {
        let disk = vec![disk_file("a.jpg", 1, 1), disk_file("b.jpg", 1, 1)];
        let plan = plan_folder_sync(&[], &disk);
        assert_eq!(plan.count(&SyncChange::New), 2);
    }
}
