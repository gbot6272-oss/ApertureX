//! Gleicht den `missing`-Status von Fotos mit der tatsächlichen
//! Existenz ihrer Originaldatei im Dateisystem ab.
//!
//! Siehe `PHASE1_PROMPT.md` Abschnitt 9, Akzeptanzkriterium 8: „Wird
//! eine Datei außerhalb der App gelöscht, markiert die App sie beim
//! nächsten Öffnen als `missing` und stürzt nicht ab." Der Katalog
//! selbst kennt `Catalog::set_photo_missing` schon länger (siehe
//! `apx_catalog::Catalog`) — dieses Modul ist die fehlende Verdrahtung,
//! die diese Methode beim Öffnen eines Ordners (`list_photos_in_folder`)
//! tatsächlich aufruft.

use std::path::Path;

use apx_catalog::{Catalog, Photo};
use apx_core::Result;

/// Prüft für jedes übergebene Foto, ob `folder_path.join(&photo.filename)`
/// noch existiert, und schreibt Abweichungen vom gespeicherten
/// `missing`-Status in den Katalog zurück. Gibt dieselben Fotos mit
/// aktualisiertem `missing`-Feld zurück, damit der Aufrufer nicht ein
/// zweites Mal aus dem Katalog lesen muss.
///
/// Reine Dateisystem-`exists()`-Prüfungen — kein Öffnen, kein Dekodieren
/// — deshalb günstig genug, um bei jedem Öffnen eines Ordners zu laufen,
/// auch bei sehr vielen Fotos.
pub fn reconcile_missing(
    catalog: &Catalog,
    folder_path: &Path,
    photos: Vec<Photo>,
) -> Result<Vec<Photo>> {
    photos
        .into_iter()
        .map(|mut photo| {
            let should_be_missing = !folder_path.join(&photo.filename).exists();
            if should_be_missing != photo.missing {
                catalog.set_photo_missing(photo.id, should_be_missing)?;
                photo.missing = should_be_missing;
            }
            Ok(photo)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::fs;

    use apx_catalog::NewPhoto;
    use apx_core::FolderId;
    use tempfile::tempdir;

    use super::*;

    /// Baut ein `NewPhoto` mit Platzhalterwerten für alle Felder außer
    /// `filename` — `folder_id` wird per Struct-Update-Syntax vom
    /// Aufrufer überschrieben, siehe die Tests unten.
    fn new_photo(filename: &str) -> NewPhoto {
        NewPhoto {
            folder_id: FolderId::new(),
            filename: filename.to_string(),
            file_size: 123,
            file_mtime: time::OffsetDateTime::now_utc(),
            content_hash: None,
            width: None,
            height: None,
            orientation: 1,
            camera_make: None,
            camera_model: None,
            lens: None,
            iso: None,
            shutter: None,
            aperture: None,
            focal_length: None,
            captured_at: None,
            gps_lat: None,
            gps_lon: None,
            media_kind: "photo".to_string(),
            duration_ms: None,
            video_codec: None,
            has_audio: None,
            frame_rate: None,
        }
    }

    #[test]
    fn marks_photo_missing_when_file_was_deleted_outside_the_app() {
        let dir = tempdir().expect("Temp-Verzeichnis");
        let present_path = dir.path().join("present.jpg");
        let deleted_path = dir.path().join("deleted.jpg");
        fs::write(&present_path, b"present").expect("Datei schreiben");
        fs::write(&deleted_path, b"deleted").expect("Datei schreiben");

        let catalog = Catalog::open_in_memory().expect("Katalog öffnen");
        let folder_id = catalog
            .find_or_create_folder(dir.path(), None)
            .expect("Ordner anlegen");
        let (present_id, _) = catalog
            .upsert_photo(&NewPhoto {
                folder_id,
                ..new_photo("present.jpg")
            })
            .expect("Foto anlegen");
        let (deleted_id, _) = catalog
            .upsert_photo(&NewPhoto {
                folder_id,
                ..new_photo("deleted.jpg")
            })
            .expect("Foto anlegen");

        // Datei "außerhalb der App" löschen, bevor der Ordner erneut
        // geöffnet wird.
        fs::remove_file(&deleted_path).expect("Datei löschen");

        let photos = catalog
            .list_photos_by_folder(folder_id)
            .expect("Fotos listen");
        assert!(
            photos.iter().all(|p| !p.missing),
            "vor der Abgleichung noch kein Foto als missing markiert"
        );

        let reconciled =
            reconcile_missing(&catalog, dir.path(), photos).expect("sollte nicht fehlschlagen");

        let present = reconciled
            .iter()
            .find(|p| p.id == present_id)
            .expect("present.jpg");
        let deleted = reconciled
            .iter()
            .find(|p| p.id == deleted_id)
            .expect("deleted.jpg");
        assert!(
            !present.missing,
            "vorhandene Datei darf nicht als missing markiert werden"
        );
        assert!(
            deleted.missing,
            "gelöschte Datei muss als missing markiert werden"
        );

        // Auch im Katalog selbst persistiert, nicht nur im Rückgabewert.
        let deleted_from_catalog = catalog.get_photo(deleted_id).expect("Foto lesen");
        assert!(deleted_from_catalog.missing);
    }

    #[test]
    fn clears_missing_flag_once_the_file_reappears() {
        let dir = tempdir().expect("Temp-Verzeichnis");
        let path = dir.path().join("restored.jpg");

        let catalog = Catalog::open_in_memory().expect("Katalog öffnen");
        let folder_id = catalog
            .find_or_create_folder(dir.path(), None)
            .expect("Ordner anlegen");
        let (photo_id, _) = catalog
            .upsert_photo(&NewPhoto {
                folder_id,
                ..new_photo("restored.jpg")
            })
            .expect("Foto anlegen");
        catalog
            .set_photo_missing(photo_id, true)
            .expect("als missing markieren");

        // Datei taucht wieder auf (z. B. aus dem Papierkorb wiederhergestellt).
        fs::write(&path, b"restored").expect("Datei schreiben");

        let photos = catalog
            .list_photos_by_folder(folder_id)
            .expect("Fotos listen");
        let reconciled =
            reconcile_missing(&catalog, dir.path(), photos).expect("sollte nicht fehlschlagen");

        let restored = reconciled
            .iter()
            .find(|p| p.id == photo_id)
            .expect("restored.jpg");
        assert!(
            !restored.missing,
            "wieder vorhandene Datei muss die missing-Markierung verlieren"
        );
    }

    #[test]
    fn does_not_panic_when_folder_itself_is_gone() {
        let dir = tempdir().expect("Temp-Verzeichnis");
        let catalog = Catalog::open_in_memory().expect("Katalog öffnen");
        let folder_id = catalog
            .find_or_create_folder(dir.path(), None)
            .expect("Ordner anlegen");
        catalog
            .upsert_photo(&NewPhoto {
                folder_id,
                ..new_photo("gone.jpg")
            })
            .expect("Foto anlegen");

        let photos = catalog
            .list_photos_by_folder(folder_id)
            .expect("Fotos listen");
        let missing_dir = dir.path().join("does-not-exist-anymore");

        // `folder_path.join(...)` auf einem nicht mehr existierenden
        // Verzeichnis ist reine Pfad-Arithmetik — `exists()` liefert dann
        // einfach `false`, kein Panic, kein I/O-Fehler.
        let reconciled =
            reconcile_missing(&catalog, &missing_dir, photos).expect("sollte nicht fehlschlagen");
        assert_eq!(reconciled.len(), 1);
        assert!(reconciled[0].missing);
    }
}
