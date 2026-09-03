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
pub mod iptc;
mod migrations;
mod models;
mod repository;

use std::path::Path;
use std::sync::{Mutex, MutexGuard};

use apx_core::{
    AppError, BatchOperationId, CollectionFolderId, CollectionId, EditHistoryId, EdlEnvelope,
    FolderId, KeywordId, PhotoId, PresetFolderId, PresetId, PresetVersionId, Result, SnapshotId,
    StackId, TagRuleId, TemplateId,
};
use rusqlite::Connection;
use time::OffsetDateTime;

pub use models::{
    CatalogStatistics, Collection, CollectionFolder, ColorLabelDefinition, EditHistoryEntry,
    FilterCriteria, Folder, HistoryPosition, Keyword, NewPhoto, Photo, Preset, PresetFolder,
    PresetVersion, Preview, PreviewLevel, Snapshot, Stack, TagRule, Template,
};
pub use repository::batch::BatchAction;
pub use repository::share::ShareDiff;

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

    // ---- Katalog-Wartung (Phase 13 Schritt 6, siehe `DECISIONS.md`
    // ADR-0040-Nachtrag IV) ------------------------------------------

    /// Führt SQLites eigene `PRAGMA integrity_check` aus — die
    /// Standardmethode, um eine SQLite-Datei auf strukturelle Schäden zu
    /// prüfen (defekte Seiten, kaputte Indizes usw.), ohne sie
    /// tatsächlich zu reparieren. Leerer Vektor = alles in Ordnung (die
    /// echte Ausgabe bei Erfolg ist die einzeilige Zeichenkette `"ok"`,
    /// die hier statt eines künstlichen leeren Erfolgsmarkers
    /// herausgefiltert wird); jede andere Zeile beschreibt einen
    /// gefundenen Fehler.
    pub fn integrity_check(&self) -> Result<Vec<String>> {
        let conn = self.lock()?;
        let mut stmt = conn
            .prepare("PRAGMA integrity_check")
            .map_err(error::map_sqlite_err)?;
        let rows = stmt
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(error::map_sqlite_err)?;
        let mut problems = Vec::new();
        for row in rows {
            let line = row.map_err(error::map_sqlite_err)?;
            if line != "ok" {
                problems.push(line);
            }
        }
        Ok(problems)
    }

    /// Führt `VACUUM` aus — baut die Datenbankdatei komplett neu auf,
    /// verwirft dabei durch Löschungen freigewordenen, aber noch
    /// belegten Speicherplatz (SQLite gibt ihn sonst nicht von selbst an
    /// das Dateisystem zurück) und defragmentiert die Seitenanordnung.
    /// Läuft in einer eigenen, impliziten Transaktion — `VACUUM`
    /// akzeptiert keine umschließende Transaktion.
    pub fn vacuum(&self) -> Result<()> {
        let conn = self.lock()?;
        conn.execute_batch("VACUUM").map_err(error::map_sqlite_err)
    }

    /// Sichert den Katalog per SQLites Online-Backup-API nach `dest` —
    /// sicher neben der weiterhin offenen Verbindung nutzbar (anders als
    /// eine rohe Dateikopie, die bei gleichzeitigem Schreibzugriff eine
    /// inkonsistente Kopie ergeben könnte). Überschreibt `dest`, falls
    /// die Datei bereits existiert (`rusqlite::Connection::backup`s
    /// eigenes Verhalten).
    pub fn backup_to(&self, dest: &Path) -> Result<()> {
        let conn = self.lock()?;
        conn.backup(rusqlite::DatabaseName::Main, dest, None)
            .map_err(error::map_sqlite_err)
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

    /// Verknüpft einen Ordner neu mit `new_path` (z. B. nach Verschieben/
    /// Umbenennen im Dateisystem) — der Aufrufer (`apx-app`) lässt danach
    /// den bestehenden Reconcile-Mechanismus erneut laufen, um die
    /// zugehörigen Fotos gegen den neuen Pfad abzugleichen.
    pub fn relink_folder(&self, id: FolderId, new_path: &Path) -> Result<()> {
        let conn = self.lock()?;
        repository::folders::update_path(&conn, id, new_path)
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

    /// Springt direkt zu einer Sequenznummer aus [`list_edit_history`]
    /// (Phase 9 Schritt 7, Zeitleisten-Ansicht) — `None`, wenn `sequence`
    /// nicht existiert.
    pub fn goto_edit(&self, photo_id: PhotoId, sequence: i64) -> Result<Option<HistoryPosition>> {
        let conn = self.lock()?;
        repository::edits::goto(&conn, photo_id, sequence)
    }

    // ---- Schnappschüsse (Phase 6 Schritt 8) ------------------------------
    // Anders als der lineare Bearbeitungsverlauf oben: siehe
    // `repository::snapshots`s Moduldoku für die Abgrenzung.

    /// Legt einen neuen Schnappschuss mit einer eigenen Kopie von `edl` an.
    pub fn create_snapshot(
        &self,
        photo_id: PhotoId,
        name: &str,
        edl: &EdlEnvelope,
    ) -> Result<SnapshotId> {
        let conn = self.lock()?;
        repository::snapshots::create(&conn, photo_id, name, edl, OffsetDateTime::now_utc())
    }

    /// Alle Schnappschüsse eines Fotos, älteste zuerst.
    pub fn list_snapshots(&self, photo_id: PhotoId) -> Result<Vec<Snapshot>> {
        let conn = self.lock()?;
        repository::snapshots::list(&conn, photo_id)
    }

    pub fn rename_snapshot(&self, snapshot_id: SnapshotId, name: &str) -> Result<()> {
        let conn = self.lock()?;
        repository::snapshots::rename(&conn, snapshot_id, name)
    }

    pub fn delete_snapshot(&self, snapshot_id: SnapshotId) -> Result<()> {
        let conn = self.lock()?;
        repository::snapshots::delete(&conn, snapshot_id)
    }

    // ---- Bewertung/Flagge/Farbe (ab Phase 3) -----------------------------

    /// Setzt die Sternebewertung (0–5) eines Fotos.
    pub fn set_photo_rating(&self, id: PhotoId, rating: u8) -> Result<()> {
        let conn = self.lock()?;
        repository::photos::set_rating(&conn, id, rating)
    }

    /// Setzt die Pick/Reject-Flagge (-1/0/1) eines Fotos.
    pub fn set_photo_flag(&self, id: PhotoId, flag: i8) -> Result<()> {
        let conn = self.lock()?;
        repository::photos::set_flag(&conn, id, flag)
    }

    /// Setzt oder löscht (`None`) die Farbmarkierung eines Fotos.
    pub fn set_photo_color_label(&self, id: PhotoId, color_label: Option<&str>) -> Result<()> {
        let conn = self.lock()?;
        repository::photos::set_color_label(&conn, id, color_label)
    }

    // ---- Karte (Phase 8 Schritt 7) -----------------------------------------

    /// Alle Fotos mit bekannten GPS-Koordinaten, ordnerübergreifend, nach
    /// Aufnahmezeit sortiert — Grundlage für Kartenansicht und
    /// Reiserouten-Ansicht.
    pub fn list_geotagged_photos(&self) -> Result<Vec<Photo>> {
        let conn = self.lock()?;
        repository::photos::list_geotagged(&conn)
    }

    /// Setzt oder löscht (`None`) die GPS-Koordinaten eines Fotos von Hand
    /// (z. B. per Klick auf die Kartenansicht platziert).
    pub fn set_photo_gps(&self, id: PhotoId, gps: Option<(f64, f64)>) -> Result<()> {
        let conn = self.lock()?;
        repository::photos::set_gps(&conn, id, gps)
    }

    // ---- Schlagworte (ab Phase 3) -----------------------------------------

    /// Verknüpft `photo_id` mit dem Schlagwort `name` — legt es bei Bedarf an.
    pub fn add_keyword(&self, photo_id: PhotoId, name: &str) -> Result<KeywordId> {
        let conn = self.lock()?;
        repository::keywords::add(&conn, photo_id, name)
    }

    /// Löst die Verknüpfung zwischen Foto und Schlagwort (das Schlagwort
    /// selbst bleibt im Katalog bestehen).
    pub fn remove_keyword(&self, photo_id: PhotoId, keyword_id: KeywordId) -> Result<()> {
        let conn = self.lock()?;
        repository::keywords::remove(&conn, photo_id, keyword_id)
    }

    pub fn list_keywords_for_photo(&self, photo_id: PhotoId) -> Result<Vec<Keyword>> {
        let conn = self.lock()?;
        repository::keywords::list_for_photo(&conn, photo_id)
    }

    pub fn list_all_keywords(&self) -> Result<Vec<Keyword>> {
        let conn = self.lock()?;
        repository::keywords::list_all(&conn)
    }

    // ---- Schlagworthierarchie/Tag-Regeln/Metadaten (ab Phase 9 Schritt 2,
    // siehe DECISIONS.md ADR-0035) -------------------------------------------

    /// Setzt das übergeordnete Schlagwort — `None` macht es zu einem
    /// Wurzel-Schlagwort.
    pub fn set_keyword_parent(
        &self,
        keyword_id: KeywordId,
        parent_id: Option<KeywordId>,
    ) -> Result<()> {
        let conn = self.lock()?;
        repository::keywords::set_parent(&conn, keyword_id, parent_id)
    }

    pub fn set_keyword_synonyms(&self, keyword_id: KeywordId, synonyms: &[String]) -> Result<()> {
        let conn = self.lock()?;
        repository::keywords::set_synonyms(&conn, keyword_id, synonyms)
    }

    pub fn delete_keyword(&self, keyword_id: KeywordId) -> Result<()> {
        let conn = self.lock()?;
        repository::keywords::delete(&conn, keyword_id)
    }

    /// Legt eine bedingte Auto-Schlagwort-Regel an — `conditions_json` ist
    /// derselbe `PresetCondition[]`-Vertrag wie bei Import-Presets, wird
    /// hier nur gespeichert, nicht ausgewertet (siehe
    /// `repository::tag_rules`s Moduldoku).
    pub fn create_tag_rule(
        &self,
        name: &str,
        keyword_id: KeywordId,
        conditions_json: &str,
    ) -> Result<TagRuleId> {
        let conn = self.lock()?;
        repository::tag_rules::create(
            &conn,
            name,
            keyword_id,
            conditions_json,
            OffsetDateTime::now_utc(),
        )
    }

    pub fn set_tag_rule_enabled(&self, id: TagRuleId, enabled: bool) -> Result<()> {
        let conn = self.lock()?;
        repository::tag_rules::set_enabled(&conn, id, enabled)
    }

    pub fn delete_tag_rule(&self, id: TagRuleId) -> Result<()> {
        let conn = self.lock()?;
        repository::tag_rules::delete(&conn, id)
    }

    pub fn list_tag_rules(&self) -> Result<Vec<TagRule>> {
        let conn = self.lock()?;
        repository::tag_rules::list_all(&conn)
    }

    /// Aktualisiert die vier IPTC-artigen Metadaten-Überschreibungen eines
    /// Fotos — deckt auch Stapel-Metadatenbearbeitung ab (Aufrufer ruft
    /// dies für jedes ausgewählte Foto einzeln auf).
    #[allow(clippy::too_many_arguments)]
    pub fn set_photo_metadata(
        &self,
        photo_id: PhotoId,
        title: Option<&str>,
        caption: Option<&str>,
        copyright: Option<&str>,
        creator: Option<&str>,
    ) -> Result<()> {
        let conn = self.lock()?;
        repository::photos::set_metadata(&conn, photo_id, title, caption, copyright, creator)
    }

    /// Ersetzt die frei benannten IPTC-Zusatzfelder eines Fotos (Phase 12
    /// Schritt 4, voller EXIF/IPTC-Editor, siehe `DECISIONS.md` ADR-0039)
    /// — wie [`Self::set_photo_metadata`] deckt das auch Stapel-
    /// Metadatenbearbeitung ab.
    pub fn set_photo_custom_metadata(
        &self,
        photo_id: PhotoId,
        metadata: &std::collections::BTreeMap<String, String>,
    ) -> Result<()> {
        let conn = self.lock()?;
        repository::photos::set_custom_metadata(&conn, photo_id, metadata)
    }

    /// Aggregierte Katalog-Statistik (Phase 9 Schritt 3) — siehe
    /// [`repository::stats`]s Moduldoku.
    pub fn catalog_statistics(&self) -> Result<CatalogStatistics> {
        let conn = self.lock()?;
        repository::stats::compute(&conn)
    }

    // ---- Sammlungen (ab Phase 3, Sammlungssätze/intelligente Sammlungen ---
    // ab Phase 9 Schritt 1, siehe DECISIONS.md ADR-0032/ADR-0035) -----------

    pub fn create_collection(
        &self,
        name: &str,
        folder_id: Option<CollectionFolderId>,
    ) -> Result<CollectionId> {
        let conn = self.lock()?;
        repository::collections::create(&conn, name, folder_id, OffsetDateTime::now_utc())
    }

    /// Legt eine intelligente Sammlung an — siehe
    /// [`repository::collections::create_smart`] für die
    /// Vereinfachung gegenüber verschachtelten UND/ODER-Regeln.
    pub fn create_smart_collection(
        &self,
        name: &str,
        folder_id: Option<CollectionFolderId>,
        criteria: &FilterCriteria,
    ) -> Result<CollectionId> {
        let conn = self.lock()?;
        repository::collections::create_smart(
            &conn,
            name,
            folder_id,
            criteria,
            OffsetDateTime::now_utc(),
        )
    }

    pub fn rename_collection(&self, id: CollectionId, name: &str) -> Result<()> {
        let conn = self.lock()?;
        repository::collections::rename(&conn, id, name)
    }

    pub fn move_collection_to_folder(
        &self,
        id: CollectionId,
        folder_id: Option<CollectionFolderId>,
    ) -> Result<()> {
        let conn = self.lock()?;
        repository::collections::move_to_folder(&conn, id, folder_id)
    }

    pub fn delete_collection(&self, id: CollectionId) -> Result<()> {
        let conn = self.lock()?;
        repository::collections::delete(&conn, id)
    }

    pub fn list_collections(&self) -> Result<Vec<Collection>> {
        let conn = self.lock()?;
        repository::collections::list_all(&conn)
    }

    /// Fügt ein Foto ans Ende einer Sammlung an — erneutes Hinzufügen
    /// desselben Fotos ist ein No-Op (siehe
    /// [`repository::collections::add_photo`]).
    pub fn add_photo_to_collection(
        &self,
        collection_id: CollectionId,
        photo_id: PhotoId,
    ) -> Result<()> {
        let conn = self.lock()?;
        repository::collections::add_photo(&conn, collection_id, photo_id)
    }

    pub fn remove_photo_from_collection(
        &self,
        collection_id: CollectionId,
        photo_id: PhotoId,
    ) -> Result<()> {
        let conn = self.lock()?;
        repository::collections::remove_photo(&conn, collection_id, photo_id)
    }

    /// Die Fotos einer Sammlung — bei einer intelligenten Sammlung live
    /// aus den gespeicherten Kriterien berechnet, sonst die festgelegte
    /// Reihenfolge.
    pub fn list_photos_in_collection(&self, collection_id: CollectionId) -> Result<Vec<Photo>> {
        let conn = self.lock()?;
        repository::collections::list_photos(&conn, collection_id)
    }

    // ---- Sammlungssätze (Phase 9 Schritt 1) --------------------------------

    pub fn create_collection_folder(
        &self,
        name: &str,
        parent_id: Option<CollectionFolderId>,
    ) -> Result<CollectionFolderId> {
        let conn = self.lock()?;
        repository::collections::create_folder(&conn, name, parent_id)
    }

    pub fn rename_collection_folder(&self, id: CollectionFolderId, name: &str) -> Result<()> {
        let conn = self.lock()?;
        repository::collections::rename_folder(&conn, id, name)
    }

    pub fn delete_collection_folder(&self, id: CollectionFolderId) -> Result<()> {
        let conn = self.lock()?;
        repository::collections::delete_folder(&conn, id)
    }

    pub fn list_collection_folders(&self) -> Result<Vec<CollectionFolder>> {
        let conn = self.lock()?;
        repository::collections::list_folders(&conn)
    }

    // ---- Virtuelle Kopien (Phase 9 Schritt 1) ------------------------------

    /// Legt eine virtuelle Kopie an — siehe
    /// [`repository::photos::create_virtual_copy`]s Moduldoku.
    pub fn create_virtual_copy(&self, source_id: PhotoId) -> Result<PhotoId> {
        let conn = self.lock()?;
        repository::photos::create_virtual_copy(&conn, source_id, OffsetDateTime::now_utc())
    }

    pub fn list_virtual_copies(&self, source_id: PhotoId) -> Result<Vec<Photo>> {
        let conn = self.lock()?;
        repository::photos::list_virtual_copies(&conn, source_id)
    }

    // ---- Stapel (Phase 9 Schritt 1) ----------------------------------------

    pub fn create_stack(&self, name: Option<&str>, photo_ids: &[PhotoId]) -> Result<StackId> {
        let conn = self.lock()?;
        repository::stacks::create(&conn, name, photo_ids, OffsetDateTime::now_utc())
    }

    pub fn delete_stack(&self, id: StackId) -> Result<()> {
        let conn = self.lock()?;
        repository::stacks::delete(&conn, id)
    }

    pub fn set_stack_cover(&self, id: StackId, cover_photo_id: PhotoId) -> Result<()> {
        let conn = self.lock()?;
        repository::stacks::set_cover(&conn, id, cover_photo_id)
    }

    pub fn list_stacks(&self) -> Result<Vec<Stack>> {
        let conn = self.lock()?;
        repository::stacks::list_all(&conn)
    }

    /// Gruppiert `photo_ids` automatisch in Stapel: aufeinanderfolgende
    /// Fotos (nach `captured_at` sortiert) innerhalb von `window_seconds`
    /// landen im selben Stapel. Fotos ohne `captured_at` bleiben
    /// unverstapelt (siehe [`repository::stacks::auto_stack_by_time`]).
    pub fn auto_stack_by_time(
        &self,
        photo_ids: &[PhotoId],
        window_seconds: i64,
    ) -> Result<Vec<StackId>> {
        let conn = self.lock()?;
        repository::stacks::auto_stack_by_time(
            &conn,
            photo_ids,
            window_seconds,
            OffsetDateTime::now_utc(),
        )
    }

    // ---- Erweiterbare Farbmarkierungen (Phase 9 Schritt 1) -----------------

    pub fn list_color_label_definitions(&self) -> Result<Vec<ColorLabelDefinition>> {
        let conn = self.lock()?;
        repository::color_labels::list_all(&conn)
    }

    pub fn create_color_label_definition(
        &self,
        name: &str,
        display_name: &str,
        hex: &str,
    ) -> Result<()> {
        let conn = self.lock()?;
        repository::color_labels::create(&conn, name, display_name, hex)
    }

    pub fn delete_color_label_definition(&self, name: &str) -> Result<()> {
        let conn = self.lock()?;
        repository::color_labels::delete(&conn, name)
    }

    // ---- Presets (ab Phase 5, siehe DECISIONS.md ADR-0031) -----------------

    pub fn create_preset_folder(
        &self,
        name: &str,
        parent_id: Option<PresetFolderId>,
    ) -> Result<PresetFolderId> {
        let conn = self.lock()?;
        repository::presets::create_folder(&conn, name, parent_id, OffsetDateTime::now_utc())
    }

    pub fn rename_preset_folder(&self, id: PresetFolderId, name: &str) -> Result<()> {
        let conn = self.lock()?;
        repository::presets::rename_folder(&conn, id, name)
    }

    /// Löscht einen Preset-Ordner — verschachtelte Unterordner fallen per
    /// Kaskade mit, enthaltene Presets bleiben erhalten und rutschen an
    /// die Wurzel (siehe [`repository::presets::delete_folder`]).
    pub fn delete_preset_folder(&self, id: PresetFolderId) -> Result<()> {
        let conn = self.lock()?;
        repository::presets::delete_folder(&conn, id)
    }

    pub fn list_preset_folders(&self) -> Result<Vec<PresetFolder>> {
        let conn = self.lock()?;
        repository::presets::list_folders(&conn)
    }

    /// Legt ein neues Preset samt seiner ersten Version an.
    /// `edl_subset_json`/`conditions_json` sind für `apx-catalog` opake
    /// JSON-Strings (siehe `repository::presets`s Moduldoku).
    pub fn create_preset(
        &self,
        folder_id: Option<PresetFolderId>,
        name: &str,
        tags: &[String],
        conditions_json: &str,
        edl_subset_json: &str,
    ) -> Result<(PresetId, PresetVersionId)> {
        let conn = self.lock()?;
        repository::presets::create(
            &conn,
            folder_id,
            name,
            tags,
            conditions_json,
            edl_subset_json,
            OffsetDateTime::now_utc(),
        )
    }

    /// Ändert Name/Ordner/Tags/Bedingungen eines Presets, ohne eine neue
    /// Version anzulegen — siehe [`Catalog::add_preset_version`] dafür.
    pub fn update_preset_metadata(
        &self,
        id: PresetId,
        folder_id: Option<PresetFolderId>,
        name: &str,
        tags: &[String],
        conditions_json: &str,
    ) -> Result<()> {
        let conn = self.lock()?;
        repository::presets::update_metadata(&conn, id, folder_id, name, tags, conditions_json)
    }

    pub fn set_preset_favorite(&self, id: PresetId, is_favorite: bool) -> Result<()> {
        let conn = self.lock()?;
        repository::presets::set_favorite(&conn, id, is_favorite)
    }

    pub fn delete_preset(&self, id: PresetId) -> Result<()> {
        let conn = self.lock()?;
        repository::presets::delete(&conn, id)
    }

    pub fn list_presets(&self) -> Result<Vec<Preset>> {
        let conn = self.lock()?;
        repository::presets::list_all(&conn)
    }

    /// Legt eine neue Version an (überschreibt die EDL-Teilmenge, ohne
    /// ältere Versionen zu löschen — siehe `repository::presets`s
    /// Moduldoku zur Versionierung ohne Undo/Redo-Zeiger).
    pub fn add_preset_version(
        &self,
        preset_id: PresetId,
        edl_subset_json: &str,
    ) -> Result<PresetVersionId> {
        let conn = self.lock()?;
        repository::presets::create_version(
            &conn,
            preset_id,
            edl_subset_json,
            OffsetDateTime::now_utc(),
        )
    }

    pub fn list_preset_versions(&self, preset_id: PresetId) -> Result<Vec<PresetVersion>> {
        let conn = self.lock()?;
        repository::presets::list_versions(&conn, preset_id)
    }

    /// Die aktuell gültige (zuletzt angelegte) Version eines Presets.
    pub fn latest_preset_version(&self, preset_id: PresetId) -> Result<PresetVersion> {
        let conn = self.lock()?;
        repository::presets::latest_version(&conn, preset_id)
    }

    // ---- Suche/Filter (ab Phase 3) -----------------------------------------

    /// Volltextsuche über Dateiname, Kamera und Objektiv (FTS5), siehe
    /// [`repository::search::search_photos`].
    pub fn search_photos(&self, query: &str) -> Result<Vec<Photo>> {
        let conn = self.lock()?;
        repository::search::search_photos(&conn, query)
    }

    /// Kombinierbarer Attributfilter (Bewertung/Flagge/Farbe/Kamera), siehe
    /// [`repository::search::filter_photos`].
    pub fn filter_photos(&self, criteria: &FilterCriteria) -> Result<Vec<Photo>> {
        let conn = self.lock()?;
        repository::search::filter_photos(&conn, criteria)
    }

    /// Kombiniert Volltextsuche (optional) und Attributfilter per UND —
    /// additiv zu [`Catalog::search_photos`]/[`Catalog::filter_photos`], die
    /// unverändert bestehen bleiben. Siehe `DECISIONS.md` ADR-0027 und
    /// [`repository::search::search_and_filter_photos`].
    pub fn search_and_filter_photos(
        &self,
        query: Option<&str>,
        criteria: &FilterCriteria,
    ) -> Result<Vec<Photo>> {
        let conn = self.lock()?;
        repository::search::search_and_filter_photos(&conn, query, criteria)
    }

    // ---- Duplikaterkennung (ab Phase 3, Schritt 8.2) -----------------------

    /// Gruppen von Fotos mit identischem Inhalt (exakter Hash-Vergleich),
    /// siehe `DECISIONS.md` ADR-0027 — reine Anzeige, verhindert den Import
    /// selbst nicht.
    pub fn list_duplicate_photo_groups(&self) -> Result<Vec<Vec<Photo>>> {
        let conn = self.lock()?;
        repository::photos::list_duplicate_groups(&conn)
    }

    // ---- Stapelverarbeitungs-Konsole (Phase 11 Schritt 9, siehe
    // DECISIONS.md ADR-0038) -------------------------------------------------

    /// Fotos, die `criteria` treffen würden — schreibt nichts.
    pub fn preview_batch_rule(&self, criteria: &FilterCriteria) -> Result<Vec<Photo>> {
        let conn = self.lock()?;
        repository::batch::preview_batch_rule(&conn, criteria)
    }

    /// Wendet `action` auf alle `criteria`-treffenden Fotos an und
    /// journalisiert jede tatsächliche Änderung — siehe
    /// `repository::batch`s Moduldoku.
    pub fn apply_batch_rule(
        &self,
        criteria: &FilterCriteria,
        action: &BatchAction,
    ) -> Result<BatchOperationId> {
        let conn = self.lock()?;
        repository::batch::apply_batch_rule(&conn, criteria, action, OffsetDateTime::now_utc())
    }

    /// Macht jede in `batch_id` journalisierte Änderung einzeln rückgängig.
    /// Gibt die Zahl tatsächlich rückgängig gemachter Änderungen zurück.
    pub fn undo_batch_operation(&self, batch_id: BatchOperationId) -> Result<usize> {
        let conn = self.lock()?;
        repository::batch::undo_batch_operation(&conn, batch_id)
    }

    // ---- Vorlagen (Phase 8 Schritt 8) --------------------------------------

    /// Legt eine neue Vorlage an — `kind` ist eine der Zeichenketten
    /// "export"/"print"/"book"/"slideshow"/"web"/"workflow",
    /// `payload_json` das jeweilige `*Options`-DTO als JSON.
    pub fn create_template(
        &self,
        kind: &str,
        name: &str,
        payload_json: &str,
    ) -> Result<TemplateId> {
        let conn = self.lock()?;
        repository::templates::create(&conn, kind, name, payload_json, OffsetDateTime::now_utc())
    }

    /// Alle Vorlagen einer Art, alphabetisch nach Namen.
    pub fn list_templates(&self, kind: &str) -> Result<Vec<Template>> {
        let conn = self.lock()?;
        repository::templates::list_by_kind(&conn, kind)
    }

    pub fn get_template(&self, id: TemplateId) -> Result<Template> {
        let conn = self.lock()?;
        repository::templates::get(&conn, id)
    }

    pub fn delete_template(&self, id: TemplateId) -> Result<()> {
        let conn = self.lock()?;
        repository::templates::delete(&conn, id)
    }

    // ---- Kollaborationsmodus (Phase 9 Schritt 10) --------------------------

    /// Findet ein lokales Foto anhand seines Inhalts-Hashes — der
    /// Matching-Schlüssel beim Import einer `.apxs`-Freigabedatei (siehe
    /// [`repository::photos::find_by_content_hash`]s Moduldoku). `None`,
    /// wenn kein lokales Foto denselben Inhalt hat.
    pub fn find_photo_by_content_hash(&self, hash: &str) -> Result<Option<Photo>> {
        let conn = self.lock()?;
        repository::photos::find_by_content_hash(&conn, hash)
    }

    /// Vergleicht den lokalen aktuellen Bearbeitungsstand eines Fotos mit
    /// einem importierten Stand (siehe [`ShareDiff`]) — reiner Vergleich,
    /// ändert nichts am Katalog.
    pub fn diff_share_edit(
        &self,
        local_edl: &EdlEnvelope,
        local_created_at: OffsetDateTime,
        incoming_edl: &EdlEnvelope,
        incoming_created_at: OffsetDateTime,
    ) -> ShareDiff {
        repository::share::diff_edit(
            local_edl,
            local_created_at,
            incoming_edl,
            incoming_created_at,
        )
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

    #[test]
    fn integrity_check_reports_no_problems_on_a_healthy_catalog() {
        let catalog = Catalog::open_in_memory().expect("sollte öffnen");
        let problems = catalog.integrity_check().expect("sollte laufen");
        assert!(
            problems.is_empty(),
            "frisch angelegter Katalog sollte keine Integritätsprobleme haben: {problems:?}"
        );
    }

    #[test]
    fn vacuum_runs_without_error_and_keeps_data_intact() {
        let catalog = Catalog::open_in_memory().expect("sollte öffnen");
        let folder_id = catalog
            .insert_folder(Path::new("/fotos"), None)
            .expect("ok");
        catalog.vacuum().expect("VACUUM sollte gelingen");
        let folder = catalog.get_folder(folder_id).expect("sollte noch da sein");
        assert_eq!(folder.path, PathBuf::from("/fotos"));
    }

    #[test]
    fn backup_to_produces_a_file_with_the_same_data() {
        let tmp = tempfile::tempdir().expect("Temp-Verzeichnis");
        let db_path = tmp.path().join("catalog.sqlite");
        let backup_path = tmp.path().join("backup.sqlite");

        let catalog = Catalog::open(&db_path).expect("sollte öffnen");
        let folder_id = catalog
            .insert_folder(Path::new("/fotos"), None)
            .expect("ok");
        catalog
            .backup_to(&backup_path)
            .expect("Backup sollte gelingen");
        assert!(backup_path.is_file());

        let restored = Catalog::open(&backup_path).expect("Backup sollte sich öffnen lassen");
        let folder = restored
            .get_folder(folder_id)
            .expect("Backup sollte denselben Ordner enthalten");
        assert_eq!(folder.path, PathBuf::from("/fotos"));
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

    /// Deckt Bewertung, Schlagworte und Sammlungen im Zusammenspiel über die
    /// öffentliche `Catalog`-API ab (Schritt 2 der Phase-3-Bibliothek).
    #[test]
    fn library_features_work_together_through_the_public_api() {
        let catalog = Catalog::open_in_memory().expect("sollte öffnen");
        let folder_id = catalog
            .insert_folder(Path::new("/fotos"), None)
            .expect("ok");
        let (photo_id, _) = catalog.upsert_photo(&sample_photo(folder_id)).expect("ok");

        catalog.set_photo_rating(photo_id, 4).expect("ok");
        catalog.set_photo_flag(photo_id, 1).expect("ok");
        catalog
            .set_photo_color_label(photo_id, Some("green"))
            .expect("ok");
        let photo = catalog.get_photo(photo_id).expect("ok");
        assert_eq!(photo.rating, 4);
        assert_eq!(photo.flag, 1);
        assert_eq!(photo.color_label.as_deref(), Some("green"));

        catalog.add_keyword(photo_id, "Testflug").expect("ok");
        assert_eq!(
            catalog.list_keywords_for_photo(photo_id).expect("ok").len(),
            1
        );

        let collection_id = catalog.create_collection("Favoriten", None).expect("ok");
        catalog
            .add_photo_to_collection(collection_id, photo_id)
            .expect("ok");
        assert_eq!(
            catalog
                .list_photos_in_collection(collection_id)
                .expect("ok")
                .len(),
            1
        );

        let found = catalog.search_photos("IMG_0001").expect("ok");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].id, photo_id);

        let filtered = catalog
            .filter_photos(&FilterCriteria {
                rating_at_least: Some(4),
                ..Default::default()
            })
            .expect("ok");
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].id, photo_id);
    }
}
