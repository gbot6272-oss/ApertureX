//! Version 3 des EDL-Schemas: fügt Version 2 das Maskensystem hinzu
//! (`masks: Vec<Mask>`, siehe `SPEC.md` §5, `DECISIONS.md` ADR-0032).
//!
//! `v2::EdlV2` bleibt unverändert bestehen (historische Einträge in
//! `apx-catalog`s `edit_history.edl_json` sind Schema-Version 1 oder 2) —
//! alle Daten werden über [`super::migrate::from_envelope`] immer zu
//! `EdlV3` hochgezogen. Wie beim v1→v2-Sprung in Phase 4 (siehe dortiger
//! Schritt 1) ist `masks` bei `v2_to_v3` schlicht leer — bestehende
//! Bearbeitungen werden dadurch nicht verändert, sie hatten schon vorher
//! keine lokalen Anpassungen.
//!
//! Alle fünf Maskentypen (Pinsel, Linearer/Radialer Verlauf, Farbbereich,
//! Luminanzbereich) sind hier bereits vollständig als Datenstruktur
//! festgelegt, auch wenn ihre tatsächliche Pipeline-/Shader-Umsetzung erst
//! in den folgenden Schritten kommt (siehe `PLAN.md` Phase 6) — dasselbe
//! Vorgehen wie bei `v2.rs`s Schritt 1.
//!
//! **Bewusst nicht Teil dieses Schemas** (siehe `DECISIONS.md` ADR-0032
//! Punkt 3): Tiefenbereich-Masken (kein Tiefendaten-Zulieferer) und die
//! fünf KI-Masken (Motiv/Himmel/Hintergrund/Objekte/Personen — Phase 7).

use serde::{Deserialize, Serialize};

use super::v2::{
    self, BasicAdjustments, ColorGradingAdjustment, ColorMixerAdjustment, CurvesAdjustment,
    DetailsAdjustment, HslAdjustment,
};

// ---- Maskengeometrie ---------------------------------------------------------

/// Ein einzelner Pinsel-Abstrichpunkt, in normierten Bildkoordinaten
/// (`0.0..=1.0`) — analog zu `v2::RepairPoint`, aber als eigenständige
/// weiche Maske statt eines Klon-/Reparatur-Versatzes.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct MaskPoint {
    pub x: f32,
    pub y: f32,
}

/// Ein zusammenhängender Pinselzug innerhalb einer Pinsel-Maskenkomponente
/// — mehrere Striche akkumulieren ihre Deckung (Maximum je Pixel, nicht
/// Summe, sonst würde mehrfaches Übermalen unbeabsichtigt über 100 %
/// Deckkraft hinaus verstärken).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BrushStroke {
    pub points: Vec<MaskPoint>,
    pub radius: f32,
    pub feather: f32,
}

/// Wie eine Maskenkomponente ihre räumliche Ausdehnung bestimmt — die
/// fünf in `SPEC.md` §5 genannten Maskentypen.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum MaskGeometry {
    Brush {
        strokes: Vec<BrushStroke>,
    },
    /// Alpha = 1 an `(x1,y1)`, linear abfallend auf 0 bei `(x2,y2)`,
    /// darüber hinaus geklemmt — dieselbe Konvention wie Lightrooms
    /// linearer Verlauf (die beiden Punkte sind bereits die volle
    /// Übergangsstrecke, kein zusätzlicher `feather`-Parameter nötig).
    LinearGradient {
        x1: f32,
        y1: f32,
        x2: f32,
        y2: f32,
    },
    /// Ellipse um `(center_x, center_y)` mit Halbachsen `radius_x`/
    /// `radius_y`, um `angle_degrees` gedreht. `feather` (`0.0..=1.0`)
    /// weicht den Rand nach innen auf (0 = harte Kante an der
    /// Ellipsengrenze, 1 = Übergang bis zum Mittelpunkt).
    RadialGradient {
        center_x: f32,
        center_y: f32,
        radius_x: f32,
        radius_y: f32,
        angle_degrees: f32,
        feather: f32,
    },
    /// Referenzfarbe (linearer Arbeitsraum) aus einem Bildklick,
    /// `tolerance` bestimmt den Abstand, ab dem ein Pixel als
    /// nicht-zugehörig gilt, `feather` weicht den Übergang auf.
    ColorRange {
        target_r: f32,
        target_g: f32,
        target_b: f32,
        tolerance: f32,
        feather: f32,
    },
    /// Luminanzbereich `range_min..=range_max` (`0.0..=1.0`), `feather`
    /// weicht beide Enden auf.
    LuminanceRange {
        range_min: f32,
        range_max: f32,
        feather: f32,
    },
    /// Eine per klassischer Bildanalyse einmalig berechnete Alpha-Bitmap
    /// (Phase 7, `apx-ai`, siehe `DECISIONS.md` ADR-0033 Punkt 3) — im
    /// Unterschied zu den fünf parametrischen Geometrietypen oben ist das
    /// Ergebnis naturgemäß eine Rasterfläche, kein Parametersatz.
    /// `width`/`height` sind die Auflösung von `alpha` (lange Kante auf
    /// 512px begrenzt, siehe `apx_core::raster::fit_within`), **nicht**
    /// die tatsächliche Bildauflösung — `stages/masks.rs` skaliert beim
    /// Rendern bilinear auf die jeweils angeforderte Zielgröße hoch.
    /// Bleibt bis zu einer erneuten Generierung unverändert (kein Re-Run
    /// bei jedem Regler-Tick).
    /// (`ai_kind` statt schlicht `kind`, weil `MaskGeometry` selbst
    /// `#[serde(tag = "kind")]` nutzt — ein gleichnamiges Variantenfeld
    /// kollidiert mit dem internen Diskriminator.)
    AiGenerated {
        ai_kind: AiMaskKind,
        width: u32,
        height: u32,
        alpha: Vec<u8>,
    },
}

