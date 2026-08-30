/**
 * TypeScript-Gegenstück zu `crates/apx-pipeline/src/edl/v3.rs` und
 * `crates/apx-core/src/edl.rs` — von Hand synchron gehalten. Seit Phase 4
 * (Schritt 1, `DECISIONS.md` ADR-0028) ist das EDL deutlich größer als
 * die ursprünglichen sieben Phase-2-Regler; seit Phase 6 (Schritt 1,
 * ADR-0032) kommt das Maskensystem (`masks`/`mask_groups`) hinzu. Die
 * hier gespiegelten Typen folgen exakt `apx_pipeline::edl::v3`s Struktur-
 * und Feldnamen (`serde`s Standard-Serialisierung, keine Umbenennungen).
 *
 * Die JSON-Form muss exakt der `serde`-Serialisierung von
 * `apx_pipeline::EdlV3` entsprechen (Feldnamen, Verschachtelung), da
 * `crate::edl::migrate::from_envelope` sie strikt gegen die Struktur
 * validiert statt fehlende Felder mit Defaults aufzufüllen.
 */

export const EDL_SCHEMA_VERSION = 3;

// ---- Grundeinstellungen (12 Regler: 7 aus Phase 2 + 5 aus Phase 4) --------

export interface WhiteBalanceAdjustment {
  temp_shift_kelvin: number;
  tint_shift: number;
}

export const NEUTRAL_WHITE_BALANCE: WhiteBalanceAdjustment = {
  temp_shift_kelvin: 0,
  tint_shift: 0,
};

export interface BasicAdjustments {
  white_balance: WhiteBalanceAdjustment;
  exposure_ev: number;
  contrast: number;
  highlights: number;
  shadows: number;
  whites: number;
  blacks: number;
  texture: number;
  clarity: number;
  dehaze: number;
  vibrance: number;
  saturation: number;
}

export const NEUTRAL_BASIC_ADJUSTMENTS: BasicAdjustments = {
  white_balance: NEUTRAL_WHITE_BALANCE,
  exposure_ev: 0,
  contrast: 0,
  highlights: 0,
  shadows: 0,
  whites: 0,
  blacks: 0,
  texture: 0,
  clarity: 0,
  dehaze: 0,
  vibrance: 0,
  saturation: 0,
};

// ---- Kurven ----------------------------------------------------------------

export interface CurvePoint {
  input: number;
  output: number;
}

/** Spiegelt Rusts intern getaggtes `#[serde(tag = "kind")]`-Enum. */
export type CurveChannel =
  | { kind: "Points"; points: CurvePoint[] }
  | { kind: "Parametric"; shadows: number; darks: number; lights: number; highlights: number };

export function identityCurve(): CurveChannel {
  return {
    kind: "Points",
    points: [
      { input: 0, output: 0 },
      { input: 1, output: 1 },
    ],
  };
}

export interface CurvesAdjustment {
  rgb: CurveChannel;
  red: CurveChannel;
  green: CurveChannel;
  blue: CurveChannel;
  luminance: CurveChannel;
}

export function neutralCurves(): CurvesAdjustment {
  return {
    rgb: identityCurve(),
    red: identityCurve(),
    green: identityCurve(),
    blue: identityCurve(),
    luminance: identityCurve(),
  };
}

/** Die vier Regler der parametrischen Kurve — Reihenfolge/Beschriftung
 * wie in Lightrooms eigenem Gradationskurven-Werkzeug (Phase 4 Schritt 4). */
// Beschriftungen bewusst mit "(Kurve)"-Suffix bei den beiden Namen, die
// sich sonst mit den gleichnamigen Grundeinstellungs-Reglern überschneiden
// würden (`Lichter`/`Tiefen`) — sowohl für Screenreader-Nutzer als auch
// für `getByRole("slider", { name })` in Tests eindeutig.
export const PARAMETRIC_CURVE_SLIDER_SPECS: readonly SliderSpec[] = [
  { key: "highlights", label: "Lichter (Kurve)", min: -100, max: 100, fineStep: 1, coarseStep: 10, neutral: 0 },
  { key: "lights", label: "Helle Töne", min: -100, max: 100, fineStep: 1, coarseStep: 10, neutral: 0 },
  { key: "darks", label: "Dunkle Töne", min: -100, max: 100, fineStep: 1, coarseStep: 10, neutral: 0 },
  { key: "shadows", label: "Tiefen (Kurve)", min: -100, max: 100, fineStep: 1, coarseStep: 10, neutral: 0 },
] as const;

/** Die fünf Kurven-Kanäle (Phase 4 Schritt 4) — Anzeigereihenfolge wie in
 * `SPEC.md` §3.2 („RGB-Verbundkurve, R/G/B einzeln, Luminanz-Kurve").
 * Von `DevelopPanel.tsx` und `MasksPanel.tsx` (Phase 6 Schritt 7)
 * gemeinsam genutzt — dieselbe Kanalauswahl-UI für globale wie für
 * masken-eigene Kurven. */
export const CURVE_CHANNEL_TABS: ReadonlyArray<{ key: keyof CurvesAdjustment; label: string }> = [
  { key: "rgb", label: "RGB" },
  { key: "red", label: "Rot" },
  { key: "green", label: "Grün" },
  { key: "blue", label: "Blau" },
  { key: "luminance", label: "Luminanz" },
];

export interface CurvePreset {
  key: string;
  label: string;
  points: CurvePoint[];
}

/** Feste Kurven-Presets (`SPEC.md` §3.2 „Kurven ... Presets"), anwendbar
 * auf jeden der fünf Kanäle. */
