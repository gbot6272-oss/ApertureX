//! Drucken (Phase 8 Schritt 3, `PLAN.md`: „Einzelbild/Kontaktbogen/
//! Bilderpaket/benutzerdefiniertes Raster, Randeinstellungen/Zellen/Zoom,
//! Druckschärfung/Farbmanagement/Druckauflösung, Speichern als JPEG").
//!
//! **Architektur:** alle vier Layout-Arten sind am Ende dieselbe generische
//! Operation — eine Liste rechteckiger Zellen (`PrintSlot`, in Zoll) auf
//! einer Seite fester Größe/Auflösung, jede mit einem bereits gerenderten
//! Foto befüllt ([`compose_page`]). [`grid_slots`] erzeugt die
//! Zell-Rechtecke für Einzelbild (1×1), Kontaktbogen und benutzerdefiniertes
//! Raster (alle drei: eine gleichmäßige `cols`×`rows`-Aufteilung, nur die
//! Werte unterscheiden sich). Bilderpaket (**bewusst vereinfacht**: feste
//! Vorlagen statt eines allgemeinen Bin-Packing-Algorithmus für beliebige
//! Papierformat-Mischungen, siehe [`picture_package_slots`]) liefert die
//! Zell-Rechtecke direkt als feste Liste.
//!
//! Druckschärfung/Farbmanagement laufen bereits in
//! `engine::render_to_pixels` (dieselben `sharpen`/`icc`-Module wie beim
//! normalen Export) — dieses Modul bekommt fertig gerenderte Zell-Bilder
//! und kümmert sich nur noch um Platzierung + Größenanpassung aufs Zell-
//! Rechteck sowie die Seiten-Rasterauflösung (`dpi`). „Speichern als JPEG"
//! ist die fertige Seite einfach durch `format::encode_rgba8` geschickt —
//! kein System-Druckdialog-Zugriff in dieser Phase (siehe ADR-0034).

use crate::error::Result;
use crate::resize;

/// Ein Zell-Rechteck auf der Druckseite, in Zoll (Ursprung oben links).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PrintSlot {
    pub x_in: f32,
    pub y_in: f32,
    pub width_in: f32,
    pub height_in: f32,
}

/// Wie ein zu großes/zu kleines Zellbild ins Zell-Rechteck eingepasst
/// wird — „Zoom" aus `PLAN.md`s Schritt-3-Beschreibung.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FitMode {
    /// Ganzes Bild sichtbar, ggf. Rand innerhalb der Zelle (wie CSS
    /// `object-fit: contain`).
    Contain,
    /// Zelle vollständig gefüllt, Bild ggf. beschnitten (wie CSS
    /// `object-fit: cover`).
    Cover,
}

/// Erzeugt eine gleichmäßige `cols`×`rows`-Rasteraufteilung innerhalb der
/// Seite (abzüglich `margin_in` an allen vier Rändern, `gap_in` zwischen
/// den Zellen) — deckt Einzelbild (`cols = rows = 1`), Kontaktbogen und
/// benutzerdefiniertes Raster gleichermaßen ab.
pub fn grid_slots(
    page_width_in: f32,
    page_height_in: f32,
    margin_in: f32,
    gap_in: f32,
    cols: u32,
    rows: u32,
) -> Vec<PrintSlot> {
    if cols == 0 || rows == 0 {
        return Vec::new();
    }
    let usable_w = (page_width_in - 2.0 * margin_in - (cols - 1) as f32 * gap_in).max(0.0);
    let usable_h = (page_height_in - 2.0 * margin_in - (rows - 1) as f32 * gap_in).max(0.0);
    let cell_w = usable_w / cols as f32;
    let cell_h = usable_h / rows as f32;

    let mut slots = Vec::with_capacity((cols * rows) as usize);
    for row in 0..rows {
        for col in 0..cols {
            slots.push(PrintSlot {
                x_in: margin_in + col as f32 * (cell_w + gap_in),
                y_in: margin_in + row as f32 * (cell_h + gap_in),
                width_in: cell_w,
                height_in: cell_h,
            });
        }
    }
    slots
}

/// Feste Bilderpaket-Vorlagen (siehe Moduldoku — bewusst kein allgemeines
/// Bin-Packing). Maße in Zoll, für eine 8×10"-Seite ausgelegt (Standard-
/// Fotopapiergröße); auf anderen Seitengrößen zentriert die Vorlage sich
/// nicht automatisch — das ist der Umfang dieser Vereinfachung.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PicturePackageTemplate {
    /// Ein 5×7" + zwei 2.5×3.5" (klassisches Kleinpaket).
    OneLargeTwoSmall,
    /// Vier 4×5"-Abzüge (Wallet-artige Aufteilung).
    FourEqual,
    /// Acht Wallet-Abzüge (2×2.5").
    EightWallet,
}

