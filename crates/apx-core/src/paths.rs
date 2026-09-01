//! Plattformkorrekte Pfade für Katalog, Cache, Logs und Einstellungen.
//!
//! Nutzt die `directories`-Crate, damit auf Windows `%APPDATA%`, auf macOS
//! `~/Library/Application Support` und auf Linux die XDG-Basisverzeichnisse
//! verwendet werden, ohne das selbst pro Plattform unterscheiden zu müssen.

use std::fs;
use std::path::{Path, PathBuf};

use directories::ProjectDirs;

use crate::error::{AppError, Result};

const QUALIFIER: &str = "dev";
const ORGANIZATION: &str = "ApertureX";
const APPLICATION: &str = "ApertureX";

/// Sammelt alle Verzeichnisse, die Aperture X zur Laufzeit braucht.
///
/// `AppPaths::discover()` ermittelt die echten Systempfade. Für Tests
/// (und für den `--portable`-Modus, der in einer späteren Phase dazukommt)
/// gibt es `AppPaths::rooted_at`, das alles unter einem beliebigen
/// Basisverzeichnis anlegt.
#[derive(Debug, Clone)]
pub struct AppPaths {
    data_dir: PathBuf,
    cache_dir: PathBuf,
    log_dir: PathBuf,
    config_dir: PathBuf,
}

impl AppPaths {
    /// Ermittelt die tatsächlichen, plattformspezifischen Systempfade.
    pub fn discover() -> Result<Self> {
        let project_dirs =
            ProjectDirs::from(QUALIFIER, ORGANIZATION, APPLICATION).ok_or_else(|| {
                AppError::Settings {
                    message: "Kein Home-Verzeichnis gefunden — Systempfade können nicht ermittelt \
                          werden."
                        .to_string(),
                }
            })?;

        let paths = Self {
            data_dir: project_dirs.data_dir().to_path_buf(),
            cache_dir: project_dirs.cache_dir().to_path_buf(),
            // Manche Plattformen (Linux) liefern kein eigenes Log-Verzeichnis
            // über `directories` — wir legen Logs konsistent unter dem
            // Datenverzeichnis ab, das ist überall vorhanden.
            log_dir: project_dirs.data_dir().join("logs"),
            config_dir: project_dirs.config_dir().to_path_buf(),
        };
        paths.ensure_all_exist()?;
        Ok(paths)
    }

    /// Legt alle Pfade unterhalb von `root` an — für Tests und den
    /// portablen Modus, ohne das echte Nutzerverzeichnis anzufassen.
    pub fn rooted_at(root: impl Into<PathBuf>) -> Result<Self> {
        let root = root.into();
        let paths = Self {
            data_dir: root.join("data"),
            cache_dir: root.join("cache"),
            log_dir: root.join("logs"),
            config_dir: root.join("config"),
        };
        paths.ensure_all_exist()?;
        Ok(paths)
    }

    fn ensure_all_exist(&self) -> Result<()> {
        for dir in [
            &self.data_dir,
            &self.cache_dir,
            &self.log_dir,
            &self.config_dir,
        ] {
            create_dir_all(dir)?;
        }
        create_dir_all(&self.preview_cache_dir())?;
        Ok(())
    }

    /// Verzeichnis für den Katalog (die SQLite-Datei selbst).
    pub fn catalog_dir(&self) -> &Path {
        &self.data_dir
    }

    /// Pfad der Standard-Katalogdatei (`catalog.sqlite`).
    pub fn default_catalog_file(&self) -> PathBuf {
        self.data_dir.join("catalog.sqlite")
    }

    /// Basis-Cache-Verzeichnis (Vorschauen, Smart Previews, temporäre
    /// Render-Kacheln ab Phase 2).
    pub fn cache_dir(&self) -> &Path {
        &self.cache_dir
    }

    /// Verzeichnis für den Vorschau-Cache, unterhalb von `cache_dir()`.
    /// Die tatsächliche Datei-Aufteilung in Unterordner (nach den ersten
    /// zwei Zeichen der Foto-ID) übernimmt `apx-app` beim Import.
    pub fn preview_cache_dir(&self) -> PathBuf {
        self.cache_dir.join("previews")
    }

    /// Verzeichnis für Log-Dateien.
    pub fn log_dir(&self) -> &Path {
        &self.log_dir
    }

    /// Verzeichnis für die Einstellungsdatei.
    pub fn config_dir(&self) -> &Path {
        &self.config_dir
    }

    /// Pfad der Einstellungsdatei (`settings.toml`).
    pub fn settings_file(&self) -> PathBuf {
        self.config_dir.join("settings.toml")
    }

    /// Pfad der Import-Presets-Datei (`import_presets.json`) — siehe
    /// `apx-app`s `import::presets`-Modul, `PLAN.md` Phase 3, Schritt 4.
    /// JSON statt TOML wie `settings_file`, weil Presets eine Liste
    /// benannter Einträge sind statt einer einzelnen Konfigurationsstruktur.
    pub fn import_presets_file(&self) -> PathBuf {
        self.config_dir.join("import_presets.json")
    }

    /// Zielverzeichnis für Tethered-Shooting-Downloads (Phase 9
    /// Schritt 11, siehe `apx-tether`) — jede Aufnahme landet zunächst
    /// hier, bevor sie über den bestehenden Import-Pfad (`import::
    /// run_with_mode`) katalogisiert wird. Unterhalb von `data_dir`, nicht
    /// `cache_dir`: die Originaldateien bleiben hier bis der Nutzer sie
    /// bewusst verschiebt/löscht, sind also keine wegwerfbaren
    /// Zwischendaten wie ein Cache.
    pub fn tether_download_dir(&self) -> PathBuf {
        self.data_dir.join("tether")
    }
}

fn create_dir_all(dir: &Path) -> Result<()> {
    fs::create_dir_all(dir).map_err(|source| AppError::io(dir, source))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rooted_paths_are_created_and_isolated() {
        let tmp = tempfile::tempdir().expect("Temp-Verzeichnis");
        let paths = AppPaths::rooted_at(tmp.path()).expect("AppPaths sollte sich anlegen lassen");

        assert!(paths.catalog_dir().exists());
        assert!(paths.cache_dir().exists());
        assert!(paths.log_dir().exists());
        assert!(paths.config_dir().exists());
        assert!(paths.preview_cache_dir().exists());

        assert!(paths.catalog_dir().starts_with(tmp.path()));
        assert!(paths.default_catalog_file().ends_with("catalog.sqlite"));
        assert!(paths.settings_file().ends_with("settings.toml"));
        assert!(paths.import_presets_file().ends_with("import_presets.json"));
    }

    #[test]
    fn discover_resolves_without_panicking() {
        // Nur ein Rauchtest: discover() darf nicht abstürzen. Der exakte
        // Pfad hängt von der Testumgebung ab und wird hier nicht geprüft.
        let result = AppPaths::discover();
        assert!(result.is_ok());
    }
}
