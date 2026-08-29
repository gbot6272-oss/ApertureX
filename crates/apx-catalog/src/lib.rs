//! `apx-catalog` — SQLite-Katalog, Migrationen und typisierte Repositories
//! für Aperture X.
//!
//! Öffentliche API ist ausschließlich [`Catalog`] — alles SQL lebt in den
//! `repository`-Untermodulen und ist `pub(crate)`, siehe `ARCHITECTURE.md`
//! Abschnitt 4 ("öffentliche API sind typisierte Repository-Funktionen,
//! kein rohes `Connection`-Handle nach außen").
//!
//! **Verbindungsstrategie (siehe `DECISIONS.md` ADR-0008):** eine einzige
//! `rusqlite::Connection`, geschützt durch einen `Mutex`. Das erfüllt die
//! in `PHASE1_PROMPT.md` Abschnitt 10 genannte Regel "ein einziger Writer,
//! Leser über einen Pool" auf die einfachste Art, die für Phase 1 robust
//! genug ist: alle Zugriffe (lesend wie schreibend) werden auf Rust-Ebene
//! serialisiert, sodass "database is locked"-Fehler durch konkurrierende
//! Zugriffe aus diesem Prozess grundsätzlich ausgeschlossen sind. WAL-Modus
//! bleibt trotzdem aktiv, damit externe Werkzeuge (z. B. ein Debug-Tool)
//! parallel lesend zugreifen können.
//!
//! `apx-catalog` hängt nur von `apx-core` ab, nicht von `apx-raw`.

#![deny(clippy::unwrap_used)]

mod error;
mod migrations;
mod models;
mod repository;

use std::path::Path;
use std::sync::{Mutex, MutexGuard};

use apx_core::{AppError, EditHistoryId, EdlEnvelope, FolderId, PhotoId, Result};
use rusqlite::Connection;
use time::OffsetDateTime;

pub use models::{
    EditHistoryEntry, Folder, HistoryPosition, NewPhoto, Photo, Preview, PreviewLevel,
};

pub struct Catalog {
    conn: Mutex<Connection>,
}

impl Catalog {
    /// Öffnet (oder legt an) die Katalog-Datenbank unter `path` und wendet
    /// alle fehlenden Migrationen an.
    pub fn open(path: &Path) -> Result<Self> {
        let conn = Connection::open(path).map_err(error::map_sqlite_err)?;
        Self::from_connection(conn)
    }