export const CURVE_PRESETS: readonly CurvePreset[] = [
  {
    key: "linear",
    label: "Linear",
    points: [
      { input: 0, output: 0 },
      { input: 1, output: 1 },
    ],
  },
  {
    key: "medium_contrast",
    label: "Leichter Kontrast",
    points: [
      { input: 0, output: 0 },
      { input: 0.25, output: 0.2 },
      { input: 0.75, output: 0.8 },
      { input: 1, output: 1 },
    ],
  },
  {
    key: "strong_contrast",
    label: "Starker Kontrast",
    points: [
      { input: 0, output: 0 },
      { input: 0.25, output: 0.12 },
      { input: 0.75, output: 0.88 },
      { input: 1, output: 1 },
    ],
  },
  {
    key: "linear_negative",
    label: "Negativ",
    points: [
      { input: 0, output: 1 },
      { input: 1, output: 0 },
    ],
  },
] as const;

// ---- HSL --------------------------------------------------------------------

export interface HslBand {
  hue: number;
  saturation: number;
  luminance: number;
}

export const NEUTRAL_HSL_BAND: HslBand = { hue: 0, saturation: 0, luminance: 0 };

export interface HslAdjustment {
  red: HslBand;
  orange: HslBand;
  yellow: HslBand;
  green: HslBand;
  aqua: HslBand;
  blue: HslBand;
  purple: HslBand;
  magenta: HslBand;
}

export const NEUTRAL_HSL: HslAdjustment = {
  red: NEUTRAL_HSL_BAND,
  orange: NEUTRAL_HSL_BAND,
  yellow: NEUTRAL_HSL_BAND,
  green: NEUTRAL_HSL_BAND,
  aqua: NEUTRAL_HSL_BAND,
  blue: NEUTRAL_HSL_BAND,
  purple: NEUTRAL_HSL_BAND,
  magenta: NEUTRAL_HSL_BAND,
};

/** Die acht festen HSL-Bänder (Phase 4 Schritt 5) — Reihenfolge/Zentren
 * wie in `crates/apx-pipeline/src/stages/hsl_color_mixer.rs`s
 * `HSL_BAND_CENTERS_DEGREES`. */
export const HSL_BAND_TABS: ReadonlyArray<{ key: keyof HslAdjustment; label: string }> = [
  { key: "red", label: "Rot" },
  { key: "orange", label: "Orange" },
  { key: "yellow", label: "Gelb" },
  { key: "green", label: "Grün" },
  { key: "aqua", label: "Aqua" },
  { key: "blue", label: "Blau" },
  { key: "purple", label: "Lila" },
  { key: "magenta", label: "Magenta" },
] as const;

/** Regler-Spezifikationen für ein einzelnes HSL-Band — dieselben drei
 * Felder für jedes der acht Bänder. */
export const HSL_BAND_SLIDER_SPECS: readonly SliderSpec[] = [
  { key: "hue", label: "Farbton", min: -100, max: 100, fineStep: 1, coarseStep: 10, neutral: 0 },
  { key: "saturation", label: "Sättigung", min: -100, max: 100, fineStep: 1, coarseStep: 10, neutral: 0 },
  { key: "luminance", label: "Luminanz", min: -100, max: 100, fineStep: 1, coarseStep: 10, neutral: 0 },
] as const;

// ---- Farbmischer erweitert --------------------------------------------------

export interface ColorMixerRegion {
  target_hue_degrees: number;
  bandwidth_degrees: number;
  feather: number;
  hue_shift: number;
  saturation_shift: number;
  luminance_shift: number;
}

export interface ColorMixerAdjustment {
  regions: ColorMixerRegion[];
}

export function neutralColorMixer(): ColorMixerAdjustment {
  return { regions: [] };
}

/** Eine neue, per Bildklick aufgenommene Region — neutrale Regler-Werte,
 * eine mittelbreite Bandbreite mit etwas Weichzeichnung als Startpunkt. */
export function newColorMixerRegion(targetHueDegrees: number): ColorMixerRegion {
  return {
    target_hue_degrees: targetHueDegrees,
    bandwidth_degrees: 30,
    feather: 0.3,
    hue_shift: 0,
    saturation_shift: 0,
    luminance_shift: 0,
  };
}

/** Obergrenze für Farbmischer-Regionen im fusionierten GPU/CPU-Pfad —
 * spiegelt `hsl_color_mixer.rs`s `MAX_COLOR_MIXER_REGIONS` (siehe dessen
 * Moduldoku für die Begründung). Das Frontend verhindert das Anlegen
 * weiterer Regionen, statt sie stillschweigend wirkungslos zu lassen. */
export const MAX_COLOR_MIXER_REGIONS = 8;

// Beschriftungen bewusst mit "-Verschiebung"-Suffix, da sie sich sonst mit
// den gleichnamigen HSL-Band-Reglern überschneiden würden (beide
// Abschnitte sind gleichzeitig sichtbar, sobald eine Region ausgewählt
// ist) — sowohl für Screenreader-Nutzer als auch für
// `getByRole("slider", { name })` in Tests eindeutig.
export const COLOR_MIXER_REGION_SLIDER_SPECS: readonly SliderSpec[] = [
  { key: "bandwidth_degrees", label: "Bandbreite", min: 5, max: 180, fineStep: 1, coarseStep: 10, neutral: 30 },
  { key: "hue_shift", label: "Farbton-Verschiebung", min: -100, max: 100, fineStep: 1, coarseStep: 10, neutral: 0 },
  { key: "saturation_shift", label: "Sättigung-Verschiebung", min: -100, max: 100, fineStep: 1, coarseStep: 10, neutral: 0 },
  { key: "luminance_shift", label: "Luminanz-Verschiebung", min: -100, max: 100, fineStep: 1, coarseStep: 10, neutral: 0 },
] as const;

