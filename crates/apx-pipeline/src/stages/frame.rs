//! Rahmen und Passepartout (Phase 32 F8).
//!
//! Drei ineinanderliegende Ränder um das Bild: außen eine Rahmenlinie,
//! darin das Passepartout (die breite, meist helle Fläche), innen eine
//! feine Keylinie direkt am Foto. Genau der Aufbau eines gerahmten
//! Abzugs.
//!
//! **Warum der Rahmen die Bildfläche NICHT vergrößert.** Er wird
//! innerhalb der vorhandenen Ausgabegröße gezeichnet: das Foto wird um
//! die Rahmenbreite verkleinert und in die Mitte gesetzt, der frei
//! gewordene Ring trägt die Ränder. Eine wachsende Leinwand würde jede
//! Größenangabe im Export, jedes Zuschnitt-Rechteck, jedes Overlay und
//! die Vorschau-Geometrie verschieben — für eine reine
//! Präsentationszugabe ein zu hoher Preis. Wer ein größeres Blatt will,
//! stellt im Export die Zielgröße größer ein.
//!
//! **Warum diese Stufe nach dem Zuschnitt läuft und nur auf der CPU.**
//! Nach dem Zuschnitt, weil ein Rahmen vor `geometry` schlicht
//! weggeschnitten würde — der Rahmen gehört um das fertige Bild. Dort
//! liegen die Daten als RGBA8 auf der CPU vor (`geometry` ändert die
//! Abmessungen und rechnet ohnehin dort). Ein Compute-Dispatch nur zum
//! Füllen von vier Rechtecken würde mehr Zeit mit Hoch- und Herunterladen
//! verbringen als mit Rechnen — anders als bei den per-Pixel-Stufen im
//! linearen Raum, die ihren WGSL-Zwilling zu Recht haben.

use crate::edl::v4::FrameAdjustment;

/// Kleinster Anteil der kürzeren Bildkante, der dem Foto bleiben muss.
///
/// Ohne diese Grenze ergäbe ein weit aufgedrehter Regler auf einem
/// schmalen Bild ein Rechteck aus reinem Passepartout — formal korrekt,
/// aber niemand will das, und die Vorschau sähe nach einem Fehler aus.
const MIN_PHOTO_FRACTION: f32 = 0.2;

/// Breiten in Pixeln, bereits auf die Bildgröße bezogen und begrenzt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameWidths {
    pub border: u32,
    pub mat: u32,
    pub inner_line: u32,
}

impl FrameWidths {
    pub fn total(&self) -> u32 {
        self.border + self.mat + self.inner_line
    }
}

/// Rechnet die Prozentregler in Pixel um — bezogen auf die **kürzere**
/// Bildkante.
///
/// Auf die kürzere Kante, damit der Rand rundherum gleich dick ist. Ein
/// Anteil je Achse ergäbe auf einem Querformat oben und unten einen
/// schmaleren Rand als links und rechts, was wie ein Fehler aussieht.
///
/// Übersteigen die Ränder zusammen das Erlaubte, werden **alle drei
/// gleichmäßig** heruntergerechnet: das Verhältnis, das der Nutzer
/// eingestellt hat, bleibt erhalten, statt dass ein Rand einseitig
/// abgeschnitten wird.
pub fn widths_for(width: u32, height: u32, adjustment: &FrameAdjustment) -> FrameWidths {
    let short_edge = width.min(height) as f32;
    let px = |percent: f32| (percent.max(0.0) / 100.0 * short_edge).round().max(0.0);

    let mut border = px(adjustment.border_width);
    let mut mat = px(adjustment.mat_width);
    let mut line = px(adjustment.inner_line_width);

    let total = border + mat + line;
    let allowed = (short_edge * (1.0 - MIN_PHOTO_FRACTION) / 2.0).floor();
    if total > allowed && total > 0.0 {
        let factor = allowed / total;
        border = (border * factor).floor();
        mat = (mat * factor).floor();
        line = (line * factor).floor();
    }

    FrameWidths {
        border: border as u32,
        mat: mat as u32,
        inner_line: line as u32,
    }
}

