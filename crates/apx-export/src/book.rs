//! Buch (Phase 8 Schritt 5, `PLAN.md`: „Seitenlayouts/Vorlagen/Text-Stile
//! als datengetriebene Layout-Engine ..., automatische Befüllung, PDF-
//! Export über `printpdf`, Druckerei-Presets als Parametersätze").
//!
//! **Architektur:** eine Buchseite ist geometrisch dasselbe Problem wie
//! eine Druckseite (`print.rs`) — rechteckige Zellen auf einer Seite
//! fester Größe. [`PageTemplate::photo_slots`] liefert dieselben
//! [`print::PrintSlot`]s, [`render_book_page`] ruft [`print::compose_page`]
//! unverändert auf; Textfelder (Titel/Bildunterschriften) sind der
//! einzige neue Baustein und laufen über
//! [`watermark::apply_text_at`]/[`watermark::rasterize_text`] — kein
//! zweiter Rendering-Codepfad. Der PDF-Export selbst bettet jede fertig
//! komponierte Seite als ein einziges Bild ein (`printpdf::RawImage`
//! direkt aus dem RGBA8-Puffer, ohne Zwischenkodierung) — `printpdf` läuft
//! deshalb bewusst ohne Standard-Features (kein `html`/`azul-layout`,
//! kein eigener Bilddecoder), siehe `Cargo.toml`.
//!
//! **Bewusste Vereinfachung** (wie `print.rs`s feste Bilderpaket-
//! Vorlagen): fünf feste Seitenvorlagen statt einer frei konfigurierbaren
//! Slot-Engine, Bildunterschriften einzeilig ohne automatischen
//! Zeilenumbruch. Druckerei-Presets (`PrintShopPreset`) sind reine
//! Parametersätze (Beschnitt/Auflösung/Hintergrund) ohne Anbieter-
//! spezifische Validierung.

use std::path::Path;

use printpdf::{Mm, Op, PdfDocument, PdfPage, PdfSaveOptions, Pt, RawImage, RawImageData, RawImageFormat, XObjectTransform};

/// Zoll → Millimeter (`printpdf`s `Mm`/`Pt`-Einheiten kennen kein `In`).
const MM_PER_INCH: f32 = 25.4;

use crate::error::{ExportError, Result};
use crate::print::{self, FitMode, PrintCell, PrintSlot};
use crate::watermark;

/// Ein Textfeld auf einer Buchseite, in Zoll (Ursprung oben links) —
/// dieselbe Koordinatenwelt wie [`PrintSlot`].
#[derive(Debug, Clone, Copy)]
pub struct TextSlot {
    pub x_in: f32,
    pub y_in: f32,
    pub width_in: f32,
    pub height_in: f32,
}

/// Feste Seitenvorlagen — siehe Moduldoku zur bewussten Vereinfachung.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageTemplate {
    /// Ein Foto, randlos über die ganze Seite.
    FullBleed,
    /// Zwei Fotos nebeneinander.
    TwoSideBySide,
    /// 2×2-Raster.
    Grid2x2,
    /// Ein Foto oben, Bildunterschrift-Textfeld darunter.
    PhotoWithCaption,
    /// Reines Textfeld (Titel-/Kapitelseite), kein Foto.
    TitlePage,
}

impl PageTemplate {
    /// Wie viele Fotoslots dieses Template hat — bestimmt, wie viele
    /// Fotos [`auto_fill_pages`] dieser Seite zuweist.
    pub fn photo_slot_count(self) -> usize {
        match self {
            PageTemplate::FullBleed | PageTemplate::PhotoWithCaption => 1,
            PageTemplate::TwoSideBySide => 2,
            PageTemplate::Grid2x2 => 4,
            PageTemplate::TitlePage => 0,
        }
    }

