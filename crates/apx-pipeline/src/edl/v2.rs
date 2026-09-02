//! Version 2 des EDL-Schemas: erweitert Version 1 um alle zehn
//! Phase-4-Werkzeugkategorien (siehe `SPEC.md` §5, `DECISIONS.md`
//! ADR-0028) plus die per ADR-0011 nach Phase 4 verschobenen
//! Grundeinstellungs-Ergänzungen (Textur, Klarheit, Dunst entfernen,
//! Dynamik, Sättigung).
//!
//! `v1::EdlV1` bleibt unverändert bestehen (historische Einträge in
//! `apx-catalog`s `edit_history.edl_json` sind Schema-Version 1) — alte
//! wie neue Daten werden über [`super::migrate::from_envelope`] immer zu
//! `EdlV2` hochgezogen, nie umgekehrt. Neue Felder folgen exakt v1s
//! Prinzip „typisierte Felder statt generischer Key-Value-Map, `serde`
//! lehnt fehlende/falsche Werte strukturell ab statt sie stillschweigend
//! zu ignorieren" — deshalb kein `#[serde(default)]` irgendwo in dieser
//! Datei, mit Ausnahme des expliziten v1→v2-Aufwärtspfads selbst (das ist
//! eine bewusste, dokumentierte Migration, keine stillschweigende
//! Reparatur eines kaputten Payloads).
//!
//! In Phase-4-Schritt 1 (dieser Commit) sind alle neuen Felder noch inert
//! — `develop::render_rgba8` projiziert `basic` weiterhin auf v1s sieben
//! Felder (`BasicAdjustments::to_v1_subset`) für den bestehenden
//! Fused-Pass. Die Shader-/CPU-Umsetzung der neuen Felder kommt in den
//! folgenden Schritten (siehe `PLAN.md` Phase 4).

use serde::{Deserialize, Serialize};

use super::v1;

// ---- Grundeinstellungen (7 aus v1 + 5 per ADR-0011 verschobene) -----------

/// Die zwölf Grundeinstellungs-Regler. Wertebereich weiterhin
/// `-100.0..=100.0` nach Lightroom-Konvention (`0.0` = keine Veränderung),
/// `exposure_ev` in Blendenstufen.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct BasicAdjustments {
    pub white_balance: v1::WhiteBalanceAdjustment,
    pub exposure_ev: f32,
    pub contrast: f32,
    pub highlights: f32,
    pub shadows: f32,
    pub whites: f32,
    pub blacks: f32,
    /// Lokaler Kontrast bei mittleren Frequenzen ("Struktur").
    pub texture: f32,
    /// Lokaler Kontrast bei niedrigen Frequenzen, schont Hauttöne stärker
    /// als `texture`.
    pub clarity: f32,
    /// Vereinfachter Dunst-entfernen-Regler — kein echtes
    /// Dark-Channel-Prior-Verfahren, siehe `PLAN.md` Phase 4 Schritt 3.
    pub dehaze: f32,
    /// Sättigung schwächer bereits gesättigter Farben stärker anhebend
    /// ("Dynamik" in der deutschen Lightroom-Übersetzung).
    pub vibrance: f32,
    pub saturation: f32,
}

impl BasicAdjustments {
    pub const NEUTRAL: Self = Self {
        white_balance: v1::WhiteBalanceAdjustment::NEUTRAL,
        exposure_ev: 0.0,
        contrast: 0.0,
        highlights: 0.0,
        shadows: 0.0,
        whites: 0.0,
        blacks: 0.0,
        texture: 0.0,
        clarity: 0.0,
        dehaze: 0.0,
        vibrance: 0.0,
        saturation: 0.0,
    };

    /// Projiziert auf die sieben v1-Felder — Übergangshilfe für den
    /// bestehenden Fused-Pass, bis Phase 4 Schritt 2 die restlichen fünf
    /// Felder tatsächlich mit aufnimmt. Verwirft dabei absichtlich keine
    /// Information dauerhaft: der EDL selbst behält alle zwölf Felder,
    /// nur dieser eine Rendering-Aufruf sieht (noch) nur sieben.
    pub fn to_v1_subset(self) -> v1::BasicAdjustments {
        v1::BasicAdjustments {
            white_balance: self.white_balance,
            exposure_ev: self.exposure_ev,
            contrast: self.contrast,
            highlights: self.highlights,
            shadows: self.shadows,
            whites: self.whites,
            blacks: self.blacks,
        }
    }
}

