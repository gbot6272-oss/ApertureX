//! Import-Job: Ordner rekursiv scannen, Metadaten in den Katalog
//! schreiben, danach Thumbnails im Worker-Pool erzeugen. Siehe
//! `PHASE1_PROMPT.md` Abschnitt 5.
//!
//! Läuft komplett in `spawn_blocking` (siehe `commands.rs`) — walkdir,
//! RAW-Metadaten-Lesen und Bilddekodierung sind alles blockierende
//! Operationen, die den async-Runtime-Thread sonst einfrieren würden
//! (bekannter Fallstrick, siehe `PHASE1_PROMPT.md` Abschnitt 10).
//!
//! Die eigentliche Job-Logik (`run`) hängt bewusst nicht direkt an
//! `tauri::AppHandle`, sondern an der kleinen [`ImportEvents`]-Abstraktion
//! — das hält den Job in Tests ohne laufenden Tauri-Kontext prüfbar (siehe
//! `tests`-Modul unten, insbesondere den Akzeptanztest "3 gültige + 1
//! kaputte Datei" aus `PHASE1_PROMPT.md` Abschnitt 8).

pub(crate) mod mode;
pub(crate) mod presets;
mod rename;
mod thumbnails;

use std::collections::HashMap;
use std::io::BufReader;
use std::path::{Path, PathBuf};

use apx_catalog::{Catalog, NewPhoto};
use apx_core::{FolderId, PhotoId};
use serde::Serialize;
use sha2::{Digest, Sha256};
use tauri::{AppHandle, Emitter};
use time::OffsetDateTime;
use tokio_util::sync::CancellationToken;

pub(crate) use mode::ImportMode;

pub(super) const THUMBNAIL_EDGE: u32 = 256;

#[derive(Debug, Clone, Serialize)]
pub struct ImportProgressPayload {
    pub done: usize,
    pub total: usize,
    pub current_file: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ImportErrorPayload {
    pub file: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ImportFinishedPayload {
    pub imported: usize,
    pub skipped: usize,
    pub error_count: usize,
    pub cancelled: bool,
    /// Anzahl Fotos, die laut exaktem Inhalts-Hash (`content_hash`) ein
    /// Duplikat eines anderen Katalogeintrags sind — reine Anzeige, siehe
    /// `DECISIONS.md` ADR-0027. Verhindert den Import nicht.
    pub duplicate_count: usize,
}

/// Senke für Import-Fortschrittsereignisse. In der echten App ist das
/// [`TauriEvents`] (sendet Tauri-IPC-Events ans Frontend); Tests können
/// eine eigene, aufzeichnende Implementierung verwenden.
pub(super) trait ImportEvents: Sync {
    fn progress(&self, done: usize, total: usize, current_file: Option<&Path>);
    fn error(&self, file: &Path, message: &str);
    fn finished(
        &self,
        imported: usize,
        skipped: usize,
        error_count: usize,
        cancelled: bool,
        duplicate_count: usize,
    );
}

/// Schickt Fortschrittsereignisse als Tauri-IPC-Events ans Frontend:
/// `import:progress`, `import:error`, `import:finished` — siehe
/// `PHASE1_PROMPT.md` Abschnitt 5.
pub(super) struct TauriEvents<'a>(pub &'a AppHandle);

impl ImportEvents for TauriEvents<'_> {
    fn progress(&self, done: usize, total: usize, current_file: Option<&Path>) {
        let payload = ImportProgressPayload {
            done,
            total,
            current_file: current_file.map(|p| p.display().to_string()),
        };
        if let Err(err) = self.0.emit("import:progress", payload) {
            tracing::warn!(%err, "import:progress-Event konnte nicht gesendet werden");
        }
    }

    fn error(&self, file: &Path, message: &str) {
        let payload = ImportErrorPayload {
            file: file.display().to_string(),
            message: message.to_string(),
        };
        if let Err(err) = self.0.emit("import:error", payload) {
            tracing::warn!(%err, "import:error-Event konnte nicht gesendet werden");
        }
    }

