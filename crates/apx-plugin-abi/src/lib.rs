//! Aperture X — Plugin-ABI (Phase 9 Schritt 9, siehe `PLAN.md`,
//! `DECISIONS.md` ADR-0035 Punkt 3): eine handgepflegte, versionierte
//! `#[repr(C)]`-Funktionszeiger-Tabelle für **einen** festen
//! Erweiterungspunkt — eine „Custom-Effekt"-Bildoperationsstufe, die
//! in-place auf einem RGBA8-Puffer arbeitet.
//!
//! **Ehrlich begrenzt** (siehe `PLAN.md`): „stabile ABI" heißt hier
//! „versionierte, geprüfte Kompatibilität für diese eine schmale,
//! handgepflegte Schnittstelle" — **keine** Zusage unbegrenzter
//! künftiger Binärkompatibilität beliebiger interner Rust-Strukturen.
//! Jede Änderung an [`ApxPluginTable`]s Feldern (Hinzufügen, Entfernen,
//! Umsortieren) MUSS [`APX_PLUGIN_ABI_VERSION`] erhöhen — der Host
//! (`apx-plugin-host`) lehnt jede abweichende Version hart ab, statt
//! stillschweigend eine falsch interpretierte Tabelle zu benutzen.
//!
//! ## Vertrag für Plugin-Autoren
//!
//! Eine Plugin-`cdylib` exportiert genau eine `extern "C"`-Funktion:
//!
//! ```ignore
//! #[no_mangle]
//! pub extern "C" fn apx_plugin_table() -> *const ApxPluginTable { ... }
//! ```
//!
//! Die zurückgegebene Tabelle (und alles, worauf sie zeigt, z. B.
//! `plugin_name`) muss für die gesamte Prozesslaufzeit gültig bleiben
//! (typischerweise `static`) — der Host ruft `apx_plugin_table()` einmal
//! beim Laden auf und hält den Zeiger, bis das Plugin entladen wird.

use std::os::raw::c_char;

/// Die aktuelle ABI-Version. Muss bei jeder Änderung an
/// [`ApxPluginTable`]s Feldreihenfolge/-typen erhöht werden.
pub const APX_PLUGIN_ABI_VERSION: u32 = 1;

/// Der einzige unterstützte Pixelpuffer-Typ (siehe Moduldoku — bewusst
/// nur einer statt eines erweiterbaren Formatsystems, das dieselbe
/// „schmale, geprüfte Schnittstelle statt unbegrenzter Flexibilität"-
/// Linie verletzen würde).
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApxPixelFormat {
    /// Interleaved 8-Bit-RGBA, `stride` Bytes je Zeile.
    Rgba8 = 0,
}

/// Rückgabecodes der Custom-Effekt-Funktion.
#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApxEffectStatus {
    Ok = 0,
    /// Das Plugin konnte mit den übergebenen Parametern/Puffermaßen
    /// nicht arbeiten (z. B. `width`/`height` von 0).
    InvalidInput = 1,
    /// Unspezifischer interner Plugin-Fehler.
    InternalError = 2,
}

/// Signatur der Custom-Effekt-Funktion — arbeitet **in-place** auf
/// `pixels` (`stride * height` Bytes, `stride >= width * 4` für
/// [`ApxPixelFormat::Rgba8`]). `param` ist ein einzelner freier
/// Gleitkomma-Parameter (Stärke/Intensität — die Bedeutung legt das
/// jeweilige Plugin fest, dieselbe „ein Parameter reicht für die meisten
/// Effekt-Plugins"-Vereinfachung wie bei den EDL-Reglern selbst).
///
/// # Safety
/// Aufrufer müssen sicherstellen, dass `pixels` auf einen gültigen,
/// beschreibbaren Puffer von mindestens `stride * height` Bytes zeigt.
/// Implementierungen dürfen `pixels` nur innerhalb dieser Grenzen lesen
/// und schreiben.
pub type ApxCustomEffectFn = unsafe extern "C" fn(
    pixels: *mut u8,
    width: u32,
    height: u32,
    stride: u32,
    format: ApxPixelFormat,
    param: f32,
) -> ApxEffectStatus;

/// Die vollständige Plugin-Funktionstabelle — von jeder Plugin-`cdylib`
/// über `apx_plugin_table()` bereitgestellt (siehe Moduldoku).
#[repr(C)]
pub struct ApxPluginTable {
    /// Muss exakt [`APX_PLUGIN_ABI_VERSION`] sein — der Host prüft dies
    /// als Allererstes und lehnt jede Abweichung ab, bevor er
    /// irgendein anderes Feld anfasst.
    pub abi_version: u32,
    /// NUL-terminierter Anzeigename, gültig für die Prozesslaufzeit
    /// (siehe Moduldoku).
    pub plugin_name: *const c_char,
    /// `None`, wenn dieses Plugin keinen Custom-Effekt anbietet (z. B.
    /// ein zukünftiger, anderer Erweiterungspunkt-Typ — auch wenn es
    /// aktuell nur diesen einen gibt, bleibt das Feld optional statt
    /// Pflicht, damit ein Nicht-Effekt-Plugin die Tabelle trotzdem
    /// gültig ausfüllen kann).
    pub apply_custom_effect: Option<ApxCustomEffectFn>,
}

// Safety: `ApxPluginTable` enthält nur einen rohen `plugin_name`-Zeiger
// (laut Vertrag ein unveränderliches, für die Prozesslaufzeit gültiges
// NUL-terminiertes Byte-Array — nie mutiert) und einen Funktionszeiger
// (Aufruf einer `extern "C" fn` ist an sich threadsicher, solange die
// Implementierung selbst keine geteilten Daten unsynchronisiert
// verändert, was außerhalb dieser ABI-Definition liegt). Eine `static
// ApxPluginTable`-Instanz (wie sie jede Plugin-`cdylib` exportiert)
// muss deshalb `Sync` sein, obwohl rohe Zeiger es standardmäßig nicht
// sind.
unsafe impl Sync for ApxPluginTable {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn abi_version_is_nonzero() {
        // `0` würde mit einer nicht initialisierten/vergessenen Tabelle
        // verwechselbar sein — ein legitimes Versions-„Nichts" gibt es
        // hier nicht.
        assert_ne!(APX_PLUGIN_ABI_VERSION, 0);
    }

    #[test]
    fn table_has_the_expected_c_layout_size_on_a_64_bit_target() {
        // Regressionswächter: eine unabsichtliche Feldänderung (die
        // ohne Versions-Bump durchrutschen würde) verändert fast immer
        // die Größe — kein Ersatz für sorgfältige Reviews, aber ein
        // billiger erster Stolperdraht. Der erwartete Wert ist die
        // tatsächliche `repr(C)`-Größe inklusive Ausrichtungs-Padding
        // (`u32` + 4 Byte Padding, damit der folgende Zeiger 8-Byte-
        // ausgerichtet liegt, + zwei 8-Byte-Zeiger) — nicht die naive
        // Summe der Feldgrößen ohne Padding.
        #[cfg(target_pointer_width = "64")]
        assert_eq!(std::mem::size_of::<ApxPluginTable>(), 24);
    }

    #[test]
    fn effect_status_ok_is_zero() {
        // Dieselbe C-Konvention wie überall sonst (0 = Erfolg) — ein
        // Plugin-Autor, der die Rust-Enum-Definition nicht kennt, aber
        // die C-Konvention kennt, liegt trotzdem richtig.
        assert_eq!(ApxEffectStatus::Ok as i32, 0);
    }
}
