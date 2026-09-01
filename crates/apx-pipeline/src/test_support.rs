//! Gemeinsame synthetische Testmuster für alle Regler-Module (siehe
//! `PLAN.md` Phase 2 Schritt 8) — vermeidet, dass jedes der sieben
//! Regler-Testmodule denselben Rampen-/Verlaufs-Code eigenständig
//! abschreibt (vor Schritt 8 stand `(0..300).map(|i| (i as f32) /
//! 300.0).collect()` siebenmal wortgleich in `stages/*.rs`).
//!
//! Nur für Tests gedacht (`#[cfg(test)] mod test_support;` in `lib.rs`)
//! — siehe `DECISIONS.md` ADR-0007: es gibt weiterhin keine echten
//! RAW-Testdateien, alle Pipeline-Tests bleiben auf diese synthetischen
//! Muster angewiesen.

/// Eine lineare Rampe von `0.0` bis knapp unter `1.0` mit `len` Werten —
/// das am häufigsten verwendete Testmuster: deckt den vollen
/// `[0, 1]`-Wertebereich linear ab, ohne echte Bilddaten zu brauchen.
pub(crate) fn ramp(len: usize) -> Vec<f32> {
    (0..len).map(|i| i as f32 / len as f32).collect()
}

/// Ein interleaved-RGB-Grauverlauf (`R == G == B` an jedem Pixel) mit
/// `pixel_count` Pixeln (`3 * pixel_count` Elemente) — für Operationen,
/// die alle drei Kanäle gleich behandeln sollen (Belichtung, Kontrast,
/// Lichter/Tiefen, Weiß/Schwarz).
pub(crate) fn gray_gradient(pixel_count: usize) -> Vec<f32> {
    ramp(pixel_count)
        .into_iter()
        .flat_map(|v| [v, v, v])
        .collect()
}

/// Ein interleaved-RGB-Muster mit gesättigten Extremwerten je Kanal
/// (`0.0`/`1.0`, zyklisch über R→G→B verschoben) mit `pixel_count`
/// Pixeln — für Operationen, die pro Kanal unterschiedlich reagieren
/// sollen (Weißabgleich-Gains).
pub(crate) fn saturated_channels(pixel_count: usize) -> Vec<f32> {
    (0..pixel_count)
        .flat_map(|i| match i % 3 {
            0 => [1.0, 0.0, 0.0],
            1 => [0.0, 1.0, 0.0],
            _ => [0.0, 0.0, 1.0],
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ramp_covers_full_range_without_reaching_one() {
        let values = ramp(10);
        assert_eq!(values.len(), 10);
        assert_eq!(values[0], 0.0);
        assert!(values[9] < 1.0 && values[9] > 0.8);
    }

    #[test]
    fn gray_gradient_has_equal_channels_per_pixel() {
        let pixels = gray_gradient(4);
        assert_eq!(pixels.len(), 4 * 3);
        for chunk in pixels.chunks_exact(3) {
            assert_eq!(chunk[0], chunk[1]);
            assert_eq!(chunk[1], chunk[2]);
        }
    }

    #[test]
    fn saturated_channels_are_pure_primaries() {
        let pixels = saturated_channels(3);
        assert_eq!(&pixels[0..3], &[1.0, 0.0, 0.0]);
        assert_eq!(&pixels[3..6], &[0.0, 1.0, 0.0]);
        assert_eq!(&pixels[6..9], &[0.0, 0.0, 1.0]);
    }
}