// ---- Color Grading (Farbräder) ---------------------------------------------

export interface ColorGradingWheel {
  hue_degrees: number;
  saturation: number;
  luminance: number;
}

export const NEUTRAL_COLOR_GRADING_WHEEL: ColorGradingWheel = { hue_degrees: 0, saturation: 0, luminance: 0 };

export interface ColorGradingAdjustment {
  shadows: ColorGradingWheel;
  midtones: ColorGradingWheel;
  highlights: ColorGradingWheel;
  global: ColorGradingWheel;
  balance: number;
  blending: number;
}

/** Die vier Color-Grading-Farbräder (Phase 4 Schritt 6) — von
 * `DevelopPanel.tsx` und `MasksPanel.tsx` (Phase 6 Schritt 7) gemeinsam
 * genutzt. */
export const COLOR_GRADING_WHEEL_TABS: ReadonlyArray<{ key: keyof Pick<ColorGradingAdjustment, "shadows" | "midtones" | "highlights" | "global">; label: string }> = [
  { key: "shadows", label: "Schatten" },
  { key: "midtones", label: "Mitteltöne" },
  { key: "highlights", label: "Lichter" },
  { key: "global", label: "Global" },
];

export const NEUTRAL_COLOR_GRADING: ColorGradingAdjustment = {
  shadows: NEUTRAL_COLOR_GRADING_WHEEL,
  midtones: NEUTRAL_COLOR_GRADING_WHEEL,
  highlights: NEUTRAL_COLOR_GRADING_WHEEL,
  global: NEUTRAL_COLOR_GRADING_WHEEL,
  balance: 0,
  blending: 50,
};

// ---- Details (Schärfung + Rauschreduzierung) -------------------------------

export interface DetailsAdjustment {
  sharpen_amount: number;
  sharpen_radius: number;
  sharpen_detail: number;
  sharpen_masking: number;
  use_deconvolution_sharpen: boolean;
  luminance_nr_amount: number;
  luminance_nr_detail: number;
  luminance_nr_contrast: number;
  color_nr_amount: number;
  color_nr_detail: number;
  color_nr_smoothness: number;
}

export const NEUTRAL_DETAILS: DetailsAdjustment = {
  sharpen_amount: 0,
  sharpen_radius: 1,
  sharpen_detail: 25,
  sharpen_masking: 0,
  use_deconvolution_sharpen: false,
  luminance_nr_amount: 0,
  luminance_nr_detail: 50,
  luminance_nr_contrast: 0,
  color_nr_amount: 0,
  color_nr_detail: 50,
  color_nr_smoothness: 50,
};

/** Die zehn numerischen Details-Regler (Phase 4 Schritt 8) — der elfte
 * Feld (`use_deconvolution_sharpen`) ist eine Checkbox, kein Regler. Von
 * `DevelopPanel.tsx` und `MasksPanel.tsx` (Phase 6 Schritt 7) gemeinsam
 * genutzt. */
export type DetailsSliderKey = keyof Omit<DetailsAdjustment, "use_deconvolution_sharpen">;

export const SHARPEN_SLIDER_SPECS: readonly SliderSpec[] = [
  { key: "sharpen_amount", label: "Schärfung: Betrag", min: 0, max: 150, fineStep: 1, coarseStep: 10, neutral: 0 },
  { key: "sharpen_radius", label: "Schärfung: Radius", min: 0.5, max: 3, fineStep: 0.1, coarseStep: 0.5, neutral: 1 },
  { key: "sharpen_detail", label: "Schärfung: Detail", min: 0, max: 100, fineStep: 1, coarseStep: 10, neutral: 25 },
  { key: "sharpen_masking", label: "Schärfung: Maskierung", min: 0, max: 100, fineStep: 1, coarseStep: 10, neutral: 0 },
];

export const LUMINANCE_NR_SLIDER_SPECS: readonly SliderSpec[] = [
  { key: "luminance_nr_amount", label: "Luminanzrauschen: Betrag", min: 0, max: 100, fineStep: 1, coarseStep: 10, neutral: 0 },
  { key: "luminance_nr_detail", label: "Luminanzrauschen: Detail", min: 0, max: 100, fineStep: 1, coarseStep: 10, neutral: 50 },
  { key: "luminance_nr_contrast", label: "Luminanzrauschen: Kontrast", min: 0, max: 100, fineStep: 1, coarseStep: 10, neutral: 0 },
];

export const COLOR_NR_SLIDER_SPECS: readonly SliderSpec[] = [
  { key: "color_nr_amount", label: "Farbrauschen: Betrag", min: 0, max: 100, fineStep: 1, coarseStep: 10, neutral: 0 },
  { key: "color_nr_detail", label: "Farbrauschen: Detail", min: 0, max: 100, fineStep: 1, coarseStep: 10, neutral: 50 },
  { key: "color_nr_smoothness", label: "Farbrauschen: Glättung", min: 0, max: 100, fineStep: 1, coarseStep: 10, neutral: 50 },
];

// ---- Objektivkorrekturen ----------------------------------------------------

/** Die fünf `SPEC.md`-Modi plus `"Off"` als neutraler Standard. */
export type UprightMode = "Off" | "Auto" | "Level" | "Vertical" | "Full" | "Guided";

export interface GuidedLine {
  x1: number;
  y1: number;
  x2: number;
  y2: number;
}

