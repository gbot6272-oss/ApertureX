//! Anwendungseinstellungen: Laden/Speichern als TOML, mit sinnvollen
//! Defaults, falls noch keine Einstellungsdatei existiert.

use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::{AppError, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    // Dark-First laut SPEC.md Abschnitt 4.
    #[default]
    Dark,
    Light,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct UiSettings {
    pub theme: Theme,
    /// Sprachcode, z. B. "de" oder "en". Lokalisierung selbst kommt erst in
    /// Phase 10, das Feld existiert aber schon, damit spätere Migrationen
    /// nicht das Schema brechen.
    pub locale: String,
    /// UI-Skalierung in Prozent, 75–200 laut SPEC.md Abschnitt 3.6
    /// (Barrierefreiheit). Wird erst in Phase 10 tatsächlich ausgewertet.
    pub ui_scale_percent: u16,
}

impl Default for UiSettings {
    fn default() -> Self {
        Self {
            theme: Theme::default(),
            locale: "de".to_string(),
            ui_scale_percent: 100,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CatalogSettings {
    /// Pfad zum zuletzt geöffneten Katalog. `None` beim allerersten Start.
    pub last_opened_catalog: Option<String>,
}

/// Einstellungen für die KI-Funktionen (Phase 7, siehe `DECISIONS.md`
/// ADR-0033) — bislang nur der vom Nutzer selbst hinterlegte
/// Anthropic-API-Schlüssel für den LLM-Preset-Generator (kein
/// mitgelieferter Schlüssel, genau wie jede andere Desktop-App mit
/// KI-Anbindung). Liegt im Klartext in derselben TOML-Einstellungsdatei
/// wie alles andere — dieselbe Vertrauensgrenze wie z. B.
/// `last_opened_catalog` (ein lokales, nicht synchronisiertes Profil).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AiSettings {
    pub anthropic_api_key: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub ui: UiSettings,
    pub catalog: CatalogSettings,
    pub ai: AiSettings,
}

impl Settings {
    /// Lädt die Einstellungen aus `path`. Existiert die Datei nicht, werden
    /// stillschweigend die Defaults zurückgegeben (kein Fehler) — das ist
    /// der Normalfall beim allerersten Start.
    pub fn load_or_default(path: &Path) -> Result<Self> {
        match fs::read_to_string(path) {
            Ok(text) => toml::from_str(&text).map_err(|source| AppError::Settings {
                message: format!(
                    "Einstellungsdatei '{}' ist ungültig: {source}",
                    path.display()
                ),
            }),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(err) => Err(AppError::io(path, err)),
        }
    }

    /// Speichert die Einstellungen als TOML nach `path`. Das
    /// Elternverzeichnis muss bereits existieren (siehe `AppPaths`).
    pub fn save(&self, path: &Path) -> Result<()> {
        let text = toml::to_string_pretty(self).map_err(|source| AppError::Settings {
            message: format!("Einstellungen konnten nicht serialisiert werden: {source}"),
        })?;
        fs::write(path, text).map_err(|source| AppError::io(path, source))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_file_yields_defaults() {
        let tmp = tempfile::tempdir().expect("Temp-Verzeichnis");
        let path = tmp.path().join("settings.toml");

        let settings = Settings::load_or_default(&path).expect("Defaults dürfen nicht scheitern");
        assert_eq!(settings, Settings::default());
        assert_eq!(settings.ui.theme, Theme::Dark);
    }

    #[test]
    fn roundtrip_save_and_load() {
        let tmp = tempfile::tempdir().expect("Temp-Verzeichnis");
        let path = tmp.path().join("settings.toml");

        let mut settings = Settings::default();
        settings.ui.theme = Theme::Light;
        settings.ui.locale = "en".to_string();
        settings.catalog.last_opened_catalog = Some("/pfad/zum/katalog.sqlite".to_string());
        settings
            .save(&path)
            .expect("Speichern darf nicht scheitern");

        let loaded = Settings::load_or_default(&path).expect("Laden darf nicht scheitern");
        assert_eq!(loaded, settings);
    }

    #[test]
    fn invalid_toml_is_reported_as_error() {
        let tmp = tempfile::tempdir().expect("Temp-Verzeichnis");
        let path = tmp.path().join("settings.toml");
        fs::write(&path, "das ist kein gültiges TOML {{{").unwrap();

        let result = Settings::load_or_default(&path);
        assert!(result.is_err());
    }
}