impl Default for BasicAdjustments {
    fn default() -> Self {
        Self::NEUTRAL
    }
}

impl From<v1::BasicAdjustments> for BasicAdjustments {
    fn from(old: v1::BasicAdjustments) -> Self {
        Self {
            white_balance: old.white_balance,
            exposure_ev: old.exposure_ev,
            contrast: old.contrast,
            highlights: old.highlights,
            shadows: old.shadows,
            whites: old.whites,
            blacks: old.blacks,
            ..Self::NEUTRAL
        }
    }
}

// ---- Kurven ----------------------------------------------------------------

/// Ein Kontrollpunkt, normiert auf `0.0..=1.0` in beiden Achsen
/// (Eingabe-/Ausgabe-Tonwert).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct CurvePoint {
    pub input: f32,
    pub output: f32,
}

/// Punktkurve (frei gesetzte Kontrollpunkte) oder parametrische Kurve
/// (vier Lightroom-typische Parameter) für einen Kanal.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum CurveChannel {
    /// Monotone kubische Spline-Interpolation zwischen den Punkten (siehe
    /// `PLAN.md` Phase 4 Schritt 4).
    Points { points: Vec<CurvePoint> },
    Parametric {
        shadows: f32,
        darks: f32,
        lights: f32,
        highlights: f32,
    },
}

impl CurveChannel {
    /// Identitätskurve: Ausgabe = Eingabe, keine Veränderung.
    pub fn identity() -> Self {
        Self::Points {
            points: vec![
                CurvePoint {
                    input: 0.0,
                    output: 0.0,
                },
                CurvePoint {
                    input: 1.0,
                    output: 1.0,
                },
            ],
        }
    }
}

/// RGB-Verbundkurve, R/G/B einzeln, plus Luminanz-Kurve.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CurvesAdjustment {
    pub rgb: CurveChannel,
    pub red: CurveChannel,
    pub green: CurveChannel,
    pub blue: CurveChannel,
    pub luminance: CurveChannel,
}

impl CurvesAdjustment {
    pub fn neutral() -> Self {
        Self {
            rgb: CurveChannel::identity(),
            red: CurveChannel::identity(),
            green: CurveChannel::identity(),
            blue: CurveChannel::identity(),
            luminance: CurveChannel::identity(),
        }
    }
}

// ---- HSL --------------------------------------------------------------------

/// Farbton-/Sättigungs-/Luminanz-Verschiebung für eines der acht
/// Lightroom-Standardfarbbänder.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct HslBand {
    pub hue: f32,
    pub saturation: f32,
    pub luminance: f32,
}

impl HslBand {
    pub const NEUTRAL: Self = Self {
        hue: 0.0,
        saturation: 0.0,
        luminance: 0.0,
    };
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct HslAdjustment {
    pub red: HslBand,
    pub orange: HslBand,
    pub yellow: HslBand,
    pub green: HslBand,
    pub aqua: HslBand,
    pub blue: HslBand,
    pub purple: HslBand,
    pub magenta: HslBand,
}

impl HslAdjustment {
    pub const NEUTRAL: Self = Self {
        red: HslBand::NEUTRAL,
        orange: HslBand::NEUTRAL,
        yellow: HslBand::NEUTRAL,
        green: HslBand::NEUTRAL,
        aqua: HslBand::NEUTRAL,
        blue: HslBand::NEUTRAL,
        purple: HslBand::NEUTRAL,
        magenta: HslBand::NEUTRAL,
    };
}

impl Default for HslAdjustment {
    fn default() -> Self {
        Self::NEUTRAL
    }
}

// ---- Farbmischer erweitert --------------------------------------------------

/// Ein benutzerdefinierter Farbbereich (siehe `SPEC.md` §3.2 „Farbmischer
/// erweitert" — per Bildklick aufgenommen, nicht eines der acht festen
/// HSL-Bänder).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ColorMixerRegion {
    /// Zielfarbton in Grad (0..360), vom Nutzer per Bildklick aufgenommen.
    pub target_hue_degrees: f32,
    /// Bandbreite um den Zielfarbton in Grad.
    pub bandwidth_degrees: f32,
    /// Weichzeichnung des Übergangs an den Bandgrenzen, `0.0..=1.0`.
    pub feather: f32,
    pub hue_shift: f32,
    pub saturation_shift: f32,
    pub luminance_shift: f32,
}

