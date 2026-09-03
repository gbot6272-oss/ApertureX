//! Version 4 des EDL-Schemas: fügt Version 3 ein Aktivieren/Überspringen
//! je Rendering-Stufe hinzu (`stage_enabled`) — die Datengrundlage für
//! den Node-Editor (Phase 9 Schritt 7, siehe `PLAN.md`, `DECISIONS.md`
//! ADR-0035 Punkt 1).
//!
//! **Wichtig**: der Node-Editor bildet **keine** frei umbaubare/
//! verzweigende Graph-Engine ab — `develop::render_rgba8`s Reihenfolge
//! bleibt exakt dieselbe feste Kette wie zuvor. Ein Knoten je Stufe,
//! in genau dieser Reihenfolge; `stage_enabled` ist die einzige neue
//! Steuergröße und bedeutet „diese Stufe überspringen" (Eingabe
//! unverändert durchreichen), nicht „diese Stufe woanders einfügen".
//! Das erhält die Renderpfad-Garantie, auf die jedes andere Modul
//! angewiesen ist (siehe Plan-Notiz zu Schritt 7).
//!
//! `v3::EdlV3` bleibt unverändert bestehen (historische
//! `edit_history.edl_json`-Einträge sind Schema-Version 1, 2 oder 3) —
//! alle Daten werden über [`super::migrate::from_envelope`] auf `EdlV4`
//! hochgezogen, genau wie beim v2→v3-Sprung in Phase 6.

use serde::{Deserialize, Serialize};

use super::v2;
use super::v3::{self, BlackAndWhiteMixerAdjustment, BlendMode, Mask, MaskGroup, Treatment};

// ---- Stufen-Aktivierung (Node-Editor) --------------------------------------

/// Ob eine einzelne Rendering-Stufe angewendet wird — ein Feld je Knoten
/// im Node-Editor, in derselben festen Reihenfolge wie
/// `develop::render_rgba8`. `false` heißt „Eingabe dieser Stufe
/// unverändert an die nächste durchreichen", die Stufenreihenfolge
/// selbst ändert sich nie.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct StageEnabled {
    pub repair: bool,
    pub calibration: bool,
    pub basic: bool,
    pub local_contrast: bool,
    pub details: bool,
    pub hsl_color_mixer: bool,
    pub color_grading: bool,
    pub lens_corrections: bool,
    pub effects: bool,
    pub masks: bool,
    pub treatment: bool,
    pub curves: bool,
    /// Mehrfachbelichtung/Layer-Compositing (Phase 14 Schritt 3, siehe
    /// `DECISIONS.md` ADR-0041) — läuft nach `curves`, vor `geometry`
    /// (siehe `stages::composite`s Moduldoku für die Begründung).
    /// `#[serde(default = "default_true")]` statt eines bloßen
    /// `#[serde(default)]`: ein gespeichertes `StageEnabled`-Objekt von
    /// vor diesem Feld muss weiterhin als „diese (damals noch nicht
    /// existierende) Stufe war aktiv" gelesen werden, nicht als
    /// „deaktiviert" (`bool`s eigener `Default` wäre `false`) —
    /// dieselbe Konvention wie [`StageEnabled::ALL`], wo jede Stufe
    /// `true` startet.
    #[serde(default = "default_true")]
    pub composite: bool,
    /// KI-Tiefenschärfe-Simulator "Virtuelle Blende" (Phase 14 Schritt 8,
    /// siehe `DECISIONS.md` ADR-0041 Nachtrag VIII) — läuft nach
    /// `effects` (Halation), vor `masks`, noch im linearen Arbeitsraum
    /// (siehe `stages::virtual_aperture`s Moduldoku). Dieselbe
    /// `default_true`-Begründung wie `composite` oben.
    #[serde(default = "default_true")]
    pub virtual_aperture: bool,
    /// KI-Stiltransfer zwischen Fotos (Phase 14 Schritt 9, siehe
    /// `DECISIONS.md` ADR-0041 Nachtrag IX) — läuft nach `composite`,
    /// vor `geometry`, im fertig entwickelten sRGB-RGBA8-Bild (siehe
    /// `stages::style_transfer`s Moduldoku). Dieselbe
    /// `default_true`-Begründung wie `composite`/`virtual_aperture` oben.
    #[serde(default = "default_true")]
    pub style_transfer: bool,
    pub geometry: bool,
}

fn default_true() -> bool {
    true
}

