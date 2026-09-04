//! Eingebaute, selbst erstellte Filter-Looks (Phase 16 Schritt 2, siehe
//! `DECISIONS.md` ADR-0043-Nachtrag) — bewusst NICHT von einer der real
//! recherchierten externen LUT-Quellen heruntergeladen: keine einzelne
//! Quelle mit "Hunderte/Tausende einheitlich lizenzierte Filter"
//! gefunden, freie LUT-Pakete haben eine über Dutzende Quellen
//! verstreute, uneinheitliche Lizenzlage (siehe ADR-0043). Fünf
//! einfache, selbst formulierte Farbverläufe als Startpunkt — dieselbe
//! Rolle wie Lightrooms eigene mitgelieferte "Creative"-Profile: original
//! erstellt, kein Redistributions-/Lizenzrisiko, weil kein fremdes Werk
//! enthalten ist. Ergänzt (nicht ersetzt) den freien `.cube`-Import aus
//! Schritt 1 — für "Hunderte/Tausende Effekte" bringt der Nutzer eigene
//! Dateien mit (siehe `lut_cube`s Moduldoku).
//!
//! Jeder Look ist eine einfache parametrische Formel (keine tabellierten
//! Referenzwerte fremder Herkunft), am selben `size`-Raster wie ein
//! echter `.cube`-Import ausgewertet — [`generate`] liefert ein
//! [`crate::edl::v4::LutFilterData`], für `stages::lut_filter`
//! ununterscheidbar von einem importierten Filter.

use crate::edl::v4::LutFilterData;

/// Dieselbe Luminanz-Gewichtung wie `apx_ai::seam_carving::luminance_map`
/// — Konsistenz statt einer zweiten, leicht abweichenden Konstante.
fn luminance(r: f32, g: f32, b: f32) -> f32 {
    0.3 * r + 0.59 * g + 0.11 * b
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuiltinLut {
    Warm,
    Cool,
    HighContrastBw,
    Faded,
    TealOrange,
}

impl BuiltinLut {
    pub const ALL: [Self; 5] = [
        Self::Warm,
        Self::Cool,
        Self::HighContrastBw,
        Self::Faded,
        Self::TealOrange,
    ];

    pub fn id(self) -> &'static str {
        match self {
            Self::Warm => "warm",
            Self::Cool => "cool",
            Self::HighContrastBw => "high_contrast_bw",
            Self::Faded => "faded",
            Self::TealOrange => "teal_orange",
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Warm => "Warm",
            Self::Cool => "Kühl",
            Self::HighContrastBw => "Kontrastreich S/W",
            Self::Faded => "Verblasst",
            Self::TealOrange => "Kino Teal-Orange",
        }
    }

    /// Die eigentliche Farbformel, unnormiert (Aufrufer klemmt auf
    /// `0.0..=1.0`, siehe [`generate`]).
    fn transform(self, r: f32, g: f32, b: f32) -> [f32; 3] {
        match self {
            // Leichte Warmton-Verschiebung: mehr Rot, weniger Blau.
            Self::Warm => [r + 0.06, g, b - 0.06],
            // Umgekehrt: mehr Blau, weniger Rot.
            Self::Cool => [r - 0.06, g, b + 0.06],
            // Entsättigt auf Luminanz, dann S-Kurven-Kontrast um den
            // Mittelwert (`1.35`-facher Abstand zu `0.5`).
            Self::HighContrastBw => {
                let l = luminance(r, g, b);
                let c = 0.5 + (l - 0.5) * 1.35;
                [c, c, c]
            }
            // Angehobene Schwarzwerte + gestauchter Kontrast je Kanal
            // leicht unterschiedlich (klassischer "Matte-Film"-Look).
            Self::Faded => [r * 0.82 + 0.10, g * 0.85 + 0.09, b * 0.88 + 0.08],
            // Split-Tone nach Luminanz: Lichter Richtung Orange
            // (`hi`-Anteil), Schatten Richtung Türkis (`lo`-Anteil).
            Self::TealOrange => {
                let l = luminance(r, g, b);
                let hi = l;
                let lo = 1.0 - l;
                [
                    r + 0.10 * hi - 0.04 * lo,
                    g + 0.03 * hi + 0.02 * lo,
                    b - 0.06 * hi + 0.08 * lo,
                ]
            }
        }
    }
}

/// Wertet einen eingebauten Look an jedem Punkt eines `size`-Rasters aus
/// — dieselbe Rasterreihenfolge wie `lut_cube::ParsedLut` (r am
/// schnellsten variierend, dann g, dann b).
pub fn generate(kind: BuiltinLut, size: u32) -> LutFilterData {
    let n = size.max(2);
    let denom = (n - 1) as f32;
    let mut table = Vec::with_capacity((n as usize).pow(3) * 3);
    for bi in 0..n {
        for gi in 0..n {
            for ri in 0..n {
                let r = ri as f32 / denom;
                let g = gi as f32 / denom;
                let b = bi as f32 / denom;
                let [or, og, ob] = kind.transform(r, g, b);
                table.push(or.clamp(0.0, 1.0));
                table.push(og.clamp(0.0, 1.0));
                table.push(ob.clamp(0.0, 1.0));
            }
        }
    }
    LutFilterData {
        name: kind.name().to_string(),
        size: n,
        table,
        domain_min: [0.0, 0.0, 0.0],
        domain_max: [1.0, 1.0, 1.0],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generates_correct_length_and_range() {
        for kind in BuiltinLut::ALL {
            let lut = generate(kind, 9);
            assert_eq!(lut.size, 9);
            assert_eq!(lut.table.len(), 9 * 9 * 9 * 3);
            assert!(lut.table.iter().all(|v| (0.0..=1.0).contains(v)));
        }
    }

    #[test]
    fn every_id_and_name_is_unique() {
        let ids: Vec<_> = BuiltinLut::ALL.iter().map(|k| k.id()).collect();
        let mut sorted_ids = ids.clone();
        sorted_ids.sort_unstable();
        sorted_ids.dedup();
        assert_eq!(ids.len(), sorted_ids.len());

        let names: Vec<_> = BuiltinLut::ALL.iter().map(|k| k.name()).collect();
        let mut sorted_names = names.clone();
        sorted_names.sort_unstable();
        sorted_names.dedup();
        assert_eq!(names.len(), sorted_names.len());
    }

    #[test]
    fn minimum_grid_size_is_clamped_to_two() {
        let lut = generate(BuiltinLut::Warm, 1);
        assert_eq!(lut.size, 2);
        assert_eq!(lut.table.len(), 2 * 2 * 2 * 3);
    }
}