/// Offene Liste benutzerdefinierter Farbbereiche (anders als die acht
/// festen HSL-Bänder oben).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ColorMixerAdjustment {
    pub regions: Vec<ColorMixerRegion>,
}

impl ColorMixerAdjustment {
    pub fn neutral() -> Self {
        Self {
            regions: Vec::new(),
        }
    }
}

// ---- Color Grading (Farbräder) ---------------------------------------------

/// Position eines Farbrads: Farbton + Sättigungsstärke + zusätzliche
/// Luminanz-Verschiebung.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ColorGradingWheel {
    pub hue_degrees: f32,
    /// Abstand vom Mittelpunkt (Sättigungsstärke), `0.0..=1.0`.
    pub saturation: f32,
    pub luminance: f32,
}

impl ColorGradingWheel {
    pub const NEUTRAL: Self = Self {
        hue_degrees: 0.0,
        saturation: 0.0,
        luminance: 0.0,
    };
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ColorGradingAdjustment {
    pub shadows: ColorGradingWheel,
    pub midtones: ColorGradingWheel,
    pub highlights: ColorGradingWheel,
    pub global: ColorGradingWheel,
    /// Balance zwischen Schatten-/Lichter-Gewichtung, `-100.0..=100.0`.
    pub balance: f32,
    /// Überblendung zwischen den drei Tonwertzonen, `0.0..=100.0`.
    pub blending: f32,
}

impl ColorGradingAdjustment {
    pub const NEUTRAL: Self = Self {
        shadows: ColorGradingWheel::NEUTRAL,
        midtones: ColorGradingWheel::NEUTRAL,
        highlights: ColorGradingWheel::NEUTRAL,
        global: ColorGradingWheel::NEUTRAL,
        balance: 0.0,
        blending: 50.0,
    };
}

impl Default for ColorGradingAdjustment {
    fn default() -> Self {
        Self::NEUTRAL
    }
}

// ---- Details (Schärfung + Rauschreduzierung) -------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct DetailsAdjustment {
    pub sharpen_amount: f32,
    pub sharpen_radius: f32,
    pub sharpen_detail: f32,
    pub sharpen_masking: f32,
    /// Deconvolution-Schärfung als Alternativmodus statt Unsharp-Masking.
    pub use_deconvolution_sharpen: bool,
    pub luminance_nr_amount: f32,
    pub luminance_nr_detail: f32,
    pub luminance_nr_contrast: f32,
    pub color_nr_amount: f32,
    pub color_nr_detail: f32,
    pub color_nr_smoothness: f32,
}

impl DetailsAdjustment {
    pub const NEUTRAL: Self = Self {
        sharpen_amount: 0.0,
        sharpen_radius: 1.0,
        sharpen_detail: 25.0,
        sharpen_masking: 0.0,
        use_deconvolution_sharpen: false,
        luminance_nr_amount: 0.0,
        luminance_nr_detail: 50.0,
        luminance_nr_contrast: 0.0,
        color_nr_amount: 0.0,
        color_nr_detail: 50.0,
        color_nr_smoothness: 50.0,
    };
}

impl Default for DetailsAdjustment {
    fn default() -> Self {
        Self::NEUTRAL
    }
}

// ---- Objektivkorrekturen ----------------------------------------------------

/// Wie die Perspektive/Upright-Korrektur bestimmt wird — die fünf
/// SPEC.md-Modi plus `Off` als neutraler Standard.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UprightMode {
    /// Keine Korrektur (Standard).
    Off,
    Auto,
    Level,
    Vertical,
    Full,
    /// Manuell markierte Linien (siehe `guided_lines`) — auf 2 statt bis
    /// zu 4 Linienpaare vereinfacht, siehe `DECISIONS.md` ADR-0028.
    Guided,
}

