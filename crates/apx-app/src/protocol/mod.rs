//! Custom-Protokoll-Handler `apx://` — liefert Vorschauen, Vollbilder und
//! (ab Phase 2) live entwickelte Bilder ans Frontend, ohne den Umweg über
//! Base64-kodierte Tauri-Commands (siehe `PHASE1_PROMPT.md` Abschnitt 6
//! und "Bekannte Fallstricke").
//!
//! Drei Anfragearten (URL-Format siehe `route`-Modul und `DECISIONS.md`
//! ADR-0009):
//! - `preview/<id>/<level>`: liest, falls vorhanden, den von `apx-app`s
//!   Import-Job erzeugten Thumbnail-Cache direkt von der Platte
//!   (schnell); fehlt er, wird live nachdekodiert (JPEG-Antwort).
//! - `image/<id>/<max_edge|'full'>`: dekodiert live in der angeforderten
//!   Auflösung (PNG-Antwort, 16-Bit-Präzision erhalten — anders als JPEG
//!   verlustfrei, wichtig für die Vollbild-Ansicht im Entwickeln-Modul ab
//!   Phase 2).
//! - `develop/<id>/<max_edge|'full'>/<edl_json>` (ab Phase 2, siehe
//!   `DECISIONS.md` ADR-0016): rendert live über `apx-pipeline`
//!   (Weißabgleich + die sieben Regler + feste Kamera→sRGB-Matrix).
//!   Antwort ist bewusst kein PNG/JPEG, sondern rohe Bytes — eine
//!   Kompression bei jedem Regler-Tick würde unnötig Zeit im
//!   16-ms-Budget kosten: die ersten 8 Bytes sind Breite und Höhe als
//!   `u32` little-endian, danach folgt interleaved RGBA8 (Alpha immer
//!   `255`). Der teure Dekodier-Schritt (`apx_raw::decode_linear`) läuft
//!   über `apx-pipeline`s `TileCache` (siehe `state`-Modul), damit ein
//!   Regler-Tick nicht jedes Mal neu demosaicen muss.
//!
//! Anfragen für denselben Schlüssel werden dedupliziert (`cache`-Modul).
//! Echtes Abbrechen einer bereits laufenden Dekodierung ist mit den hier
//! verfügbaren Mitteln (ein OS-Thread ohne kooperative Abbruchpunkte
//! innerhalb von `rawler`) nicht möglich — das Abbrechen beim Bildwechsel
//! ist daher Aufgabe des Frontends: es nutzt `fetch()` mit
//! `AbortController` statt eines einfachen `<img src>`, sodass ein
//! veraltetes Ergebnis zumindest nicht mehr verarbeitet wird, sobald das
//! nächste Bild angefordert wurde (siehe Viewer, Schritt 9).

mod cache;
mod route;

use std::path::PathBuf;

use apx_catalog::{Catalog, PreviewLevel};
use apx_core::PhotoId;
use image::DynamicImage;
use tauri::http::{header, Request, Response, StatusCode};
use tauri::{AppHandle, Manager, Runtime};

use cache::ImageCache;
use route::ImageRequest;

use crate::state::AppState;

const THUMBNAIL_EDGE: u32 = 256;
const STANDARD_EDGE: u32 = 2048;

/// Registriert den `apx://`-Handler auf dem Tauri-`Builder`. Läuft in
/// einem eigenen OS-Thread pro Anfrage (asynchroner Handler), damit die
/// teils rechenintensive Dekodierung den Tauri-eigenen IPC-Thread nicht
/// blockiert (bekannter Fallstrick, siehe `PHASE1_PROMPT.md` Abschnitt 10).
pub fn register<R: Runtime>(builder: tauri::Builder<R>) -> tauri::Builder<R> {
    let cache = std::sync::Arc::new(ImageCache::default());
    builder.register_asynchronous_uri_scheme_protocol("apx", move |ctx, request, responder| {
        let cache = cache.clone();
        let app_handle = ctx.app_handle().clone();
        std::thread::spawn(move || {
            let response = handle(&app_handle, &request, &cache);
            responder.respond(response);
        });
    })
}

fn handle<R: Runtime>(
    app: &AppHandle<R>,
    request: &Request<Vec<u8>>,
    cache: &ImageCache,
) -> Response<Vec<u8>> {
    match handle_inner(app, request, cache) {
        Ok(response) => response,
        Err(err) => error_response(&err),
    }
}

