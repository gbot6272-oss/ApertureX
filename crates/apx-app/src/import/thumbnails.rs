//! Thumbnail-Erzeugung im Worker-Pool (Schritt 4 aus `PHASE1_PROMPT.md`
//! Abschnitt 5): bevorzugt aus der eingebetteten Vorschau der RAW-Datei,
//! sonst per (Half-Size-)Dekodierung über `apx-raw`.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use apx_catalog::{Catalog, PreviewLevel};
use apx_core::PhotoId;
use image::DynamicImage;
use rayon::prelude::*;
use tokio_util::sync::CancellationToken;

use super::{ImportEvents, THUMBNAIL_EDGE};

/// Erzeugt Thumbnails für `targets` in einem Worker-Pool mit
/// `physische Kerne − 1` Threads (mindestens 1). `done_offset`/`total_steps`
/// setzen die Fortschrittsanzeige nach der Metadaten-Scan-Phase fort, statt
/// wieder bei 0 anzufangen. Gibt die Anzahl fehlgeschlagener Thumbnails
/// zurück (Details gehen bereits einzeln über `events.error(...)` hinaus).
pub(super) fn generate(
    events: &impl ImportEvents,
    catalog: &Catalog,
    cache_root: &Path,
    cancel: &CancellationToken,
    targets: &[(PhotoId, PathBuf)],
    done_offset: usize,
    total_steps: usize,
) -> usize {
    if targets.is_empty() {
        return 0;
    }

    let worker_count = num_cpus::get_physical().saturating_sub(1).max(1);
    let pool = match rayon::ThreadPoolBuilder::new()
        .num_threads(worker_count)
        .build()
    {
        Ok(pool) => pool,
        Err(err) => {
            // Kann eigentlich nicht scheitern (keine ungewöhnliche
            // Konfiguration hier), aber lieber sauber melden als
            // abzustürzen, falls doch.
            tracing::error!(%err, "Thumbnail-Worker-Pool konnte nicht erstellt werden");
            for (_, path) in targets {
                events.error(
                    path,
                    &format!("Thumbnail-Worker-Pool nicht verfügbar: {err}"),
                );
            }
            return targets.len();
        }
    };

    let done_counter = AtomicUsize::new(done_offset);
    let error_count = AtomicUsize::new(0);

    pool.install(|| {
        targets.par_iter().for_each(|(photo_id, source_path)| {
            if cancel.is_cancelled() {
                return;
            }

            let done = done_counter.fetch_add(1, Ordering::SeqCst) + 1;
            events.progress(done, total_steps, Some(source_path));

            if let Err(message) = generate_one(catalog, cache_root, *photo_id, source_path) {
                events.error(source_path, &message);
                error_count.fetch_add(1, Ordering::SeqCst);
            }
        });
    });

    error_count.into_inner()
}

fn generate_one(
    catalog: &Catalog,
    cache_root: &Path,
    photo_id: PhotoId,
    source_path: &Path,
) -> Result<(), String> {
    let image = load_source_image(source_path)?;
    let thumbnail = downscale_to_thumbnail(image);

    let cache_path = thumbnail_cache_path(cache_root, photo_id);
    if let Some(dir) = cache_path.parent() {
        std::fs::create_dir_all(dir)
            .map_err(|err| format!("Cache-Verzeichnis nicht anlegbar: {err}"))?;
    }
    thumbnail
        .to_rgb8()
        .save_with_format(&cache_path, image::ImageFormat::Jpeg)
        .map_err(|err| format!("Thumbnail nicht speicherbar: {err}"))?;

    catalog
        .upsert_preview(photo_id, PreviewLevel::Thumbnail, &cache_path)
        .map_err(|err| err.to_string())
}

/// Bevorzugt die eingebettete Vorschau (schnell, kein Demosaicing nötig).
/// Ist keine vorhanden oder schlägt die Extraktion fehl, wird stattdessen
/// über `apx_raw::decode` mit `max_edge = THUMBNAIL_EDGE` dekodiert — für
/// RAWs nutzt das intern den günstigeren Half-Size-Pfad (siehe
/// `apx-raw`s `pipeline`-Modul).
fn load_source_image(source_path: &Path) -> Result<DynamicImage, String> {
    match apx_raw::extract_embedded_preview(source_path) {
        Ok(Some(preview)) => Ok(preview),
        Ok(None) => decode_fallback(source_path),
        Err(err) => {
            tracing::debug!(path = %source_path.display(), %err, "Eingebettete Vorschau nicht extrahierbar, weiche auf Dekodierung aus");
            decode_fallback(source_path)
        }
    }
}

fn decode_fallback(source_path: &Path) -> Result<DynamicImage, String> {
    let decoded =
        apx_raw::decode(source_path, Some(THUMBNAIL_EDGE)).map_err(|err| err.to_string())?;
    decoded
        .into_dynamic_image()
        .ok_or_else(|| "Dekodiertes Bild hat inkonsistente Maße".to_string())
}

fn downscale_to_thumbnail(image: DynamicImage) -> DynamicImage {
    if image.width().max(image.height()) > THUMBNAIL_EDGE {
        image.resize(
            THUMBNAIL_EDGE,
            THUMBNAIL_EDGE,
            image::imageops::FilterType::Lanczos3,
        )
    } else {
        image
    }
}

/// `<cache_root>/previews/<erste 2 Zeichen der ID>/<id>_0.jpg` — die
/// Unterordner-Aufteilung verhindert Verzeichnisse mit hunderttausenden
/// Dateien (siehe `PHASE1_PROMPT.md` Abschnitt 5). `_0` steht für Level 0
/// (Thumbnail), siehe `apx_catalog::PreviewLevel`.
fn thumbnail_cache_path(cache_root: &Path, photo_id: PhotoId) -> PathBuf {
    let id = photo_id.to_string();
    let prefix: String = id.chars().take(2).collect();
    cache_root.join(prefix).join(format!("{id}_0.jpg"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_path_uses_two_char_prefix_subfolder() {
        let id = PhotoId::new();
        let path = thumbnail_cache_path(Path::new("/cache/previews"), id);
        let id_str = id.to_string();
        let expected_prefix: String = id_str.chars().take(2).collect();

        assert!(path.starts_with(Path::new("/cache/previews").join(&expected_prefix)));
        assert_eq!(
            path.file_name().and_then(|f| f.to_str()),
            Some(format!("{id_str}_0.jpg").as_str())
        );
    }

    #[test]
    fn downscale_shrinks_oversized_image_to_thumbnail_edge() {
        let image = DynamicImage::new_rgb8(1000, 500);
        let thumb = downscale_to_thumbnail(image);
        assert_eq!(thumb.width(), THUMBNAIL_EDGE);
        assert!(thumb.height() <= THUMBNAIL_EDGE);
    }

    #[test]
    fn downscale_leaves_small_image_untouched() {
        let image = DynamicImage::new_rgb8(100, 80);
        let thumb = downscale_to_thumbnail(image);
        assert_eq!((thumb.width(), thumb.height()), (100, 80));
    }
}
