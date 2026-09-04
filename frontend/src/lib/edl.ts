/**
 * TypeScript-Gegenstück zu `crates/apx-pipeline/src/edl/v3.rs` und
 * `crates/apx-core/src/edl.rs` — von Hand synchron gehalten. Seit Phase 4
 * (Schritt 1, `DECISIONS.md` ADR-0028) ist das EDL deutlich größer als
 * die ursprünglichen sieben Phase-2-Regler; seit Phase 6 (Schritt 1,
 * ADR-0032) kommt das Maskensystem (`masks`/`mask_groups`) hinzu. Die
 * hier gespiegelten Typen folgen exakt `apx_pipeline::edl::v4`s Struktur-
 * und Feldnamen (`serde`s Standard-Serialisierung, keine Umbenennungen).
 *
 * Die JSON-Form muss exakt der `serde`-Serialisierung von
 * `apx_pipeline::EdlV4` entsprechen (Feldnamen, Verschachtelung), da
 * `crate::edl::migrate::from_envelope` sie strikt gegen die Struktur
 * validiert statt fehlende Felder mit Defaults aufzufüllen.
 */

export const EDL_SCHEMA_VERSION = 4;

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

/** Bandzentren in Grad, exakte Reihenfolge/Werte wie
 * `crates/apx-pipeline/src/stages/hsl_color_mixer.rs`s
 * `HSL_BAND_CENTERS_DEGREES` — für das zielgerichtete Anpassungswerkzeug
 * (TAT, Phase 11 Schritt 6): ordnet einen im Viewer gesampelten Farbton
 * dem nächstgelegenen der acht festen Bänder zu. */
const HSL_BAND_CENTERS_DEGREES: Record<keyof HslAdjustment, number> = {
  red: 0,
  orange: 30,
  yellow: 60,
  green: 120,
  aqua: 180,
  blue: 240,
  purple: 270,
  magenta: 300,
};

/** Kürzester Kreisabstand zweier Farbtöne in Grad (0..180), wie Rusts
 * `color_math::circular_distance_degrees`. */
function circularHueDistanceDegrees(a: number, b: number): number {
  const diff = Math.abs(a - b) % 360;
  return diff > 180 ? 360 - diff : diff;
}

/** Ordnet einen Farbton (Grad) dem nächstgelegenen der acht festen
 * HSL-Bänder zu — siehe TAT-Moduldoku in `store/index.ts`. */
export function nearestHslBand(hueDegrees: number): keyof HslAdjustment {
  let closest: keyof HslAdjustment = "red";
  let closestDistance = Infinity;
  for (const tab of HSL_BAND_TABS) {
    const distance = circularHueDistanceDegrees(hueDegrees, HSL_BAND_CENTERS_DEGREES[tab.key]);
    if (distance < closestDistance) {
      closestDistance = distance;
      closest = tab.key;
    }
  }
  return closest;
}

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
  /** Referenz auf ein Profil — eines der drei Alt-Beispielprofile
   * (ADR-0028) oder ein echter LensFun-Datenbankeintrag (seit Phase 12
   * Schritt 3, siehe `DECISIONS.md` ADR-0039), `null` = kein Profil
   * zugeordnet. */
  profile_id: string | null;
  ca_red_cyan: number;
  ca_blue_yellow: number;
  auto_ca: boolean;
  vignette_amount: number;
  distortion_amount: number;
  upright_mode: UprightMode;
  guided_lines: GuidedLine[];
  manual_transform: ManualTransform;
  /** Ergebnis einer eigenen Kalibrierung aus markierten geraden Linien
   * (Phase 12 Schritt 3 Teil B, siehe `DECISIONS.md` ADR-0039) — hat
   * Vorrang vor `profile_id`s Verzeichnungswert, wenn gesetzt. */
  custom_distortion_k1: number | null;
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
    custom_distortion_k1: null,
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
  /** Echte Halation-/Bloom-Simulation (Phase 14 Schritt 4, siehe
   * `DECISIONS.md` ADR-0041) — Lightroom Classic "cannot create true
   * film halation, only a soft bloom approximation". `0..=100`. */
  halation_amount: number;
  /** Bruchteil der Bildbreite für den Bloom-Weichzeichnungsradius,
   * `0..=100` (Prozent-Regler). */
  halation_radius: number;
  /** Farbton der Bloom-Einfärbung in Grad (`0..=360`) — echte
   * Filmhalation ist charakteristisch rot-orange. */
  halation_hue: number;
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
  halation_amount: 0,
  halation_radius: 30,
  halation_hue: 15,
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

/** Lightroom Classic "cannot create true film halation, only a soft
 * bloom approximation" (siehe `DECISIONS.md` ADR-0041, Recherche-Tabelle
 * Punkt 8). */
