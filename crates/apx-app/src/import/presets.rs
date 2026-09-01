//! Import-Presets: gespeicherte Kombination aus Modus, Zielordner und
//! Umbenennungsmuster, als JSON-Datei im Einstellungsverzeichnis (siehe
//! `apx_core::AppPaths::config_dir`) — analog zu `apx_core::Settings`s
//! Lade-/Speicherschema, aber JSON statt TOML, weil Presets eine Liste
//! benannter Einträge sind statt einer einzelnen Konfigurationsstruktur.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "mode")]
pub(crate) enum PresetMode {
    AddInPlace,
    Copy { target_dir: PathBuf },
    Move { target_dir: PathBuf },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct ImportPreset {
    pub name: String,
    pub mode: PresetMode,
    pub rename_pattern: Option<String>,
}

/// Lädt alle gespeicherten Presets aus `path`. Existiert die Datei nicht,
/// wird stillschweigend eine leere Liste zurückgegeben (kein Fehler) —
/// Normalfall beim allerersten Start, analog zu `Settings::load_or_default`.
pub(crate) fn load_presets(path: &Path) -> Result<Vec<ImportPreset>, String> {
    match fs::read_to_string(path) {
        Ok(text) => serde_json::from_str(&text)
            .map_err(|err| format!("Presets-Datei '{}' ist ungültig: {err}", path.display())),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(err) => Err(format!(
            "Presets-Datei '{}' nicht lesbar: {err}",
            path.display()
        )),
    }
}

pub(crate) fn save_presets(path: &Path, presets: &[ImportPreset]) -> Result<(), String> {
    let text = serde_json::to_string_pretty(presets)
        .map_err(|err| format!("Presets nicht serialisierbar: {err}"))?;
    fs::write(path, text)
        .map_err(|err| format!("Presets-Datei '{}' nicht schreibbar: {err}", path.display()))
}

/// Fügt `preset` hinzu oder ersetzt ein vorhandenes Preset gleichen Namens
/// (Namen sind der einzige Schlüssel, es gibt keine separate ID) und
/// speichert die aktualisierte Liste sofort.
pub(crate) fn upsert_preset(
    path: &Path,
    preset: ImportPreset,
) -> Result<Vec<ImportPreset>, String> {
    let mut presets = load_presets(path)?;
    if let Some(existing) = presets.iter_mut().find(|p| p.name == preset.name) {
        *existing = preset;
    } else {
        presets.push(preset);
    }
    save_presets(path, &presets)?;
    Ok(presets)
}

pub(crate) fn delete_preset(path: &Path, name: &str) -> Result<Vec<ImportPreset>, String> {
    let mut presets = load_presets(path)?;
    presets.retain(|p| p.name != name);
    save_presets(path, &presets)?;
    Ok(presets)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(name: &str) -> ImportPreset {
        ImportPreset {
            name: name.to_string(),
            mode: PresetMode::Copy {
                target_dir: PathBuf::from("/ziel"),
            },
            rename_pattern: Some("{date}_{seq}".to_string()),
        }
    }

    #[test]
    fn missing_file_yields_empty_list() {
        let tmp = tempfile::tempdir().expect("Temp-Verzeichnis");
        let path = tmp.path().join("presets.json");
        assert_eq!(load_presets(&path).expect("ok"), Vec::new());
    }

    #[test]
    fn upsert_then_load_roundtrips() {
        let tmp = tempfile::tempdir().expect("Temp-Verzeichnis");
        let path = tmp.path().join("presets.json");

        upsert_preset(&path, sample("Urlaub")).expect("ok");
        let loaded = load_presets(&path).expect("ok");
        assert_eq!(loaded, vec![sample("Urlaub")]);
    }

    #[test]
    fn upsert_with_same_name_replaces_instead_of_duplicating() {
        let tmp = tempfile::tempdir().expect("Temp-Verzeichnis");
        let path = tmp.path().join("presets.json");

        upsert_preset(&path, sample("Urlaub")).expect("ok");
        let mut updated = sample("Urlaub");
        updated.rename_pattern = None;
        upsert_preset(&path, updated.clone()).expect("ok");

        let loaded = load_presets(&path).expect("ok");
        assert_eq!(loaded, vec![updated]);
    }

    #[test]
    fn delete_removes_only_the_named_preset() {
        let tmp = tempfile::tempdir().expect("Temp-Verzeichnis");
        let path = tmp.path().join("presets.json");

        upsert_preset(&path, sample("A")).expect("ok");
        upsert_preset(&path, sample("B")).expect("ok");

        let remaining = delete_preset(&path, "A").expect("ok");
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].name, "B");
    }

    #[test]
    fn invalid_json_is_reported_as_error() {
        let tmp = tempfile::tempdir().expect("Temp-Verzeichnis");
        let path = tmp.path().join("presets.json");
        std::fs::write(&path, "das ist kein gültiges JSON {{{").expect("schreibbar");

        assert!(load_presets(&path).is_err());
    }
}
