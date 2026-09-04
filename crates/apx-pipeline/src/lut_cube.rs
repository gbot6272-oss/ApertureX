//! Selbst implementierter Parser für Adobe/Iridas-`.cube`-3D-LUT-Dateien
//! (Phase 16 Schritt 1, siehe `DECISIONS.md` ADR-0043) — dasselbe "kein
//! lizenzunklares/kaum gepflegtes Drittanbieter-Crate, stattdessen selbst
//! implementieren"-Muster wie `apx_ai::seam_carving` (Phase 15 Schritt 4):
//! die real geprüften Rust-Crates für `.cube`-Anwendung (`wagahai_lut`,
//! `lut-cube`) sind kaum verbreitet, Wartungsstatus unklar — der Parser
//! selbst ist dagegen simpel genug (ein Textformat, kein Binärformat),
//! um ihn ohne Risiko selbst zu schreiben.
//!
//! `.cube` ist ein offenes, patentfreies Textdateiformat (Industrie-
//! standard, u. a. von Lightroom, Premiere, DaVinci Resolve, Capture One
//! genutzt) — das Format selbst ist nicht schutzfähig, nur einzelne
//! `.cube`-*Dateien* (die konkreten Look-Daten) tragen ggf. eine eigene
//! Lizenz (siehe `THIRD_PARTY.md`s Phase-16-Sektion für die je Quelle
//! geprüfte Herkunft des Starter-Sets aus Schritt 2).
//!
//! **Bewusst nur 3D-LUTs** (`LUT_3D_SIZE`) — 1D-LUTs (`LUT_1D_SIZE`,
//! reine Tonwertkurven ohne Kreuzkanal-Wirkung) sind für "Foto-Filter/
//! Looks" die deutlich seltenere Variante und werden mit einer klaren
//! Fehlermeldung abgelehnt statt stillschweigend falsch interpretiert.
//!
//! **`DOMAIN_MIN`/`DOMAIN_MAX`**: laut Spezifikation optionale Angabe des
//! Eingabe-Wertebereichs (Standard `0 0 0`..`1 1 1`). Wird geparst und
//! bei der Anwendung (`stages::lut_filter`) zur Normierung genutzt — die
//! meisten frei verfügbaren `.cube`-Dateien nutzen den Standardbereich,
//! einige (insbesondere log-Farbraum-LUTs) weichen ab.
//!
//! **Unbekannte Schlüsselwörter werden übersprungen statt hart
//! abgelehnt** (z. B. herstellerspezifische Erweiterungen wie
//! `LUT_1D_INPUT_RANGE`) — dieselbe Toleranz-Haltung wie bei jedem
//! anderen Text-Format-Parser dieses Projekts (siehe z. B.
//! `import::presets`s Umgang mit unbekannten `.lrtemplate`-Feldern).

use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum LutParseError {
    MissingSize,
    Only1dSupported,
    SizeOutOfRange(u32),
    UnexpectedDataLine(usize),
    NotEnoughData { expected: usize, found: usize },
    InvalidNumber(usize),
}

impl fmt::Display for LutParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingSize => write!(
                f,
                "Keine 'LUT_3D_SIZE'-Zeile gefunden — keine gültige .cube-3D-LUT-Datei"
            ),
            Self::Only1dSupported => write!(
                f,
                "Datei enthält nur eine 1D-LUT ('LUT_1D_SIZE') — nur 3D-LUTs werden unterstützt"
            ),
            Self::SizeOutOfRange(n) => write!(
                f,
                "LUT_3D_SIZE {n} außerhalb des gültigen Bereichs (2..=256 laut Spezifikation)"
            ),
            Self::UnexpectedDataLine(line) => {
                write!(f, "Zeile {line}: erwartete drei Fließkommazahlen (r g b)")
            }
            Self::NotEnoughData { expected, found } => write!(
                f,
                "Zu wenige Datenzeilen: erwartet {expected} Werte, gefunden {found}"
            ),
            Self::InvalidNumber(line) => write!(f, "Zeile {line}: ungültige Zahl"),
        }
    }
}

impl std::error::Error for LutParseError {}

/// Ergebnis eines geparsten 3D-`.cube`-LUT-Rasters.
#[derive(Debug, Clone, PartialEq)]
pub struct ParsedLut {
    pub title: Option<String>,
    pub size: u32,
    /// `size^3 * 3` Floats, r am schnellsten variierend, dann g, dann b
    /// (Standard-`.cube`-Reihenfolge): Index eines Kanals `ch` am
    /// Rasterpunkt `(r, g, b)` ist `((b * size + g) * size + r) * 3 + ch`.
    pub table: Vec<f32>,
    pub domain_min: [f32; 3],
    pub domain_max: [f32; 3],
}