    pub fn photo_slots(self, page_width_in: f32, page_height_in: f32, margin_in: f32) -> Vec<PrintSlot> {
        match self {
            PageTemplate::FullBleed => vec![PrintSlot {
                x_in: 0.0,
                y_in: 0.0,
                width_in: page_width_in,
                height_in: page_height_in,
            }],
            PageTemplate::TwoSideBySide => print::grid_slots(page_width_in, page_height_in, margin_in, margin_in, 2, 1),
            PageTemplate::Grid2x2 => print::grid_slots(page_width_in, page_height_in, margin_in, margin_in, 2, 2),
            PageTemplate::PhotoWithCaption => vec![PrintSlot {
                x_in: margin_in,
                y_in: margin_in,
                width_in: (page_width_in - 2.0 * margin_in).max(0.1),
                height_in: (page_height_in * 0.8 - margin_in).max(0.1),
            }],
            PageTemplate::TitlePage => Vec::new(),
        }
    }

    /// Textfelder dieses Templates — leer außer bei `PhotoWithCaption`/`TitlePage`.
    pub fn text_slots(self, page_width_in: f32, page_height_in: f32, margin_in: f32) -> Vec<TextSlot> {
        match self {
            PageTemplate::PhotoWithCaption => vec![TextSlot {
                x_in: margin_in,
                y_in: page_height_in * 0.8,
                width_in: (page_width_in - 2.0 * margin_in).max(0.1),
                height_in: (page_height_in * 0.2 - margin_in).max(0.1),
            }],
            PageTemplate::TitlePage => vec![TextSlot {
                x_in: margin_in,
                y_in: page_height_in * 0.4,
                width_in: (page_width_in - 2.0 * margin_in).max(0.1),
                height_in: page_height_in * 0.2,
            }],
            _ => Vec::new(),
        }
    }
}

/// Verteilt `photo_ids` reihum auf Seiten, je nach `template.photo_slot_count()`
/// Fotos pro Seite ("automatische Befüllung") — die letzte Seite bekommt
/// ggf. weniger Fotos als Slots vorhanden sind. Leer bei `photo_slot_count() == 0`
/// (Titelseiten werden separat hinzugefügt, nicht automatisch befüllt).
pub fn auto_fill_pages(photo_ids: &[String], template: PageTemplate) -> Vec<Vec<String>> {
    let per_page = template.photo_slot_count();
    if per_page == 0 || photo_ids.is_empty() {
        return Vec::new();
    }
    photo_ids.chunks(per_page).map(|chunk| chunk.to_vec()).collect()
}

/// Ein bereits gerendertes Foto (siehe `engine::render_to_pixels`), analog
/// zu `print::PrintCell` aber ohne festen Slot — [`render_book_page`]
/// ordnet es reihum den Fotoslots des Templates zu.
pub struct BookPagePhoto<'a> {
    pub width: u32,
    pub height: u32,
    pub rgba: &'a [u8],
}