#[derive(Debug)]
struct HandlerError {
    status: StatusCode,
    message: String,
}

impl HandlerError {
    fn bad_request(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            message: message.into(),
        }
    }

    fn not_found(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            message: message.into(),
        }
    }

    fn internal(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: message.into(),
        }
    }
}

impl From<apx_core::AppError> for HandlerError {
    fn from(err: apx_core::AppError) -> Self {
        match &err {
            apx_core::AppError::NotFound { .. } => Self::not_found(err.to_string()),
            _ => Self::internal(err.to_string()),
        }
    }
}

fn handle_inner<R: Runtime>(
    app: &AppHandle<R>,
    request: &Request<Vec<u8>>,
    cache: &ImageCache,
) -> Result<Response<Vec<u8>>, HandlerError> {
    let parsed =
        route::parse(request.uri().path()).map_err(|err| HandlerError::bad_request(err.0))?;
    let state = app.state::<AppState>();
    let catalog = state.catalog.clone();
    let pipeline = state.pipeline.clone();
    let tile_cache = state.tile_cache.clone();

    let (content_type, cache_key) = response_meta(&parsed);
    let bytes = cache
        .get_or_compute(cache_key, move || {
            compute(&catalog, &pipeline, &tile_cache, &parsed).map_err(|err| err.message)
        })
        .map_err(HandlerError::internal)?;

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, content_type)
        // Bilder ändern sich unter derselben ID in Phase 1 nicht (keine
        // Bearbeitung, kein erneuter Import mit anderem Inhalt unter
        // derselben Foto-ID) — moderate Cache-Dauer statt "immutable",
        // damit ein künftiges Re-Rendering (Phase 2, Bearbeitung) nicht
        // durch einen zu aggressiven Cache blockiert wird.
        .header(header::CACHE_CONTROL, "private, max-age=86400")
        .body((*bytes).clone())
        .map_err(|err| HandlerError::internal(err.to_string()))
}

fn response_meta(request: &ImageRequest) -> (&'static str, String) {
    match request {
        ImageRequest::Preview { photo_id, level } => {
            ("image/jpeg", format!("preview:{photo_id}:{level}"))
        }
        ImageRequest::Image { photo_id, max_edge } => (
            "image/png",
            format!("image:{photo_id}:{}", format_max_edge(*max_edge)),
        ),
        ImageRequest::Develop {
            photo_id,
            max_edge,
            edl_json,
        } => (
            // Kein Standard-Bildformat, siehe Modul-Doku: die ersten 8
            // Bytes sind Breite/Höhe (u32 little-endian), danach rohes
            // RGBA8. `edl_json` steckt bewusst im Cache-Schlüssel, nicht
            // nur photo_id/max_edge — zwei verschiedene Bearbeitungs-
            // zustände desselben Fotos sind zwei verschiedene Anfragen.
            "application/x-apx-develop-rgba8",
            format!(
                "develop:{photo_id}:{}:{edl_json}",
                format_max_edge(*max_edge)
            ),
        ),
    }
}

fn format_max_edge(max_edge: Option<u32>) -> String {
    max_edge
        .map(|e| e.to_string())
        .unwrap_or_else(|| "full".to_string())
}

fn compute(
    catalog: &Catalog,
    pipeline: &apx_pipeline::GpuContext,
    tile_cache: &apx_pipeline::tile_cache::TileCache,
    request: &ImageRequest,
) -> Result<Vec<u8>, HandlerError> {
    match request {
        ImageRequest::Preview { photo_id, level } => compute_preview(catalog, *photo_id, *level),
        ImageRequest::Image { photo_id, max_edge } => {
            compute_full_image(catalog, *photo_id, *max_edge)
        }
        ImageRequest::Develop {
            photo_id,
            max_edge,
            edl_json,
        } => compute_develop(
            catalog, pipeline, tile_cache, *photo_id, *max_edge, edl_json,
        ),
    }
}