export interface ManualTransform {
  vertical: number;
  horizontal: number;
  rotate_degrees: number;
  aspect: number;
  scale: number;
  offset_x: number;
  offset_y: number;
}

export const NEUTRAL_MANUAL_TRANSFORM: ManualTransform = {
  vertical: 0,
  horizontal: 0,
  rotate_degrees: 0,
  aspect: 0,
  scale: 100,
  offset_x: 0,
  offset_y: 0,
};

export interface LensCorrectionAdjustment {
  /** Referenz auf ein Profil in der eingebauten Mini-Profildatenbank
   * (siehe `DECISIONS.md` ADR-0028), `null` = kein Profil zugeordnet. */
  profile_id: string | null;
  ca_red_cyan: number;
  ca_blue_yellow: number;
  auto_ca: boolean;
  vignette_amount: number;
  distortion_amount: number;
  upright_mode: UprightMode;
  guided_lines: GuidedLine[];
  manual_transform: ManualTransform;
}

export function neutralLensCorrections(): LensCorrectionAdjustment {
  return {
    profile_id: null,
    ca_red_cyan: 0,
    ca_blue_yellow: 0,
    auto_ca: false,
    vignette_amount: 0,
    distortion_amount: 0,
    upright_mode: "Off",
    guided_lines: [],
    manual_transform: NEUTRAL_MANUAL_TRANSFORM,
  };
}

/** Spiegelt `crates/apx-pipeline/lens_profiles/*.json` für das Dropdown
 * — nur Name/ID, die eigentliche Korrekturberechnung passiert
 * ausschließlich serverseitig (siehe `lens_profiles.rs`). */
export const LENS_PROFILE_OPTIONS: ReadonlyArray<{ value: string | null; label: string }> = [
  { value: null, label: "Kein Profil" },
  { value: "generic-wide", label: "Generisches Weitwinkel" },
  { value: "generic-standard", label: "Generisches Standardzoom" },
  { value: "generic-tele", label: "Generisches Teleobjektiv" },
];

export const UPRIGHT_MODE_OPTIONS: ReadonlyArray<{ value: UprightMode; label: string }> = [
  { value: "Off", label: "Aus" },
  { value: "Auto", label: "Automatisch" },
  { value: "Level", label: "Waagerecht" },
  { value: "Vertical", label: "Senkrecht" },
  { value: "Full", label: "Vollständig" },
  { value: "Guided", label: "Geführt (2 Hilfslinien)" },
];

export const LENS_CA_SLIDER_SPECS: readonly SliderSpec[] = [
  { key: "ca_red_cyan", label: "CA: Rot/Cyan", min: -100, max: 100, fineStep: 1, coarseStep: 10, neutral: 0 },
  { key: "ca_blue_yellow", label: "CA: Blau/Gelb", min: -100, max: 100, fineStep: 1, coarseStep: 10, neutral: 0 },
];

export const LENS_SLIDER_SPECS: readonly SliderSpec[] = [
  { key: "vignette_amount", label: "Vignettierung", min: -100, max: 100, fineStep: 1, coarseStep: 10, neutral: 0 },
  { key: "distortion_amount", label: "Verzeichnung", min: -100, max: 100, fineStep: 1, coarseStep: 10, neutral: 0 },
];

export const MANUAL_TRANSFORM_SLIDER_SPECS: readonly SliderSpec[] = [
  { key: "vertical", label: "Transformation: Vertikal", min: -100, max: 100, fineStep: 1, coarseStep: 10, neutral: 0 },
  { key: "horizontal", label: "Transformation: Horizontal", min: -100, max: 100, fineStep: 1, coarseStep: 10, neutral: 0 },
  { key: "rotate_degrees", label: "Transformation: Drehen", min: -45, max: 45, fineStep: 0.1, coarseStep: 1, neutral: 0 },
  { key: "aspect", label: "Transformation: Seitenverhältnis", min: -100, max: 100, fineStep: 1, coarseStep: 10, neutral: 0 },
  { key: "scale", label: "Transformation: Skalieren", min: 50, max: 150, fineStep: 1, coarseStep: 10, neutral: 100 },
  { key: "offset_x", label: "Transformation: Versatz X", min: -100, max: 100, fineStep: 1, coarseStep: 10, neutral: 0 },
  { key: "offset_y", label: "Transformation: Versatz Y", min: -100, max: 100, fineStep: 1, coarseStep: 10, neutral: 0 },
];

// ---- Effekte ----------------------------------------------------------------

export interface EffectsAdjustment {
  post_vignette_amount: number;
  post_vignette_midpoint: number;
  post_vignette_roundness: number;
  post_vignette_feather: number;
  post_vignette_highlights: number;
  grain_amount: number;
  grain_size: number;
  grain_roughness: number;
}

export const NEUTRAL_EFFECTS: EffectsAdjustment = {
  post_vignette_amount: 0,
  post_vignette_midpoint: 50,
  post_vignette_roundness: 0,
  post_vignette_feather: 50,
  post_vignette_highlights: 0,
  grain_amount: 0,
  grain_size: 25,
  grain_roughness: 50,
};

export const POST_VIGNETTE_SLIDER_SPECS: readonly SliderSpec[] = [
  { key: "post_vignette_amount", label: "Vignettierung: Betrag", min: -100, max: 100, fineStep: 1, coarseStep: 10, neutral: 0 },
  { key: "post_vignette_midpoint", label: "Vignettierung: Mittelpunkt", min: 0, max: 100, fineStep: 1, coarseStep: 10, neutral: 50 },
  { key: "post_vignette_roundness", label: "Vignettierung: Rundheit", min: -100, max: 100, fineStep: 1, coarseStep: 10, neutral: 0 },
  { key: "post_vignette_feather", label: "Vignettierung: Weiche Kante", min: 0, max: 100, fineStep: 1, coarseStep: 10, neutral: 50 },
  { key: "post_vignette_highlights", label: "Vignettierung: Lichter", min: 0, max: 100, fineStep: 1, coarseStep: 10, neutral: 0 },
];

