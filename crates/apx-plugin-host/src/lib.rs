//! Aperture X — Plugin-Lader (Phase 9 Schritt 9, `SPEC.md` §5, siehe
//! `PLAN.md`, `DECISIONS.md` ADR-0035 Punkt 3). Lädt eine Plugin-`cdylib`
//! per `libloading` (real, reif — kein selbstgebauter `dlopen`-Wrapper),
//! sucht darin das eine vereinbarte Symbol `apx_plugin_table` (siehe
//! `apx-plugin-abi`s Moduldoku für den vollständigen ABI-Vertrag) und
//! prüft dessen `abi_version` **hart**: jede Abweichung wird abgelehnt,
//! nicht zu erraten versucht.
//!
//! **Ehrlich begrenzt** (siehe `PLAN.md`): dies ist eine schmale,
//! handgepflegte Schnittstelle für genau einen Erweiterungspunkt — kein
//! allgemeines Plugin-Framework mit beliebigen Hooks.

use std::ffi::CStr;
use std::path::Path;

use apx_plugin_abi::{ApxEffectStatus, ApxPixelFormat, ApxPluginTable, APX_PLUGIN_ABI_VERSION};
use libloading::{Library, Symbol};
use thiserror::Error;

pub type Result<T> = std::result::Result<T, PluginError>;

#[derive(Debug, Error)]
pub enum PluginError {
    #[error("Plugin-Datei '{path}' konnte nicht geladen werden: {message}")]
    Load { path: String, message: String },

    #[error(
        "Plugin-Datei '{path}' exportiert nicht das erwartete Symbol 'apx_plugin_table': {message}"
    )]
    MissingSymbol { path: String, message: String },

    #[error("Plugin-Datei '{path}' hat eine ungültige (null) Funktionstabelle zurückgegeben")]
    NullTable { path: String },

    #[error(
        "Plugin-Datei '{path}' hat ABI-Version {found}, diese Aperture-X-Version erwartet {expected} — Plugin wird abgelehnt statt geraten geladen"
    )]
    AbiVersionMismatch {
        path: String,
        expected: u32,
        found: u32,
    },

    #[error("Plugin '{name}' bietet keinen Custom-Effekt an (apply_custom_effect ist None)")]
    NoCustomEffect { name: String },

    #[error("Puffer zu klein für Plugin-Aufruf: {message}")]
    BufferTooSmall { message: String },

    #[error("Plugin '{name}' meldete einen Fehler ({status:?})")]
    EffectFailed {
        name: String,
        status: ApxEffectStatus,
    },
}

impl From<PluginError> for apx_core::AppError {
    fn from(err: PluginError) -> Self {
        apx_core::AppError::plugin(err.to_string())
    }
}

/// Ein geladenes Plugin — hält die Bibliothek (`Library`) am Leben,
/// solange dieser Wert existiert (Felder droppen in Deklarations-
/// reihenfolge, `library` fällt also zuletzt weg).
pub struct LoadedPlugin {
    table: *const ApxPluginTable,
    name: String,
    // Muss nach `table`/`name` deklariert bleiben, damit die Bibliothek
    // erst entladen wird, nachdem dieser Wert (und damit jeder Zugriff
    // auf `table`) nicht mehr existiert — Rust droppt Felder in
    // Deklarationsreihenfolge.
    _library: Library,
}

