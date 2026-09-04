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
    /// Automatisches Hautglätten (Phase 15 Schritt 5) — läuft nach
    /// `style_transfer`, vor `sky_replace` (siehe `stages::
    /// skin_smoothing`s Moduldoku). Dieselbe `default_true`-Begründung
    /// wie `style_transfer` oben.
    #[serde(default = "default_true")]
    pub skin_smoothing: bool,
    /// Himmelsaustausch (Phase 14 Schritt 10) — läuft nach `style_transfer`,
    /// vor `geometry`.
    #[serde(default = "default_true")]
    pub sky_replace: bool,
    /// Filter-/LUT-Bibliothek (Phase 16 Schritt 1, siehe `DECISIONS.md`
    /// ADR-0043) — läuft nach `sky_replace`, vor `liquify`: als letzte
    /// Farb-Stufe vor den rein geometrischen/verformenden Stufen, wie ein
    /// abschließender "Look"-Pass in professionellen Grading-Werkzeugen
    /// (siehe `stages::lut_filter`s Moduldoku). Dieselbe
    /// `default_true`-Begründung wie `sky_replace` oben.
    #[serde(default = "default_true")]
    pub lut_filter: bool,
    /// Verflüssigen (Phase 15 Schritt 3) — läuft nach `sky_replace`, vor
    /// `geometry`, im fertig entwickelten sRGB-RGBA8-Bild (siehe
    /// `stages::liquify`s Moduldoku). Dieselbe `default_true`-Begründung
    /// wie `sky_replace` oben.
    #[serde(default = "default_true")]
    pub liquify: bool,
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
        skin_smoothing: true,
        sky_replace: true,
        lut_filter: true,
        liquify: true,
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
    /// Photoshop-Funktion "Blend-If" (Phase 15 Schritt 2, siehe
    /// `DECISIONS.md` ADR-0042) — Lightroom hat keine Tonwertbereich-
    /// Blending-Regler. Additiv, `#[serde(default)]` liest eine
    /// gespeicherte Ebene ohne dieses Feld als `0.0` (unverändertes
    /// bisheriges Verhalten: keine Abblendung nach Tonwert). Luminanz der
    /// **darunterliegenden** Ebene unterhalb dieses Werts wird weich
    /// ausgeblendet statt hart abgeschnitten (siehe `stages::composite`s
    /// Moduldoku für die feste Rampenbreite).
    #[serde(default)]
    pub blend_if_shadow_cutoff: f32,
    /// Gegenstück für Lichter — `#[serde(default = "default_blend_if_highlight_cutoff")]`
    /// liest eine gespeicherte Ebene ohne dieses Feld als `1.0`
    /// (unverändertes bisheriges Verhalten).
    #[serde(default = "default_blend_if_highlight_cutoff")]
    pub blend_if_highlight_cutoff: f32,
}

fn default_blend_if_highlight_cutoff() -> f32 {
    1.0
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

// ---- Himmelsaustausch (Phase 14 Schritt 10) --------------------------------

/// Einmalig per `apx_ai::sky_replace::composite` berechnetes, bereits
/// belichtungsangeglichenes Vollbild (Himmel ersetzt) — `None` = kein
/// Austausch berechnet.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SkyReplacePatch {
    pub bitmap_width: u32,
    pub bitmap_height: u32,
    pub pixels: Vec<u8>,
}

// ---- Automatisches Hautglätten (Phase 15 Schritt 5) ------------------------

/// Einmalig vorab per `apx-app`s `smooth_skin`-Command berechnetes,
/// bereits geglättetes Vollbild (gesichtsbewusste Frequenztrennung, siehe
/// `DECISIONS.md` ADR-0042) — dasselbe „einmal berechnen, bei jedem
/// Rendern nur noch skalieren"-Muster wie `StyleTransferPatch`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SkinSmoothingPatch {
    pub bitmap_width: u32,
    pub bitmap_height: u32,
    pub pixels: Vec<u8>,
}

