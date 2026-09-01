//! Umbenennungs-Tokensystem für den Import (Copy/Move-Modus, siehe
//! `DECISIONS.md` ADR-0025) — reine Funktion ohne Dateisystemzugriff, damit
//! sie ohne Testdateien geprüft werden kann.
//!
//! Unterstützte Tokens in einem Muster wie `"{date}_{seq}_{camera}"`:
//! - `{date}`: Aufnahmedatum als `YYYYMMDD`, oder — falls unbekannt — das
//!   Datum der Dateisystem-Änderungszeit.
//! - `{seq}`: fortlaufende Nummer innerhalb des Imports, vierstellig
//!   nullgepolstert (`0001`, `0002`, …).
//! - `{camera}`: Kameramodell, dateinamen-sicher bereinigt; `"Kamera"` als
//!   Platzhalter, falls unbekannt.
//! - `{original}`: ursprünglicher Dateiname ohne Endung.
//!
//! Die Dateiendung wird bewusst nicht Teil des Musters — sie wird vom
//! Aufrufer aus der Originaldatei übernommen (siehe
//! `crate::import::mode::stage_file_for_mode`).

use time::OffsetDateTime;

pub(crate) struct RenameTokens<'a> {
    /// Aufnahmedatum, falls aus EXIF bekannt — sonst die
    /// Dateisystem-Änderungszeit (immer vorhanden).
    pub date: OffsetDateTime,
    /// Fortlaufende Nummer innerhalb dieses Imports, bei 1 beginnend.
    pub seq: usize,
    pub camera: Option<&'a str>,
    /// Ursprünglicher Dateiname ohne Endung.
    pub original_stem: &'a str,
}

/// Ersetzt alle bekannten Tokens in `pattern`. Unbekannte `{…}`-Platzhalter
/// bleiben unverändert stehen (kein Fehler) — ein Tippfehler im Muster
/// führt so zu einem auffälligen, aber nicht abstürzenden Dateinamen.
pub(crate) fn render_rename_pattern(pattern: &str, tokens: &RenameTokens<'_>) -> String {
    let date = format_date_yyyymmdd(tokens.date);
    let camera = sanitize_for_filename(tokens.camera.unwrap_or("Kamera"));

    pattern
        .replace("{date}", &date)
        .replace("{seq}", &format!("{:04}", tokens.seq))
        .replace("{camera}", &camera)
        .replace("{original}", &sanitize_for_filename(tokens.original_stem))
}

/// Formatiert `dt` als `YYYYMMDD`, ohne auf das `time::macros`-Feature
/// angewiesen zu sein (nicht Teil der Workspace-Feature-Auswahl, siehe
/// Root-`Cargo.toml`) — reine Ganzzahl-Arithmetik über die vom `time`-Crate
/// ohnehin bereitgestellten Kalenderfelder.
fn format_date_yyyymmdd(dt: OffsetDateTime) -> String {
    format!("{:04}{:02}{:02}", dt.year(), u8::from(dt.month()), dt.day())
}

/// Entfernt Zeichen, die in Dateinamen auf mindestens einer
/// Zielplattform (Windows) verboten sind — Kamera-Modellnamen wie
/// `"Canon EOS R5"` enthalten sonst harmlose, aber besser vermiedene
/// Leerzeichen bleiben erlaubt.
fn sanitize_for_filename(raw: &str) -> String {
    raw.chars()
        .map(|c| if "\\/:*?\"<>|".contains(c) { '_' } else { c })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::Date;

    fn sample_date(year: i32, month: u8, day: u8) -> OffsetDateTime {
        Date::from_calendar_date(year, month.try_into().expect("gültiger Monat"), day)
            .expect("gültiges Datum")
            .midnight()
            .assume_utc()
    }

    #[test]
    fn replaces_all_known_tokens() {
        let tokens = RenameTokens {
            date: sample_date(2026, 3, 15),
            seq: 7,
            camera: Some("Canon EOS R5"),
            original_stem: "IMG_0042",
        };
        let result = render_rename_pattern("{date}_{seq}_{camera}_{original}", &tokens);
        assert_eq!(result, "20260315_0007_Canon EOS R5_IMG_0042");
    }

    #[test]
    fn seq_is_zero_padded_to_four_digits() {
        let tokens = RenameTokens {
            date: sample_date(2026, 1, 1),
            seq: 3,
            camera: None,
            original_stem: "a",
        };
        assert!(render_rename_pattern("{seq}", &tokens).starts_with("0003"));
    }

    #[test]
    fn missing_camera_falls_back_to_placeholder() {
        let tokens = RenameTokens {
            date: sample_date(2026, 1, 1),
            seq: 1,
            camera: None,
            original_stem: "a",
        };
        assert_eq!(render_rename_pattern("{camera}", &tokens), "Kamera");
    }

    #[test]
    fn forbidden_filename_characters_are_sanitized() {
        let tokens = RenameTokens {
            date: sample_date(2026, 1, 1),
            seq: 1,
            camera: Some("Nikon Z9:Pro?"),
            original_stem: "a",
        };
        assert_eq!(render_rename_pattern("{camera}", &tokens), "Nikon Z9_Pro_");
    }

    #[test]
    fn unknown_token_is_left_untouched() {
        let tokens = RenameTokens {
            date: sample_date(2026, 1, 1),
            seq: 1,
            camera: None,
            original_stem: "a",
        };
        assert_eq!(render_rename_pattern("{unbekannt}", &tokens), "{unbekannt}");
    }
}