fn compute_preview(
    catalog: &Catalog,
    photo_id: PhotoId,
    level_num: u8,
) -> Result<Vec<u8>, HandlerError> {
    let level = match level_num {
        0 => PreviewLevel::Thumbnail,
        1 => PreviewLevel::Standard,
        2 => PreviewLevel::Full,
        other => {
            return Err(HandlerError::bad_request(format!(
                "ungültige Vorschau-Stufe {other} (erwartet 0, 1 oder 2)"
            )))
        }
    };

    if let Some(cached) = catalog.get_preview(photo_id, level)? {
        if let Ok(bytes) = std::fs::read(&cached.path) {
            return Ok(bytes);
        }
        // Cache-Eintrag verweist auf eine nicht (mehr) vorhandene Datei
        // (z. B. manuell gelöscht) — unten frisch dekodieren statt hart
        // zu scheitern.
        tracing::warn!(path = %cached.path.display(), "Vorschau-Cache-Eintrag verweist auf fehlende Datei, dekodiere neu");
    }

    let source_path = resolve_source_path(catalog, photo_id)?;
    let max_edge = match level {
        PreviewLevel::Thumbnail => Some(THUMBNAIL_EDGE),
        PreviewLevel::Standard => Some(STANDARD_EDGE),
        PreviewLevel::Full => None,
    };
    let image = decode_to_dynamic_image(&source_path, max_edge)?;
    encode(image, image::ImageFormat::Jpeg)
}

fn compute_full_image(
    catalog: &Catalog,
    photo_id: PhotoId,
    max_edge: Option<u32>,
) -> Result<Vec<u8>, HandlerError> {
    let source_path = resolve_source_path(catalog, photo_id)?;
    let image = decode_to_dynamic_image(&source_path, max_edge)?;
    encode(image, image::ImageFormat::Png)
}

/// Rendert `photo_id` live über `apx-pipeline` mit dem in `edl_json`
/// beschriebenen Bearbeitungszustand — siehe Modul-Doku für das
/// Antwortformat (8-Byte-Breite/Höhe-Header + rohes RGBA8).
fn compute_develop(
    catalog: &Catalog,
    pipeline: &apx_pipeline::GpuContext,
    tile_cache: &apx_pipeline::tile_cache::TileCache,
    photo_id: PhotoId,
    max_edge: Option<u32>,
    edl_json: &str,
) -> Result<Vec<u8>, HandlerError> {
    let envelope = apx_core::EdlEnvelope::from_json_str(edl_json)?;
    let edl = apx_pipeline::edl::from_envelope(&envelope).map_err(apx_core::AppError::from)?;

    let source_path = resolve_source_path(catalog, photo_id)?;

    // Zwei getrennte Zeitmessungen statt einer gemeinsamen: der teure
    // Dekodier-Schritt läuft (Cache-Treffer vorausgesetzt) nur beim
    // allerersten Regler-Tick eines Fotos, das Rendern dagegen bei jedem
    // Tick — für das 16-ms-Ziel (SPEC.md §2.4) zählt fast ausschließlich
    // Letzteres. Siehe PLAN.md Phase 2 Schritt 7 zur ehrlichen
    // Performance-Dokumentation.
    let decode_started = std::time::Instant::now();
    let linear = tile_cache.get_or_decode(photo_id, max_edge, || {
        apx_raw::decode_linear(&source_path, max_edge)
    })?;
    let decode_elapsed = decode_started.elapsed();

    let render_started = std::time::Instant::now();
    let pixels = apx_pipeline::develop::render_rgba8(Some(pipeline), &linear, &edl)
        .map_err(apx_core::AppError::from)?;
    let render_elapsed = render_started.elapsed();

    tracing::debug!(
        photo_id = %photo_id,
        width = linear.width,
        height = linear.height,
        decode_ms = decode_elapsed.as_secs_f64() * 1000.0,
        render_ms = render_elapsed.as_secs_f64() * 1000.0,
        "compute_develop abgeschlossen"
    );

    let mut framed = Vec::with_capacity(8 + pixels.len());
    framed.extend_from_slice(&linear.width.to_le_bytes());
    framed.extend_from_slice(&linear.height.to_le_bytes());
    framed.extend_from_slice(&pixels);
    Ok(framed)
}

fn resolve_source_path(catalog: &Catalog, photo_id: PhotoId) -> Result<PathBuf, HandlerError> {
    let photo = catalog.get_photo(photo_id)?;
    let folder = catalog.get_folder(photo.folder_id)?;
    Ok(folder.path.join(photo.filename))
}

