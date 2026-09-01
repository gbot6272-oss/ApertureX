//! Bildorientierung.
//!
//! `apx-raw` definiert einen eigenen `Orientation`-Typ statt `rawler`s Typ
//! direkt in der öffentlichen API zu verwenden — so bleibt die
//! RAW-Bibliothek austauschbar (siehe `DECISIONS.md` ADR-0002, Punkt 2:
//! `apx-raw` soll später ggf. dynamisch verlinkt oder ersetzt werden
//! können), ohne dass Aufrufer von `apx-raw` von `rawler`-Typen abhängen.
//!
//! **Wichtig gegen den Fallstrick "Orientierung doppelt angewendet"
//! (siehe `PHASE1_PROMPT.md` Abschnitt 10):** Die Orientierung wird
//! ausschließlich hier, beim Dekodieren in [`crate::decode`], auf die
//! Pixeldaten angewendet. Das Frontend bekommt bereits korrekt gedrehte
//! Bilddaten und darf keine eigene Rotation mehr vornehmen.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Orientation {
    Normal,
    Rotate90,
    Rotate180,
    Rotate270,
    FlipHorizontal,
    FlipVertical,
    Transpose,
    Transverse,
}

impl From<rawler::Orientation> for Orientation {
    fn from(value: rawler::Orientation) -> Self {
        match value {
            rawler::Orientation::Normal => Orientation::Normal,
            rawler::Orientation::Rotate90 => Orientation::Rotate90,
            rawler::Orientation::Rotate180 => Orientation::Rotate180,
            rawler::Orientation::Rotate270 => Orientation::Rotate270,
            rawler::Orientation::HorizontalFlip => Orientation::FlipHorizontal,
            rawler::Orientation::VerticalFlip => Orientation::FlipVertical,
            rawler::Orientation::Transpose => Orientation::Transpose,
            rawler::Orientation::Transverse => Orientation::Transverse,
            // Unbekannte/fehlende Orientierung wird als "Normal" behandelt —
            // besser ein unrotiertes Bild als eine geratene Drehung.
            rawler::Orientation::Unknown => Orientation::Normal,
        }
    }
}

impl Orientation {
    /// Ob diese Orientierung Breite und Höhe vertauscht (90°/270°-Drehungen
    /// und die Transpose-Varianten).
    pub fn swaps_dimensions(self) -> bool {
        matches!(
            self,
            Orientation::Rotate90
                | Orientation::Rotate270
                | Orientation::Transpose
                | Orientation::Transverse
        )
    }

    /// Wendet die Orientierung auf einen interleaved RGB16-Buffer
    /// (`3 * width * height` Elemente, Zeile für Zeile) an und gibt den neu
    /// angeordneten Buffer plus die resultierende (width, height) zurück.
    pub fn apply_rgb16(self, pixels: &[u16], width: u32, height: u32) -> (Vec<u16>, u32, u32) {
        self.apply_rgb(pixels, width, height)
    }

    /// Wie [`Orientation::apply_rgb16`], aber für einen interleaved
    /// RGB-`f32`-Puffer — dieselbe Geometrie, nur vor der 16-Bit-
    /// Quantisierung angewendet. Siehe `apx_raw::decode_linear` und
    /// `DECISIONS.md` ADR-0015.
    pub fn apply_rgb_f32(self, pixels: &[f32], width: u32, height: u32) -> (Vec<f32>, u32, u32) {
        self.apply_rgb(pixels, width, height)
    }