export const HALATION_SLIDER_SPECS: readonly SliderSpec[] = [
  { key: "halation_amount", label: "Halation: Betrag", min: 0, max: 100, fineStep: 1, coarseStep: 10, neutral: 0 },
  { key: "halation_radius", label: "Halation: Radius", min: 0, max: 100, fineStep: 1, coarseStep: 10, neutral: 30 },
  // Label bewusst als "Farbton (Halation)" statt "Halation: Farbton" —
  // sonst wäre der zugängliche Name "Halation: Farbton (Zahlenwert)" eine
  // Teilzeichenkette von "Farbton (Zahlenwert)" (Playwrights `name`-Option
  // matcht standardmäßig als Teilstring) und würde bestehende
  // `.nth()`-Disambiguierungen wie in `masks-flow.spec.ts` verschieben —
  // dasselbe Muster wie "Farbton (Rot)" bei der HSL-/Kalibrierungs-Zeile.
  { key: "halation_hue", label: "Farbton (Halation)", min: 0, max: 360, fineStep: 1, coarseStep: 15, neutral: 15 },
];

/** KI-Tiefenschärfe-Simulator "Virtuelle Blende" (Phase 14 Schritt 8) —
 * Lightroom hat "keine KI-Tiefenschätzung/synthetisches Bokeh" (siehe
 * `DECISIONS.md` ADR-0041, Recherche-Tabelle Punkt 1). Wie
 * `REPAIR_RADIUS_SPEC` (`0..=100` UI-Skala für einen intern
 * `0.0..=1.0`-Bruchteil, siehe `VirtualApertureAdjustment.amount`). */
export const VIRTUAL_APERTURE_SLIDER_SPECS: readonly SliderSpec[] = [
  { key: "amount", label: "Virtuelle Blende: Betrag", min: 0, max: 100, fineStep: 1, coarseStep: 10, neutral: 0 },
];

/** KI-Stiltransfer zwischen Fotos (Phase 14 Schritt 9) — Lightroom hat
 * dafür kein Äquivalent. Wie `VIRTUAL_APERTURE_SLIDER_SPECS` (`0..=100`
 * UI-Skala für einen intern `0.0..=1.0`-Bruchteil, siehe
 * `StyleTransferAdjustment.amount`). */
export const STYLE_TRANSFER_SLIDER_SPECS: readonly SliderSpec[] = [
  { key: "amount", label: "Stiltransfer: Betrag", min: 0, max: 100, fineStep: 1, coarseStep: 10, neutral: 0 },
];

// ---- Kalibrierung -----------------------------------------------------------

export type ProcessVersion = "V1";

export interface PrimaryColorAdjustment {
  hue: number;
  saturation: number;
}

export const NEUTRAL_PRIMARY_COLOR: PrimaryColorAdjustment = { hue: 0, saturation: 0 };

/** Aus einer importierten `.dcp`-Datei einmalig ausgelesene Profildaten
 * (Phase 13 Schritt 3, siehe `DECISIONS.md` ADR-0040-Nachtrag) — direkt im EDL
 * gespeichert, dasselbe Muster wie `AiFillPatch`. Hat Vorrang vor
 * `CalibrationAdjustment.camera_profile`s Handliste, wenn gesetzt. */
export interface DcpProfileData {
  name: string;
  hue_divisions: number;
  sat_divisions: number;
  val_divisions: number;
  hue_sat_map: Array<[number, number, number]>;
  tone_curve: Array<[number, number]>;
}