impl StageEnabled {
    /// Der Ausgangszustand: jede Stufe aktiv — identisch zum Verhalten
    /// vor diesem Schema-Sprung.
    pub const ALL: Self = Self {
        repair: true,
        calibration: true,
        basic: true,
        local_contrast: true,
        details: true,
        hsl_color_mixer: true,
        color_grading: true,
        lens_corrections: true,
        effects: true,
        masks: true,
        treatment: true,
        curves: true,
        composite: true,
        virtual_aperture: true,
        style_transfer: true,
        geometry: true,
    };
}

impl Default for StageEnabled {
    fn default() -> Self {
        Self::ALL
    }
}

// ---- Mehrfachbelichtung/Layer-Compositing (Phase 14 Schritt 3) -------------

/// Ein einmalig aufgelöstes Foto oder eine Textur, als fertige Bitmap
/// gespeichert (Phase 14 Schritt 3, siehe `DECISIONS.md` ADR-0041) —
/// dasselbe „einmal per Command auflösen, bei jedem Rendern nur noch
/// skalieren"-Muster wie `v2::AiFillPatch`/`v2::CanvasExtensionPatch`.
/// Diese Crate hat keinen Katalog-/Dateisystemzugriff — das Auflösen
/// eines `photo_id`-Verweises oder einer vom Nutzer gewählten
/// Textur-Datei passiert außerhalb, in `apx-app`s Tauri-Commands; hier
/// kommt nur noch das fertige Ergebnis an. `pixels` ist interleaved RGB
/// (`0..=255`), `bitmap_width * bitmap_height * 3` Zahlen lang —
/// dieselbe Konvention wie `AiFillPatch::pixels`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CompositeLayerSource {
    pub bitmap_width: u32,
    pub bitmap_height: u32,
    pub pixels: Vec<u8>,
}

/// Eine einzelne Ebene für Mehrfachbelichtung/Compositing — Lightroom
/// Classic selbst hat "keine klassischen Ebenen-Kompositionsfähigkeiten
/// wie Photoshop" (siehe `DECISIONS.md` ADR-0041s Recherche-Tabelle,
/// Punkt 5). Wiederverwendet dieselben Blend-Modi wie die Masken-Stufe
/// (`v3::BlendMode`, `stages::masks::blend_pixel`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CompositeLayer {
    pub visible: bool,
    pub blend_mode: BlendMode,
    /// `0.0..=1.0`.
    pub opacity: f32,
    /// Bruchteil der Leinwandgröße, um den die Ebene skaliert wird —
    /// `1.0` deckt die Leinwand (an ihrem eigenen Seitenverhältnis
    /// gestreckt) exakt ab, dieselbe Größenreferenz wie
    /// `v2::CanvasExtension`s normierte Bruchteile.
    pub scale: f32,
    /// Normierte Position (`0.0..=1.0`) des Ebenen-**Mittelpunkts** auf
    /// der Leinwand — `0.5`/`0.5` zentriert die Ebene.
    pub offset_x: f32,
    pub offset_y: f32,
    pub source: CompositeLayerSource,
}

// ---- KI-Tiefenschärfe-Simulator "Virtuelle Blende" (Phase 14 Schritt 8) ----

/// Einmalig vorab per `apx_ai::depth::DepthSession::estimate_rgb8`
/// berechnete, `0..=255`-normierte Tiefenkarte (`255` = am nächsten) —
/// dasselbe „einmal berechnen, bei jedem Rendern nur noch skalieren"-
/// Muster wie `v2::AiFillPatch`/`CompositeLayerSource`. `depth` ist EIN
/// Byte je Pixel (kein RGB), `bitmap_width * bitmap_height` Bytes lang.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DepthMapPatch {
    pub bitmap_width: u32,
    pub bitmap_height: u32,
    pub depth: Vec<u8>,
}

/// KI-Tiefenschärfe-Simulator "Virtuelle Blende" (Phase 14 Schritt 8,
/// siehe `DECISIONS.md` ADR-0041 Nachtrag VIII, Recherche-Tabelle
/// Punkt 1): Lightroom hat keine KI-Tiefenschätzung/synthetisches Bokeh
/// — nur die vorhandene grobe Unschärfe-Heuristik in ApertureX selbst
/// (Laplace-Varianz, `stages::masks`s `MaskGeometry::BlurDepthApprox`,
/// Phase 11 Schritt 7). `focus_x`/`focus_y` sind normierte
/// Bildkoordinaten (`0.0..=1.0`) des angeklickten Fokuspunkts, `amount`
/// (`0.0..=100.0`) die "Blendenöffnung" — je größer, desto stärker
/// blendet der Unschärferadius mit wachsendem Tiefenabstand vom
/// Fokuspunkt auf. Ohne `depth_map` (Tiefenkarte noch nicht berechnet)
/// bleibt die Stufe ein No-Op — dieselbe „noch nicht berechnet"-
/// Konvention wie `v2::RepairStroke::ai_fill`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VirtualApertureAdjustment {
    pub focus_x: f32,
    pub focus_y: f32,
    pub amount: f32,
    #[serde(default)]
    pub depth_map: Option<DepthMapPatch>,
}