fn to_byte(value: f32) -> u8 {
    (value.clamp(0.0, 1.0) * 255.0).round() as u8
}

/// Bilineare Abtastung des Quellbildes an einer Gleitkomma-Position.
fn sample_bilinear(rgba: &[u8], width: u32, height: u32, x: f32, y: f32) -> [u8; 4] {
    let max_x = (width - 1) as f32;
    let max_y = (height - 1) as f32;
    let x = x.clamp(0.0, max_x);
    let y = y.clamp(0.0, max_y);
    let x0 = x.floor();
    let y0 = y.floor();
    let fx = x - x0;
    let fy = y - y0;
    let x0 = x0 as u32;
    let y0 = y0 as u32;
    let x1 = (x0 + 1).min(width - 1);
    let y1 = (y0 + 1).min(height - 1);

    let at = |px: u32, py: u32, channel: usize| -> f32 {
        rgba[((py * width + px) as usize) * 4 + channel] as f32
    };

    let mut out = [0u8; 4];
    for (channel, value) in out.iter_mut().enumerate() {
        let top = at(x0, y0, channel) + (at(x1, y0, channel) - at(x0, y0, channel)) * fx;
        let bottom = at(x0, y1, channel) + (at(x1, y1, channel) - at(x0, y1, channel)) * fx;
        *value = (top + (bottom - top) * fy).round().clamp(0.0, 255.0) as u8;
    }
    out
}

/// Zeichnet den Rahmen. Abmessungen bleiben unverändert.
///
/// Bei Breite 0 auf allen drei Rändern wird die Eingabe **unverändert**
/// zurückgegeben (bit-genau, siehe Test) — ein gespeichertes EDL ohne
/// Rahmen rendert damit exakt wie vor dieser Stufe.
pub fn apply(rgba: &[u8], width: u32, height: u32, adjustment: &FrameAdjustment) -> Vec<u8> {
    let widths = widths_for(width, height, adjustment);
    let total = widths.total();
    if total == 0 || width == 0 || height == 0 {
        return rgba.to_vec();
    }

    let border = to_rgba(adjustment.border_color);
    let mat = to_rgba(adjustment.mat_color);
    let line = to_rgba(adjustment.inner_line_color);

    let inner_w = width.saturating_sub(total * 2).max(1);
    let inner_h = height.saturating_sub(total * 2).max(1);

    let mut out = vec![0u8; (width * height) as usize * 4];
    for y in 0..height {
        for x in 0..width {
            let idx = ((y * width + x) as usize) * 4;
            // Abstand zum nächsten Bildrand entscheidet über das Band.
            let edge = x.min(y).min(width - 1 - x).min(height - 1 - y);
            let pixel = if edge < widths.border {
                border
            } else if edge < widths.border + widths.mat {
                mat
            } else if edge < total {
                line
            } else {
                // Das ganze Foto in das innere Rechteck skalieren — nichts
                // vom Bild geht verloren, es wird nur kleiner.
                let u = (x - total) as f32 / (inner_w.max(1) as f32 - 1.0).max(1.0);
                let v = (y - total) as f32 / (inner_h.max(1) as f32 - 1.0).max(1.0);
                sample_bilinear(
                    rgba,
                    width,
                    height,
                    u * (width - 1) as f32,
                    v * (height - 1) as f32,
                )
            };
            out[idx..idx + 4].copy_from_slice(&pixel);
        }
    }
    out
}