/// Rendert eine einzelne Buchseite: Fotos über `print::compose_page` in
/// die Fotoslots des Templates, optional eine Bildunterschrift/ein Titel
/// mittig in das erste Textfeld. Mehr Textfelder als ein Eintrag hat
/// dieses Modul bislang nicht (siehe Moduldoku).
#[allow(clippy::too_many_arguments)]
pub fn render_book_page(
    template: PageTemplate,
    page_width_in: f32,
    page_height_in: f32,
    dpi: u32,
    margin_in: f32,
    background_rgb: [u8; 3],
    photos: &[BookPagePhoto],
    fit: FitMode,
    caption: Option<&str>,
    font_bytes: Option<&[u8]>,
    caption_color: [u8; 3],
) -> Result<(u32, u32, Vec<u8>)> {
    let photo_slots = template.photo_slots(page_width_in, page_height_in, margin_in);
    let cells: Vec<PrintCell> = photo_slots
        .iter()
        .zip(photos.iter())
        .map(|(slot, photo)| PrintCell {
            slot: *slot,
            width: photo.width,
            height: photo.height,
            rgba: photo.rgba,
            fit,
        })
        .collect();

    let (page_w, page_h, mut page_pixels) = print::compose_page(page_width_in, page_height_in, dpi, background_rgb, &cells)?;

    if let (Some(text), Some(slot)) = (caption, template.text_slots(page_width_in, page_height_in, margin_in).first()) {
        let font_bytes = font_bytes.ok_or_else(|| {
            ExportError::Unsupported("Bildunterschrift gesetzt, aber keine Schriftdatei ausgewählt".to_string())
        })?;
        let slot_x = (slot.x_in * dpi as f32).round() as i64;
        let slot_y = (slot.y_in * dpi as f32).round() as i64;
        let slot_w = (slot.width_in * dpi as f32).round().max(1.0) as u32;
        let slot_h = (slot.height_in * dpi as f32).round().max(1.0) as u32;
        // Schriftgröße an die Feldhöhe gekoppelt (grobe Heuristik, kein
        // automatischer Zeilenumbruch — siehe Moduldoku).
        let font_size_px = (slot_h as f32 * 0.5).clamp(8.0, 96.0);

        let (text_w, text_h, _) = watermark::rasterize_text(font_bytes, text, font_size_px, caption_color)?;
        let origin_x = slot_x + ((slot_w as i64 - text_w as i64) / 2).max(0);
        let origin_y = slot_y + ((slot_h as i64 - text_h as i64) / 2).max(0);
        watermark::apply_text_at(
            page_w,
            page_h,
            &mut page_pixels,
            font_bytes,
            text,
            font_size_px,
            caption_color,
            origin_x,
            origin_y,
            1.0,
        )?;
    }

    Ok((page_w, page_h, page_pixels))
}

/// Druckerei-Preset — reiner Parametersatz (Beschnitt/Auflösung/
/// Hintergrund), keine anbieterspezifische Validierung (siehe Moduldoku).
#[derive(Debug, Clone, Copy)]
pub struct PrintShopPreset {
    pub name: &'static str,
    pub bleed_in: f32,
    pub dpi: u32,
    pub background_rgb: [u8; 3],
}

pub const PRINT_SHOP_PRESETS: &[PrintShopPreset] = &[
    PrintShopPreset { name: "Digitaldruck (Standard, 300 dpi)", bleed_in: 0.125, dpi: 300, background_rgb: [255, 255, 255] },
    PrintShopPreset { name: "Fotobuch (Premium, 400 dpi)", bleed_in: 0.125, dpi: 400, background_rgb: [255, 255, 255] },
    PrintShopPreset { name: "Softcover (kein Beschnitt, 250 dpi)", bleed_in: 0.0, dpi: 250, background_rgb: [255, 255, 255] },
];