    fn finished(
        &self,
        imported: usize,
        skipped: usize,
        error_count: usize,
        cancelled: bool,
        duplicate_count: usize,
    ) {
        let payload = ImportFinishedPayload {
            imported,
            skipped,
            error_count,
            cancelled,
            duplicate_count,
        };
        if let Err(err) = self.0.emit("import:finished", payload) {
            tracing::warn!(%err, "import:finished-Event konnte nicht gesendet werden");
        }
    }
}

/// Führt den gesamten Import-Job synchron aus (läuft im Aufrufer bereits
/// in `spawn_blocking`). Sendet währenddessen Fortschritts- und
/// Fehlerereignisse über `events`, am Ende genau ein Abschlussereignis.
///
/// Einzeldatei-Fehler werden ausschließlich über `events.error(...)` nach
/// außen gereicht (das Frontend sammelt sie dort) — hier im Job selbst
/// wird nur die Anzahl mitgezählt, für das Abschlussereignis und die
/// Entscheidung, wie viele Schritte die Fortschrittsanzeige insgesamt hat.
///
/// `mode` wählt den Import-Modus (ab Phase 3, siehe `import::mode`):
/// bei [`ImportMode::AddInPlace`] bleibt jede Datei, wo sie ist
/// (unveränderte Semantik seit Phase 1); bei [`ImportMode::Copy`]/
/// [`ImportMode::Move`] wird sie vor dem Katalogisieren in den Zielordner
/// kopiert/verschoben, optional nach `rename_pattern` umbenannt (siehe
/// `import::rename`).
pub(super) fn run_with_mode(
    events: &impl ImportEvents,
    catalog: &Catalog,
    cache_root: &Path,
    folder: &Path,
    cancel: &CancellationToken,
    mode: &ImportMode,
    rename_pattern: Option<&str>,
) {
    let entries = scan_supported_files(folder);
    let total_files = entries.len();

    // Wurzel der Ordnerbaum-Hierarchie, die beim Import nachgebildet wird
    // (siehe `ensure_folder`, `DECISIONS.md` ADR-0027): bei `AddInPlace`
    // der gescannte Ordner selbst; bei `Copy`/`Move` der Zielordner, weil
    // die Dateien danach dort liegen, nicht mehr im Quellordner.
    let hierarchy_root: &Path = match mode {
        ImportMode::AddInPlace => folder,
        ImportMode::Copy(dir) | ImportMode::Move(dir) => dir,
    };

    let mut folder_cache: HashMap<PathBuf, FolderId> = HashMap::new();
    let mut imported = 0usize;
    let mut skipped = 0usize;
    let mut error_count = 0usize;
    let mut thumbnail_targets: Vec<(PhotoId, PathBuf)> = Vec::new();

    events.progress(0, total_files, None);

    for (index, file_path) in entries.iter().enumerate() {
        if cancel.is_cancelled() {
            break;
        }
        events.progress(index, total_files, Some(file_path));

        match import_single_file(
            catalog,
            &mut folder_cache,
            file_path,
            hierarchy_root,
            mode,
            rename_pattern,
            index + 1,
        ) {
            Ok(SingleFileOutcome::Imported(photo_id, staged_path)) => {
                imported += 1;
                thumbnail_targets.push((photo_id, staged_path));
            }
            Ok(SingleFileOutcome::Unchanged) => skipped += 1,
            Err(message) => {
                events.error(file_path, &message);
                error_count += 1;
            }
        }
    }

    let cancelled_during_scan = cancel.is_cancelled();
    let scan_done = entries.len().min(imported + skipped + error_count);
    let total_steps = total_files + thumbnail_targets.len();

    if !cancelled_during_scan {
        error_count += thumbnails::generate(
            events,
            catalog,
            cache_root,
            cancel,
            &thumbnail_targets,
            scan_done,
            total_steps,
        );
    }

    // Duplikaterkennung per exaktem Hash (Schritt 8.2, ADR-0027): rein
    // informativ, läuft über den kompletten Katalog (nicht nur die eben
    // importierten Dateien), damit auch Duplikate über mehrere
    // Import-Läufe hinweg gefunden werden. Ein Fehler hier (SQL-Problem)
    // darf den Import selbst nicht scheitern lassen.
    let duplicate_count = catalog
        .list_duplicate_photo_groups()
        .map(|groups| groups.iter().map(Vec::len).sum())
        .unwrap_or_else(|err| {
            tracing::warn!(%err, "Duplikatgruppen konnten nicht ermittelt werden");
            0
        });

    events.progress(total_steps, total_steps, None);
    events.finished(
        imported,
        skipped,
        error_count,
        cancel.is_cancelled(),
        duplicate_count,
    );

    tracing::info!(
        imported,
        skipped,
        errors = error_count,
        cancelled = cancel.is_cancelled(),
        duplicate_count,
        "Import abgeschlossen"
    );
}