    /// Öffnet einen rein speicherresidenten Katalog — für Tests, oder für
    /// den in Phase 3 geplanten "Smart Preview"-Offline-Modus.
    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory().map_err(error::map_sqlite_err)?;
        Self::from_connection(conn)
    }

    fn from_connection(conn: Connection) -> Result<Self> {
        configure(&conn)?;
        migrations::apply(&conn)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    fn lock(&self) -> Result<MutexGuard<'_, Connection>> {
        self.conn.lock().map_err(|_| AppError::Database {
            message: "Katalog-Verbindung ist vergiftet — ein vorheriger Zugriff ist während einer \
                      Transaktion abgestürzt"
                .to_string(),
        })
    }

    /// Führt `f` in einer SQLite-Transaktion aus. Schlägt `f` fehl (oder
    /// gibt einen Fehler zurück), wird die Transaktion zurückgerollt statt
    /// committet. Für alle Schreibvorgänge, die mehr als eine Zeile
    /// betreffen (siehe `SPEC.md` Abschnitt 2.3).
    pub fn transaction<T>(&self, f: impl FnOnce(&Connection) -> Result<T>) -> Result<T> {
        let mut conn = self.lock()?;
        let tx = conn.transaction().map_err(error::map_sqlite_err)?;
        let result = f(&tx)?;
        tx.commit().map_err(error::map_sqlite_err)?;
        Ok(result)
    }

    // ---- Ordner ------------------------------------------------------

    pub fn insert_folder(&self, path: &Path, parent_id: Option<FolderId>) -> Result<FolderId> {
        let conn = self.lock()?;
        repository::folders::insert(&conn, path, parent_id, OffsetDateTime::now_utc())
    }

    /// Findet einen Ordner anhand seines Pfads oder legt ihn an, falls er
    /// noch nicht existiert. Wird vom Import verwendet.
    pub fn find_or_create_folder(
        &self,
        path: &Path,
        parent_id: Option<FolderId>,
    ) -> Result<FolderId> {
        let conn = self.lock()?;
        repository::folders::find_or_create(&conn, path, parent_id, OffsetDateTime::now_utc())
    }

    pub fn find_folder_by_path(&self, path: &Path) -> Result<Option<Folder>> {
        let conn = self.lock()?;
        repository::folders::find_by_path(&conn, path)
    }

    pub fn get_folder(&self, id: FolderId) -> Result<Folder> {
        let conn = self.lock()?;
        repository::folders::get(&conn, id)
    }

    pub fn list_folders(&self) -> Result<Vec<Folder>> {
        let conn = self.lock()?;
        repository::folders::list_all(&conn)
    }

    // ---- Fotos ---------------------------------------------------------

    /// Legt ein Foto an oder aktualisiert es, siehe
    /// [`repository::photos::upsert`] für die genaue Semantik.
    pub fn upsert_photo(&self, new_photo: &NewPhoto) -> Result<(PhotoId, bool)> {
        let conn = self.lock()?;
        repository::photos::upsert(&conn, new_photo, OffsetDateTime::now_utc())
    }

    pub fn get_photo(&self, id: PhotoId) -> Result<Photo> {
        let conn = self.lock()?;
        repository::photos::get(&conn, id)
    }

    pub fn list_photos_by_folder(&self, folder_id: FolderId) -> Result<Vec<Photo>> {
        let conn = self.lock()?;
        repository::photos::list_by_folder(&conn, folder_id)
    }

    pub fn count_photos_in_folder(&self, folder_id: FolderId) -> Result<u64> {
        let conn = self.lock()?;
        repository::photos::count_by_folder(&conn, folder_id)
    }

    /// Markiert ein Foto als `missing` (Originaldatei außerhalb der App
    /// gelöscht/verschoben) oder hebt die Markierung wieder auf.
    pub fn set_photo_missing(&self, id: PhotoId, missing: bool) -> Result<()> {
        let conn = self.lock()?;
        repository::photos::set_missing(&conn, id, missing)
    }

    // ---- Vorschauen ------------------------------------------------------

    pub fn upsert_preview(
        &self,
        photo_id: PhotoId,
        level: PreviewLevel,
        path: &Path,
    ) -> Result<()> {
        let conn = self.lock()?;
        repository::previews::upsert(&conn, photo_id, level, path, OffsetDateTime::now_utc())
    }

    pub fn get_preview(&self, photo_id: PhotoId, level: PreviewLevel) -> Result<Option<Preview>> {
        let conn = self.lock()?;
        repository::previews::get(&conn, photo_id, level)
    }

    pub fn list_previews_for_photo(&self, photo_id: PhotoId) -> Result<Vec<Preview>> {
        let conn = self.lock()?;
        repository::previews::list_for_photo(&conn, photo_id)
    }

    // ---- Bearbeitungsverlauf (ab Phase 2) --------------------------------

    /// Speichert `edl` als neuen, aktiven Bearbeitungsschritt für `photo_id`
    /// — siehe [`repository::edits::commit`] für die genaue Semantik
    /// (verwirft eine zuvor per [`Catalog::undo_edit`] erreichte
    /// „Zukunft").
    pub fn commit_edit(
        &self,
        photo_id: PhotoId,
        edl: &EdlEnvelope,
        label: Option<&str>,
    ) -> Result<EditHistoryId> {
        let conn = self.lock()?;
        repository::edits::commit(&conn, photo_id, edl, label, OffsetDateTime::now_utc())
    }

    /// Der aktuell aktive Bearbeitungsstand für `photo_id`.
    pub fn current_edit(&self, photo_id: PhotoId) -> Result<HistoryPosition> {
        let conn = self.lock()?;
        repository::edits::current(&conn, photo_id)
    }

    /// Geht einen Bearbeitungsschritt zurück. `None`, wenn schon am
    /// Ausgangszustand (kein Rückgängig möglich).
    pub fn undo_edit(&self, photo_id: PhotoId) -> Result<Option<HistoryPosition>> {
        let conn = self.lock()?;
        repository::edits::undo(&conn, photo_id)
    }

    /// Geht einen Bearbeitungsschritt vor. `None`, wenn nichts zu
    /// wiederholen ist.
    pub fn redo_edit(&self, photo_id: PhotoId) -> Result<Option<HistoryPosition>> {
        let conn = self.lock()?;
        repository::edits::redo(&conn, photo_id)
    }

    /// Der vollständige Bearbeitungsverlauf eines Fotos, älteste Sequenz
    /// zuerst.
    pub fn list_edit_history(&self, photo_id: PhotoId) -> Result<Vec<EditHistoryEntry>> {
        let conn = self.lock()?;
        repository::edits::list_history(&conn, photo_id)
    }
}