export const GRAIN_SLIDER_SPECS: readonly SliderSpec[] = [
  { key: "grain_amount", label: "Körnung: Betrag", min: 0, max: 100, fineStep: 1, coarseStep: 10, neutral: 0 },
  { key: "grain_size", label: "Körnung: Größe", min: 1, max: 100, fineStep: 1, coarseStep: 10, neutral: 25 },
  { key: "grain_roughness", label: "Körnung: Unregelmäßigkeit", min: 0, max: 100, fineStep: 1, coarseStep: 10, neutral: 50 },
];

// ---- Kalibrierung -----------------------------------------------------------

export type ProcessVersion = "V1";

export interface PrimaryColorAdjustment {
  hue: number;
  saturation: number;
}

export const NEUTRAL_PRIMARY_COLOR: PrimaryColorAdjustment = { hue: 0, saturation: 0 };

export interface CalibrationAdjustment {
  process_version: ProcessVersion;
  shadow_tint: number;
  red_primary: PrimaryColorAdjustment;
  green_primary: PrimaryColorAdjustment;
  blue_primary: PrimaryColorAdjustment;
  /** Name eines eingebauten Kameraprofils (kein DCP-Import, siehe
   * ADR-0028), `null` = Standardprofil. */
  camera_profile: string | null;
}

export function neutralCalibration(): CalibrationAdjustment {
  return {
    process_version: "V1",
    shadow_tint: 0,
    red_primary: NEUTRAL_PRIMARY_COLOR,
    green_primary: NEUTRAL_PRIMARY_COLOR,
    blue_primary: NEUTRAL_PRIMARY_COLOR,
    camera_profile: null,
  };
}

/** Die drei Kalibrierungs-Primärfarben — Anzeigereihenfolge Rot/Grün/Blau. */
export const CALIBRATION_PRIMARY_ROWS: ReadonlyArray<{
  key: keyof Pick<CalibrationAdjustment, "red_primary" | "green_primary" | "blue_primary">;
  label: string;
}> = [
  { key: "red_primary", label: "Rot" },
  { key: "green_primary", label: "Grün" },
  { key: "blue_primary", label: "Blau" },
];

/** Spiegelt `crates/apx-pipeline/src/stages/calibration.rs`s
 * `CAMERA_PROFILES`-Liste für das Dropdown — nur Namen, die eigentliche
 * Sättigungs-/Kontrast-Umrechnung passiert ausschließlich serverseitig.
 * `null` = Standardprofil (siehe `neutralCalibration`), entspricht
 * funktional dem `"Standard"`-Eintrag der Rust-Liste. */
export const CAMERA_PROFILE_OPTIONS: ReadonlyArray<{ value: string | null; label: string }> = [
  { value: null, label: "Standard" },
  { value: "Neutral", label: "Neutral" },
  { value: "Vivid", label: "Vivid" },
  { value: "Landscape", label: "Landscape" },
  { value: "Portrait", label: "Portrait" },
  { value: "Monochrome", label: "Monochrome" },
];

// ---- Geometrie (Crop/Rotation) ----------------------------------------------

export type GridOverlay = "None" | "Thirds" | "GoldenRatio" | "Diagonals" | "Spiral" | "Triangles";

export interface CropRect {
  x: number;
  y: number;
  width: number;
  height: number;
}

/** Das ganze Bild, kein Beschnitt. */
export const FULL_CROP_RECT: CropRect = { x: 0, y: 0, width: 1, height: 1 };

export interface GeometryAdjustment {
  crop: CropRect;
  /** `null` = freie Seitenverhältniswahl, sonst Breite/Höhe-Verhältnis. */
  aspect_ratio: number | null;
  angle_degrees: number;
  overlay: GridOverlay;
  /** Vereinfachte Auto-Ausrichtung: nur EXIF-Orientierung, siehe ADR-0028. */
  auto_horizon: boolean;
}

export const NEUTRAL_GEOMETRY: GeometryAdjustment = {
  crop: FULL_CROP_RECT,
  aspect_ratio: null,
  angle_degrees: 0,
  overlay: "None",
  auto_horizon: false,
};

export const ASPECT_RATIO_PRESETS: ReadonlyArray<{ value: number | null; label: string }> = [
  { value: null, label: "Frei" },
  { value: 1, label: "1:1" },
  { value: 4 / 3, label: "4:3" },
  { value: 3 / 2, label: "3:2" },
  { value: 16 / 9, label: "16:9" },
];

export const GRID_OVERLAY_OPTIONS: ReadonlyArray<{ value: GridOverlay; label: string }> = [
  { value: "None", label: "Kein Raster" },
  { value: "Thirds", label: "Drittel" },
  { value: "GoldenRatio", label: "Goldener Schnitt" },
  { value: "Diagonals", label: "Diagonalen" },
  { value: "Spiral", label: "Spirale" },
  { value: "Triangles", label: "Dreiecke" },
];

// ---- Reparatur (Klonen/Reparieren) ------------------------------------------

export type RepairMode = "Clone" | "Heal";

export interface RepairPoint {
  x: number;
  y: number;
}

/** Ein einzelner Klon-/Reparatur-Pinselzug. Bewusst **nicht** Teil von
 * Phase 4: Auto-Quellenfindung, inhaltsbasiertes Füllen (siehe ADR-0028). */