/// Welche der fünf KI-Masken-Heuristiken (`apx-ai::segmentation`) eine
/// [`MaskGeometry::AiGenerated`]-Bitmap erzeugt hat — rein informativ
/// fürs Frontend (z. B. Anzeige-Label „Motiv"/„Himmel"), die Pipeline
/// selbst behandelt jede Variante identisch (nur das Alpha zählt).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AiMaskKind {
    Subject,
    Sky,
    Background,
    ClickRegion,
    Person,
}

impl MaskGeometry {
    /// Eine leere Pinsel-Maske — der neutrale Startzustand beim Anlegen
    /// einer neuen Maske (Nutzer malt die eigentliche Form erst danach).
    pub fn empty_brush() -> Self {
        Self::Brush {
            strokes: Vec::new(),
        }
    }
}

/// Wie eine Maskenkomponente mit der akkumulierten Alpha der
/// vorangehenden Komponenten *derselben* Maske verrechnet wird (`SPEC.md`
/// §5: „Maskenkombination").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MaskCombine {
    Add,
    Subtract,
    Intersect,
}

/// Eine Maske kann aus mehreren Komponenten zusammengesetzt sein (z. B.
/// Pinsel + Farbbereich, kombiniert per Schneiden) — jede Komponente
/// trägt ihre eigene Geometrie, eine eigene Invertierung und wie sie mit
/// den vorangehenden Komponenten verrechnet wird. Die erste Komponente
/// einer Maske ignoriert `combine` (nichts, womit sie kombiniert werden
/// könnte) — wird beim Auswerten wie `Add` gegen eine anfangs leere
/// (überall `0.0`) Alpha behandelt.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MaskComponent {
    pub geometry: MaskGeometry,
    pub combine: MaskCombine,
    pub invert: bool,
}

// ---- Pro Maske verfügbare Werkzeuge ------------------------------------------

/// Die ton-/farb-/detailbezogenen Werkzeuge, die pro Maske zur Verfügung
/// stehen (`DECISIONS.md` ADR-0032 Punkt 2) — bewusst ohne
/// Objektivkorrekturen/Effekte/Kalibrierung/Geometrie/Reparatur, die
/// strukturell Ganzbild-Operationen bleiben.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MaskAdjustments {
    pub basic: BasicAdjustments,
    pub curves: CurvesAdjustment,
    pub hsl: HslAdjustment,
    pub color_mixer: ColorMixerAdjustment,
    pub color_grading: ColorGradingAdjustment,
    pub details: DetailsAdjustment,
}

impl MaskAdjustments {
    pub fn neutral() -> Self {
        Self {
            basic: BasicAdjustments::NEUTRAL,
            curves: CurvesAdjustment::neutral(),
            hsl: HslAdjustment::NEUTRAL,
            color_mixer: ColorMixerAdjustment::neutral(),
            color_grading: ColorGradingAdjustment::NEUTRAL,
            details: DetailsAdjustment::NEUTRAL,
        }
    }
}

impl Default for MaskAdjustments {
    fn default() -> Self {
        Self::neutral()
    }
}