fn configure(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "PRAGMA journal_mode = WAL;
         PRAGMA synchronous = NORMAL;
         PRAGMA foreign_keys = ON;
         PRAGMA busy_timeout = 5000;",
    )
    .map_err(error::map_sqlite_err)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn sample_photo(folder_id: FolderId) -> NewPhoto {
        NewPhoto {
            folder_id,
            filename: "IMG_0001.CR2".to_string(),
            file_size: 12_345_678,
            file_mtime: OffsetDateTime::now_utc()
                .replace_nanosecond(0)
                .expect("gültig"),
            content_hash: Some("deadbeef".to_string()),
            width: Some(6000),
            height: Some(4000),
            orientation: 1,
            camera_make: Some("Canon".to_string()),
            camera_model: Some("EOS R5".to_string()),
            lens: None,
            iso: Some(200),
            shutter: Some(0.004),
            aperture: Some(4.0),
            focal_length: Some(85.0),
            captured_at: None,
            gps_lat: None,
            gps_lon: None,
        }
    }

    #[test]
    fn open_in_memory_runs_migrations() {
        let catalog = Catalog::open_in_memory().expect("sollte öffnen");
        assert!(catalog.list_folders().expect("ok").is_empty());
    }

    #[test]
    fn open_on_disk_persists_across_reopen() {
        let tmp = tempfile::tempdir().expect("Temp-Verzeichnis");
        let db_path = tmp.path().join("catalog.sqlite");

        let folder_id;
        let photo_id;
        {
            let catalog = Catalog::open(&db_path).expect("sollte öffnen");
            folder_id = catalog
                .insert_folder(Path::new("/fotos"), None)
                .expect("ok");
            let (id, _) = catalog.upsert_photo(&sample_photo(folder_id)).expect("ok");
            photo_id = id;
        }
        // Katalog wird hier geschlossen (Drop) und danach neu geöffnet —
        // simuliert einen App-Neustart.
        {
            let catalog = Catalog::open(&db_path).expect("sollte erneut öffnen");
            let folder = catalog
                .get_folder(folder_id)
                .expect("Ordner sollte noch da sein");
            assert_eq!(folder.path, PathBuf::from("/fotos"));
            let photo = catalog
                .get_photo(photo_id)
                .expect("Foto sollte noch da sein");
            assert_eq!(photo.filename, "IMG_0001.CR2");
        }
    }

    /// SPEC.md §7 Definition-of-Done, Punkt 6: "In der EDL serialisierbar
    /// und nach Neustart identisch reproduzierbar" — siehe `PLAN.md` Phase
    /// 2 Schritt 10. Analog zu [`open_on_disk_persists_across_reopen`],
    /// aber für `edit_history`/`edit_current` statt `photos`/`folders`.
    #[test]
    fn edit_history_persists_across_reopen() {
        let tmp = tempfile::tempdir().expect("Temp-Verzeichnis");
        let db_path = tmp.path().join("catalog.sqlite");

        let photo_id;
        let edl = EdlEnvelope::new(
            1,
            serde_json::json!({ "exposure_ev": 0.75, "marker": "reopen-test" }),
        );
        {
            let catalog = Catalog::open(&db_path).expect("sollte öffnen");
            let folder_id = catalog
                .insert_folder(Path::new("/fotos"), None)
                .expect("ok");
            let (id, _) = catalog.upsert_photo(&sample_photo(folder_id)).expect("ok");
            photo_id = id;
            catalog
                .commit_edit(photo_id, &edl, Some("Testbearbeitung"))
                .expect("sollte committen");
        }
        // Katalog wird hier geschlossen (Drop) und danach neu geöffnet —
        // simuliert einen App-Neustart.
        {
            let catalog = Catalog::open(&db_path).expect("sollte erneut öffnen");
            match catalog.current_edit(photo_id).expect("sollte lesbar sein") {
                HistoryPosition::At(entry) => {
                    assert_eq!(entry.label.as_deref(), Some("Testbearbeitung"));
                    assert_eq!(
                        entry.edl, edl,
                        "EDL muss nach Neustart identisch reproduzierbar sein"
                    );
                }
                HistoryPosition::Neutral => {
                    panic!("Bearbeitungsstand sollte den Neustart überleben")
                }
            }
        }
    }

    #[test]
    fn transaction_rolls_back_on_error() {
        let catalog = Catalog::open_in_memory().expect("sollte öffnen");
        let result: Result<()> = catalog.transaction(|conn| {
            repository::folders::insert(conn, Path::new("/a"), None, OffsetDateTime::now_utc())?;
            Err(AppError::Cancelled("Testabbruch".to_string()))
        });
        assert!(result.is_err());
        assert!(
            catalog.list_folders().expect("ok").is_empty(),
            "Rollback muss den Insert rückgängig machen"
        );
    }

    #[test]
    fn transaction_commits_on_success() {
        let catalog = Catalog::open_in_memory().expect("sollte öffnen");
        catalog
            .transaction(|conn| {
                repository::folders::insert(
                    conn,
                    Path::new("/a"),
                    None,
                    OffsetDateTime::now_utc(),
                )?;
                repository::folders::insert(
                    conn,
                    Path::new("/b"),
                    None,
                    OffsetDateTime::now_utc(),
                )?;
                Ok(())
            })
            .expect("Transaktion darf nicht scheitern");
        assert_eq!(catalog.list_folders().expect("ok").len(), 2);
    }

    #[test]
    fn duplicate_import_of_same_folder_adds_no_duplicates() {
        let catalog = Catalog::open_in_memory().expect("sollte öffnen");
        let folder_id = catalog
            .find_or_create_folder(Path::new("/fotos"), None)
            .expect("ok");
        let (id_a, _) = catalog.upsert_photo(&sample_photo(folder_id)).expect("ok");
        let (id_b, changed) = catalog.upsert_photo(&sample_photo(folder_id)).expect("ok");

        assert_eq!(id_a, id_b);
        assert!(!changed);
        assert_eq!(catalog.count_photos_in_folder(folder_id).expect("ok"), 1);
    }

    #[test]
    fn foreign_key_cascade_is_enforced() {
        let catalog = Catalog::open_in_memory().expect("sollte öffnen");
        let folder_id = catalog
            .insert_folder(Path::new("/fotos"), None)
            .expect("ok");
        let (photo_id, _) = catalog.upsert_photo(&sample_photo(folder_id)).expect("ok");
        catalog
            .upsert_preview(
                photo_id,
                PreviewLevel::Thumbnail,
                Path::new("/cache/th.jpg"),
            )
            .expect("ok");

        catalog
            .transaction(|conn| {
                conn.execute("DELETE FROM folders WHERE id = ?1", [folder_id.to_string()])
                    .map(|_| ())
                    .map_err(error::map_sqlite_err)
            })
            .expect("Delete darf nicht scheitern");

        assert!(
            catalog.get_photo(photo_id).is_err(),
            "Foto sollte per Kaskade gelöscht sein"
        );
        assert!(
            catalog
                .list_previews_for_photo(photo_id)
                .expect("ok")
                .is_empty(),
            "Preview sollte per Kaskade gelöscht sein"
        );
    }
}