impl VirtualApertureAdjustment {
    pub const NEUTRAL: Self = Self {
        focus_x: 0.5,
        focus_y: 0.5,
        amount: 0.0,
        depth_map: None,
    };
}

impl Default for VirtualApertureAdjustment {
    fn default() -> Self {
        Self::NEUTRAL
    }
}

// ---- KI-Stiltransfer zwischen Fotos (Phase 14 Schritt 9) -------------------

/// Einmalig vorab per `apx_ai::style_transfer::StyleTransferSession::
/// stylize_rgb8` berechnetes stilisiertes Bild — dasselbe „einmal
/// berechnen, bei jedem Rendern nur noch skalieren"-Muster wie
/// `CompositeLayerSource`/`DepthMapPatch`. `pixels` ist interleaved RGB
/// (`0..=255`), `bitmap_width * bitmap_height * 3` Zahlen lang.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StyleTransferPatch {
    pub bitmap_width: u32,
    pub bitmap_height: u32,
    pub pixels: Vec<u8>,
}

/// KI-Stiltransfer zwischen Fotos (Phase 14 Schritt 9, siehe
/// `DECISIONS.md` ADR-0041 Nachtrag IX, Recherche-Tabelle Punkt 7):
/// Lightroom hat dafür kein Äquivalent. Anders als ursprünglich erhofft
/// (ein *beliebiges* Referenzfoto als Stilvorlage) bewusst auf fünf
/// fest lizenzierte `fast_neural_style`-Netze beschränkt (siehe
/// `apx_ai::style_transfer`s Moduldoku) — welcher der fünf Stile gewählt
/// ist, steckt bereits im vorab berechneten `patch` und muss dieser
/// Crate (die keinen Modell-/Katalogzugriff hat) nicht bekannt sein.
/// `amount` (`0.0..=1.0`) blendet linear zwischen dem unveränderten
/// Bild (`0.0`) und dem vollen Stiltransfer-Ergebnis (`1.0`) — dieselbe
/// Deckkraft-Konvention wie `CompositeLayer::opacity`, hier aber
/// global statt pro Ebene. Ohne `patch` (noch nicht berechnet) bleibt
/// die Stufe ein No-Op — dieselbe „noch nicht berechnet"-Konvention wie
/// `VirtualApertureAdjustment::depth_map`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StyleTransferAdjustment {
    pub amount: f32,
    #[serde(default)]
    pub patch: Option<StyleTransferPatch>,
}

impl StyleTransferAdjustment {
    pub const NEUTRAL: Self = Self {
        amount: 0.0,
        patch: None,
    };
}

impl Default for StyleTransferAdjustment {
    fn default() -> Self {
        Self::NEUTRAL
    }
}

// ---- Der vollständige EDL v4 -----------------------------------------------

/// Die konkrete EDL-Struktur für Schema-Version 4 — siehe
/// [`crate::edl::EDL_SCHEMA_VERSION`] und [`crate::edl::migrate`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EdlV4 {
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
    pub masks: Vec<Mask>,
    pub mask_groups: Vec<MaskGroup>,
    pub treatment: Treatment,
    pub bw_mixer: BlackAndWhiteMixerAdjustment,
    /// Node-Editor (Phase 9 Schritt 7) — additiv, siehe Moduldoku oben.
    #[serde(default)]
    pub stage_enabled: StageEnabled,
    /// Mehrfachbelichtung/Layer-Compositing (Phase 14 Schritt 3) —
    /// additiv, `#[serde(default)]` liest ein gespeichertes `EdlV4` ohne
    /// dieses Feld als leere Ebenenliste (unverändertes bisheriges
    /// Verhalten).
    #[serde(default)]
    pub composite_layers: Vec<CompositeLayer>,
    /// KI-Tiefenschärfe-Simulator "Virtuelle Blende" (Phase 14
    /// Schritt 8) — additiv, `#[serde(default)]` liest ein gespeichertes
    /// `EdlV4` ohne dieses Feld als
    /// `VirtualApertureAdjustment::NEUTRAL` (unverändertes bisheriges
    /// Verhalten, keine Tiefenkarte berechnet).
    #[serde(default)]
    pub virtual_aperture: VirtualApertureAdjustment,
    /// KI-Stiltransfer zwischen Fotos (Phase 14 Schritt 9) — additiv,
    /// `#[serde(default)]` liest ein gespeichertes `EdlV4` ohne dieses
    /// Feld als `StyleTransferAdjustment::NEUTRAL` (unverändertes
    /// bisheriges Verhalten, kein Stiltransfer berechnet).
    #[serde(default)]
    pub style_transfer: StyleTransferAdjustment,
}

