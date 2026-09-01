//! Beispiel-Plugin für `apx-plugin-host`s Integrationstest (Phase 9
//! Schritt 9, siehe `apx-plugin-abi`s Moduldoku für den vollständigen
//! Vertrag). Ein einziger Custom-Effekt: Farben invertieren
//! (`255 - kanal`), `param` (`0.0..=1.0`) mischt zwischen Original
//! (`0.0`) und voller Invertierung (`1.0`).
//!
//! Kein Bestandteil der ausgelieferten App — reine Testinfrastruktur,
//! die beweist, dass der ABI-Vertrag über eine echte Prozessgrenze
//! (`dlopen`/`LoadLibrary`) hinweg tatsächlich funktioniert, statt nur
//! auf dem Papier zu existieren.

use std::os::raw::c_char;
use std::slice;

use apx_plugin_abi::{
    ApxCustomEffectFn, ApxEffectStatus, ApxPixelFormat, ApxPluginTable, APX_PLUGIN_ABI_VERSION,
};

const PLUGIN_NAME: &[u8] = b"Aperture X Beispiel-Plugin (Invertieren)\0";

/// Die eigentliche Effekt-Implementierung — invertiert jeden Farbkanal,
/// linear mit `param` gemischt (`0.0` = unverändert, `1.0` = voll
/// invertiert). Alpha bleibt unverändert.
extern "C" fn apply_invert(
    pixels: *mut u8,
    width: u32,
    height: u32,
    stride: u32,
    format: ApxPixelFormat,
    param: f32,
) -> ApxEffectStatus {
    if format != ApxPixelFormat::Rgba8 {
        return ApxEffectStatus::InvalidInput;
    }
    if width == 0 || height == 0 || stride < width * 4 || pixels.is_null() {
        return ApxEffectStatus::InvalidInput;
    }
    let mix = param.clamp(0.0, 1.0);
    // Safety: der Aufrufer garantiert laut ABI-Vertrag mindestens
    // `stride * height` gültige, beschreibbare Bytes ab `pixels`.
    let buffer =
        unsafe { slice::from_raw_parts_mut(pixels, (stride as usize) * (height as usize)) };
    for y in 0..height as usize {
        let row_start = y * stride as usize;
        for x in 0..width as usize {
            let pixel_index = row_start + x * 4;
            for channel in 0..3 {
                let original = buffer[pixel_index + channel] as f32;
                let inverted = 255.0 - original;
                buffer[pixel_index + channel] =
                    (original + mix * (inverted - original)).round() as u8;
            }
            // Alpha (Index 3) bleibt unverändert.
        }
    }
    ApxEffectStatus::Ok
}

static PLUGIN_TABLE: ApxPluginTable = ApxPluginTable {
    abi_version: APX_PLUGIN_ABI_VERSION,
    plugin_name: PLUGIN_NAME.as_ptr() as *const c_char,
    apply_custom_effect: Some(apply_invert as ApxCustomEffectFn),
};

/// Der einzige exportierte Einstiegspunkt, den `apx-plugin-host` per
/// `dlopen`/`libloading` sucht (siehe `apx-plugin-abi`s Moduldoku für
/// den vollständigen Vertrag).
///
/// # Safety
/// Wird nur von `apx-plugin-host` über die C-ABI aufgerufen; die
/// zurückgegebene Tabelle ist `'static` (`static PLUGIN_TABLE`).
#[no_mangle]
pub extern "C" fn apx_plugin_table() -> *const ApxPluginTable {
    &PLUGIN_TABLE
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn full_strength_inverts_exactly() {
        let mut pixels = solid(2, 2, 10, 20, 30);
        let status = apply_invert(pixels.as_mut_ptr(), 2, 2, 2 * 4, ApxPixelFormat::Rgba8, 1.0);
        assert_eq!(status, ApxEffectStatus::Ok);
        assert_eq!(&pixels[0..4], &[245, 235, 225, 255]);
    }

    #[test]
    fn zero_strength_is_identity() {
        let original = solid(2, 2, 10, 20, 30);
        let mut pixels = original.clone();
        let status = apply_invert(pixels.as_mut_ptr(), 2, 2, 2 * 4, ApxPixelFormat::Rgba8, 0.0);
        assert_eq!(status, ApxEffectStatus::Ok);
        assert_eq!(pixels, original);
    }

    #[test]
    fn rejects_a_zero_sized_image() {
        let mut pixels: Vec<u8> = Vec::new();
        let status = apply_invert(pixels.as_mut_ptr(), 0, 0, 0, ApxPixelFormat::Rgba8, 1.0);
        assert_eq!(status, ApxEffectStatus::InvalidInput);
    }

    #[test]
    fn exported_table_has_the_current_abi_version_and_an_effect_function() {
        let table = unsafe { &*apx_plugin_table() };
        assert_eq!(table.abi_version, APX_PLUGIN_ABI_VERSION);
        assert!(table.apply_custom_effect.is_some());
        assert!(!table.plugin_name.is_null());
    }
}