fn to_rgba(color: [f32; 3]) -> [u8; 4] {
    [to_byte(color[0]), to_byte(color[1]), to_byte(color[2]), 255]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn solid(width: u32, height: u32, color: [u8; 4]) -> Vec<u8> {
        color
            .iter()
            .copied()
            .cycle()
            .take((width * height) as usize * 4)
            .collect()
    }

    fn pixel(rgba: &[u8], width: u32, x: u32, y: u32) -> [u8; 4] {
        let idx = ((y * width + x) as usize) * 4;
        [rgba[idx], rgba[idx + 1], rgba[idx + 2], rgba[idx + 3]]
    }

    #[test]
    fn a_neutral_frame_returns_the_image_bit_for_bit() {
        // Der Rückwärtskompatibilitäts-Test: ein gespeichertes EDL ohne
        // Rahmenfelder liest sie als Neutralwert und muss exakt dasselbe
        // Bild ergeben wie vor dieser Stufe.
        let image = solid(16, 12, [10, 20, 30, 255]);
        let out = apply(&image, 16, 12, &FrameAdjustment::NEUTRAL);
        assert_eq!(out, image);
    }

    #[test]
    fn the_output_keeps_the_input_dimensions() {
        let image = solid(40, 30, [128, 128, 128, 255]);
        let adjustment = FrameAdjustment {
            mat_width: 10.0,
            border_width: 2.0,
            ..FrameAdjustment::NEUTRAL
        };
        let out = apply(&image, 40, 30, &adjustment);
        assert_eq!(out.len(), image.len());
    }

    #[test]
    fn the_bands_appear_outside_in_border_then_mat_then_line() {
        let image = solid(100, 100, [200, 200, 200, 255]);
        let adjustment = FrameAdjustment {
            border_width: 2.0,
            border_color: [0.0, 0.0, 0.0],
            mat_width: 8.0,
            mat_color: [1.0, 1.0, 1.0],
            inner_line_width: 1.0,
            inner_line_color: [1.0, 0.0, 0.0],
        };
        let out = apply(&image, 100, 100, &adjustment);
        // 2 % / 8 % / 1 % von 100 px = 2 / 8 / 1 px.
        assert_eq!(pixel(&out, 100, 0, 50), [0, 0, 0, 255], "äußerste Linie");
        assert_eq!(
            pixel(&out, 100, 5, 50),
            [255, 255, 255, 255],
            "Passepartout"
        );
        assert_eq!(pixel(&out, 100, 10, 50), [255, 0, 0, 255], "Keylinie");
        assert_eq!(pixel(&out, 100, 50, 50), [200, 200, 200, 255], "Foto");
    }

    #[test]
    fn widths_use_the_shorter_edge_so_the_border_is_equally_thick_all_around() {
        // Querformat 200x100: 10 % müssen 10 px sein (kurze Kante), nicht
        // 20 px links/rechts und 10 px oben/unten.
        let widths = widths_for(
            200,
            100,
            &FrameAdjustment {
                mat_width: 10.0,
                ..FrameAdjustment::NEUTRAL
            },
        );
        assert_eq!(widths.mat, 10);
    }

    #[test]
    fn oversized_widths_are_scaled_down_together_keeping_their_ratio() {
        // 60 % + 30 % + 10 % wären zusammen die ganze Bildbreite. Erlaubt
        // ist (1 − 0,2)/2 = 40 % je Seite.
        let widths = widths_for(
            100,
            100,
            &FrameAdjustment {
                border_width: 10.0,
                mat_width: 60.0,
                inner_line_width: 30.0,
                ..FrameAdjustment::NEUTRAL
            },
        );
        assert!(widths.total() <= 40, "gesamt war {}", widths.total());
        // Das Verhältnis 1 : 6 : 3 bleibt ungefähr erhalten.
        assert!(widths.mat > widths.inner_line);
        assert!(widths.inner_line > widths.border);
    }

    #[test]
    fn the_whole_photo_stays_visible_just_smaller() {
        // Ein Bild mit einer markanten Ecke: nach dem Rahmen muss die
        // Ecke im inneren Rechteck wieder auftauchen, nicht abgeschnitten
        // sein.
        let mut image = solid(50, 50, [0, 0, 0, 255]);
        // Zeile 0, Spalte 49 — die rechte obere Ecke.
        let corner = 49_usize * 4;
        image[corner..corner + 4].copy_from_slice(&[255, 255, 255, 255]);

        let out = apply(
            &image,
            50,
            50,
            &FrameAdjustment {
                mat_width: 10.0,
                ..FrameAdjustment::NEUTRAL
            },
        );
        // Rechte obere Ecke des inneren Rechtecks (5 px Rand bei 10 %).
        let inner_corner = pixel(&out, 50, 44, 5);
        assert!(
            inner_corner[0] > 100,
            "helle Ecke fehlt, war {inner_corner:?}"
        );
    }
}