/// Automatisches Hautglätten (Phase 15 Schritt 5, siehe `DECISIONS.md`
/// ADR-0042 — Lightroom hat kein automatisches, gesichtserkennungs-
/// gestütztes Hautglätten, nur den manuellen Anpassungspinsel). Kombiniert
/// `apx_ai::faces::detect_face_regions`/`segmentation::person_alpha` und
/// `stages::frequency_separation::split/combine` (mit kleinerem
/// `radius_px` als der Reparatur-Standardwert) zu einem einzigen
/// Automatik-Befehl. `amount` (`0.0..=1.0`) blendet linear zwischen dem
/// unveränderten Bild und dem vollen Glättungsergebnis — dieselbe
/// Deckkraft-Konvention wie `StyleTransferAdjustment::amount`. Ohne
/// `patch` (noch nicht berechnet) bleibt die Stufe ein No-Op.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SkinSmoothingAdjustment {
    pub amount: f32,
    #[serde(default)]
    pub patch: Option<SkinSmoothingPatch>,
}

impl SkinSmoothingAdjustment {
    pub const NEUTRAL: Self = Self {
        amount: 0.0,
        patch: None,
    };
}

impl Default for SkinSmoothingAdjustment {
    fn default() -> Self {
        Self::NEUTRAL
    }
}

// ---- Filter-/LUT-Bibliothek (Phase 16 Schritt 1) ---------------------------

/// Ein einmalig geparstes 3D-`.cube`-LUT-Raster (siehe
/// `lut_cube::parse_cube_bytes`) — dasselbe "einmal auflösen, als Zahlen
/// im EDL ablegen"-Muster wie `StyleTransferPatch`/`SkinSmoothingPatch`:
/// die vollständigen Rasterdaten werden direkt hier eingebettet statt nur
/// ein Dateipfad referenziert, damit ein Katalog portabel bleibt (kein
/// stiller Bruch, wenn die ursprüngliche `.cube`-Datei später verschoben
/// oder gelöscht wird). Größenordnung ist unkritisch: ein 33er-Raster
/// sind ~432 KB, deutlich kleiner als ein `StyleTransferPatch`/
/// `SkyReplacePatch`, die bereits ein volles Bild einbetten.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LutFilterData {
    pub name: String,
    pub size: u32,
    /// `size^3 * 3` Floats, r am schnellsten variierend — siehe
    /// `lut_cube::ParsedLut::table`s Moduldoku für die genaue Indizierung.
    pub table: Vec<f32>,
    pub domain_min: [f32; 3],
    pub domain_max: [f32; 3],
}

/// Ein Punkt im gemalten Pfad eines Filter-Pinselstrichs — normierte
/// Bildkoordinaten (0..1), dieselbe Konvention wie `LiquifyPoint`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct LutFilterPoint {
    pub x: f32,
    pub y: f32,
}

/// Ein einzelner Filter-Pinselstrich (Phase 16 Schritt 3, siehe
/// `DECISIONS.md` ADR-0043) — punktuelle statt globaler Filter-Anwendung.
/// Gleiche Form wie `LiquifyStroke` (`center_path`/`radius`/`strength`,
/// normiert wie dort), bewusst **nicht** über die bestehende
/// `Mask`/`MaskAdjustments`-Infrastruktur gelöst: Masken laufen noch im
/// linearen Arbeitsraum (`stages::masks`s Moduldoku), ein `.cube`-LUT ist
/// aber für gamma-kodierte, bildschirmreferenzierte Werte gedacht —
/// dieselbe Pipeline-Position wie der globale `lut_filter`-Durchlauf ist
/// hier wichtiger als Wiederverwendung der Masken-Struktur (siehe
/// `stages::lut_filter`s Moduldoku für die genaue Gewichtsberechnung).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LutFilterStroke {
    pub center_path: Vec<LutFilterPoint>,
    pub radius: f32,
    pub strength: f32,
}