export interface CalibrationAdjustment {
  process_version: ProcessVersion;
  shadow_tint: number;
  red_primary: PrimaryColorAdjustment;
  green_primary: PrimaryColorAdjustment;
  blue_primary: PrimaryColorAdjustment;
  /** Name eines eingebauten Kameraprofils (kein DCP-Import, siehe
   * ADR-0028), `null` = Standardprofil. Ignoriert, solange `dcp_profile`
   * gesetzt ist. */
  camera_profile: string | null;
  /** Echte, aus einer `.dcp`-Datei gelesene Profildaten (Phase 13
   * Schritt 3) — `undefined`/fehlend = kein Import, `camera_profile`
   * bleibt maßgeblich. */
  dcp_profile?: DcpProfileData;
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

/** Vorab von `run_ai_outpaint` berechnetes, bereits zusammengesetztes
 * Bitmap der erweiterten Leinwand (Original + KI-erzeugter Rand) — wird
 * bei jedem Rendern nur noch auf die tatsächliche Zielgröße hoch-/
 * herunterskaliert (dasselbe „einmal berechnen, immer wieder
 * skalieren"-Muster wie `AiFillPatch`, siehe `CanvasExtension`s Doku
 * unten). */
export interface CanvasExtensionPatch {
  bitmap_width: number;
  bitmap_height: number;
  pixels: number[];
}

/** KI-Ausfüllen über die Bildränder hinaus (Phase 14 Schritt 1, siehe
 * `DECISIONS.md` ADR-0041). `margin_left`/`margin_top`/`margin_right`/
 * `margin_bottom` sind normierte Bruchteile der *aktuellen* Bildbreite/
 * -höhe (`0.0..=1.0`, dieselbe Konvention wie `CropRect`) — anders als
 * `CanvasExtensionPatch::bitmap_width/height` (eine feste
 * Speicherauflösung, die bei jedem Rendern passend skaliert wird) legen
 * die Margen unmittelbar das neue Seitenverhältnis fest und müssen daher
 * mit dem Bild mitskalieren statt eine absolute Pixelzahl zu sein. Ohne
 * `patch` (Ränder gewählt, aber „Anwenden" noch nicht ausgelöst) bleibt
 * die Erweiterung ein No-Op — dieselbe Konvention wie ein frischer
 * `AiInpaint`-Reparaturstrich ohne `ai_fill`. */
export interface CanvasExtension {
  margin_left: number;
  margin_top: number;
  margin_right: number;
  margin_bottom: number;
  patch: CanvasExtensionPatch | null;
}

export interface GeometryAdjustment {
  crop: CropRect;
  /** `null` = freie Seitenverhältniswahl, sonst Breite/Höhe-Verhältnis. */
  aspect_ratio: number | null;
  angle_degrees: number;
  overlay: GridOverlay;
  /** Vereinfachte Auto-Ausrichtung: nur EXIF-Orientierung, siehe ADR-0028. */
  auto_horizon: boolean;
  /** `null`, solange keine Leinwand-Erweiterung gewählt wurde (Phase 14
   * Schritt 1) — additiv, siehe `CanvasExtension`s Doku. */
  canvas_extension: CanvasExtension | null;
}

export const NEUTRAL_GEOMETRY: GeometryAdjustment = {
  crop: FULL_CROP_RECT,
  aspect_ratio: null,
  angle_degrees: 0,
  overlay: "None",
  auto_horizon: false,
  canvas_extension: null,
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

/** `ContentAwareFill` seit Phase 7 (siehe `DECISIONS.md` ADR-0033 Punkt
 * 4) — `source` wird für diesen Modus ignoriert, der Füllinhalt kommt
 * aus der Bildumgebung statt einem manuell gesetzten Quellpunkt.
 * `AiInpaint` seit Phase 13 Schritt 1 (siehe ADR-0040) — ebenfalls ohne
 * `source`, der Füllinhalt kommt aus einer einmalig per echtem
 * LaMa-Modell berechneten [`AiFillPatch`], die erst nach einem
 * ausdrücklichen „Anwenden" (Tauri-Command, siehe `store/index.ts`)
 * gesetzt wird — bis dahin ist der Strich ein No-Op. */
export type RepairMode = "Clone" | "Heal" | "ContentAwareFill" | "AiInpaint";

/** Welche Frequenz-Ebene ein Strich betrifft (Phase 14 Schritt 2, siehe
 * `DECISIONS.md` ADR-0041) — `"Normal"` ist das bisherige Verhalten
 * (Strich wirkt direkt auf das volle Bild). `"LowFrequency"`/
 * `"HighFrequency"` lassen ihn stattdessen gezielt nur auf Ton/Farbe
 * bzw. Textur/Kanten wirken (`stages::frequency_separation`, siehe
 * `edl/v2.rs`s `RepairLayer`-Kommentar). */
export type RepairLayer = "Normal" | "LowFrequency" | "HighFrequency";

export interface RepairPoint {
  x: number;
  y: number;
}

/** Ergebnis eines einmaligen KI-Ausfüllen-Laufs (Phase 13 Schritt 1, siehe
 * `DECISIONS.md` ADR-0040) — `x`/`y`/`width`/`height` sind normierte
 * Bildkoordinaten (`0.0..=1.0`, wie `RepairPoint`/`RepairStroke.radius`),
 * die gespeicherte `pixels`-Bitmap hat dagegen ihre eigene, von der
 * Analyse-Auflösung vorgegebene feste Größe (`bitmap_width`/
 * `bitmap_height`) — `stages::repair` skaliert sie beim Einsetzen
 * bilinear auf die Zielrechteck-Größe hoch (siehe `edl/v2.rs`s
 * `AiFillPatch`-Kommentar). `pixels` ist interleaved RGB (`0..=255`),
 * `bitmap_width * bitmap_height * 3` Zahlen lang. */
export interface AiFillPatch {
  x: number;
  y: number;
  width: number;
  height: number;
  bitmap_width: number;
  bitmap_height: number;
  pixels: number[];
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
  /** Nur für `mode === "AiInpaint"` relevant — `undefined`/fehlend liest
   * im Rust-Backend als `None` (additives Feld, siehe `edl/v2.rs`s
   * `RepairStroke::ai_fill`-Kommentar). */
  ai_fill?: AiFillPatch;
  /** Frequenztrennung (Phase 14 Schritt 2) — siehe `RepairLayer`s Doku. */
  layer: RepairLayer;
}

// ---- Verflüssigen (Liquify, Phase 15 Schritt 3) ----------------------------

/** Verformungsmodus (Photoshop-Namensgebung) — siehe `stages::liquify`s
 * Moduldoku für die genaue Wirkung jedes Modus. */
export type LiquifyMode = "Push" | "Twirl" | "Pucker" | "Bloat";

export interface LiquifyPoint {
  x: number;
  y: number;
}

/** Ein einzelner Verflüssigen-Pinselzug (Phase 15 Schritt 3, siehe
 * `DECISIONS.md` ADR-0042 — Photoshop-exklusiv, Lightroom hat kein
 * Verformungswerkzeug). `radius`/`strength` sind normiert wie
 * `RepairStroke.radius` (Bruchteil der Bildbreite bzw. 0..1). */
export interface LiquifyStroke {
  center_path: LiquifyPoint[];
  radius: number;
  strength: number;
  mode: LiquifyMode;
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
  /** Auto-Mask (Phase 12 Schritt 2, siehe `DECISIONS.md` ADR-0039):
   * dämpft die Deckkraft dieses Strichs an starken lokalen Bildkanten
   * (`masks.rs`s `relative_sharpness_map`), damit der Pinsel nicht über
   * scharfe Kanten hinweg "ausblutet" — wie Lightrooms gleichnamige
   * Option. */
  auto_mask: boolean;
}

/** Die fünf KI-Masken-Heuristiken (Phase 7, siehe `DECISIONS.md`
 * ADR-0033) — klassische Bildverarbeitung statt echter ONNX-Modelle,
 * siehe `apx-ai::segmentation`s Moduldoku. Rein informativ fürs
 * Anzeige-Label; die Pipeline behandelt jede Variante identisch. */
export type AiMaskKind = "Subject" | "Sky" | "Background" | "ClickRegion" | "Person";

export const AI_MASK_KIND_LABELS: Record<AiMaskKind, string> = {
  Subject: "Motiv",
  Sky: "Himmel",
  Background: "Hintergrund",
  ClickRegion: "Objekte",
  Person: "Personen",
};

/** Spiegelt Rusts intern getaggtes `#[serde(tag = "kind")]`-Enum — die
 * fünf `SPEC.md` §5 genannten Maskentypen plus die ab Phase 7
 * hinzugekommene KI-generierte Rasterfläche und die ab Phase 11 Schritt 7
 * hinzugekommene Unschärfe-basierte Tiefennäherung (siehe
 * `DECISIONS.md` ADR-0038 — echter „Tiefenbereich" wie in ADR-0032
 * Punkt 3 zurückgestellt bleibt weiterhin nicht Teil dieses Schemas, es
 * gibt in diesem Projekt nirgends echte Tiefendaten). */
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
  | { kind: "LuminanceRange"; range_min: number; range_max: number; feather: number }
  | { kind: "AiGenerated"; ai_kind: AiMaskKind; width: number; height: number; alpha: number[] }
  | { kind: "BlurDepthApprox"; threshold: number };

/** Dekodiert eine Base64-Ein-Kanal-Bitmap (`AiMaskAlphaDto.alpha_base64`,
 * siehe `lib/tauri.ts`) in ein Array von Byte-Werten (`0..=255`) — genau
 * das Format, das `MaskGeometry::AiGenerated.alpha` erwartet (Rusts
 * `Vec<u8>` serialisiert als JSON-Zahlenarray, nicht als Base64-String;
 * Base64 ist nur die kompakte Kodierung für den einmaligen IPC-Transport
 * der Tauri-Antwort). `atob` ist in jedem Tauri-Webview verfügbar (Chrome/
 * WebKit), kein zusätzliches Paket nötig.
 */
export function base64ToByteArray(base64: string): number[] {
  const binary = atob(base64);
  const bytes = new Array<number>(binary.length);
  for (let i = 0; i < binary.length; i += 1) {
    bytes[i] = binary.charCodeAt(i);
  }
  return bytes;
}

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
export type BlendMode = "Normal" | "Multiply" | "SoftLight" | "Color" | "Luminosity" | "Screen";

export const BLEND_MODE_OPTIONS: ReadonlyArray<{ value: BlendMode; label: string }> = [
  { value: "Normal", label: "Normal" },
  { value: "Multiply", label: "Multiplizieren" },
  { value: "SoftLight", label: "Weiches Licht" },
  { value: "Color", label: "Farbe" },
  { value: "Luminosity", label: "Luminanz" },
  { value: "Screen", label: "Negativ multiplizieren" },
];

export type OverlayColor = "Red" | "Green" | "Blue" | "Yellow" | "Magenta";

export const OVERLAY_COLOR_OPTIONS: ReadonlyArray<{ value: OverlayColor; label: string }> = [
  { value: "Red", label: "Rot" },
  { value: "Green", label: "Grün" },
  { value: "Blue", label: "Blau" },
  { value: "Yellow", label: "Gelb" },
  { value: "Magenta", label: "Magenta" },
];

/** Kräftige, auf dunklem wie hellem Bildgrund gut sichtbare Werte je
 * `OverlayColor` — für das Masken-Farbüberlagerung im Viewer (Phase 12
 * Schritt 1, siehe `DECISIONS.md` ADR-0039), nicht für UI-Chrome (dort
 * gelten die Theme-Tokens aus `index.css`). */
export const OVERLAY_COLOR_HEX: Record<OverlayColor, string> = {
  Red: "#ff3b30",
  Green: "#34c759",
  Blue: "#0a84ff",
  Yellow: "#ffd60a",
  Magenta: "#ff2d95",
};

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

export type RadialGradientGeometry = Extract<MaskGeometry, { kind: "RadialGradient" }>;

/** Randpunkte der (ggf. rotierten) Radialverlauf-Ellipse in Bild-
 * Bruchteil-Koordinaten (Phase 12 Schritt 2, siehe `DECISIONS.md`
 * ADR-0039) — exakte Umkehrung von `masks.rs`s `radial_gradient_alpha`-
 * Rotationsformel, damit `MaskOverlay`/`MaskColorOverlay` dieselbe Form
 * zeichnen, die die Pipeline tatsächlich berechnet.
 *
 * **Wichtig:** das ist eine Rotation im *Bruchteilsraum* (x/y je eigener
 * Kantenlänge normiert, wie die gesamte Maskengeometrie in diesem
 * Projekt), keine physische Bildschirm-Rotation — bei einem nicht-
 * quadratischen Foto unterscheiden sich beide sichtbar. Das ist keine
 * Vereinfachung dieser Funktion, sondern spiegelt exakt, wie
 * `radial_gradient_alpha` selbst rechnet. */
export function radialGradientBoundaryPoints(geometry: RadialGradientGeometry, steps = 48): MaskPoint[] {
  const angle = (geometry.angle_degrees * Math.PI) / 180;
  const cosA = Math.cos(angle);
  const sinA = Math.sin(angle);
  const points: MaskPoint[] = [];
  for (let i = 0; i < steps; i += 1) {
    const t = (i / steps) * Math.PI * 2;
    const localX = geometry.radius_x * Math.cos(t);
    const localY = geometry.radius_y * Math.sin(t);
    points.push({
      x: geometry.center_x + localX * cosA - localY * sinA,
      y: geometry.center_y + localX * sinA + localY * cosA,
    });
  }
  return points;
}

/** Ziehgriff-Positionen für die unabhängigen Radius-Achsen + den
 * Rotations-Griff (Phase 12 Schritt 2) — dieselbe Parametrisierung wie
 * [`radialGradientBoundaryPoints`] an den Stellen `t=0`/`t=π/2`, der
 * Rotations-Griff sitzt etwas weiter außen auf dem `t=0`-Strahl. */
export function radialGradientAxisHandlePositions(geometry: RadialGradientGeometry): {
  radiusX: MaskPoint;
  radiusY: MaskPoint;
  rotation: MaskPoint;
} {
  const angle = (geometry.angle_degrees * Math.PI) / 180;
  const cosA = Math.cos(angle);
  const sinA = Math.sin(angle);
  const rotationDist = geometry.radius_x + 0.06;
  return {
    radiusX: { x: geometry.center_x + geometry.radius_x * cosA, y: geometry.center_y + geometry.radius_x * sinA },
    radiusY: { x: geometry.center_x - geometry.radius_y * sinA, y: geometry.center_y + geometry.radius_y * cosA },
    rotation: { x: geometry.center_x + rotationDist * cosA, y: geometry.center_y + rotationDist * sinA },
  };
}

/** Obere Tonwerthälfte (Lichter) als plausibler Startzustand — dieselbe
 * Konvention wie Lightrooms Luminanzbereichs-Maske, die üblicherweise
 * zuerst auf „Lichter" steht. */
export function defaultLuminanceRangeGeometry(): MaskGeometry {
  return { kind: "LuminanceRange", range_min: 0.5, range_max: 1, feather: 0.1 };
}

/** Mittlerer Schwellwert als plausibler Startzustand — siehe
 * `MaskGeometry`s Moduldoku zur Unschärfe-basierten Tiefennäherung
 * (Phase 11 Schritt 7). */
export function defaultBlurDepthApproxGeometry(): MaskGeometry {
  return { kind: "BlurDepthApprox", threshold: 0.5 };
}

export interface MaskGroup {
  id: string;
  name: string;
  visible: boolean;
}

/** Die Masken, die tatsächlich wirken/angezeigt werden sollen: `mask.visible`
 * UND (keine Gruppe zugeordnet ODER die zugeordnete Gruppe ist selbst
 * sichtbar) — Spiegelbild von `apx_pipeline::stages::masks::visible_masks`
 * (siehe dessen Moduldoku), hier für die clientseitige Masken-
 * Farbüberlagerung (Phase 12 Schritt 1) statt einer Pipeline-Anfrage
 * genutzt. */
export function visibleMasks(masks: readonly Mask[], groups: readonly MaskGroup[]): Mask[] {
  return masks.filter((mask) => {
    if (!mask.visible) return false;
    if (mask.group_id === null) return true;
    const group = groups.find((g) => g.id === mask.group_id);
    return group ? group.visible : true;
  });
}

export const MASK_SLIDER_SPECS: readonly SliderSpec[] = [
  { key: "opacity", label: "Maske: Deckkraft", min: 0, max: 100, fineStep: 1, coarseStep: 10, neutral: 100 },
  { key: "feather", label: "Maske: Weichzeichnung", min: 0, max: 100, fineStep: 1, coarseStep: 10, neutral: 0 },
];

// ---- Der vollständige EDL-Payload -------------------------------------------

/** Farbe oder Schwarzweiß (Phase 9 Schritt 5) — spiegelt
 * `apx_pipeline::edl::v3::Treatment`. */
export type Treatment = "Color" | "BlackAndWhite";

/** Acht Luminanzgewichte je Farbton-Band (dieselben acht Bänder wie
 * {@link HslAdjustment}), `100` = unverändert. Siehe `bw_mixer.rs`s
 * Moduldoku für die bewusste Vereinfachung bei den Standardwerten. */
export interface BlackAndWhiteMixerAdjustment {
  red: number;
  orange: number;
  yellow: number;
  green: number;
  aqua: number;
  blue: number;
  purple: number;
  magenta: number;
}

export const NEUTRAL_BW_MIXER: BlackAndWhiteMixerAdjustment = {
  red: 100,
  orange: 100,
  yellow: 100,
  green: 100,
  aqua: 100,
  blue: 100,
  purple: 100,
  magenta: 100,
};

/** Aktivieren/Überspringen je Rendering-Stufe (Phase 9 Schritt 7,
 * Node-Editor) — spiegelt `apx_pipeline::edl::v4::StageEnabled`. Ein
 * Feld je Knoten, in exakt der Reihenfolge, in der `develop::render_rgba8`
 * sie anwendet; `false` reicht die Stufe unverändert durch, verschiebt sie
 * aber nicht in der Kette (siehe `DECISIONS.md` ADR-0035 Punkt 1: fester
 * Graph, kein frei umbaubarer). */
export interface StageEnabled {
  repair: boolean;
  calibration: boolean;
  basic: boolean;
  local_contrast: boolean;
  details: boolean;
  hsl_color_mixer: boolean;
  color_grading: boolean;
  lens_corrections: boolean;
  effects: boolean;
  masks: boolean;
  treatment: boolean;
  curves: boolean;
  /** Mehrfachbelichtung/Layer-Compositing (Phase 14 Schritt 3) — läuft
   * nach `curves`, vor `geometry`. */
  composite: boolean;
  /** KI-Tiefenschärfe-Simulator "Virtuelle Blende" (Phase 14 Schritt 8) —
   * läuft nach dem Halation-Kurzschluss, vor `masks` (siehe `develop.rs`s
   * Moduldoku). Vorne in der Deklaration platziert wie auf der Rust-Seite
   * (`edl/v4.rs`s `StageEnabled`), auch wenn die tatsächliche
   * Rendering-Reihenfolge eine andere ist. */
  virtual_aperture: boolean;
  /** KI-Stiltransfer zwischen Fotos (Phase 14 Schritt 9) — läuft nach
   * `composite`, vor `geometry`, im fertig entwickelten sRGB-RGBA8-Bild
   * (siehe `stages::style_transfer`s Moduldoku). */
  style_transfer: boolean;
  sky_replace: boolean;
  /** Verflüssigen (Phase 15 Schritt 3) — läuft nach `sky_replace`, vor
   * `geometry`, im fertig entwickelten sRGB-RGBA8-Bild (siehe
   * `stages::liquify`s Moduldoku). */
  liquify: boolean;
  geometry: boolean;
}

export const NEUTRAL_STAGE_ENABLED: StageEnabled = {
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
  sky_replace: true,
  liquify: true,
  geometry: true,
};

/** Ein einmalig aufgelöstes Foto oder eine Textur, als fertige Bitmap
 * gespeichert (Phase 14 Schritt 3, siehe `DECISIONS.md` ADR-0041) —
 * dasselbe „einmal per Command auflösen, bei jedem Rendern nur noch
 * skalieren"-Muster wie `AiFillPatch`/`CanvasExtensionPatch`. `pixels`
 * ist interleaved RGB (`0..=255`), `bitmap_width * bitmap_height * 3`
 * Zahlen lang. */
export interface CompositeLayerSource {
  bitmap_width: number;
  bitmap_height: number;
  pixels: number[];
}

/** Eine einzelne Ebene für Mehrfachbelichtung/Compositing — Lightroom
 * Classic hat "keine klassischen Ebenen-Kompositionsfähigkeiten wie
 * Photoshop" (siehe `DECISIONS.md` ADR-0041). Wiederverwendet dieselben
 * Blend-Modi wie die Masken-Stufe. `scale`: Bruchteil der Leinwandgröße
 * (`1.0` deckt die Leinwand ab). `offset_x`/`offset_y`: normierte
 * Position (`0.0..=1.0`) des Ebenen-*Mittelpunkts* (`0.5`/`0.5` =
 * zentriert). */
export interface CompositeLayer {
  visible: boolean;
  blend_mode: BlendMode;
  opacity: number;
  scale: number;
  offset_x: number;
  offset_y: number;
  source: CompositeLayerSource;
  /** Blend-If (Phase 15 Schritt 2, Photoshop-exklusiv) — Luminanz-
   * Schwellenwerte des Basis-Pixels, unterhalb/oberhalb derer die Ebene
   * weich ausgeblendet wird. Neutralwerte `0.0`/`1.0` sind ein No-Op. */
  blend_if_shadow_cutoff: number;
  blend_if_highlight_cutoff: number;
}

/** Eine einmalig berechnete Tiefenkarte (Phase 14 Schritt 8, MiDaS v2.1
 * small) — dasselbe „einmal per Command auflösen, bei jedem Rendern nur
 * noch skalieren"-Muster wie `CompositeLayerSource`. `depth_base64` ist
 * base64-kodiertes Graustufen-Rohmaterial (`bitmap_width * bitmap_height`
 * Bytes, `0..=255`, näher = heller) — die Frontend-Seite dekodiert das
 * nicht selbst, sondern reicht es nur unverändert an die Rust-Seite
 * zurück (`VirtualApertureAdjustment.depth_map`), die es beim Rendern
 * bilinear auf die tatsächliche Bildgröße skaliert. */
export interface DepthMapPatch {
  bitmap_width: number;
  bitmap_height: number;
  /** Auf der Rust-Seite `Vec<u8>` (`depth`), hier als base64-String
   * transportiert — siehe `DepthMapDto` in `lib/tauri.ts`. */
  depth: string;
}

/** KI-Tiefenschärfe-Simulator "Virtuelle Blende" (Phase 14 Schritt 8) —
 * spiegelt `apx_pipeline::edl::v4::VirtualApertureAdjustment`.
 * `focus_x`/`focus_y`: normierter Fokuspunkt (`0.0..=1.0`), per Klick ins
 * Bild gesetzt (dasselbe Muster wie `ClickRegion`-KI-Masken). `amount`:
 * `0.0..=1.0`, virtueller Blendenwert (0 = keine Wirkung). Ohne
 * berechnete Tiefenkarte (`depth_map === null`) bleibt die Stufe
 * wirkungslos, selbst bei `amount > 0` (siehe `virtual_aperture.rs`s
 * Moduldoku). */
export interface VirtualApertureAdjustment {
  focus_x: number;
  focus_y: number;
  amount: number;
  depth_map: DepthMapPatch | null;
}

export const NEUTRAL_VIRTUAL_APERTURE: VirtualApertureAdjustment = {
  focus_x: 0.5,
  focus_y: 0.5,
  amount: 0.0,
  depth_map: null,
};

/** Einmalig berechnetes Stiltransfer-Ergebnis (Phase 14 Schritt 9) —
 * dasselbe „einmal per Command auflösen, bei jedem Rendern nur noch
 * skalieren"-Muster wie `CompositeLayerSource`/`DepthMapPatch`.
 * `pixels` ist interleaved RGB (`0..=255`), base64-kodiert — die
 * Frontend-Seite dekodiert das nicht selbst, sondern reicht es nur
 * unverändert an die Rust-Seite zurück (`StyleTransferAdjustment.patch`),
 * die es beim Rendern bilinear auf die tatsächliche Bildgröße skaliert. */
export interface StyleTransferPatch {
  bitmap_width: number;
  bitmap_height: number;
  /** Auf der Rust-Seite `Vec<u8>` (`pixels`), hier als base64-String
   * transportiert — siehe `StyleTransferPatchDto` in `lib/tauri.ts`. */
  pixels: string;
}

/** KI-Stiltransfer zwischen Fotos (Phase 14 Schritt 9) — spiegelt
 * `apx_pipeline::edl::v4::StyleTransferAdjustment`. `amount`:
 * `0.0..=1.0`, blendet linear zwischen unverändertem Bild (`0.0`) und
 * vollem Stiltransfer-Ergebnis (`1.0`). Ohne berechnetes Ergebnis
 * (`patch === null`) bleibt die Stufe wirkungslos, selbst bei
 * `amount > 0` (siehe `style_transfer.rs`s Moduldoku) — welcher der
 * fünf festen Stile zuletzt gewählt wurde, steckt bereits im
 * berechneten `patch` und ist hier nicht separat nachgeführt. */
export interface StyleTransferAdjustment {
  amount: number;
  patch: StyleTransferPatch | null;
}

export const NEUTRAL_STYLE_TRANSFER: StyleTransferAdjustment = {
  amount: 0.0,
  patch: null,
};

/** Die fünf real lizenzierten, festen Stile (`onnx/models`, MIT,
 * `fast_neural_style`, siehe `apx_ai::style_transfer::StyleKind`) — kein
 * beliebiges Referenzfoto als Stilvorlage (siehe `DECISIONS.md`
 * ADR-0041 Nachtrag IX für die Begründung). `id` spiegelt
 * `StyleKind::id()` exakt. */
export interface StyleTransferStyle {
  id: string;
  label: string;
}

export const STYLE_TRANSFER_STYLES: readonly StyleTransferStyle[] = [
  { id: "candy", label: "Candy" },
  { id: "mosaic", label: "Mosaik" },
  { id: "rain-princess", label: "Rain Princess" },
  { id: "udnie", label: "Udnie" },
  { id: "pointilism", label: "Pointillismus" },
] as const;

/** Ein Knoten im Node-Editor — Anzeigereihenfolge identisch zur
 * tatsächlichen Rendering-Reihenfolge (siehe `develop.rs`s Moduldoku).
 * `panel` benennt das bereits bestehende Bedienfeld, das ein Klick auf
 * den Knoten öffnet — kein neues Panel je Knoten, der Node-Editor
 * navigiert nur zum jeweils zuständigen bestehenden Regler-Abschnitt. */
export interface StageNodeSpec {
  key: keyof StageEnabled;
  label: string;
}

export const STAGE_NODE_SPECS: readonly StageNodeSpec[] = [
  { key: "repair", label: "Reparatur" },
  { key: "calibration", label: "Kalibrierung" },
  { key: "basic", label: "Grundeinstellungen" },
  { key: "local_contrast", label: "Textur/Klarheit" },
  { key: "details", label: "Details" },
  { key: "hsl_color_mixer", label: "HSL/Farbmischer" },
  { key: "color_grading", label: "Color Grading" },
  { key: "lens_corrections", label: "Objektivkorrekturen" },
  { key: "effects", label: "Effekte" },
  { key: "masks", label: "Masken" },
  { key: "treatment", label: "Behandlung (SW-Mixer)" },
  { key: "curves", label: "Kurven" },
  { key: "composite", label: "Compositing" },
  { key: "virtual_aperture", label: "Virtuelle Blende" },
  { key: "style_transfer", label: "Stiltransfer" },
  { key: "sky_replace", label: "Himmelsaustausch" },
  { key: "liquify", label: "Verflüssigen" },
  { key: "geometry", label: "Geometrie" },
] as const;

/** Spiegelt `apx_pipeline::edl::v4::EdlV4` — der komplette Inhalt eines
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
  treatment: Treatment;
  bw_mixer: BlackAndWhiteMixerAdjustment;
  stage_enabled: StageEnabled;
  composite_layers: CompositeLayer[];
  virtual_aperture: VirtualApertureAdjustment;
  style_transfer: StyleTransferAdjustment;
  sky_replace: SkyReplacePatch | null;
  liquify_strokes: LiquifyStroke[];
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
    treatment: "Color",
    bw_mixer: NEUTRAL_BW_MIXER,
    stage_enabled: NEUTRAL_STAGE_ENABLED,
    composite_layers: [],
    virtual_aperture: NEUTRAL_VIRTUAL_APERTURE,
    style_transfer: NEUTRAL_STYLE_TRANSFER,
    sky_replace: null,
    liquify_strokes: [],
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

// ---- Schwarzweiß-Mixer (Phase 9 Schritt 5) ----------------------------------

/** Dieselben acht Bänder/Beschriftungen wie {@link HSL_BAND_TABS} — der
 * Schwarzweiß-Mixer gewichtet nach demselben Farbton-Schema. */
export const BW_MIXER_BAND_TABS: ReadonlyArray<{ key: keyof BlackAndWhiteMixerAdjustment; label: string }> = [
  { key: "red", label: "Rot" },
  { key: "orange", label: "Orange" },
  { key: "yellow", label: "Gelb" },
  { key: "green", label: "Grün" },
  { key: "aqua", label: "Aqua" },
  { key: "blue", label: "Blau" },
  { key: "purple", label: "Lila" },
  { key: "magenta", label: "Magenta" },
] as const;

export const BW_MIXER_SLIDER_SPEC: SliderSpec = { key: "weight", label: "Gewicht", min: 0, max: 200, fineStep: 1, coarseStep: 10, neutral: 100 };

export function writeBwMixerField(mixer: BlackAndWhiteMixerAdjustment, band: keyof BlackAndWhiteMixerAdjustment, value: number): void {
  mixer[band] = value;
}

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

export interface SkyReplacePatch {
  bitmap_width: number;
  bitmap_height: number;
  pixels: string;
}