/// Eine vom Nutzer im Guided-Modus markierte Hilfslinie, in normierten
/// Bildkoordinaten (`0.0..=1.0`).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct GuidedLine {
    pub x1: f32,
    pub y1: f32,
    pub x2: f32,
    pub y2: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ManualTransform {
    pub vertical: f32,
    pub horizontal: f32,
    pub rotate_degrees: f32,
    pub aspect: f32,
    /// Prozent, `100.0` = keine Skalierung.
    pub scale: f32,
    pub offset_x: f32,
    pub offset_y: f32,
}

impl ManualTransform {
    pub const NEUTRAL: Self = Self {
        vertical: 0.0,
        horizontal: 0.0,
        rotate_degrees: 0.0,
        aspect: 0.0,
        scale: 100.0,
        offset_x: 0.0,
        offset_y: 0.0,
    };
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LensCorrectionAdjustment {
    /// Referenz auf ein Profil — entweder eines der drei Alt-Beispiel-
    /// profile (ADR-0028) oder ein echter LensFun-Datenbankeintrag
    /// (`lensfun:{maker}|{model}`, seit Phase 12 Schritt 3, siehe
    /// `DECISIONS.md` ADR-0039), `None` = kein Profil zugeordnet.
    pub profile_id: Option<String>,
    pub ca_red_cyan: f32,
    pub ca_blue_yellow: f32,
    pub auto_ca: bool,
    pub vignette_amount: f32,
    pub distortion_amount: f32,
    pub upright_mode: UprightMode,
    pub guided_lines: Vec<GuidedLine>,
    pub manual_transform: ManualTransform,
    /// Ergebnis einer eigenen Kalibrierung aus vom Nutzer markierten,
    /// in der Realität geraden Linien (Phase 12 Schritt 3 Teil B, siehe
    /// `DECISIONS.md` ADR-0039 und `apx_ai::lens_calibration`) — hat
    /// Vorrang vor `profile_id`s Verzeichnungswert, wenn gesetzt (die
    /// übrigen drei Profilwerte, Vignette/CA, bleiben unberührt, siehe
    /// `apx_ai::lens_calibration`s Moduldoku zur bewussten Beschränkung
    /// auf Verzeichnung). Additiv per `#[serde(default)]` statt eines
    /// Schema-Version-Sprungs — dieselbe Konvention wie `BrushStroke::
    /// auto_mask` in Phase 12 Schritt 2.
    #[serde(default)]
    pub custom_distortion_k1: Option<f32>,
}

impl LensCorrectionAdjustment {
    pub const NEUTRAL: Self = Self {
        profile_id: None,
        ca_red_cyan: 0.0,
        ca_blue_yellow: 0.0,
        auto_ca: false,
        vignette_amount: 0.0,
        distortion_amount: 0.0,
        upright_mode: UprightMode::Off,
        guided_lines: Vec::new(),
        manual_transform: ManualTransform::NEUTRAL,
        custom_distortion_k1: None,
    };

    pub fn neutral() -> Self {
        Self::NEUTRAL
    }
}

// ---- Effekte ----------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct EffectsAdjustment {
    pub post_vignette_amount: f32,
    pub post_vignette_midpoint: f32,
    pub post_vignette_roundness: f32,
    pub post_vignette_feather: f32,
    pub post_vignette_highlights: f32,
    pub grain_amount: f32,
    pub grain_size: f32,
    pub grain_roughness: f32,
}

impl EffectsAdjustment {
    pub const NEUTRAL: Self = Self {
        post_vignette_amount: 0.0,
        post_vignette_midpoint: 50.0,
        post_vignette_roundness: 0.0,
        post_vignette_feather: 50.0,
        post_vignette_highlights: 0.0,
        grain_amount: 0.0,
        grain_size: 25.0,
        grain_roughness: 50.0,
    };
}

impl Default for EffectsAdjustment {
    fn default() -> Self {
        Self::NEUTRAL
    }
}

// ---- Kalibrierung -----------------------------------------------------------