/// Filter-/LUT-Anwendung (Phase 16 Schritt 1, siehe `DECISIONS.md`
/// ADR-0043). `strength` (`0.0..=1.0`) blendet linear zwischen dem
/// unveränderten Bild und dem vollen LUT-Ergebnis — dieselbe Deckkraft-
/// Konvention wie `StyleTransferAdjustment::amount`/
/// `SkinSmoothingAdjustment::amount`. Ohne `lut` (kein Filter gewählt)
/// bleibt die Stufe ein No-Op. `strokes` (Schritt 3): leer heißt "im
/// ganzen Bild bei `strength`", nicht-leer beschränkt die Anwendung auf
/// die gemalten Bereiche (`strength` wirkt dann als globaler
/// Gesamt-Multiplikator über den Pinsel-Ergebnissen, siehe
/// `stages::lut_filter`s Moduldoku).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LutFilterAdjustment {
    pub strength: f32,
    #[serde(default)]
    pub lut: Option<LutFilterData>,
    #[serde(default)]
    pub strokes: Vec<LutFilterStroke>,
}

impl LutFilterAdjustment {
    pub const NEUTRAL: Self = Self {
        strength: 1.0,
        lut: None,
        strokes: Vec::new(),
    };
}

impl Default for LutFilterAdjustment {
    fn default() -> Self {
        Self::NEUTRAL
    }
}

// ---- Verflüssigen (Liquify, Phase 15 Schritt 3) ----------------------------

/// Ein Punkt im gemalten Pfad eines Verflüssigen-Strichs — normierte
/// Bildkoordinaten (0..1), dieselbe Konvention wie `v2::RepairPoint`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct LiquifyPoint {
    pub x: f32,
    pub y: f32,
}

/// Verformungsmodus (Photoshop-Namensgebung) — siehe `stages::liquify`s
/// Moduldoku für die genaue Wirkung jedes Modus.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LiquifyMode {
    Push,
    Twirl,
    Pucker,
    Bloat,
}

/// Ein einzelner Verflüssigen-Pinselzug (Phase 15 Schritt 3, siehe
/// `DECISIONS.md` ADR-0042 — Photoshop-exklusiv, Lightroom hat kein
/// Verformungswerkzeug). `radius`/`strength` sind normiert wie
/// `v2::RepairStroke`s `radius` (Bruchteil der Bildbreite bzw. 0..1).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LiquifyStroke {
    pub center_path: Vec<LiquifyPoint>,
    pub radius: f32,
    pub strength: f32,
    pub mode: LiquifyMode,
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
    /// Automatisches Hautglätten (Phase 15 Schritt 5) — additiv,
    /// `#[serde(default)]` liest ein gespeichertes `EdlV4` ohne dieses
    /// Feld als `SkinSmoothingAdjustment::NEUTRAL` (unverändertes
    /// bisheriges Verhalten, keine Glättung berechnet).
    #[serde(default)]
    pub skin_smoothing: SkinSmoothingAdjustment,
    /// Himmelsaustausch (Phase 14 Schritt 10) — additiv, `#[serde(default)]`.
    #[serde(default)]
    pub sky_replace: Option<SkyReplacePatch>,
    /// Filter-/LUT-Bibliothek (Phase 16 Schritt 1) — additiv,
    /// `#[serde(default)]` liest ein gespeichertes `EdlV4` ohne dieses
    /// Feld als `LutFilterAdjustment::NEUTRAL` (unverändertes bisheriges
    /// Verhalten, kein Filter gewählt).
    #[serde(default)]
    pub lut_filter: LutFilterAdjustment,
    /// Verflüssigen (Phase 15 Schritt 3) — additiv, `#[serde(default)]`
    /// liest ein gespeichertes `EdlV4` ohne dieses Feld als leere
    /// Strichliste (unverändertes bisheriges Verhalten).
    #[serde(default)]
    pub liquify_strokes: Vec<LiquifyStroke>,
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
            skin_smoothing: SkinSmoothingAdjustment::NEUTRAL,
            sky_replace: None,
            lut_filter: LutFilterAdjustment::NEUTRAL,
            liquify_strokes: Vec::new(),
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
            skin_smoothing: SkinSmoothingAdjustment::NEUTRAL,
            sky_replace: None,
            lut_filter: LutFilterAdjustment::NEUTRAL,
            liquify_strokes: Vec::new(),
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