pub fn picture_package_slots(template: PicturePackageTemplate) -> Vec<PrintSlot> {
    match template {
        PicturePackageTemplate::OneLargeTwoSmall => vec![
            PrintSlot {
                x_in: 0.25,
                y_in: 0.25,
                width_in: 5.0,
                height_in: 7.0,
            },
            PrintSlot {
                x_in: 5.5,
                y_in: 0.25,
                width_in: 2.5,
                height_in: 3.5,
            },
            PrintSlot {
                x_in: 5.5,
                y_in: 4.0,
                width_in: 2.5,
                height_in: 3.5,
            },
        ],
        PicturePackageTemplate::FourEqual => grid_slots(8.0, 10.0, 0.25, 0.25, 2, 2),
        PicturePackageTemplate::EightWallet => grid_slots(8.0, 10.0, 0.25, 0.15, 2, 4),
    }
}

/// Ein bereits gerendertes Zellbild (siehe `engine::render_to_pixels`)
/// zusammen mit dem Zell-Rechteck, in das es eingepasst werden soll.
pub struct PrintCell<'a> {
    pub slot: PrintSlot,
    pub width: u32,
    pub height: u32,
    pub rgba: &'a [u8],
    pub fit: FitMode,
}

/// Setzt `cells` auf einer `page_width_in`×`page_height_in`"-Seite bei
/// `dpi` Pixel/Zoll zusammen, auf einem `background_rgb`-farbenen
/// Hintergrund (z. B. Weiß) — die Auflösung der Ausgabe, nicht der
/// Eingabefotos.
pub fn compose_page(
    page_width_in: f32,
    page_height_in: f32,
    dpi: u32,
    background_rgb: [u8; 3],
    cells: &[PrintCell],
) -> Result<(u32, u32, Vec<u8>)> {
    let page_w = (page_width_in * dpi as f32).round().max(1.0) as u32;
    let page_h = (page_height_in * dpi as f32).round().max(1.0) as u32;

    let mut page = vec![0u8; page_w as usize * page_h as usize * 4];
    for pixel in page.chunks_exact_mut(4) {
        pixel[0] = background_rgb[0];
        pixel[1] = background_rgb[1];
        pixel[2] = background_rgb[2];
        pixel[3] = 255;
    }

    for cell in cells {
        let slot_w = ((cell.slot.width_in * dpi as f32).round().max(1.0)) as u32;
        let slot_h = ((cell.slot.height_in * dpi as f32).round().max(1.0)) as u32;
        let origin_x = (cell.slot.x_in * dpi as f32).round() as i64;
        let origin_y = (cell.slot.y_in * dpi as f32).round() as i64;

        let (fit_w, fit_h) = fit_dimensions(cell.width, cell.height, slot_w, slot_h, cell.fit);
        let resized = resize::resize_rgba8(cell.width, cell.height, cell.rgba, fit_w, fit_h)?;

        // Bei `Contain` zentriert innerhalb der Zelle, bei `Cover` bereits
        // zellfüllend (Beschnitt passiert implizit durch den unten
        // folgenden Clip an den Zellgrenzen).
        let cell_offset_x = origin_x + ((slot_w as i64 - fit_w as i64) / 2).max(0);
        let cell_offset_y = origin_y + ((slot_h as i64 - fit_h as i64) / 2).max(0);
        let clip_w = fit_w.min(slot_w);
        let clip_h = fit_h.min(slot_h);
        let src_skip_x = (fit_w.saturating_sub(clip_w)) / 2;
        let src_skip_y = (fit_h.saturating_sub(clip_h)) / 2;

        for y in 0..clip_h {
            let dst_y = cell_offset_y + y as i64;
            if dst_y < 0 || dst_y >= page_h as i64 {
                continue;
            }
            for x in 0..clip_w {
                let dst_x = cell_offset_x + x as i64;
                if dst_x < 0 || dst_x >= page_w as i64 {
                    continue;
                }
                let src_idx =
                    ((y + src_skip_y) as usize * fit_w as usize + (x + src_skip_x) as usize) * 4;
                let dst_idx = (dst_y as usize * page_w as usize + dst_x as usize) * 4;
                page[dst_idx..dst_idx + 4].copy_from_slice(&resized[src_idx..src_idx + 4]);
            }
        }
    }

    Ok((page_w, page_h, page))
}