export interface RepairStroke {
  mode: RepairMode;
  source: RepairPoint;
  target_path: RepairPoint[];
  radius: number;
  feather: number;
  opacity: number;
}

// ---- Masken (Phase 6, siehe DECISIONS.md ADR-0032) --------------------------

export interface MaskPoint {
  x: number;
  y: number;
}

export interface BrushStroke {
  points: MaskPoint[];
  radius: number;
  feather: number;
}

/** Spiegelt Rusts intern getaggtes `#[serde(tag = "kind")]`-Enum — die
 * fünf `SPEC.md` §5 genannten Maskentypen (Tiefenbereich/KI-Masken sind
 * bewusst nicht Teil dieses Schemas, siehe ADR-0032 Punkt 3). */
export type MaskGeometry =
  | { kind: "Brush"; strokes: BrushStroke[] }
  | { kind: "LinearGradient"; x1: number; y1: number; x2: number; y2: number }
  | {
      kind: "RadialGradient";
      center_x: number;
      center_y: number;
      radius_x: number;
      radius_y: number;
      angle_degrees: number;
      feather: number;
    }
  | { kind: "ColorRange"; target_r: number; target_g: number; target_b: number; tolerance: number; feather: number }
  | { kind: "LuminanceRange"; range_min: number; range_max: number; feather: number };

export function emptyBrushGeometry(): MaskGeometry {
  return { kind: "Brush", strokes: [] };
}

export type MaskCombine = "Add" | "Subtract" | "Intersect";

export interface MaskComponent {
  geometry: MaskGeometry;
  combine: MaskCombine;
  invert: boolean;
}

/** Die ton-/farb-/detailbezogenen Werkzeuge, die pro Maske zur Verfügung
 * stehen (`DECISIONS.md` ADR-0032 Punkt 2) — bewusst ohne
 * Objektivkorrekturen/Effekte/Kalibrierung/Geometrie/Reparatur. */
export interface MaskAdjustments {
  basic: BasicAdjustments;
  curves: CurvesAdjustment;
  hsl: HslAdjustment;
  color_mixer: ColorMixerAdjustment;
  color_grading: ColorGradingAdjustment;
  details: DetailsAdjustment;
}

export function neutralMaskAdjustments(): MaskAdjustments {
  return {
    basic: NEUTRAL_BASIC_ADJUSTMENTS,
    curves: neutralCurves(),
    hsl: NEUTRAL_HSL,
    color_mixer: neutralColorMixer(),
    color_grading: NEUTRAL_COLOR_GRADING,
    details: NEUTRAL_DETAILS,
  };
}

/** `SPEC.md` §5: „Normal" plus die namentlich genannten Beispiele
 * (Multiplizieren, Weiches Licht, Farbe, Luminanz). */
export type BlendMode = "Normal" | "Multiply" | "SoftLight" | "Color" | "Luminosity";

export const BLEND_MODE_OPTIONS: ReadonlyArray<{ value: BlendMode; label: string }> = [
  { value: "Normal", label: "Normal" },
  { value: "Multiply", label: "Multiplizieren" },
  { value: "SoftLight", label: "Weiches Licht" },
  { value: "Color", label: "Farbe" },
  { value: "Luminosity", label: "Luminanz" },
];

export type OverlayColor = "Red" | "Green" | "Blue" | "Yellow" | "Magenta";

export const OVERLAY_COLOR_OPTIONS: ReadonlyArray<{ value: OverlayColor; label: string }> = [
  { value: "Red", label: "Rot" },
  { value: "Green", label: "Grün" },
  { value: "Blue", label: "Blau" },
  { value: "Yellow", label: "Gelb" },
  { value: "Magenta", label: "Magenta" },
];

/** Eine lokale Anpassung (`SPEC.md` §3.3) — `id` clientseitig vergeben
 * (Masken leben ausschließlich im opaken EDL-JSON-Blob, nie als eigene
 * Katalogzeile). */
export interface Mask {
  id: string;
  name: string;
  components: MaskComponent[];
  adjustments: MaskAdjustments;
  opacity: number;
  feather: number;
  invert: boolean;
  blend_mode: BlendMode;
  visible: boolean;
  group_id: string | null;
  overlay_color: OverlayColor;
}

/** Eine neue Maske mit einer einzelnen Startkomponente und neutralen
 * Anpassungen — der Startzustand beim Anlegen einer Maske im Frontend,
 * unabhängig vom Geometrietyp. */
export function newMask(id: string, name: string, geometry: MaskGeometry): Mask {
  return {
    id,
    name,
    components: [{ geometry, combine: "Add", invert: false }],
    adjustments: neutralMaskAdjustments(),
    opacity: 100,
    feather: 0,
    invert: false,
    blend_mode: "Normal",
    visible: true,
    group_id: null,
    overlay_color: "Red",
  };
}

export function newBrushMask(id: string, name: string): Mask {
  return newMask(id, name, emptyBrushGeometry());
}

/** Senkrechter Verlauf über das mittlere Drittel des Bildes — ein
 * plausibler Startzustand, den der Nutzer danach per Ziehgriffen im
 * Viewer verschiebt (Phase 6 Schritt 3). */
export function defaultLinearGradientGeometry(): MaskGeometry {
  return { kind: "LinearGradient", x1: 0.5, y1: 0.2, x2: 0.5, y2: 0.8 };
}

/** Zentrierter Radialverlauf über etwa ein Drittel der kürzeren
 * Bildkante — derselbe Zweck wie [`defaultLinearGradientGeometry`]. */