/// Es gibt (noch) keine ältere Prozessversion — die Variante existiert
/// nur, damit eine künftige Versionsänderung bestehende EDLs nicht
/// stillschweigend anders rendert (siehe `SPEC.md` §2.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProcessVersion {
    V1,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PrimaryColorAdjustment {
    pub hue: f32,
    pub saturation: f32,
}

impl PrimaryColorAdjustment {
    pub const NEUTRAL: Self = Self {
        hue: 0.0,
        saturation: 0.0,
    };
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CalibrationAdjustment {
    pub process_version: ProcessVersion,
    pub shadow_tint: f32,
    pub red_primary: PrimaryColorAdjustment,
    pub green_primary: PrimaryColorAdjustment,
    pub blue_primary: PrimaryColorAdjustment,
    /// Name eines Profils in der kleinen eingebauten Kameraprofil-Liste
    /// (kein DCP-Import, siehe `DECISIONS.md` ADR-0028), `None` =
    /// Standardprofil.
    pub camera_profile: Option<String>,
}

impl CalibrationAdjustment {
    pub const NEUTRAL: Self = Self {
        process_version: ProcessVersion::V1,
        shadow_tint: 0.0,
        red_primary: PrimaryColorAdjustment::NEUTRAL,
        green_primary: PrimaryColorAdjustment::NEUTRAL,
        blue_primary: PrimaryColorAdjustment::NEUTRAL,
        camera_profile: None,
    };

    pub fn neutral() -> Self {
        Self::NEUTRAL
    }
}

// ---- Geometrie (Crop/Rotation) ----------------------------------------------

/// Rasterüberlagerung beim Freistellen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GridOverlay {
    None,
    Thirds,
    GoldenRatio,
    Diagonals,
    Spiral,
    Triangles,
}

/// Freistellungsrechteck in normierten Bildkoordinaten (`0.0..=1.0`).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct CropRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl CropRect {
    /// Das ganze Bild, kein Beschnitt.
    pub const FULL: Self = Self {
        x: 0.0,
        y: 0.0,
        width: 1.0,
        height: 1.0,
    };
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct GeometryAdjustment {
    pub crop: CropRect,
    /// `None` = freie Seitenverhältniswahl, sonst Breite/Höhe-Verhältnis.
    pub aspect_ratio: Option<f32>,
    pub angle_degrees: f32,
    pub overlay: GridOverlay,
    /// Vereinfachte Auto-Ausrichtung: nur EXIF-Orientierung, kein echtes
    /// Kantenerkennungs-Verfahren (siehe `DECISIONS.md` ADR-0028).
    pub auto_horizon: bool,
}

impl GeometryAdjustment {
    pub const NEUTRAL: Self = Self {
        crop: CropRect::FULL,
        aspect_ratio: None,
        angle_degrees: 0.0,
        overlay: GridOverlay::None,
        auto_horizon: false,
    };
}

impl Default for GeometryAdjustment {
    fn default() -> Self {
        Self::NEUTRAL
    }
}

// ---- Reparatur (Klonen/Reparieren) ------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RepairMode {
    Clone,
    Heal,
    /// Inhaltsbasiertes Füllen (Phase 7, `DECISIONS.md` ADR-0033 Punkt 4)
    /// — sucht den Füllinhalt selbst aus der Bildumgebung, statt ihn von
    /// einem manuell gesetzten Quellpunkt zu kopieren; `source` wird
    /// dabei ignoriert. Läuft als vereinfachtes PatchMatch **CPU-only**
    /// (siehe `stages::repair`s Moduldoku).
    ContentAwareFill,
    /// Echtes KI-Ausfüllen per LaMa-Inferenz (Phase 13 Schritt 1, siehe
    /// `DECISIONS.md` ADR-0040) — wie `ContentAwareFill` ignoriert dieser
    /// Modus `source` (kein manueller Quellpunkt), liefert das Ergebnis
    /// aber nicht live aus einer CPU-Heuristik, sondern aus einem einmalig
    /// vorab berechneten [`RepairStroke::ai_fill`]-Patch (neuronale
    /// Inferenz ist zu teuer für jeden Regler-Tick, siehe `stages::repair`s
    /// Moduldoku). Ohne gesetztes `ai_fill` (z. B. direkt nach dem Malen,
    /// bevor der Nutzer „Anwenden" bestätigt hat) bleibt der Strich ein
    /// No-Op — derselbe „noch nicht berechnet"-Zustand wie eine frische
    /// `MaskGeometry::AiGenerated` vor ihrem ersten Lauf.
    AiInpaint,
}