fn fit_dimensions(
    src_w: u32,
    src_h: u32,
    target_w: u32,
    target_h: u32,
    fit: FitMode,
) -> (u32, u32) {
    if src_w == 0 || src_h == 0 || target_w == 0 || target_h == 0 {
        return (target_w.max(1), target_h.max(1));
    }
    let src_aspect = src_w as f32 / src_h as f32;
    let target_aspect = target_w as f32 / target_h as f32;
    let scale_to_width_first = match fit {
        FitMode::Contain => src_aspect > target_aspect,
        FitMode::Cover => src_aspect <= target_aspect,
    };
    if scale_to_width_first {
        let w = target_w;
        let h = ((target_w as f32 / src_aspect).round().max(1.0)) as u32;
        (w, h)
    } else {
        let h = target_h;
        let w = ((target_h as f32 * src_aspect).round().max(1.0)) as u32;
        (w, h)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grid_slots_single_cell_fills_page_minus_margin() {
        let slots = grid_slots(8.0, 10.0, 0.5, 0.25, 1, 1);
        assert_eq!(slots.len(), 1);
        assert_eq!(slots[0].x_in, 0.5);
        assert_eq!(slots[0].y_in, 0.5);
        assert_eq!(slots[0].width_in, 7.0);
        assert_eq!(slots[0].height_in, 9.0);
    }

    #[test]
    fn grid_slots_contact_sheet_produces_cols_times_rows_cells() {
        let slots = grid_slots(8.0, 10.0, 0.25, 0.1, 3, 4);
        assert_eq!(slots.len(), 12);
    }

    #[test]
    fn grid_slots_returns_empty_for_zero_cols_or_rows() {
        assert!(grid_slots(8.0, 10.0, 0.0, 0.0, 0, 5).is_empty());
    }

    #[test]
    fn picture_package_templates_each_produce_at_least_one_slot() {
        assert!(!picture_package_slots(PicturePackageTemplate::OneLargeTwoSmall).is_empty());
        assert!(!picture_package_slots(PicturePackageTemplate::FourEqual).is_empty());
        assert!(!picture_package_slots(PicturePackageTemplate::EightWallet).is_empty());
    }

    fn solid_image(width: u32, height: u32, rgba: [u8; 4]) -> Vec<u8> {
        (0..width * height).flat_map(|_| rgba).collect()
    }

    #[test]
    fn compose_page_produces_requested_pixel_dimensions_at_given_dpi() {
        let (w, h, _) = compose_page(4.0, 6.0, 100, [255, 255, 255], &[]).unwrap();
        assert_eq!(w, 400);
        assert_eq!(h, 600);
    }

    #[test]
    fn compose_page_fills_background_where_no_cell_covers() {
        let (_, _, page) = compose_page(1.0, 1.0, 10, [10, 20, 30], &[]).unwrap();
        assert_eq!(&page[0..4], &[10, 20, 30, 255]);
    }

    #[test]
    fn compose_page_places_a_cell_image_inside_its_slot() {
        let cell_rgba = solid_image(4, 4, [255, 0, 0, 255]);
        let cells = [PrintCell {
            slot: PrintSlot {
                x_in: 0.0,
                y_in: 0.0,
                width_in: 1.0,
                height_in: 1.0,
            },
            width: 4,
            height: 4,
            rgba: &cell_rgba,
            fit: FitMode::Contain,
        }];
        let (w, _, page) = compose_page(2.0, 1.0, 10, [0, 0, 0], &cells).unwrap();
        // Zelle deckt die linke Bildhälfte ab (0..10 von 20 Pixeln Breite).
        assert_eq!(&page[0..4], &[255, 0, 0, 255]);
        // Rechts von der Zelle bleibt Hintergrund.
        let right_pixel_idx = (w as usize - 1) * 4;
        assert_eq!(&page[right_pixel_idx..right_pixel_idx + 4], &[0, 0, 0, 255]);
    }

    #[test]
    fn fit_contain_preserves_full_source_within_bounds() {
        let (w, h) = fit_dimensions(400, 200, 100, 100, FitMode::Contain);
        assert!(w <= 100 && h <= 100);
        assert_eq!(w as f32 / h as f32, 2.0);
    }

    #[test]
    fn fit_cover_fills_target_bounds_at_least_in_one_dimension() {
        let (w, h) = fit_dimensions(400, 200, 100, 100, FitMode::Cover);
        assert!(w >= 100 || h >= 100);
    }
}
