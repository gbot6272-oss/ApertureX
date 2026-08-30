//! Metadaten-Filter für den Export (Phase 8 Schritt 2, `PLAN.md`:
//! „Metadaten-Filter, welche EXIF/IPTC-Felder mit exportiert werden").
//!
//! **Bewusste Vereinfachung:** keiner der fünf Ausgabeformate-Encoder aus
//! `format.rs` (alle über das `image`-Crate) unterstützt das Schreiben
//! beliebiger Metadaten. Dieses Modul implementiert stattdessen einen
//! minimalen, echten EXIF-Writer für **JPEG** — ein flaches IFD0 mit einer
//! kleinen, kuratierten Tag-Auswahl (Make/Model/DateTime/Copyright/
//! Artist, alle ASCII-Strings), der als APP1-Segment direkt nach dem
//! JPEG-SOI-Marker eingefügt wird (siehe [`embed_into_jpeg`]). PNG/TIFF/
//! WebP/AVIF exportieren weiterhin ohne eingebettete Metadaten — das ist
//! kein Rückschritt (sie hatten davor auch keine), nur ein noch nicht
//! erweiterter Anwendungsbereich. **GPS-Koordinaten und `DateTimeOriginal`
//! (das würde ein zusätzliches Exif-Sub-IFD brauchen) bleiben ebenfalls
//! zurückgestellt** — hier wird bewusst das einfachere IFD0-`DateTime`-Tag
//! verwendet statt der vollen Exif-Sub-IFD-Struktur.

use crate::error::{ExportError, Result};

/// Welche Felder ein Export einbetten soll — `None` heißt jeweils
/// „weglassen", nicht „leer einbetten".
#[derive(Debug, Clone, Default)]
pub struct MetadataFilter {
    pub make: Option<String>,
    pub model: Option<String>,
    /// `"YYYY:MM:DD HH:MM:SS"` — EXIF-Datumsformat, siehe Moduldoku.
    pub date_time: Option<String>,
    pub copyright: Option<String>,
    pub artist: Option<String>,
}

impl MetadataFilter {
    fn entries(&self) -> Vec<(u16, &str)> {
        let mut entries = Vec::new();
        if let Some(v) = &self.make {
            entries.push((0x010F, v.as_str())); // Make
        }
        if let Some(v) = &self.model {
            entries.push((0x0110, v.as_str())); // Model
        }
        if let Some(v) = &self.date_time {
            entries.push((0x0132, v.as_str())); // DateTime
        }
        if let Some(v) = &self.artist {
            entries.push((0x013B, v.as_str())); // Artist
        }
        if let Some(v) = &self.copyright {
            entries.push((0x8298, v.as_str())); // Copyright
        }
        entries
    }

    pub fn is_empty(&self) -> bool {
        self.entries().is_empty()
    }
}

/// Baut ein vollständiges JPEG-APP1-Segment (Marker, Länge, `Exif\0\0`,
/// ein minimaler kleinen-Endian-TIFF-Header und ein flaches IFD0) aus
/// `filter`. `None`, wenn `filter` leer ist (nichts einzubetten).
pub fn build_exif_app1_segment(filter: &MetadataFilter) -> Option<Vec<u8>> {
    let entries = filter.entries();
    if entries.is_empty() {
        return None;
    }

    // TIFF-Header (little-endian "II") beginnt bei Offset 0 innerhalb
    // dieses Bereichs; alle Offsets in den IFD-Einträgen sind relativ
    // dazu, siehe TIFF-6.0-Spezifikation.
    let mut tiff = Vec::new();
    tiff.extend_from_slice(b"II"); // Byte-Reihenfolge: Intel/little-endian
    tiff.extend_from_slice(&42u16.to_le_bytes()); // TIFF-Magic-Zahl
    tiff.extend_from_slice(&8u32.to_le_bytes()); // Offset zu IFD0

    let entry_count = entries.len() as u16;
    // IFD0-Startoffset ist fix 8 (direkt nach dem 8-Byte-Header) —
    // Einträge à 12 Byte, danach 4 Byte „nächstes IFD" (0 = keins), dann
    // der Datenbereich für Werte > 4 Byte (jeder unserer ASCII-Strings,
    // sofern nicht zufällig ≤ 4 Byte inkl. Nullterminierung).
    let ifd_start = 8u32;
    let data_area_start = ifd_start + 2 + u32::from(entry_count) * 12 + 4;

    let mut ifd = Vec::new();
    ifd.extend_from_slice(&entry_count.to_le_bytes());

    let mut data_area = Vec::new();
    let mut next_data_offset = data_area_start;
    for (tag, value) in &entries {
        let mut bytes = value.as_bytes().to_vec();
        bytes.push(0); // Nullterminierung, wie TIFF-ASCII-Typ es verlangt
        let count = bytes.len() as u32;

        ifd.extend_from_slice(&tag.to_le_bytes());
        ifd.extend_from_slice(&2u16.to_le_bytes()); // Typ 2 = ASCII
        ifd.extend_from_slice(&count.to_le_bytes());

        if bytes.len() <= 4 {
            let mut inline = [0u8; 4];
            inline[..bytes.len()].copy_from_slice(&bytes);
            ifd.extend_from_slice(&inline);
        } else {
            ifd.extend_from_slice(&next_data_offset.to_le_bytes());
            next_data_offset += bytes.len() as u32;
            data_area.extend_from_slice(&bytes);
        }
    }
    ifd.extend_from_slice(&0u32.to_le_bytes()); // kein weiteres IFD

    tiff.extend_from_slice(&ifd);
    tiff.extend_from_slice(&data_area);

    let mut segment = Vec::new();
    segment.extend_from_slice(&[0xFF, 0xE1]); // APP1-Marker
    let payload_len = (2 + 6 + tiff.len()) as u16; // Länge inkl. der 2 Längen-Bytes selbst
    segment.extend_from_slice(&payload_len.to_be_bytes());
    segment.extend_from_slice(b"Exif\0\0");
    segment.extend_from_slice(&tiff);
    Some(segment)
}