/// Ergebnis eines einmaligen KI-Ausfüllen-Laufs (Phase 13 Schritt 1, siehe
/// `DECISIONS.md` ADR-0040) — wie `edl::v3::MaskGeometry::AiGenerated` eine
/// per `apx-ai` einmalig vorab berechnete Rasterfläche, die die Pipeline
/// bei jedem Render-Tick nur noch günstig wieder einsetzt, statt die
/// Inferenz erneut zu fahren.
///
/// Die Patch-**Position** (`x`/`y`/`width`/`height`) ist normiert
/// (`0.0..=1.0`, Bruchteil der Bildbreite/-höhe) — dieselbe
/// auflösungsunabhängige Konvention wie `RepairPoint`/`RepairStroke`s
/// `radius`. Die gespeicherte `pixels`-Bitmap selbst hat dagegen ihre
/// **eigene**, von der Analyse-Auflösung
/// (`apx_ai::segmentation::ANALYSIS_MAX_EDGE`, derselbe Cap wie jede
/// andere KI-Bildanalyse dieses Projekts) vorgegebene feste Größe
/// (`bitmap_width`/`bitmap_height`) —
/// kann von der tatsächlichen Render-Auflösung abweichen (Vorschau vs.
/// Vollbildexport). `stages::repair` skaliert sie beim Einsetzen bilinear
/// auf die Pixelgröße des normierten Zielrechtecks hoch — derselbe
/// Kompromiss wie bei `MaskGeometry::AiGenerated`, aus demselben Grund
/// (kein Re-Run der teuren Inferenz bei jedem Regler-Tick oder jeder
/// Export-Auflösung).
///
/// `pixels` ist interleaved RGB (`0..=255`),
/// `bitmap_width * bitmap_height * 3` Bytes lang — dieselbe
/// unkomprimierte Roh-Array-in-JSON-Konvention wie
/// `MaskGeometry::AiGenerated::alpha` (aufgebläht, aber etabliertes
/// Projektmuster, kein neues Kompaktformat erfinden).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AiFillPatch {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub bitmap_width: u32,
    pub bitmap_height: u32,
    pub pixels: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct RepairPoint {
    pub x: f32,
    pub y: f32,
}

/// Ein einzelner Klon-/Reparatur-Pinselzug — jeder Strich ist einzeln
/// entfernbar/undo-fähig (siehe `PLAN.md` Phase 4 Schritt 12). Bewusst
/// **nicht** Teil dieses Schritts: Auto-Quellenfindung, inhaltsbasiertes
/// Füllen (siehe `DECISIONS.md` ADR-0028).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RepairStroke {
    pub mode: RepairMode,
    pub source: RepairPoint,
    /// Gemalter Zielpfad — mindestens ein Punkt.
    pub target_path: Vec<RepairPoint>,
    pub radius: f32,
    pub feather: f32,
    pub opacity: f32,
    /// Nur für `mode == RepairMode::AiInpaint` relevant — das Ergebnis des
    /// einmaligen KI-Ausfüllen-Laufs (Phase 13 Schritt 1). `#[serde(default)]`
    /// statt Schema-Version-Sprung, additiv wie `BrushStroke::auto_mask`
    /// (siehe dessen Kommentar in `edl/v3.rs`) — ein gespeicherter Strich
    /// ohne dieses Feld liest weiterhin als `None` (für jeden anderen
    /// `mode` ohnehin der einzig sinnvolle Wert).
    #[serde(default)]
    pub ai_fill: Option<AiFillPatch>,
}

// ---- Der vollständige EDL v2 -----------------------------------------------