enum SingleFileOutcome {
    /// Enthält zusätzlich den tatsächlichen Speicherort — bei Copy/Move
    /// unterscheidet sich der vom ursprünglich gescannten `file_path`, und
    /// die Thumbnail-Erzeugung muss vom neuen Ort lesen.
    Imported(PhotoId, PathBuf),
    Unchanged,
}

fn scan_supported_files(folder: &Path) -> Vec<PathBuf> {
    walkdir::WalkDir::new(folder)
        .into_iter()
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_type().is_file())
        .map(|entry| entry.into_path())
        .filter(|path| apx_raw::is_supported_extension(path))
        .collect()
}

/// Findet den Katalog-Ordnereintrag für `dir` oder legt ihn an — inklusive
/// aller Elternordner bis (und mit) `hierarchy_root`, deren `parent_id`
/// dadurch eine korrekte Mehrebenen-Kette bildet (Schritt 8.5,
/// `DECISIONS.md` ADR-0027). `hierarchy_root` selbst bekommt `parent_id =
/// None`. Liegt `dir` unerwartet nicht unterhalb von `hierarchy_root`
/// (z. B. bei einem aus dem Baum herausführenden Symlink), wird defensiv
/// kein Elternteil gesetzt statt bis zum Dateisystem-Wurzelverzeichnis zu
/// rekursieren.
fn ensure_folder(
    catalog: &Catalog,
    dir: &Path,
    hierarchy_root: &Path,
    cache: &mut HashMap<PathBuf, FolderId>,
) -> Result<FolderId, String> {
    if let Some(id) = cache.get(dir) {
        return Ok(*id);
    }

    let parent_id = if dir != hierarchy_root && dir.starts_with(hierarchy_root) {
        dir.parent()
            .map(|parent| ensure_folder(catalog, parent, hierarchy_root, cache))
            .transpose()?
    } else {
        None
    };

    let id = catalog
        .find_or_create_folder(dir, parent_id)
        .map_err(|err| err.to_string())?;
    cache.insert(dir.to_path_buf(), id);
    Ok(id)
}

/// Berechnet den SHA-256-Hash einer Datei per Streaming (`BufReader` +
/// `std::io::copy` direkt in den Hasher) — liest die Datei dafür nie
/// vollständig auf einmal ein, auch nicht bei mehreren hundert MB großen
/// RAW-Dateien. Grundlage für die exakte Duplikaterkennung (Schritt 8.2,
/// `DECISIONS.md` ADR-0027).
pub(crate) fn compute_content_hash(path: &Path) -> Result<String, String> {
    let file = std::fs::File::open(path)
        .map_err(|err| format!("Datei für Hash-Berechnung nicht lesbar: {err}"))?;
    let mut reader = BufReader::new(file);
    let mut hasher = Sha256::new();
    std::io::copy(&mut reader, &mut hasher)
        .map_err(|err| format!("Hash-Berechnung fehlgeschlagen: {err}"))?;
    Ok(format!("{:x}", hasher.finalize()))
}