/// Fügt `app1_segment` direkt nach dem JPEG-SOI-Marker (`FFD8`) in
/// `jpeg_bytes` ein. Gibt einen Fehler zurück, wenn `jpeg_bytes` nicht mit
/// einem gültigen SOI beginnt (kein JPEG) — kein stiller Fehlschlag mit
/// einer beschädigten Datei.
pub fn embed_into_jpeg(jpeg_bytes: &[u8], app1_segment: &[u8]) -> Result<Vec<u8>> {
    if jpeg_bytes.len() < 2 || jpeg_bytes[0] != 0xFF || jpeg_bytes[1] != 0xD8 {
        return Err(ExportError::Unsupported(
            "Eingabe ist kein gültiges JPEG (fehlender SOI-Marker)".to_string(),
        ));
    }
    let mut out = Vec::with_capacity(jpeg_bytes.len() + app1_segment.len());
    out.extend_from_slice(&jpeg_bytes[0..2]);
    out.extend_from_slice(app1_segment);
    out.extend_from_slice(&jpeg_bytes[2..]);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_filter_produces_no_segment() {
        assert!(build_exif_app1_segment(&MetadataFilter::default()).is_none());
    }

    #[test]
    fn segment_has_valid_app1_header_and_exif_signature() {
        let filter = MetadataFilter {
            make: Some("Canon".to_string()),
            model: Some("EOS R5".to_string()),
            ..Default::default()
        };
        let segment = build_exif_app1_segment(&filter).unwrap();
        assert_eq!(&segment[0..2], &[0xFF, 0xE1]);
        assert_eq!(&segment[4..10], b"Exif\0\0");
        // TIFF-Header direkt danach: "II" + Magic 42 (little-endian).
        assert_eq!(&segment[10..12], b"II");
        assert_eq!(u16::from_le_bytes([segment[12], segment[13]]), 42);
    }

    #[test]
    fn segment_length_field_matches_actual_payload() {
        let filter = MetadataFilter {
            copyright: Some("© 2026 Test".to_string()),
            ..Default::default()
        };
        let segment = build_exif_app1_segment(&filter).unwrap();
        let declared_len = u16::from_be_bytes([segment[2], segment[3]]) as usize;
        // Deklarierte Länge zählt die 2 Längen-Bytes mit, aber nicht den
        // 2-Byte-Marker davor.
        assert_eq!(declared_len, segment.len() - 2);
    }

    #[test]
    fn embed_into_jpeg_inserts_segment_right_after_soi() {
        let fake_jpeg = [0xFFu8, 0xD8, 0xFF, 0xE0, 0x00, 0x10]; // SOI + Anfang eines APP0
        let filter = MetadataFilter {
            artist: Some("Ada Lovelace".to_string()),
            ..Default::default()
        };
        let segment = build_exif_app1_segment(&filter).unwrap();
        let out = embed_into_jpeg(&fake_jpeg, &segment).unwrap();
        assert_eq!(&out[0..2], &[0xFF, 0xD8]);
        assert_eq!(&out[2..2 + segment.len()], segment.as_slice());
        assert_eq!(&out[2 + segment.len()..], &fake_jpeg[2..]);
    }

    #[test]
    fn embed_into_jpeg_rejects_non_jpeg_input() {
        let filter = MetadataFilter {
            make: Some("X".to_string()),
            ..Default::default()
        };
        let segment = build_exif_app1_segment(&filter).unwrap();
        let err = embed_into_jpeg(&[0x00, 0x01, 0x02], &segment).unwrap_err();
        assert!(matches!(err, ExportError::Unsupported(_)));
    }

    #[test]
    fn long_value_is_stored_in_the_data_area_not_inline() {
        // "EOS R5" ist 6 Byte + Nullterminierung = 7 > 4 -> Datenbereich.
        let filter = MetadataFilter {
            model: Some("EOS R5".to_string()),
            ..Default::default()
        };
        let segment = build_exif_app1_segment(&filter).unwrap();
        // Der String sollte irgendwo im Segment als Bytes auftauchen
        // (im Datenbereich nach dem IFD).
        let needle = b"EOS R5\0";
        assert!(segment.windows(needle.len()).any(|w| w == needle));
    }
}