export function defaultRadialGradientGeometry(): MaskGeometry {
  return {
    kind: "RadialGradient",
    center_x: 0.5,
    center_y: 0.5,
    radius_x: 0.3,
    radius_y: 0.3,
    angle_degrees: 0,
    feather: 0.5,
  };
}

/** Neutrales Mittelgrau als Platzhalter-Zielfarbe, bis der Nutzer per
 * Bildklick eine echte Farbe aufnimmt (Phase 6 Schritt 5) — passende
 * Toleranz/Weichzeichnung, damit die Maske vor dem ersten Klick schon
 * eine sichtbare, aber nicht das ganze Bild abdeckende Fläche zeigt. */
export function defaultColorRangeGeometry(): MaskGeometry {
  return { kind: "ColorRange", target_r: 0.5, target_g: 0.5, target_b: 0.5, tolerance: 0.15, feather: 0.1 };
}

/** Obere Tonwerthälfte (Lichter) als plausibler Startzustand — dieselbe
 * Konvention wie Lightrooms Luminanzbereichs-Maske, die üblicherweise
 * zuerst auf „Lichter" steht. */
export function defaultLuminanceRangeGeometry(): MaskGeometry {
  return { kind: "LuminanceRange", range_min: 0.5, range_max: 1, feather: 0.1 };
}

export interface MaskGroup {
  id: string;
  name: string;
  visible: boolean;
}

export const MASK_SLIDER_SPECS: readonly SliderSpec[] = [
  { key: "opacity", label: "Maske: Deckkraft", min: 0, max: 100, fineStep: 1, coarseStep: 10, neutral: 100 },
  { key: "feather", label: "Maske: Weichzeichnung", min: 0, max: 100, fineStep: 1, coarseStep: 10, neutral: 0 },
];

// ---- Der vollständige EDL-Payload -------------------------------------------

/** Spiegelt `apx_pipeline::edl::v3::EdlV3` — der komplette Inhalt eines
 * `EdlEnvelope.payload`. */
export interface EdlPayload {
  basic: BasicAdjustments;
  curves: CurvesAdjustment;
  hsl: HslAdjustment;
  color_mixer: ColorMixerAdjustment;
  color_grading: ColorGradingAdjustment;
  details: DetailsAdjustment;
  lens_corrections: LensCorrectionAdjustment;
  effects: EffectsAdjustment;
  calibration: CalibrationAdjustment;
  geometry: GeometryAdjustment;
  repair: RepairStroke[];
  masks: Mask[];
  mask_groups: MaskGroup[];
}

export function neutralEdlPayload(): EdlPayload {
  return {
    basic: NEUTRAL_BASIC_ADJUSTMENTS,
    curves: neutralCurves(),
    hsl: NEUTRAL_HSL,
    color_mixer: neutralColorMixer(),
    color_grading: NEUTRAL_COLOR_GRADING,
    details: NEUTRAL_DETAILS,
    lens_corrections: neutralLensCorrections(),
    effects: NEUTRAL_EFFECTS,
    calibration: neutralCalibration(),
    geometry: NEUTRAL_GEOMETRY,
    repair: [],
    masks: [],
    mask_groups: [],
  };
}

/** Baut die JSON-Serialisierung eines `EdlEnvelope` (siehe
 * `apx_core::EdlEnvelope`), wie sie sowohl die `develop/...`-Protokoll-
 * Route als auch `apply_develop_edit` erwarten. */
export function buildEdlEnvelopeJson(payload: EdlPayload): string {
  return JSON.stringify({
    schema_version: EDL_SCHEMA_VERSION,
    payload,
  });
}

/** Liest ein `EdlPayload` aus einem `EdlEnvelope`-JSON-String (z. B. aus
 * `current_develop_edit`/`undo_develop_edit`/`redo_develop_edit`). Gibt
 * bei unbekannter Schema-Version oder unlesbarer Nutzlast `null` zurück,
 * statt einen Absturz zu riskieren — der Aufrufer entscheidet dann, ob er
 * auf `neutralEdlPayload()` zurückfällt. */
export function parseEdlEnvelopeJson(json: string): EdlPayload | null {
  try {
    const parsed: unknown = JSON.parse(json);
    if (typeof parsed !== "object" || parsed === null) return null;
    const envelope = parsed as { schema_version?: unknown; payload?: unknown };
    if (envelope.schema_version !== EDL_SCHEMA_VERSION) return null;
    const payload = envelope.payload;
    if (typeof payload !== "object" || payload === null) return null;
    // Keine tiefe Struktur-Validierung (Feld für Feld) — anders als die
    // Rust-Seite (die `serde` strukturell prüfen lässt) reicht hier ein
    // grober Plausibilitätscheck, da diese Funktion nur auf Antworten
    // angewendet wird, die dasselbe Backend gerade erst geschrieben hat.
    return payload as EdlPayload;
  } catch {
    return null;
  }
}

/** Ein einzelner Regler-Eintrag für `DevelopPanel` — Wertebereich und
 * Schrittweiten nach `SPEC.md` §4 (Pfeiltasten = Feinschritt, Umschalt +
 * Pfeiltasten = Grobschritt). */
export interface SliderSpec {
  key: string;
  label: string;
  min: number;
  max: number;
  /** Schrittweite bei einfachem Pfeiltasten-Druck. */
  fineStep: number;
  /** Schrittweite bei Umschalt+Pfeiltasten. */
  coarseStep: number;
  neutral: number;
}