// ---- Ebenen-Mischmodi ---------------------------------------------------------

/// Wie die maskiert bearbeitete Bildkopie mit dem bisherigen Bildzustand
/// zurückgemischt wird — `Normal` plus die in `SPEC.md` §3.3 namentlich
/// genannten Beispiele („Multiplizieren, Weiches Licht, Farbe,
/// Luminanz").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BlendMode {
    Normal,
    Multiply,
    SoftLight,
    Color,
    Luminosity,
}

/// Überlagerungsfarbe zur Masken-Visualisierung im Viewer (`SPEC.md`
/// §3.3: „Maskenüberlagerung in wählbarer Farbe").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OverlayColor {
    Red,
    Green,
    Blue,
    Yellow,
    Magenta,
}

// ---- Die Maske selbst ---------------------------------------------------------

/// Eine lokale Anpassung (`SPEC.md` §3.3) — clientseitig vergebene `id`
/// (keine `apx_core`-ID: Masken leben ausschließlich im opaken
/// EDL-JSON-Blob, nie als eigene Katalogzeile, siehe `ARCHITECTURE.md`
/// §5).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Mask {
    pub id: String,
    pub name: String,
    /// Mindestens eine Komponente — leer ergibt überall Alpha 0 (Maske
    /// ohne Wirkung), ist aber kein Fehlerzustand (Zwischenschritt beim
    /// Anlegen, bevor die erste Komponente hinzugefügt wird).
    pub components: Vec<MaskComponent>,
    pub adjustments: MaskAdjustments,
    /// Gesamtdeckkraft der Maske, `0.0..=100.0`.
    pub opacity: f32,
    /// Zusätzliche globale Weichzeichnung der zusammengesetzten Alpha,
    /// `0.0..=100.0`.
    pub feather: f32,
    /// Invertiert die *gesamte* zusammengesetzte Maske (nach den
    /// Komponenten, vor Deckkraft/Weichzeichnung).
    pub invert: bool,
    pub blend_mode: BlendMode,
    pub visible: bool,
    /// `None` = keiner Gruppe zugeordnet.
    pub group_id: Option<String>,
    pub overlay_color: OverlayColor,
}

impl Mask {
    /// Eine neue, leere Pinsel-Maske mit neutralen Anpassungen — der
    /// Startzustand beim Anlegen einer Maske im Frontend.
    pub fn new_brush(id: String, name: String) -> Self {
        Self {
            id,
            name,
            components: vec![MaskComponent {
                geometry: MaskGeometry::empty_brush(),
                combine: MaskCombine::Add,
                invert: false,
            }],
            adjustments: MaskAdjustments::neutral(),
            opacity: 100.0,
            feather: 0.0,
            invert: false,
            blend_mode: BlendMode::Normal,
            visible: true,
            group_id: None,
            overlay_color: OverlayColor::Red,
        }
    }
}

/// Eine Maskengruppe (`SPEC.md` §3.3: „Maskengruppen, Umbenennen,
/// Ein-/Ausblenden") — rein organisatorisch, hat selbst keine Geometrie
/// oder Anpassungen.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MaskGroup {
    pub id: String,
    pub name: String,
    pub visible: bool,
}

// ---- Der vollständige EDL v3 -----------------------------------------------

/// Die konkrete EDL-Struktur für Schema-Version 3 — siehe
/// [`crate::edl::EDL_SCHEMA_VERSION`] und [`crate::edl::migrate`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EdlV3 {
    pub basic: v2::BasicAdjustments,
    pub curves: v2::CurvesAdjustment,
    pub hsl: v2::HslAdjustment,
    pub color_mixer: v2::ColorMixerAdjustment,
    pub color_grading: v2::ColorGradingAdjustment,
    pub details: v2::DetailsAdjustment,
    pub lens_corrections: v2::LensCorrectionAdjustment,
    pub effects: v2::EffectsAdjustment,
    pub calibration: v2::CalibrationAdjustment,
    pub geometry: v2::GeometryAdjustment,
    pub repair: Vec<v2::RepairStroke>,
    /// Reihenfolge ist die Anwendungsreihenfolge — spätere Masken
    /// überschreiben/überlagern frühere an denselben Pixeln, gesteuert
    /// über ihren jeweiligen `blend_mode` (siehe `DECISIONS.md`
    /// ADR-0032 Punkt 4).
    pub masks: Vec<Mask>,
    pub mask_groups: Vec<MaskGroup>,
}