fn decode_to_dynamic_image(
    source_path: &std::path::Path,
    max_edge: Option<u32>,
) -> Result<DynamicImage, HandlerError> {
    let decoded = apx_raw::decode(source_path, max_edge)?;
    decoded
        .into_dynamic_image()
        .ok_or_else(|| HandlerError::internal("Dekodiertes Bild hat inkonsistente Maße"))
}

fn encode(image: DynamicImage, format: image::ImageFormat) -> Result<Vec<u8>, HandlerError> {
    let mut buffer = Vec::new();
    image
        .write_to(&mut std::io::Cursor::new(&mut buffer), format)
        .map_err(|err| HandlerError::internal(format!("Kodierung fehlgeschlagen: {err}")))?;
    Ok(buffer)
}

fn error_response(err: &HandlerError) -> Response<Vec<u8>> {
    tracing::warn!(status = %err.status, message = %err.message, "apx://-Anfrage fehlgeschlagen");
    Response::builder()
        .status(err.status)
        .header(header::CONTENT_TYPE, "text/plain; charset=utf-8")
        .body(err.message.clone().into_bytes())
        .unwrap_or_else(|_| Response::new(Vec::new()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use apx_catalog::NewPhoto;
    use time::OffsetDateTime;

    fn write_valid_jpeg(path: &std::path::Path) {
        let image = image::RgbImage::from_pixel(64, 48, image::Rgb([90, 140, 200]));
        image::DynamicImage::ImageRgb8(image)
            .save_with_format(path, image::ImageFormat::Jpeg)
            .expect("Test-JPEG sollte sich speichern lassen");
    }

    fn setup_photo(tmp: &std::path::Path, catalog: &Catalog) -> PhotoId {
        write_valid_jpeg(&tmp.join("foto.jpg"));
        let folder_id = catalog
            .find_or_create_folder(tmp, None)
            .expect("Ordner anlegbar");
        let (photo_id, _) = catalog
            .upsert_photo(&NewPhoto {
                folder_id,
                filename: "foto.jpg".to_string(),
                file_size: std::fs::metadata(tmp.join("foto.jpg"))
                    .expect("Metadaten")
                    .len(),
                file_mtime: OffsetDateTime::now_utc(),
                content_hash: None,
                width: Some(64),
                height: Some(48),
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
            })
            .expect("Foto anlegbar");
        photo_id
    }

    #[test]
    fn compute_preview_decodes_when_no_cache_entry_exists() {
        let tmp = tempfile::tempdir().expect("Temp-Verzeichnis");
        let catalog = Catalog::open_in_memory().expect("Katalog");
        let photo_id = setup_photo(tmp.path(), &catalog);

        let bytes = compute_preview(&catalog, photo_id, 0).expect("sollte dekodieren");
        assert!(!bytes.is_empty());
        assert_eq!(
            image::guess_format(&bytes).ok(),
            Some(image::ImageFormat::Jpeg)
        );
    }

    #[test]
    fn compute_preview_prefers_cached_file_when_present() {
        let tmp = tempfile::tempdir().expect("Temp-Verzeichnis");
        let catalog = Catalog::open_in_memory().expect("Katalog");
        let photo_id = setup_photo(tmp.path(), &catalog);

        let cached_path = tmp.path().join("cached-thumb.jpg");
        write_valid_jpeg(&cached_path);
        catalog
            .upsert_preview(photo_id, PreviewLevel::Thumbnail, &cached_path)
            .expect("Preview-Eintrag anlegbar");

        let bytes = compute_preview(&catalog, photo_id, 0).expect("sollte lesen");
        assert_eq!(bytes, std::fs::read(&cached_path).expect("Datei lesbar"));
    }

    #[test]
    fn compute_preview_rejects_invalid_level() {
        let tmp = tempfile::tempdir().expect("Temp-Verzeichnis");
        let catalog = Catalog::open_in_memory().expect("Katalog");
        let photo_id = setup_photo(tmp.path(), &catalog);

        assert!(compute_preview(&catalog, photo_id, 9).is_err());
    }

    #[test]
    fn compute_full_image_produces_png() {
        let tmp = tempfile::tempdir().expect("Temp-Verzeichnis");
        let catalog = Catalog::open_in_memory().expect("Katalog");
        let photo_id = setup_photo(tmp.path(), &catalog);

        let bytes = compute_full_image(&catalog, photo_id, Some(32)).expect("sollte dekodieren");
        assert_eq!(
            image::guess_format(&bytes).ok(),
            Some(image::ImageFormat::Png)
        );
    }

    #[test]
    fn compute_for_unknown_photo_id_is_not_found() {
        let catalog = Catalog::open_in_memory().expect("Katalog");
        let result = compute_full_image(&catalog, PhotoId::new(), None);
        assert!(result.is_err());
    }

    #[test]
    fn response_meta_produces_distinct_cache_keys_per_request_shape() {
        let id = PhotoId::new();
        let (_, key_preview) = response_meta(&ImageRequest::Preview {
            photo_id: id,
            level: 0,
        });
        let (_, key_image_full) = response_meta(&ImageRequest::Image {
            photo_id: id,
            max_edge: None,
        });
        let (_, key_image_2560) = response_meta(&ImageRequest::Image {
            photo_id: id,
            max_edge: Some(2560),
        });
        let (_, key_develop_neutral) = response_meta(&ImageRequest::Develop {
            photo_id: id,
            max_edge: Some(2560),
            edl_json: "{}".to_string(),
        });
        let (_, key_develop_other_edl) = response_meta(&ImageRequest::Develop {
            photo_id: id,
            max_edge: Some(2560),
            edl_json: "{\"exposure_ev\":1.0}".to_string(),
        });

        assert_ne!(key_preview, key_image_full);
        assert_ne!(key_image_full, key_image_2560);
        assert_ne!(key_image_2560, key_develop_neutral);
        assert_ne!(
            key_develop_neutral, key_develop_other_edl,
            "zwei verschiedene EDL-Zustände desselben Fotos müssen unterschiedliche Cache-Schlüssel ergeben"
        );
    }

    fn neutral_edl_json() -> String {
        apx_core::EdlEnvelope::new(
            apx_pipeline::EDL_SCHEMA_VERSION,
            serde_json::to_value(apx_pipeline::EdlV1::NEUTRAL).expect("EDL serialisierbar"),
        )
        .to_json_string()
        .expect("Umschlag serialisierbar")
    }

    #[test]
    fn compute_develop_produces_framed_rgba8_matching_photo_dimensions() {
        let tmp = tempfile::tempdir().expect("Temp-Verzeichnis");
        let catalog = Catalog::open_in_memory().expect("Katalog");
        let photo_id = setup_photo(tmp.path(), &catalog);
        let pipeline = apx_pipeline::GpuContext::new_blocking();
        let pipeline = match pipeline {
            Ok(ctx) => ctx,
            Err(_) => {
                eprintln!("übersprungen: kein GPU-Adapter in dieser Umgebung verfügbar");
                return;
            }
        };
        let tile_cache = apx_pipeline::tile_cache::TileCache::new();

        let bytes = compute_develop(
            &catalog,
            &pipeline,
            &tile_cache,
            photo_id,
            Some(32),
            &neutral_edl_json(),
        )
        .expect("sollte rendern");

        assert!(bytes.len() > 8, "Antwort muss mindestens den Header tragen");
        let width = u32::from_le_bytes(bytes[0..4].try_into().expect("4 Bytes"));
        let height = u32::from_le_bytes(bytes[4..8].try_into().expect("4 Bytes"));
        assert_eq!(
            bytes.len() - 8,
            (width * height * 4) as usize,
            "Rest der Antwort muss genau width*height*4 RGBA8-Bytes sein"
        );
    }

    #[test]
    fn compute_develop_rejects_malformed_edl_json() {
        let tmp = tempfile::tempdir().expect("Temp-Verzeichnis");
        let catalog = Catalog::open_in_memory().expect("Katalog");
        let photo_id = setup_photo(tmp.path(), &catalog);
        let pipeline = match apx_pipeline::GpuContext::new_blocking() {
            Ok(ctx) => ctx,
            Err(_) => {
                eprintln!("übersprungen: kein GPU-Adapter in dieser Umgebung verfügbar");
                return;
            }
        };
        let tile_cache = apx_pipeline::tile_cache::TileCache::new();

        let result = compute_develop(
            &catalog,
            &pipeline,
            &tile_cache,
            photo_id,
            Some(32),
            "nicht-valides-json",
        );
        assert!(result.is_err());
    }
}