    fn apply_rgb<T: Copy + Default>(
        self,
        pixels: &[T],
        width: u32,
        height: u32,
    ) -> (Vec<T>, u32, u32) {
        let (w, h) = (width as usize, height as usize);
        debug_assert_eq!(pixels.len(), w * h * 3);

        let (out_w, out_h) = if self.swaps_dimensions() {
            (h, w)
        } else {
            (w, h)
        };
        let mut out = vec![T::default(); out_w * out_h * 3];

        // Für jeden Ausgabe-Pixel (ox, oy) wird die passende Quellposition
        // (sx, sy) im Originalbild berechnet. Das ist einfacher und
        // weniger fehleranfällig, als für jede der acht Orientierungen
        // eine eigene Kopierschleife zu schreiben.
        for oy in 0..out_h {
            for ox in 0..out_w {
                let (sx, sy) = match self {
                    Orientation::Normal => (ox, oy),
                    Orientation::FlipHorizontal => (w - 1 - ox, oy),
                    Orientation::Rotate180 => (w - 1 - ox, h - 1 - oy),
                    Orientation::FlipVertical => (ox, h - 1 - oy),
                    Orientation::Transpose => (oy, ox),
                    Orientation::Rotate90 => (oy, h - 1 - ox),
                    Orientation::Transverse => (h - 1 - oy, w - 1 - ox),
                    Orientation::Rotate270 => (w - 1 - oy, ox),
                };
                let src_idx = (sy * w + sx) * 3;
                let dst_idx = (oy * out_w + ox) * 3;
                out[dst_idx..dst_idx + 3].copy_from_slice(&pixels[src_idx..src_idx + 3]);
            }
        }

        (out, out_w as u32, out_h as u32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_2x1() -> (Vec<u16>, u32, u32) {
        // Zwei Pixel nebeneinander: links rot, rechts grün.
        (vec![255, 0, 0, 0, 255, 0], 2, 1)
    }

    #[test]
    fn normal_is_identity() {
        let (pixels, w, h) = make_2x1();
        let (out, ow, oh) = Orientation::Normal.apply_rgb16(&pixels, w, h);
        assert_eq!((ow, oh), (w, h));
        assert_eq!(out, pixels);
    }

    #[test]
    fn horizontal_flip_swaps_pixels() {
        let (pixels, w, h) = make_2x1();
        let (out, ow, oh) = Orientation::FlipHorizontal.apply_rgb16(&pixels, w, h);
        assert_eq!((ow, oh), (w, h));
        // Rot und Grün müssen die Plätze getauscht haben.
        assert_eq!(&out[0..3], &[0, 255, 0]);
        assert_eq!(&out[3..6], &[255, 0, 0]);
    }

    #[test]
    fn rotate90_swaps_dimensions() {
        let (pixels, w, h) = make_2x1();
        assert!(Orientation::Rotate90.swaps_dimensions());
        let (out, ow, oh) = Orientation::Rotate90.apply_rgb16(&pixels, w, h);
        assert_eq!((ow, oh), (h, w));
        assert_eq!(out.len(), pixels.len());
    }

    #[test]
    fn unknown_rawler_orientation_maps_to_normal() {
        let converted: Orientation = rawler::Orientation::Unknown.into();
        assert_eq!(converted, Orientation::Normal);
    }

    #[test]
    fn rotate180_is_involution_for_even_dimensions() {
        let pixels: Vec<u16> = (0..(4 * 2 * 3)).map(|v| v as u16).collect();
        let (once, w, h) = Orientation::Rotate180.apply_rgb16(&pixels, 4, 2);
        let (twice, w2, h2) = Orientation::Rotate180.apply_rgb16(&once, w, h);
        assert_eq!((w2, h2), (4, 2));
        assert_eq!(twice, pixels);
    }

    // Ab hier: dieselben Fälle wie oben, aber für apply_rgb_f32 (siehe
    // apx_raw::decode_linear, DECISIONS.md ADR-0015) — beweist, dass die
    // generische apply_rgb-Implementierung für beide Elementtypen
    // identisch funktioniert.

    fn make_2x1_f32() -> (Vec<f32>, u32, u32) {
        (vec![1.0, 0.0, 0.0, 0.0, 1.0, 0.0], 2, 1)
    }

    #[test]
    fn f32_normal_is_identity() {
        let (pixels, w, h) = make_2x1_f32();
        let (out, ow, oh) = Orientation::Normal.apply_rgb_f32(&pixels, w, h);
        assert_eq!((ow, oh), (w, h));
        assert_eq!(out, pixels);
    }

    #[test]
    fn f32_horizontal_flip_swaps_pixels() {
        let (pixels, w, h) = make_2x1_f32();
        let (out, ow, oh) = Orientation::FlipHorizontal.apply_rgb_f32(&pixels, w, h);
        assert_eq!((ow, oh), (w, h));
        assert_eq!(&out[0..3], &[0.0, 1.0, 0.0]);
        assert_eq!(&out[3..6], &[1.0, 0.0, 0.0]);
    }

    #[test]
    fn f32_rotate90_swaps_dimensions() {
        let (pixels, w, h) = make_2x1_f32();
        let (out, ow, oh) = Orientation::Rotate90.apply_rgb_f32(&pixels, w, h);
        assert_eq!((ow, oh), (h, w));
        assert_eq!(out.len(), pixels.len());
    }
}
