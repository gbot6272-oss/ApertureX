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

mod thumbnails;

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use apx_catalog::{Catalog, NewPhoto};
use apx_core::{FolderId, PhotoId};
use serde::Serialize;
use tauri::{AppHandle, Emitter};
use time::OffsetDateTime;
use tokio_util::sync::CancellationToken;

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
}

/// Senke für Import-Fortschrittsereignisse. In der echten App ist das
/// [`TauriEvents`] (sendet Tauri-IPC-Events ans Frontend); Tests können
/// eine eigene, aufzeichnende Implementierung verwenden.
pub(super) trait ImportEvents: Sync {
    fn progress(&self, done: usize, total: usize, current_file: Option<&Path>);
    fn error(&self, file: &Path, message: &str);
    fn finished(&self, imported: usize, skipped: usize, error_count: usize, cancelled: bool);
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

    fn finished(&self, imported: usize, skipped: usize, error_count: usize, cancelled: bool) {
        let payload = ImportFinishedPayload {
            imported,
            skipped,
            error_count,
            cancelled,
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
pub(super) fn run(
    events: &impl ImportEvents,
    catalog: &Catalog,
    cache_root: &Path,
    folder: &Path,
    cancel: &CancellationToken,
) {
    let entries = scan_supported_files(folder);
    let total_files = entries.len();

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

        match import_single_file(catalog, &mut folder_cache, file_path) {
            Ok(SingleFileOutcome::Imported(photo_id)) => {
                imported += 1;
                thumbnail_targets.push((photo_id, file_path.clone()));
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

    events.progress(total_steps, total_steps, None);
    events.finished(imported, skipped, error_count, cancel.is_cancelled());

    tracing::info!(
        imported,
        skipped,
        errors = error_count,
        cancelled = cancel.is_cancelled(),
        "Import abgeschlossen"
    );
}

enum SingleFileOutcome {
    Imported(PhotoId),
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

fn ensure_folder(
    catalog: &Catalog,
    dir: &Path,
    cache: &mut HashMap<PathBuf, FolderId>,
) -> Result<FolderId, String> {
    if let Some(id) = cache.get(dir) {
        return Ok(*id);
    }
    let id = catalog
        .find_or_create_folder(dir, None)
        .map_err(|err| err.to_string())?;
    cache.insert(dir.to_path_buf(), id);
    Ok(id)
}

/// Importiert eine einzelne Datei: Ordner sicherstellen, Metadaten lesen,
/// Katalogzeile anlegen/aktualisieren. Ein Fehler hier betrifft nur diese
/// eine Datei — der Aufrufer sammelt ihn und macht mit der nächsten Datei
/// weiter (siehe Modul-Doku).
///
/// **Hinweis zur Ordner-Hierarchie:** Jede Datei wird ihrem unmittelbaren
/// Elternverzeichnis zugeordnet, ohne die volle Verzeichniskette bis zum
/// gewählten Import-Ordner als `parent_id`-Kette nachzubilden. Der volle
/// Ordnerbaum mit Synchronisation ist ein Phase-3-Feature (siehe
/// `FEATURES.md`); Phase 1 braucht nur eine korrekte flache Zuordnung
/// Foto → Verzeichnis.
fn import_single_file(
    catalog: &Catalog,
    folder_cache: &mut HashMap<PathBuf, FolderId>,
    path: &Path,
) -> Result<SingleFileOutcome, String> {
    let parent = path
        .parent()
        .ok_or_else(|| "Datei hat kein Elternverzeichnis".to_string())?;
    let folder_id = ensure_folder(catalog, parent, folder_cache)?;

    let filename = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| "Dateiname ist kein gültiges UTF-8".to_string())?
        .to_string();

    let fs_meta =
        std::fs::metadata(path).map_err(|err| format!("Dateiinformationen nicht lesbar: {err}"))?;
    let file_size = fs_meta.len();
    let file_mtime: OffsetDateTime = fs_meta
        .modified()
        .map_err(|err| format!("Änderungszeit nicht lesbar: {err}"))?
        .into();

    let raw_meta = apx_raw::read_metadata(path).map_err(|err| err.to_string())?;

    let new_photo = NewPhoto {
        folder_id,
        filename,
        file_size,
        file_mtime,
        content_hash: None, // Hash-basierte Duplikaterkennung ist Phase 3.
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
        SingleFileOutcome::Imported(photo_id)
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
        let result = import_single_file(&catalog, &mut cache, &broken);
        assert!(
            result.is_err(),
            "kaputte Datei muss einen Fehler liefern, nicht panicken"
        );
    }

    /// Zeichnet alle Events auf statt sie über Tauri zu senden — macht
    /// `run()` in Tests ohne laufenden Tauri-Kontext prüfbar.
    #[derive(Default)]
    struct RecordingEvents {
        errors: Mutex<Vec<(PathBuf, String)>>,
        finished: Mutex<Option<(usize, usize, usize, bool)>>,
    }

    impl ImportEvents for RecordingEvents {
        fn progress(&self, _done: usize, _total: usize, _current_file: Option<&Path>) {}

        fn error(&self, file: &Path, message: &str) {
            self.errors
                .lock()
                .expect("Mutex")
                .push((file.to_path_buf(), message.to_string()));
        }

        fn finished(&self, imported: usize, skipped: usize, error_count: usize, cancelled: bool) {
            *self.finished.lock().expect("Mutex") =
                Some((imported, skipped, error_count, cancelled));
        }
    }

    /// Erzeugt eine minimale, aber gültige JPEG-Datei — genug, damit
    /// `apx_raw::read_metadata`/`decode` (Fallback-Pfad, kein RAW nötig)
    /// sie erfolgreich verarbeitet. Für echte RAW-Formate fehlen laut
    /// `DECISIONS.md` ADR-0007 noch Testdateien (Netzwerkzugriff auf
    /// raw.pixls.us in dieser Umgebung blockiert); der Import-Job selbst
    /// ist formatunabhängig und wird hier vollständig durchgetestet.
    fn write_valid_jpeg(path: &Path) {
        let image = image::RgbImage::from_pixel(32, 24, image::Rgb([120, 80, 40]));
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
        for name in ["a.jpg", "b.jpg", "c.jpg"] {
            write_valid_jpeg(&tmp.path().join(name));
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

        run(&events, &catalog, &cache_root, tmp.path(), &cancel);

        let finished = events
            .finished
            .lock()
            .expect("Mutex")
            .expect("finished() muss aufgerufen worden sein");
        let (imported, skipped, error_count, cancelled) = finished;
        assert_eq!(imported, 3, "3 gültige Dateien müssen importiert werden");
        assert_eq!(skipped, 0);
        assert_eq!(error_count, 1, "genau 1 Fehler für die kaputte Datei");
        assert!(!cancelled);

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
            catalog
                .count_photos_in_folder(folders[0].id)
                .expect("Anzahl lesbar"),
            3
        );
    }

    #[test]
    fn import_run_is_idempotent_on_second_pass() {
        let tmp = tempfile::tempdir().expect("Temp-Verzeichnis");
        write_valid_jpeg(&tmp.path().join("a.jpg"));

        let catalog = Catalog::open_in_memory().expect("Katalog öffnen");
        let cache_root = tmp.path().join("cache");
        let cancel = CancellationToken::new();

        run(
            &RecordingEvents::default(),
            &catalog,
            &cache_root,
            tmp.path(),
            &cancel,
        );
        run(
            &RecordingEvents::default(),
            &catalog,
            &cache_root,
            tmp.path(),
            &cancel,
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
}