/// Die konkrete EDL-Struktur für Schema-Version 2 — siehe
/// [`crate::edl::EDL_SCHEMA_VERSION`] und [`crate::edl::migrate`] für die
/// Umwandlung von/zu `apx_core::EdlEnvelope` (inklusive des
/// Aufwärtspfads von `EdlV1`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EdlV2 {
    pub basic: BasicAdjustments,
    pub curves: CurvesAdjustment,
    pub hsl: HslAdjustment,
    pub color_mixer: ColorMixerAdjustment,
    pub color_grading: ColorGradingAdjustment,
    pub details: DetailsAdjustment,
    pub lens_corrections: LensCorrectionAdjustment,
    pub effects: EffectsAdjustment,
    pub calibration: CalibrationAdjustment,
    pub geometry: GeometryAdjustment,
    pub repair: Vec<RepairStroke>,
}

impl EdlV2 {
    /// Die neutrale Bearbeitung: alle Regler unverändert (Ausgabe =
    /// Eingabe), keine Reparatur-Striche.
    pub fn neutral() -> Self {
        Self {
            basic: BasicAdjustments::NEUTRAL,
            curves: CurvesAdjustment::neutral(),
            hsl: HslAdjustment::NEUTRAL,
            color_mixer: ColorMixerAdjustment::neutral(),
            color_grading: ColorGradingAdjustment::NEUTRAL,
            details: DetailsAdjustment::NEUTRAL,
            lens_corrections: LensCorrectionAdjustment::neutral(),
            effects: EffectsAdjustment::NEUTRAL,
            calibration: CalibrationAdjustment::neutral(),
            geometry: GeometryAdjustment::NEUTRAL,
            repair: Vec::new(),
        }
    }

    /// Zieht ein `EdlV1` (sieben Grundregler) auf `EdlV2` hoch — alles
    /// über `basic` hinaus bleibt neutral. Einziger Aufrufer ist
    /// [`super::migrate::from_envelope`], wenn ein alter, gespeicherter
    /// Umschlag noch `schema_version == 1` trägt.
    pub fn from_v1(old: v1::EdlV1) -> Self {
        Self {
            basic: old.basic.into(),
            ..Self::neutral()
        }
    }
}

impl Default for EdlV2 {
    fn default() -> Self {
        Self::neutral()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn neutral_roundtrips_through_json() {
        let edl = EdlV2::neutral();
        let json = serde_json::to_string(&edl).expect("sollte serialisieren");
        let parsed: EdlV2 = serde_json::from_str(&json).expect("sollte parsen");
        assert_eq!(edl, parsed);
    }

    #[test]
    fn default_equals_neutral() {
        assert_eq!(EdlV2::default(), EdlV2::neutral());
    }

    #[test]
    fn from_v1_preserves_the_seven_basic_fields_and_neutralizes_the_rest() {
        let old = v1::EdlV1 {
            basic: v1::BasicAdjustments {
                exposure_ev: 0.5,
                contrast: 10.0,
                ..v1::BasicAdjustments::NEUTRAL
            },
        };
        let upgraded = EdlV2::from_v1(old);
        assert_eq!(upgraded.basic.exposure_ev, 0.5);
        assert_eq!(upgraded.basic.contrast, 10.0);
        assert_eq!(upgraded.basic.texture, 0.0, "neue Felder bleiben neutral");
        assert_eq!(upgraded.curves, CurvesAdjustment::neutral());
        assert_eq!(upgraded.repair, Vec::new());
    }

    #[test]
    fn to_v1_subset_projects_back_the_seven_fields() {
        let basic = BasicAdjustments {
            exposure_ev: -1.0,
            texture: 50.0,
            ..BasicAdjustments::NEUTRAL
        };
        let projected = basic.to_v1_subset();
        assert_eq!(projected.exposure_ev, -1.0);
        assert_eq!(
            projected,
            v1::BasicAdjustments {
                exposure_ev: -1.0,
                ..v1::BasicAdjustments::NEUTRAL
            }
        );
    }

    #[test]
    fn curve_channel_kind_is_present_in_serialized_json() {
        let curve = CurveChannel::identity();
        let value = serde_json::to_value(&curve).expect("sollte serialisieren");
        assert_eq!(value["kind"], "Points");
    }
}