impl EdlV3 {
    /// Die neutrale Bearbeitung: alle Regler unverändert, keine
    /// Reparatur-Striche, keine Masken.
    pub fn neutral() -> Self {
        Self {
            basic: v2::BasicAdjustments::NEUTRAL,
            curves: v2::CurvesAdjustment::neutral(),
            hsl: v2::HslAdjustment::NEUTRAL,
            color_mixer: v2::ColorMixerAdjustment::neutral(),
            color_grading: v2::ColorGradingAdjustment::NEUTRAL,
            details: v2::DetailsAdjustment::NEUTRAL,
            lens_corrections: v2::LensCorrectionAdjustment::neutral(),
            effects: v2::EffectsAdjustment::NEUTRAL,
            calibration: v2::CalibrationAdjustment::neutral(),
            geometry: v2::GeometryAdjustment::NEUTRAL,
            repair: Vec::new(),
            masks: Vec::new(),
            mask_groups: Vec::new(),
        }
    }

    /// Zieht ein `EdlV2` hoch — alles darin bleibt unverändert übernommen,
    /// `masks`/`mask_groups` starten leer. Einziger Aufrufer ist
    /// [`super::migrate::from_envelope`], wenn ein gespeicherter Umschlag
    /// noch `schema_version == 2` trägt.
    pub fn from_v2(old: v2::EdlV2) -> Self {
        Self {
            basic: old.basic,
            curves: old.curves,
            hsl: old.hsl,
            color_mixer: old.color_mixer,
            color_grading: old.color_grading,
            details: old.details,
            lens_corrections: old.lens_corrections,
            effects: old.effects,
            calibration: old.calibration,
            geometry: old.geometry,
            repair: old.repair,
            masks: Vec::new(),
            mask_groups: Vec::new(),
        }
    }
}

impl Default for EdlV3 {
    fn default() -> Self {
        Self::neutral()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn neutral_roundtrips_through_json() {
        let edl = EdlV3::neutral();
        let json = serde_json::to_string(&edl).expect("sollte serialisieren");
        let parsed: EdlV3 = serde_json::from_str(&json).expect("sollte parsen");
        assert_eq!(edl, parsed);
    }

    #[test]
    fn default_equals_neutral() {
        assert_eq!(EdlV3::default(), EdlV3::neutral());
    }

    #[test]
    fn from_v2_preserves_all_fields_and_starts_with_no_masks() {
        let old = v2::EdlV2 {
            basic: v2::BasicAdjustments {
                exposure_ev: 0.5,
                ..v2::BasicAdjustments::NEUTRAL
            },
            ..v2::EdlV2::neutral()
        };
        let upgraded = EdlV3::from_v2(old);
        assert_eq!(upgraded.basic.exposure_ev, 0.5);
        assert_eq!(upgraded.masks, Vec::new());
        assert_eq!(upgraded.mask_groups, Vec::new());
    }

    #[test]
    fn mask_with_multiple_components_roundtrips() {
        let mask = Mask {
            components: vec![
                MaskComponent {
                    geometry: MaskGeometry::LinearGradient {
                        x1: 0.0,
                        y1: 0.0,
                        x2: 1.0,
                        y2: 1.0,
                    },
                    combine: MaskCombine::Add,
                    invert: false,
                },
                MaskComponent {
                    geometry: MaskGeometry::ColorRange {
                        target_r: 0.8,
                        target_g: 0.2,
                        target_b: 0.2,
                        tolerance: 0.1,
                        feather: 0.2,
                    },
                    combine: MaskCombine::Intersect,
                    invert: true,
                },
            ],
            ..Mask::new_brush("mask-1".to_string(), "Himmel".to_string())
        };
        let json = serde_json::to_string(&mask).expect("sollte serialisieren");
        let parsed: Mask = serde_json::from_str(&json).expect("sollte parsen");
        assert_eq!(mask, parsed);
    }

    #[test]
    fn mask_geometry_kind_is_present_in_serialized_json() {
        let geometry = MaskGeometry::empty_brush();
        let value = serde_json::to_value(&geometry).expect("sollte serialisieren");
        assert_eq!(value["kind"], "Brush");
    }

    #[test]
    fn new_brush_mask_has_neutral_adjustments_and_is_visible() {
        let mask = Mask::new_brush("id".to_string(), "Neue Maske".to_string());
        assert_eq!(mask.adjustments, MaskAdjustments::neutral());
        assert!(mask.visible);
        assert_eq!(mask.components.len(), 1);
    }
}
