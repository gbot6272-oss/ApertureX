//! Aperture X — Skript-API (Phase 9 Schritt 9, `SPEC.md` §5, siehe
//! `PLAN.md`, `DECISIONS.md` ADR-0035 Punkt 3): führt ein Rhai-Skript
//! gegen den aktuellen Bearbeitungsstand eines Fotos aus.
//!
//! **Bewusst schmale API** (wie in `PLAN.md` gefordert): Skripte sehen
//! nicht das komplette `EdlV4` als Struktur, sondern rufen einzelne
//! primitiv-typisierte Getter/Setter auf (`edl.get_exposure()`,
//! `edl.set_exposure(1.5)`) — nur die zwölf Grundeinstellungs-Regler
//! (`BasicAdjustments`) plus Farbe/Schwarzweiß-Umschalter sind
//! abgedeckt, nicht Kurven/HSL/Masken/Objektivkorrekturen/etc. Das
//! genügt für den Hauptanwendungsfall (Stapel-Belichtungskorrektur per
//! Skript über viele Fotos) ohne die riesige Fläche des vollständigen
//! EDL an Rhai zu exponieren.
//!
//! `rhai` ist reines Rust (keine C-Bibliothek, im Gegensatz zu üblichen
//! Lua-Bindings) und sandboxbar — [`run_script`] begrenzt
//! Operationszahl/Aufrufschachtelung, damit ein fehlerhaftes/böswilliges
//! Skript nicht den Host blockiert oder abstürzt.

use apx_pipeline::edl::{EdlV4, Treatment};
use rhai::{Engine, Scope};
use thiserror::Error;

pub type Result<T> = std::result::Result<T, ScriptError>;

#[derive(Debug, Error)]
pub enum ScriptError {
    #[error("Skript-Syntax-/Laufzeitfehler: {message}")]
    Evaluation { message: String },

    #[error("Skript hat die 'edl'-Variable nach dem Lauf entfernt oder überschrieben")]
    MissingResult,
}

impl From<ScriptError> for apx_core::AppError {
    fn from(err: ScriptError) -> Self {
        apx_core::AppError::script(err.to_string())
    }
}

/// Obergrenze der von einem Skript ausgeführten Rhai-Operationen —
/// verhindert eine Endlosschleife im Nutzerskript, die den Host
/// blockieren würde (großzügig genug für jedes sinnvolle Batch-
/// Bearbeitungsskript, siehe Moduldoku).
const MAX_OPERATIONS: u64 = 200_000;

/// Bewusst schmaler Rhai-Wrapper um `EdlV4` — nur die Felder unten sind
/// überhaupt aus einem Skript heraus erreichbar (siehe Moduldoku).
#[derive(Debug, Clone)]
struct ScriptEdl(EdlV4);

macro_rules! basic_accessor {
    ($get:ident, $set:ident, $field:ident, $min:expr, $max:expr) => {
        fn $get(&mut self) -> f64 {
            self.0.basic.$field as f64
        }
        fn $set(&mut self, value: f64) {
            self.0.basic.$field = (value as f32).clamp($min, $max);
        }
    };
}

impl ScriptEdl {
    basic_accessor!(get_exposure, set_exposure, exposure_ev, -5.0, 5.0);
    basic_accessor!(get_contrast, set_contrast, contrast, -100.0, 100.0);
    basic_accessor!(get_highlights, set_highlights, highlights, -100.0, 100.0);
    basic_accessor!(get_shadows, set_shadows, shadows, -100.0, 100.0);
    basic_accessor!(get_whites, set_whites, whites, -100.0, 100.0);
    basic_accessor!(get_blacks, set_blacks, blacks, -100.0, 100.0);
    basic_accessor!(get_texture, set_texture, texture, -100.0, 100.0);
    basic_accessor!(get_clarity, set_clarity, clarity, -100.0, 100.0);
    basic_accessor!(get_dehaze, set_dehaze, dehaze, -100.0, 100.0);
    basic_accessor!(get_vibrance, set_vibrance, vibrance, -100.0, 100.0);
    basic_accessor!(get_saturation, set_saturation, saturation, -100.0, 100.0);

    fn get_temp_shift(&mut self) -> f64 {
        self.0.basic.white_balance.temp_shift_kelvin as f64
    }
    fn set_temp_shift(&mut self, value: f64) {
        self.0.basic.white_balance.temp_shift_kelvin = (value as f32).clamp(-2000.0, 2000.0);
    }
    fn get_tint_shift(&mut self) -> f64 {
        self.0.basic.white_balance.tint_shift as f64
    }
    fn set_tint_shift(&mut self, value: f64) {
        self.0.basic.white_balance.tint_shift = (value as f32).clamp(-100.0, 100.0);
    }

    fn get_black_and_white(&mut self) -> bool {
        self.0.treatment == Treatment::BlackAndWhite
    }
    fn set_black_and_white(&mut self, value: bool) {
        self.0.treatment = if value {
            Treatment::BlackAndWhite
        } else {
            Treatment::Color
        };
    }
}