impl EdlV4 {
    /// Die neutrale Bearbeitung: alle Regler unverändert, keine
    /// Reparatur-Striche, keine Masken, jede Stufe aktiv.
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
            treatment: Treatment::Color,
            bw_mixer: BlackAndWhiteMixerAdjustment::NEUTRAL,
            stage_enabled: StageEnabled::ALL,
            composite_layers: Vec::new(),
            virtual_aperture: VirtualApertureAdjustment::NEUTRAL,
            style_transfer: StyleTransferAdjustment::NEUTRAL,
        }
    }

    /// Zieht ein `EdlV3` hoch — alles darin bleibt unverändert übernommen,
    /// jede Stufe startet aktiv (dasselbe sichtbare Ergebnis wie vorher,
    /// nur jetzt mit expliziten Ein/Aus-Feldern). Einziger Aufrufer ist
    /// [`super::migrate::from_envelope`], wenn ein gespeicherter Umschlag
    /// noch `schema_version == 3` trägt.
    pub fn from_v3(old: v3::EdlV3) -> Self {
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
            masks: old.masks,
            mask_groups: old.mask_groups,
            treatment: old.treatment,
            bw_mixer: old.bw_mixer,
            stage_enabled: StageEnabled::ALL,
            composite_layers: Vec::new(),
            virtual_aperture: VirtualApertureAdjustment::NEUTRAL,
            style_transfer: StyleTransferAdjustment::NEUTRAL,
        }
    }
}

impl Default for EdlV4 {
    fn default() -> Self {
        Self::neutral()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn neutral_roundtrips_through_json() {
        let edl = EdlV4::neutral();
        let json = serde_json::to_string(&edl).expect("sollte serialisieren");
        let parsed: EdlV4 = serde_json::from_str(&json).expect("sollte parsen");
        assert_eq!(edl, parsed);
    }

    #[test]
    fn default_equals_neutral() {
        assert_eq!(EdlV4::default(), EdlV4::neutral());
    }

    #[test]
    fn from_v3_preserves_all_fields_and_enables_every_stage() {
        let old = v3::EdlV3 {
            basic: v2::BasicAdjustments {
                exposure_ev: 0.5,
                ..v2::BasicAdjustments::NEUTRAL
            },
            ..v3::EdlV3::neutral()
        };
        let upgraded = EdlV4::from_v3(old);
        assert_eq!(upgraded.basic.exposure_ev, 0.5);
        assert_eq!(upgraded.stage_enabled, StageEnabled::ALL);
    }

    #[test]
    fn old_payload_without_stage_enabled_field_defaults_to_all_enabled() {
        // Simuliert einen v4-Umschlag, der (z. B. handgeschrieben) das
        // Feld nicht trägt — `#[serde(default)]` muss ihn trotzdem lesbar
        // halten, dieselbe additive Disziplin wie beim SW-Mixer in
        // Schritt 5.
        let json = serde_json::to_value(EdlV4::neutral()).expect("serialisieren");
        let mut object = json.as_object().expect("Objekt").clone();
        object.remove("stage_enabled");
        let parsed: EdlV4 =
            serde_json::from_value(serde_json::Value::Object(object)).expect("sollte parsen");
        assert_eq!(parsed.stage_enabled, StageEnabled::ALL);
    }

    #[test]
    fn disabling_a_stage_serializes_and_survives_roundtrip() {
        let edl = EdlV4 {
            stage_enabled: StageEnabled {
                details: false,
                effects: false,
                ..StageEnabled::ALL
            },
            ..EdlV4::neutral()
        };
        let json = serde_json::to_string(&edl).expect("serialisieren");
        let parsed: EdlV4 = serde_json::from_str(&json).expect("parsen");
        assert!(!parsed.stage_enabled.details);
        assert!(!parsed.stage_enabled.effects);
        assert!(
            parsed.stage_enabled.basic,
            "andere Stufen bleiben unberührt"
        );
    }
}