/// Parst den Inhalt einer `.cube`-Datei. Siehe Moduldoku für die
/// unterstützte Teilmenge der Spezifikation.
pub fn parse_cube_bytes(bytes: &[u8]) -> Result<ParsedLut, LutParseError> {
    let text = String::from_utf8_lossy(bytes);
    let mut title = None;
    let mut size: Option<u32> = None;
    let mut saw_1d = false;
    let mut domain_min = [0.0f32; 3];
    let mut domain_max = [1.0f32; 3];
    let mut data: Vec<f32> = Vec::new();

    for (line_no, raw_line) in text.lines().enumerate() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut parts = line.split_whitespace();
        let Some(first) = parts.next() else {
            continue;
        };
        match first.to_ascii_uppercase().as_str() {
            "TITLE" => {
                let rest = line[first.len()..].trim();
                title = Some(rest.trim_matches('"').to_string());
            }
            "LUT_3D_SIZE" => {
                let n: u32 = parts
                    .next()
                    .and_then(|s| s.parse().ok())
                    .ok_or(LutParseError::InvalidNumber(line_no + 1))?;
                if !(2..=256).contains(&n) {
                    return Err(LutParseError::SizeOutOfRange(n));
                }
                size = Some(n);
            }
            "LUT_1D_SIZE" => {
                saw_1d = true;
            }
            "DOMAIN_MIN" => {
                domain_min = parse_triplet(&mut parts, line_no)?;
            }
            "DOMAIN_MAX" => {
                domain_max = parse_triplet(&mut parts, line_no)?;
            }
            other => {
                // Entweder eine Datenzeile (drei Fließkommazahlen) oder
                // ein unbekanntes Schlüsselwort — Letzteres wird
                // stillschweigend übersprungen (siehe Moduldoku).
                if let Ok(r) = first.parse::<f32>() {
                    let g: f32 = parts
                        .next()
                        .and_then(|s| s.parse().ok())
                        .ok_or(LutParseError::UnexpectedDataLine(line_no + 1))?;
                    let b: f32 = parts
                        .next()
                        .and_then(|s| s.parse().ok())
                        .ok_or(LutParseError::UnexpectedDataLine(line_no + 1))?;
                    data.push(r);
                    data.push(g);
                    data.push(b);
                }
                let _ = other;
            }
        }
    }

    let Some(size) = size else {
        if saw_1d {
            return Err(LutParseError::Only1dSupported);
        }
        return Err(LutParseError::MissingSize);
    };

    let expected = (size as usize).pow(3) * 3;
    if data.len() < expected {
        return Err(LutParseError::NotEnoughData {
            expected,
            found: data.len(),
        });
    }
    data.truncate(expected);

    Ok(ParsedLut {
        title,
        size,
        table: data,
        domain_min,
        domain_max,
    })
}

fn parse_triplet<'a>(
    parts: &mut impl Iterator<Item = &'a str>,
    line_no: usize,
) -> Result<[f32; 3], LutParseError> {
    let mut out = [0.0f32; 3];
    for slot in out.iter_mut() {
        *slot = parts
            .next()
            .and_then(|s| s.parse().ok())
            .ok_or(LutParseError::InvalidNumber(line_no + 1))?;
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Kleinstmögliche gültige 3D-LUT (Kantenlänge 2 → 8 Rasterpunkte):
    /// eine reine Identität (jeder Punkt bildet auf sich selbst ab).
    fn identity_cube_2() -> String {
        let mut out = String::from("TITLE \"Identity\"\nLUT_3D_SIZE 2\n");
        for b in 0..2 {
            for g in 0..2 {
                for r in 0..2 {
                    out.push_str(&format!("{r}.0 {g}.0 {b}.0\n"));
                }
            }
        }
        out
    }

    #[test]
    fn parses_minimal_identity_cube() {
        let parsed = parse_cube_bytes(identity_cube_2().as_bytes()).unwrap();
        assert_eq!(parsed.title.as_deref(), Some("Identity"));
        assert_eq!(parsed.size, 2);
        assert_eq!(parsed.table.len(), 8 * 3);
        assert_eq!(parsed.domain_min, [0.0, 0.0, 0.0]);
        assert_eq!(parsed.domain_max, [1.0, 1.0, 1.0]);
        // Letzter Punkt (r=1,g=1,b=1) bildet auf (1,1,1) ab.
        let last = &parsed.table[parsed.table.len() - 3..];
        assert_eq!(last, &[1.0, 1.0, 1.0]);
    }

    #[test]
    fn ignores_comments_and_unknown_keywords() {
        let mut text = String::from("# ein Kommentar\nLUT_3D_SIZE 2\nSOME_VENDOR_KEY 42\n");
        for line in identity_cube_2().lines().skip(2) {
            text.push_str(line);
            text.push('\n');
        }
        let parsed = parse_cube_bytes(text.as_bytes()).unwrap();
        assert_eq!(parsed.size, 2);
        assert_eq!(parsed.table.len(), 8 * 3);
    }

    #[test]
    fn parses_domain_min_max() {
        let mut text =
            String::from("LUT_3D_SIZE 2\nDOMAIN_MIN 0.1 0.2 0.3\nDOMAIN_MAX 0.9 0.8 0.7\n");
        for line in identity_cube_2().lines().skip(2) {
            text.push_str(line);
            text.push('\n');
        }
        let parsed = parse_cube_bytes(text.as_bytes()).unwrap();
        assert_eq!(parsed.domain_min, [0.1, 0.2, 0.3]);
        assert_eq!(parsed.domain_max, [0.9, 0.8, 0.7]);
    }

    #[test]
    fn rejects_missing_size() {
        let err = parse_cube_bytes(b"0.0 0.0 0.0\n").unwrap_err();
        assert_eq!(err, LutParseError::MissingSize);
    }

    #[test]
    fn rejects_1d_only_lut() {
        let err = parse_cube_bytes(b"LUT_1D_SIZE 4\n0.0 0.0 0.0\n").unwrap_err();
        assert_eq!(err, LutParseError::Only1dSupported);
    }

    #[test]
    fn rejects_size_out_of_range() {
        let err = parse_cube_bytes(b"LUT_3D_SIZE 1\n").unwrap_err();
        assert_eq!(err, LutParseError::SizeOutOfRange(1));
    }

    #[test]
    fn rejects_truncated_data() {
        let err = parse_cube_bytes(b"LUT_3D_SIZE 2\n0.0 0.0 0.0\n1.0 0.0 0.0\n").unwrap_err();
        assert_eq!(
            err,
            LutParseError::NotEnoughData {
                expected: 8 * 3,
                found: 2 * 3
            }
        );
    }
}