fn build_engine() -> Engine {
    let mut engine = Engine::new();
    engine.set_max_operations(MAX_OPERATIONS);
    engine.set_max_call_levels(32);
    engine.set_max_expr_depths(64, 32);
    engine
        .register_type_with_name::<ScriptEdl>("Edl")
        .register_fn("get_exposure", ScriptEdl::get_exposure)
        .register_fn("set_exposure", ScriptEdl::set_exposure)
        .register_fn("get_contrast", ScriptEdl::get_contrast)
        .register_fn("set_contrast", ScriptEdl::set_contrast)
        .register_fn("get_highlights", ScriptEdl::get_highlights)
        .register_fn("set_highlights", ScriptEdl::set_highlights)
        .register_fn("get_shadows", ScriptEdl::get_shadows)
        .register_fn("set_shadows", ScriptEdl::set_shadows)
        .register_fn("get_whites", ScriptEdl::get_whites)
        .register_fn("set_whites", ScriptEdl::set_whites)
        .register_fn("get_blacks", ScriptEdl::get_blacks)
        .register_fn("set_blacks", ScriptEdl::set_blacks)
        .register_fn("get_texture", ScriptEdl::get_texture)
        .register_fn("set_texture", ScriptEdl::set_texture)
        .register_fn("get_clarity", ScriptEdl::get_clarity)
        .register_fn("set_clarity", ScriptEdl::set_clarity)
        .register_fn("get_dehaze", ScriptEdl::get_dehaze)
        .register_fn("set_dehaze", ScriptEdl::set_dehaze)
        .register_fn("get_vibrance", ScriptEdl::get_vibrance)
        .register_fn("set_vibrance", ScriptEdl::set_vibrance)
        .register_fn("get_saturation", ScriptEdl::get_saturation)
        .register_fn("set_saturation", ScriptEdl::set_saturation)
        .register_fn("get_temp_shift", ScriptEdl::get_temp_shift)
        .register_fn("set_temp_shift", ScriptEdl::set_temp_shift)
        .register_fn("get_tint_shift", ScriptEdl::get_tint_shift)
        .register_fn("set_tint_shift", ScriptEdl::set_tint_shift)
        .register_fn("get_black_and_white", ScriptEdl::get_black_and_white)
        .register_fn("set_black_and_white", ScriptEdl::set_black_and_white);
    engine
}

/// Führt `script` gegen `edl` aus — das Skript sieht eine Variable
/// `edl` vom Typ `Edl` (siehe [`ScriptEdl`]) und ruft deren
/// Getter/Setter auf, z. B.:
///
/// ```text
/// edl.set_exposure(edl.get_exposure() + 0.3);
/// edl.set_contrast(10.0);
/// ```
///
/// Gibt das veränderte `EdlV4` zurück, oder einen Fehler bei
/// Syntax-/Laufzeitfehlern.
pub fn run_script(edl: EdlV4, script: &str) -> Result<EdlV4> {
    let engine = build_engine();
    let mut scope = Scope::new();
    scope.push("edl", ScriptEdl(edl));
    engine
        .run_with_scope(&mut scope, script)
        .map_err(|err| ScriptError::Evaluation {
            message: err.to_string(),
        })?;
    scope
        .get_value::<ScriptEdl>("edl")
        .map(|wrapped| wrapped.0)
        .ok_or(ScriptError::MissingResult)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sets_exposure_absolute() {
        let edl = EdlV4::neutral();
        let result = run_script(edl, "edl.set_exposure(1.5);").expect("sollte laufen");
        assert_eq!(result.basic.exposure_ev, 1.5);
    }

    #[test]
    fn reads_and_writes_relative_to_the_current_value() {
        let mut edl = EdlV4::neutral();
        edl.basic.exposure_ev = 0.5;
        let result =
            run_script(edl, "edl.set_exposure(edl.get_exposure() + 0.3);").expect("sollte laufen");
        assert!((result.basic.exposure_ev - 0.8).abs() < 1e-5);
    }

    #[test]
    fn clamps_out_of_range_values_to_the_slider_bounds() {
        let edl = EdlV4::neutral();
        let result = run_script(edl, "edl.set_exposure(999.0);").expect("sollte laufen");
        assert_eq!(result.basic.exposure_ev, 5.0);
    }

    #[test]
    fn sets_multiple_fields_including_white_balance_and_treatment() {
        let edl = EdlV4::neutral();
        let script = r#"
            edl.set_contrast(10.0);
            edl.set_temp_shift(200.0);
            edl.set_black_and_white(true);
        "#;
        let result = run_script(edl, script).expect("sollte laufen");
        assert_eq!(result.basic.contrast, 10.0);
        assert_eq!(result.basic.white_balance.temp_shift_kelvin, 200.0);
        assert_eq!(result.treatment, Treatment::BlackAndWhite);
    }

    #[test]
    fn rejects_a_syntax_error() {
        let edl = EdlV4::neutral();
        let result = run_script(edl, "edl.set_exposure(");
        assert!(matches!(result, Err(ScriptError::Evaluation { .. })));
    }

    #[test]
    fn a_runaway_loop_is_stopped_by_the_operation_limit() {
        let edl = EdlV4::neutral();
        let result = run_script(edl, "loop { edl.set_exposure(1.0); }");
        assert!(matches!(result, Err(ScriptError::Evaluation { .. })));
    }

    #[test]
    fn an_empty_script_leaves_the_edl_unchanged() {
        let edl = EdlV4::neutral();
        let result = run_script(edl.clone(), "").expect("sollte laufen");
        assert_eq!(result, edl);
    }
}