impl LoadedPlugin {
    /// Lädt die Plugin-`cdylib` unter `path` und prüft sofort die
    /// ABI-Version — ein Ablehnungsgrund hier ist immer ein harter
    /// Fehler, nie eine stillschweigende Bestwahl-Interpretation.
    pub fn load(path: &Path) -> Result<Self> {
        let path_display = path.display().to_string();
        // Safety: `libloading::Library::new` ist laut dessen eigener
        // Dokumentation unsicher, weil eine geladene Bibliothek beim
        // Laden beliebigen Code ausführen kann (Initialisierer) — hier
        // bewusst akzeptiert: ein Plugin ist per Definition Code, den
        // der Nutzer explizit lädt (derselbe Vertrauensrahmen wie ein
        // ausführbares Programm starten).
        let library = unsafe { Library::new(path) }.map_err(|err| PluginError::Load {
            path: path_display.clone(),
            message: err.to_string(),
        })?;

        // Safety: das Symbol wird laut `apx-plugin-abi`s Vertrag als
        // `extern "C" fn() -> *const ApxPluginTable` exportiert; ein
        // falsch deklariertes Plugin-Symbol ist ein Plugin-Bug, den wir
        // hier nicht verhindern können (dieselbe Grenze wie bei jedem
        // FFI-Aufruf).
        let table_fn: Symbol<unsafe extern "C" fn() -> *const ApxPluginTable> = unsafe {
            library.get(b"apx_plugin_table\0")
        }
        .map_err(|err| PluginError::MissingSymbol {
            path: path_display.clone(),
            message: err.to_string(),
        })?;

        let table_ptr = unsafe { table_fn() };
        if table_ptr.is_null() {
            return Err(PluginError::NullTable { path: path_display });
        }
        // Safety: `table_ptr` kommt aus dem Plugin selbst; laut Vertrag
        // ist es für die Prozesslaufzeit gültig, solange `library`
        // geladen bleibt (was dieser `struct` durch Feldreihenfolge
        // garantiert).
        let table = unsafe { &*table_ptr };
        if table.abi_version != APX_PLUGIN_ABI_VERSION {
            return Err(PluginError::AbiVersionMismatch {
                path: path_display,
                expected: APX_PLUGIN_ABI_VERSION,
                found: table.abi_version,
            });
        }

        let name = if table.plugin_name.is_null() {
            "(unbenannt)".to_string()
        } else {
            // Safety: `plugin_name` ist laut Vertrag ein gültiger,
            // NUL-terminierter, für die Prozesslaufzeit gültiger Zeiger.
            unsafe { CStr::from_ptr(table.plugin_name) }
                .to_string_lossy()
                .into_owned()
        };

        Ok(Self {
            table: table_ptr,
            name,
            _library: library,
        })
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    /// Ruft den Custom-Effekt des Plugins in-place auf `pixels` auf
    /// (`width * height * 4` Bytes RGBA8, dicht gepackt — `stride`
    /// entspricht also `width * 4`).
    pub fn apply_custom_effect_rgba8(
        &self,
        pixels: &mut [u8],
        width: u32,
        height: u32,
        param: f32,
    ) -> Result<()> {
        // Safety: `self.table` bleibt gültig, solange `self` (und damit
        // `self._library`) existiert.
        let table = unsafe { &*self.table };
        let func = table
            .apply_custom_effect
            .ok_or_else(|| PluginError::NoCustomEffect {
                name: self.name.clone(),
            })?;

        let stride = width
            .checked_mul(4)
            .ok_or_else(|| PluginError::BufferTooSmall {
                message: "Breite zu groß für RGBA8-Zeilenlänge".to_string(),
            })?;
        let required = (stride as usize)
            .checked_mul(height as usize)
            .ok_or_else(|| PluginError::BufferTooSmall {
                message: "Puffergröße überläuft".to_string(),
            })?;
        if pixels.len() < required {
            return Err(PluginError::BufferTooSmall {
                message: format!(
                    "Puffer hat {} Bytes, gebraucht werden mindestens {required} ({width}x{height} RGBA8)",
                    pixels.len()
                ),
            });
        }

        // Safety: `pixels` ist mindestens `stride * height` Bytes groß
        // (oben geprüft), `func` erfüllt laut `apx-plugin-abi`s Vertrag
        // dieselbe Garantie in die andere Richtung (schreibt/liest nur
        // innerhalb dieser Grenzen).
        let status = unsafe {
            func(
                pixels.as_mut_ptr(),
                width,
                height,
                stride,
                ApxPixelFormat::Rgba8,
                param,
            )
        };
        match status {
            ApxEffectStatus::Ok => Ok(()),
            other => Err(PluginError::EffectFailed {
                name: self.name.clone(),
                status: other,
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Pfad zur gebauten `apx-plugin-example`-Bibliothek — dieselbe
    /// Cargo-`target`-Konvention, die `cargo build`/`cargo test` in
    /// dieser Sandbox tatsächlich verwendet (kein `target-dir`-
    /// Override). **Nur in dieser Linux-Sandbox verifiziert** — die
    /// `.dylib`/`.dll`-Zweige unten folgen derselben Konvention für
    /// macOS/Windows, sind hier aber nie tatsächlich ausgeführt worden
    /// (ehrlich vermerkt, dieselbe Einschränkung wie an anderer Stelle
    /// im Projekt bei plattformspezifischem Code).
    fn example_plugin_path() -> std::path::PathBuf {
        let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .expect("apx-plugin-host liegt unter <workspace>/crates/apx-plugin-host");
        let file_name = if cfg!(target_os = "macos") {
            "libapx_plugin_example.dylib"
        } else if cfg!(target_os = "windows") {
            "apx_plugin_example.dll"
        } else {
            "libapx_plugin_example.so"
        };
        workspace_root.join("target").join("debug").join(file_name)
    }

    fn solid(width: u32, height: u32, r: u8, g: u8, b: u8) -> Vec<u8> {
        let mut pixels = vec![0u8; (width * height * 4) as usize];
        for chunk in pixels.chunks_exact_mut(4) {
            chunk[0] = r;
            chunk[1] = g;
            chunk[2] = b;
            chunk[3] = 255;
        }
        pixels
    }

    #[test]
    fn loads_the_example_plugin_and_reports_its_name() {
        let plugin = LoadedPlugin::load(&example_plugin_path()).expect("sollte laden");
        assert!(plugin.name().contains("Beispiel-Plugin"));
    }

    #[test]
    fn applies_the_example_plugins_invert_effect_across_the_dlopen_boundary() {
        let plugin = LoadedPlugin::load(&example_plugin_path()).expect("sollte laden");
        let mut pixels = solid(2, 2, 10, 20, 30);
        plugin
            .apply_custom_effect_rgba8(&mut pixels, 2, 2, 1.0)
            .expect("sollte anwenden");
        assert_eq!(&pixels[0..4], &[245, 235, 225, 255]);
    }

    #[test]
    fn rejects_a_nonexistent_plugin_file() {
        let result = LoadedPlugin::load(Path::new("/nicht/vorhanden.so"));
        assert!(matches!(result, Err(PluginError::Load { .. })));
    }

    #[test]
    fn rejects_a_buffer_too_small_for_the_requested_dimensions() {
        let plugin = LoadedPlugin::load(&example_plugin_path()).expect("sollte laden");
        let mut too_small = vec![0u8; 4]; // nur 1 Pixel, aber 2x2 angefordert
        let result = plugin.apply_custom_effect_rgba8(&mut too_small, 2, 2, 1.0);
        assert!(matches!(result, Err(PluginError::BufferTooSmall { .. })));
    }
}