export const BASIC_SLIDER_SPECS: readonly SliderSpec[] = [
  { key: "temp_shift_kelvin", label: "Temperatur", min: -2000, max: 2000, fineStep: 10, coarseStep: 100, neutral: 0 },
  { key: "tint_shift", label: "Tint", min: -100, max: 100, fineStep: 1, coarseStep: 10, neutral: 0 },
  { key: "exposure_ev", label: "Belichtung", min: -5, max: 5, fineStep: 0.01, coarseStep: 0.1, neutral: 0 },
  { key: "contrast", label: "Kontrast", min: -100, max: 100, fineStep: 1, coarseStep: 10, neutral: 0 },
  { key: "highlights", label: "Lichter", min: -100, max: 100, fineStep: 1, coarseStep: 10, neutral: 0 },
  { key: "shadows", label: "Tiefen", min: -100, max: 100, fineStep: 1, coarseStep: 10, neutral: 0 },
  { key: "whites", label: "Weiß", min: -100, max: 100, fineStep: 1, coarseStep: 10, neutral: 0 },
  { key: "blacks", label: "Schwarz", min: -100, max: 100, fineStep: 1, coarseStep: 10, neutral: 0 },
  // Die fünf per ADR-0011/ADR-0028 nach Phase 4 verschobenen Regler
  // (Phase 4 Schritt 3) — Reihenfolge wie in `SPEC.md` §3.2 aufgezählt.
  { key: "texture", label: "Textur", min: -100, max: 100, fineStep: 1, coarseStep: 10, neutral: 0 },
  { key: "clarity", label: "Klarheit", min: -100, max: 100, fineStep: 1, coarseStep: 10, neutral: 0 },
  { key: "dehaze", label: "Dunst entfernen", min: -100, max: 100, fineStep: 1, coarseStep: 10, neutral: 0 },
  { key: "vibrance", label: "Dynamik", min: -100, max: 100, fineStep: 1, coarseStep: 10, neutral: 0 },
  { key: "saturation", label: "Sättigung", min: -100, max: 100, fineStep: 1, coarseStep: 10, neutral: 0 },
] as const;

// ---- Weißabgleich-Pipette + Kamera-Presets (Phase 4 Schritt 3) --------------

export interface WhiteBalancePreset {
  key: string;
  label: string;
  temp_shift_kelvin: number;
  tint_shift: number;
}

/**
 * Feste Weißabgleich-Presets (`SPEC.md` §3.2 „Presets pro Kamera"). Ohne
 * echte Kamerakalibrierung (siehe `DECISIONS.md` ADR-0028: kein
 * DCP-/Adobe-Profil-Import) sind das keine physikalisch kalibrierten
 * Absolutwerte, sondern grobe, für die meisten Kameras plausible
 * Verschiebungen relativ zum As-shot-Weißabgleich — bewusste
 * Vereinfachung derselben Art wie die in ADR-0028 dokumentierten
 * CV-Vereinfachungen. Anders als die Pipette (die additiv zum
 * *aktuellen* Wert korrigiert) setzt ein Preset den Weißabgleich absolut
 * — konsistent mit Lightrooms eigenem Verhalten.
 */
export const WHITE_BALANCE_PRESETS: readonly WhiteBalancePreset[] = [
  { key: "as_shot", label: "Wie aufgenommen", temp_shift_kelvin: 0, tint_shift: 0 },
  { key: "daylight", label: "Tageslicht", temp_shift_kelvin: 200, tint_shift: 0 },
  { key: "cloudy", label: "Bewölkt", temp_shift_kelvin: 500, tint_shift: 10 },
  { key: "shade", label: "Schatten", temp_shift_kelvin: 800, tint_shift: 15 },
  { key: "tungsten", label: "Kunstlicht", temp_shift_kelvin: -1200, tint_shift: -5 },
  { key: "flash", label: "Blitz", temp_shift_kelvin: 300, tint_shift: 0 },
  { key: "fluorescent", label: "Leuchtstoffröhre", temp_shift_kelvin: -600, tint_shift: 20 },
] as const;

export function clampSliderValue(value: number, spec: Pick<SliderSpec, "min" | "max">): number {
  return Math.min(spec.max, Math.max(spec.min, value));
}

/** Wert nach einem Pfeiltasten-Druck (siehe `SPEC.md` §4: „Pfeiltasten =
 * Feinschritt, Umschalt = Grobschritt"). */
export function applyArrowStep(value: number, direction: 1 | -1, spec: SliderSpec, coarse: boolean): number {
  const step = coarse ? spec.coarseStep : spec.fineStep;
  return clampSliderValue(value + direction * step, spec);
}

/** Liest ein verschachteltes `BasicAdjustments`-Feld über den
 * `SliderSpec`-Schlüssel (`"temp_shift_kelvin"`/`"tint_shift"` liegen
 * unter `white_balance`, alle anderen direkt unter `basic`). */
export function readBasicField(basic: BasicAdjustments, key: string): number {
  if (key === "temp_shift_kelvin" || key === "tint_shift") {
    return basic.white_balance[key];
  }
  return basic[key as Exclude<keyof BasicAdjustments, "white_balance">];
}

/** Schreibt ein Feld über denselben Schlüssel wie [`readBasicField`] —
 * mutiert `basic` direkt (zum Aufruf innerhalb eines Immer-`set()`-Blocks
 * im Store gedacht, siehe `store/index.ts`). */
export function writeBasicField(basic: BasicAdjustments, key: string, value: number): void {
  if (key === "temp_shift_kelvin" || key === "tint_shift") {
    basic.white_balance[key] = value;
    return;
  }
  basic[key as Exclude<keyof BasicAdjustments, "white_balance">] = value;
}