/// Importiert eine einzelne Datei: Ordner sicherstellen, Metadaten lesen,
/// Katalogzeile anlegen/aktualisieren. Ein Fehler hier betrifft nur diese
/// eine Datei — der Aufrufer sammelt ihn und macht mit der nächsten Datei
/// weiter (siehe Modul-Doku).
///
/// Die volle Ordner-Hierarchie zwischen `hierarchy_root` und der Datei wird
/// als mehrstufige `parent_id`-Kette angelegt (siehe [`ensure_folder`]).
fn import_single_file(
    catalog: &Catalog,
    folder_cache: &mut HashMap<PathBuf, FolderId>,
    path: &Path,
    hierarchy_root: &Path,
    mode: &ImportMode,
    rename_pattern: Option<&str>,
    seq: usize,
) -> Result<SingleFileOutcome, String> {
    // Metadaten werden immer vom *ursprünglichen* Pfad gelesen: bei
    // ImportMode::Move existiert die Quelldatei nach dem Staging unten
    // nicht mehr, ein zweiter Lesezugriff wäre also ohnehin unmöglich —
    // und für Copy/AddInPlace liefert das identische Bytes wie ein
    // Lesen von der Kopie.
    let raw_meta = apx_raw::read_metadata(path).map_err(|err| err.to_string())?;

    let staged_path = mode::stage_file_for_mode(
        mode,
        path,
        rename_pattern,
        seq,
        non_empty_ref(&raw_meta.camera_model),
        raw_meta.captured_at,
        OffsetDateTime::now_utc(),
    )?;

    let parent = staged_path
        .parent()
        .ok_or_else(|| "Datei hat kein Elternverzeichnis".to_string())?;
    let folder_id = ensure_folder(catalog, parent, hierarchy_root, folder_cache)?;

    let filename = staged_path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| "Dateiname ist kein gültiges UTF-8".to_string())?
        .to_string();

    let fs_meta = std::fs::metadata(&staged_path)
        .map_err(|err| format!("Dateiinformationen nicht lesbar: {err}"))?;
    let file_size = fs_meta.len();
    let file_mtime: OffsetDateTime = fs_meta
        .modified()
        .map_err(|err| format!("Änderungszeit nicht lesbar: {err}"))?
        .into();
    let content_hash = compute_content_hash(&staged_path)?;

    let new_photo = NewPhoto {
        folder_id,
        filename,
        file_size,
        file_mtime,
        content_hash: Some(content_hash),
        width: Some(raw_meta.width),
        height: Some(raw_meta.height),
        orientation: orientation_to_exif_code(raw_meta.orientation),
        camera_make: non_empty(raw_meta.camera_make),
        camera_model: non_empty(raw_meta.camera_model),
        lens: raw_meta.lens,
        iso: raw_meta.iso,
        shutter: raw_meta.shutter,
        aperture: raw_meta.aperture,
        focal_length: raw_meta.focal_length,
        captured_at: raw_meta.captured_at,
        gps_lat: raw_meta.gps.map(|(lat, _)| lat),
        gps_lon: raw_meta.gps.map(|(_, lon)| lon),
    };

    let (photo_id, changed) = catalog
        .upsert_photo(&new_photo)
        .map_err(|err| err.to_string())?;
    Ok(if changed {
        SingleFileOutcome::Imported(photo_id, staged_path)
    } else {
        SingleFileOutcome::Unchanged
    })
}

fn non_empty(value: String) -> Option<String> {
    if value.trim().is_empty() {
        None
    } else {
        Some(value)
    }
}

/// Wie [`non_empty`], aber ohne den `String` zu konsumieren — für
/// [`mode::stage_file_for_mode`]s `camera`-Token-Parameter, das nur einen
/// `&str` braucht.
fn non_empty_ref(value: &str) -> Option<&str> {
    if value.trim().is_empty() {
        None
    } else {
        Some(value)
    }
}