/// Baut eine mehrseitige PDF-Datei aus bereits gerenderten Seiten
/// (`(width_px, height_px, rgba8)` je Seite, siehe [`render_book_page`])
/// und schreibt sie nach `dest_path`. Jede Seite wird als ein einziges
/// Bild-XObject eingebettet — `dpi` bestimmt zusammen mit der
/// Pixelgröße die physische Seitengröße in der PDF-Datei.
pub fn build_pdf(pages: &[(u32, u32, Vec<u8>)], dpi: u32, dest_path: &Path) -> Result<Vec<u8>> {
    if pages.is_empty() {
        return Err(ExportError::Unsupported("Buch enthält keine Seiten".to_string()));
    }

    let mut doc = PdfDocument::new("Aperture X Fotobuch");
    let mut pdf_pages = Vec::with_capacity(pages.len());

    for (width, height, rgba) in pages {
        let image = RawImage {
            pixels: RawImageData::U8(rgba.clone()),
            width: *width as usize,
            height: *height as usize,
            data_format: RawImageFormat::RGBA8,
            tag: Vec::new(),
        };
        let xobject_id = doc.add_image(&image);

        let page_width_in = *width as f32 / dpi as f32;
        let page_height_in = *height as f32 / dpi as f32;
        let ops = vec![Op::UseXobject {
            id: xobject_id,
            transform: XObjectTransform {
                translate_x: Some(Pt(0.0)),
                translate_y: Some(Pt(0.0)),
                dpi: Some(dpi as f32),
                ..Default::default()
            },
        }];
        pdf_pages.push(PdfPage::new(
            Mm(page_width_in * MM_PER_INCH),
            Mm(page_height_in * MM_PER_INCH),
            ops,
        ));
    }

    let mut warnings = Vec::new();
    let bytes = doc.with_pages(pdf_pages).save(&PdfSaveOptions::default(), &mut warnings);

    std::fs::write(dest_path, &bytes).map_err(|err| ExportError::Io {
        path: dest_path.display().to_string(),
        message: err.to_string(),
    })?;
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn solid_photo(width: u32, height: u32, rgb: [u8; 3]) -> Vec<u8> {
        (0..width * height).flat_map(|_| [rgb[0], rgb[1], rgb[2], 255]).collect()
    }

    #[test]
    fn photo_slot_count_matches_each_templates_layout() {
        assert_eq!(PageTemplate::FullBleed.photo_slot_count(), 1);
        assert_eq!(PageTemplate::TwoSideBySide.photo_slot_count(), 2);
        assert_eq!(PageTemplate::Grid2x2.photo_slot_count(), 4);
        assert_eq!(PageTemplate::PhotoWithCaption.photo_slot_count(), 1);
        assert_eq!(PageTemplate::TitlePage.photo_slot_count(), 0);
    }

    #[test]
    fn auto_fill_pages_chunks_photos_by_slot_count() {
        let photos: Vec<String> = (0..5).map(|i| format!("p{i}")).collect();
        let pages = auto_fill_pages(&photos, PageTemplate::TwoSideBySide);
        assert_eq!(pages.len(), 3); // 2 + 2 + 1
        assert_eq!(pages[2].len(), 1);
    }

    #[test]
    fn auto_fill_pages_is_empty_for_title_page_template() {
        let photos: Vec<String> = vec!["p0".to_string()];
        assert!(auto_fill_pages(&photos, PageTemplate::TitlePage).is_empty());
    }

    #[test]
    fn render_book_page_produces_requested_pixel_dimensions() {
        let rgba = solid_photo(4, 4, [200, 100, 50]);
        let photos = [BookPagePhoto { width: 4, height: 4, rgba: &rgba }];
        let (w, h, _) = render_book_page(
            PageTemplate::FullBleed,
            4.0,
            6.0,
            100,
            0.25,
            [255, 255, 255],
            &photos,
            FitMode::Cover,
            None,
            None,
            [0, 0, 0],
        )
        .unwrap();
        assert_eq!((w, h), (400, 600));
    }

    #[test]
    fn caption_without_font_is_a_clean_error() {
        let err = render_book_page(
            PageTemplate::PhotoWithCaption,
            4.0,
            4.0,
            50,
            0.1,
            [255, 255, 255],
            &[],
            FitMode::Contain,
            Some("Urlaub 2026"),
            None,
            [0, 0, 0],
        )
        .unwrap_err();
        assert!(matches!(err, ExportError::Unsupported(_)));
    }

    #[test]
    fn build_pdf_writes_a_file_starting_with_the_pdf_signature() {
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("buch.pdf");
        let page = (100u32, 100u32, solid_photo(100, 100, [10, 20, 30]));
        let bytes = build_pdf(&[page], 100, &dest).unwrap();
        assert!(bytes.starts_with(b"%PDF"));
        assert_eq!(std::fs::read(&dest).unwrap(), bytes);
    }

    #[test]
    fn build_pdf_rejects_empty_page_list() {
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("leer.pdf");
        let err = build_pdf(&[], 300, &dest).unwrap_err();
        assert!(matches!(err, ExportError::Unsupported(_)));
    }
}