/// Wandelt `apx_raw::Orientation` zurück in den numerischen EXIF-Code
/// (1–8), wie er in der `photos.orientation`-Spalte gespeichert wird.
/// Muss zur Umkehrung von `rawler::decoders::Orientation::from_u16`
/// passen (siehe `apx-raw`s `orientation`-Modul).
fn orientation_to_exif_code(orientation: apx_raw::Orientation) -> u16 {
    use apx_raw::Orientation;
    match orientation {
        Orientation::Normal => 1,
        Orientation::FlipHorizontal => 2,
        Orientation::Rotate180 => 3,
        Orientation::FlipVertical => 4,
        Orientation::Transpose => 5,
        Orientation::Rotate90 => 6,
        Orientation::Transverse => 7,
        Orientation::Rotate270 => 8,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    #[test]
    fn non_empty_filters_blank_strings() {
        assert_eq!(non_empty("".to_string()), None);
        assert_eq!(non_empty("   ".to_string()), None);
        assert_eq!(non_empty("Canon".to_string()), Some("Canon".to_string()));
    }

    #[test]
    fn orientation_round_trips_through_exif_codes() {
        use apx_raw::Orientation;
        let all = [
            (Orientation::Normal, 1),
            (Orientation::FlipHorizontal, 2),
            (Orientation::Rotate180, 3),
            (Orientation::FlipVertical, 4),
            (Orientation::Transpose, 5),
            (Orientation::Rotate90, 6),
            (Orientation::Transverse, 7),
            (Orientation::Rotate270, 8),
        ];
        for (orientation, code) in all {
            assert_eq!(orientation_to_exif_code(orientation), code);
        }
    }

    #[test]
    fn scan_supported_files_filters_by_extension_and_recurses() {
        let tmp = tempfile::tempdir().expect("Temp-Verzeichnis");
        std::fs::write(tmp.path().join("a.cr2"), b"x").expect("Datei schreibbar");
        std::fs::write(tmp.path().join("notiz.txt"), b"x").expect("Datei schreibbar");
        let sub = tmp.path().join("unterordner");
        std::fs::create_dir(&sub).expect("Unterordner anlegbar");
        std::fs::write(sub.join("b.dng"), b"x").expect("Datei schreibbar");

        let files = scan_supported_files(tmp.path());
        assert_eq!(
            files.len(),
            2,
            "nur .cr2 und .dng, nicht .txt, aber rekursiv"
        );
    }

    #[test]
    fn import_single_file_reports_error_for_broken_file_without_panicking() {
        let tmp = tempfile::tempdir().expect("Temp-Verzeichnis");
        let catalog = Catalog::open_in_memory().expect("Katalog öffnen");
        let broken = tmp.path().join("kaputt.cr2");
        std::fs::write(&broken, b"das ist kein gueltiges CR2").expect("Datei schreibbar");

        let mut cache = HashMap::new();
        let result = import_single_file(
            &catalog,
            &mut cache,
            &broken,
            tmp.path(),
            &ImportMode::AddInPlace,
            None,
            1,
        );
        assert!(
            result.is_err(),
            "kaputte Datei muss einen Fehler liefern, nicht panicken"
        );
    }

    /// Zeichnet alle Events auf statt sie über Tauri zu senden — macht
    /// `run()` in Tests ohne laufenden Tauri-Kontext prüfbar.
    #[derive(Default)]
    #[allow(clippy::type_complexity)]
    struct RecordingEvents {
        errors: Mutex<Vec<(PathBuf, String)>>,
        finished: Mutex<Option<(usize, usize, usize, bool, usize)>>,
    }

    impl ImportEvents for RecordingEvents {
        fn progress(&self, _done: usize, _total: usize, _current_file: Option<&Path>) {}

        fn error(&self, file: &Path, message: &str) {
            self.errors
                .lock()
                .expect("Mutex")
                .push((file.to_path_buf(), message.to_string()));
        }

        fn finished(
            &self,
            imported: usize,
            skipped: usize,
            error_count: usize,
            cancelled: bool,
            duplicate_count: usize,
        ) {
            *self.finished.lock().expect("Mutex") =
                Some((imported, skipped, error_count, cancelled, duplicate_count));
        }
    }

    /// Erzeugt eine minimale, aber gültige JPEG-Datei — genug, damit
    /// `apx_raw::read_metadata`/`decode` (Fallback-Pfad, kein RAW nötig)
    /// sie erfolgreich verarbeitet. Für echte RAW-Formate fehlen laut
    /// `DECISIONS.md` ADR-0007 noch Testdateien (Netzwerkzugriff auf
    /// raw.pixls.us in dieser Umgebung blockiert); der Import-Job selbst
    /// ist formatunabhängig und wird hier vollständig durchgetestet.
    ///
    /// Handverlesene, deutlich unterscheidbare Farben für [`write_valid_jpeg`]
    /// — die Differenzen liegen bewusst weit über der JPEG-Quantisierungs-
    /// schwelle (ein einzelner Helligkeitsschritt würde nach verlustbehafteter
    /// Kompression manchmal auf denselben Wert runden, siehe die zunächst
    /// fehlgeschlagene Fassung dieses Tests), damit unterschiedliche
    /// `variant`-Werte garantiert unterschiedliche komprimierte Bytes (und
    /// damit unterschiedliche `content_hash`-Werte) ergeben.
    const VARIANT_PALETTE: [[u8; 3]; 10] = [
        [120, 80, 40],
        [40, 200, 90],
        [200, 40, 160],
        [10, 10, 220],
        [220, 180, 10],
        [90, 220, 220],
        [180, 90, 10],
        [10, 220, 10],
        [220, 10, 10],
        [140, 140, 220],
    ];

    /// Erzeugt eine minimale, aber gültige JPEG-Datei — genug, damit
    /// `apx_raw::read_metadata`/`decode` (Fallback-Pfad, kein RAW nötig)
    /// sie erfolgreich verarbeitet. Für echte RAW-Formate fehlen laut
    /// `DECISIONS.md` ADR-0007 noch Testdateien (Netzwerkzugriff auf
    /// raw.pixls.us in dieser Umgebung blockiert); der Import-Job selbst
    /// ist formatunabhängig und wird hier vollständig durchgetestet.
    ///
    /// `variant` wählt eine Farbe aus [`VARIANT_PALETTE`] — derselbe Wert
    /// erzeugt inhaltlich identische Dateien (gleicher `content_hash`, siehe
    /// `duplicate_photos_are_detected_by_content_hash`), unterschiedliche
    /// Werte inhaltlich unterschiedliche.
    fn write_valid_jpeg(path: &Path, variant: u8) {
        let color = VARIANT_PALETTE[variant as usize % VARIANT_PALETTE.len()];
        let image = image::RgbImage::from_pixel(32, 24, image::Rgb(color));
        image::DynamicImage::ImageRgb8(image)
            .save_with_format(path, image::ImageFormat::Jpeg)
            .expect("Test-JPEG sollte sich speichern lassen");
    }

    /// Akzeptanztest aus `PHASE1_PROMPT.md` Abschnitt 8: Ordner mit 3
    /// gültigen und 1 kaputten Datei → 3 Fotos importiert, 1 Fehler
    /// gemeldet, Job abgeschlossen (kein Absturz, `finished`-Event kommt).
    #[test]
    fn import_run_handles_three_valid_and_one_broken_file() {
        let tmp = tempfile::tempdir().expect("Temp-Verzeichnis");
        for (variant, name) in ["a.jpg", "b.jpg", "c.jpg"].into_iter().enumerate() {
            write_valid_jpeg(&tmp.path().join(name), variant as u8);
        }
        std::fs::write(
            tmp.path().join("kaputt.jpg"),
            b"das ist kein gueltiges JPEG",
        )
        .expect("Datei schreibbar");

        let catalog = Catalog::open_in_memory().expect("Katalog öffnen");
        let cache_root = tmp.path().join("cache");
        let events = RecordingEvents::default();
        let cancel = CancellationToken::new();

        run_with_mode(
            &events,
            &catalog,
            &cache_root,
            tmp.path(),
            &cancel,
            &ImportMode::AddInPlace,
            None,
        );

        let finished = events
            .finished
            .lock()
            .expect("Mutex")
            .expect("finished() muss aufgerufen worden sein");
        let (imported, skipped, error_count, cancelled, duplicate_count) = finished;
        assert_eq!(imported, 3, "3 gültige Dateien müssen importiert werden");
        assert_eq!(skipped, 0);
        assert_eq!(error_count, 1, "genau 1 Fehler für die kaputte Datei");
        assert!(!cancelled);
        assert_eq!(
            duplicate_count, 0,
            "drei unterschiedliche Bilder sind keine Duplikate"
        );

        let recorded_errors = events.errors.lock().expect("Mutex");
        assert_eq!(recorded_errors.len(), 1);
        assert!(recorded_errors[0].0.ends_with("kaputt.jpg"));

        // Katalog muss die 3 gültigen Fotos tatsächlich enthalten.
        let folders = catalog.list_folders().expect("Ordner lesbar");
        assert_eq!(
            folders.len(),
            1,
            "alle drei Dateien liegen im selben Verzeichnis"
        );
        assert_eq!(
            folders[0].parent_id, None,
            "der gewählte Import-Ordner selbst ist die Hierarchie-Wurzel, hat also keinen Elternordner"
        );
        assert_eq!(
            catalog
                .count_photos_in_folder(folders[0].id)
                .expect("Anzahl lesbar"),
            3
        );
    }

    /// Belegt Schritt 8.2 (`DECISIONS.md` ADR-0027): zwei Dateien mit
    /// exakt identischem Inhalt (gleiches `variant`, siehe
    /// `write_valid_jpeg`) werden beide importiert (kein Ausschluss aus dem
    /// Katalog), aber als Duplikatgruppe erkannt — sowohl im
    /// Abschlussereignis als auch über `Catalog::list_duplicate_photo_groups`.
    #[test]
    fn duplicate_photos_are_detected_by_content_hash() {
        let tmp = tempfile::tempdir().expect("Temp-Verzeichnis");
        write_valid_jpeg(&tmp.path().join("original.jpg"), 7);
        write_valid_jpeg(&tmp.path().join("kopie.jpg"), 7);
        write_valid_jpeg(&tmp.path().join("einzelstueck.jpg"), 9);

        let catalog = Catalog::open_in_memory().expect("Katalog öffnen");
        let cache_root = tmp.path().join("cache");
        let events = RecordingEvents::default();
        let cancel = CancellationToken::new();

        run_with_mode(
            &events,
            &catalog,
            &cache_root,
            tmp.path(),
            &cancel,
            &ImportMode::AddInPlace,
            None,
        );

        let finished = events
            .finished
            .lock()
            .expect("Mutex")
            .expect("finished() muss aufgerufen worden sein");
        let (imported, _, _, _, duplicate_count) = finished;
        assert_eq!(imported, 3, "alle drei Dateien werden importiert");
        assert_eq!(
            duplicate_count, 2,
            "genau die zwei inhaltsgleichen Dateien zählen als Duplikate"
        );

        let groups = catalog
            .list_duplicate_photo_groups()
            .expect("Duplikatgruppen lesbar");
        assert_eq!(groups.len(), 1, "genau eine Duplikatgruppe");
        assert_eq!(groups[0].len(), 2);
        let mut filenames: Vec<&str> = groups[0].iter().map(|p| p.filename.as_str()).collect();
        filenames.sort_unstable();
        assert_eq!(filenames, vec!["kopie.jpg", "original.jpg"]);
    }

    /// Belegt Schritt 8.5 (`DECISIONS.md` ADR-0027): mehrstufige
    /// Unterordner unter dem gewählten Import-Ordner ergeben eine korrekte
    /// mehrstufige `parent_id`-Kette bis zur Hierarchie-Wurzel.
    #[test]
    fn nested_subfolders_form_a_multi_level_parent_chain() {
        let tmp = tempfile::tempdir().expect("Temp-Verzeichnis");
        let root = tmp.path().join("quelle");
        let level1 = root.join("2024");
        let level2 = level1.join("urlaub");
        std::fs::create_dir_all(&level2).expect("Unterordner anlegbar");
        write_valid_jpeg(&level2.join("bild.jpg"), 0);

        let catalog = Catalog::open_in_memory().expect("Katalog öffnen");
        let cache_root = tmp.path().join("cache");
        let cancel = CancellationToken::new();

        run_with_mode(
            &RecordingEvents::default(),
            &catalog,
            &cache_root,
            &root,
            &cancel,
            &ImportMode::AddInPlace,
            None,
        );

        let folders = catalog.list_folders().expect("Ordner lesbar");
        assert_eq!(
            folders.len(),
            3,
            "Wurzel, `2024` und `2024/urlaub` müssen alle drei angelegt werden"
        );

        let by_path = |path: &Path| {
            folders
                .iter()
                .find(|f| f.path == path)
                .unwrap_or_else(|| panic!("Ordner {} muss existieren", path.display()))
        };
        let root_folder = by_path(&root);
        let level1_folder = by_path(&level1);
        let level2_folder = by_path(&level2);

        assert_eq!(
            root_folder.parent_id, None,
            "der Import-Ordner selbst ist die Hierarchie-Wurzel"
        );
        assert_eq!(level1_folder.parent_id, Some(root_folder.id));
        assert_eq!(level2_folder.parent_id, Some(level1_folder.id));
    }

    #[test]
    fn import_run_is_idempotent_on_second_pass() {
        let tmp = tempfile::tempdir().expect("Temp-Verzeichnis");
        write_valid_jpeg(&tmp.path().join("a.jpg"), 0);

        let catalog = Catalog::open_in_memory().expect("Katalog öffnen");
        let cache_root = tmp.path().join("cache");
        let cancel = CancellationToken::new();

        run_with_mode(
            &RecordingEvents::default(),
            &catalog,
            &cache_root,
            tmp.path(),
            &cancel,
            &ImportMode::AddInPlace,
            None,
        );
        run_with_mode(
            &RecordingEvents::default(),
            &catalog,
            &cache_root,
            tmp.path(),
            &cancel,
            &ImportMode::AddInPlace,
            None,
        );

        let folders = catalog.list_folders().expect("Ordner lesbar");
        assert_eq!(
            catalog
                .count_photos_in_folder(folders[0].id)
                .expect("Anzahl lesbar"),
            1,
            "zweiter Import derselben unveränderten Datei darf kein Duplikat anlegen"
        );
    }

    /// Belegt Schritt 4 der Phase-3-Bibliothek End-to-End: `Copy`-Modus mit
    /// Umbenennungsmuster kopiert die Quelldatei (die im Quellordner
    /// erhalten bleibt) unter neuem Namen in den Zielordner, und der
    /// Katalogeintrag zeigt auf den neuen Ort samt neuem Dateinamen.
    #[test]
    fn copy_mode_with_rename_pattern_relocates_and_renames_while_keeping_source() {
        let tmp = tempfile::tempdir().expect("Temp-Verzeichnis");
        let source_dir = tmp.path().join("quelle");
        std::fs::create_dir(&source_dir).expect("Quellordner anlegbar");
        write_valid_jpeg(&source_dir.join("IMG_0001.jpg"), 0);
        let target_dir = tmp.path().join("ziel");

        let catalog = Catalog::open_in_memory().expect("Katalog öffnen");
        let cache_root = tmp.path().join("cache");
        let cancel = CancellationToken::new();

        run_with_mode(
            &RecordingEvents::default(),
            &catalog,
            &cache_root,
            &source_dir,
            &cancel,
            &ImportMode::Copy(target_dir.clone()),
            Some("bild_{seq}"),
        );

        assert!(
            source_dir.join("IMG_0001.jpg").exists(),
            "Copy darf die Quelldatei nicht entfernen"
        );
        assert!(
            target_dir.join("bild_0001.jpg").exists(),
            "Zieldatei mit umbenanntem Namen muss existieren"
        );

        let folders = catalog.list_folders().expect("Ordner lesbar");
        assert_eq!(folders.len(), 1, "Foto gehört zum Zielordner");
        assert_eq!(folders[0].path, target_dir);
        let photos = catalog
            .list_photos_by_folder(folders[0].id)
            .expect("Fotos lesbar");
        assert_eq!(photos.len(), 1);
        assert_eq!(photos[0].filename, "bild_0001.jpg");
    }
}
