import { useEffect, useRef, useState } from "react";
import { useShallow } from "zustand/react/shallow";

import {
  ASPECT_RATIO_PRESETS,
  BASIC_SLIDER_SPECS,
  BLEND_MODE_OPTIONS,
  BW_MIXER_BAND_TABS,
  BW_MIXER_SLIDER_SPEC,
  CALIBRATION_PRIMARY_ROWS,
  CAMERA_PROFILE_OPTIONS,
  COLOR_GRADING_WHEEL_TABS,
  COLOR_MIXER_REGION_SLIDER_SPECS,
  COLOR_NR_SLIDER_SPECS,
  CURVE_CHANNEL_TABS,
  GRAIN_SLIDER_SPECS,
  GRID_OVERLAY_OPTIONS,
  HALATION_SLIDER_SPECS,
  HSL_BAND_SLIDER_SPECS,
  HSL_BAND_TABS,
  LENS_CA_SLIDER_SPECS,
  LENS_PROFILE_OPTIONS,
  LENS_SLIDER_SPECS,
  LUMINANCE_NR_SLIDER_SPECS,
  MANUAL_TRANSFORM_SLIDER_SPECS,
  MAX_COLOR_MIXER_REGIONS,
  POST_VIGNETTE_SLIDER_SPECS,
  readBasicField,
  SHARPEN_SLIDER_SPECS,
  STAGE_NODE_SPECS,
  UPRIGHT_MODE_OPTIONS,
  WHITE_BALANCE_PRESETS,
  type BlackAndWhiteMixerAdjustment,
  type BlendMode,
  type ColorMixerRegion,
  type CurvesAdjustment,
  type DetailsSliderKey,
  type EffectsAdjustment,
  type GridOverlay,
  type GuidedLine,
  type HslAdjustment,
  type LensCorrectionAdjustment,
  type LiquifyMode,
  type ManualTransform,
  type RepairLayer,
  type RepairMode,
  type SliderSpec,
  type StageEnabled,
} from "../lib/edl";
import { matchesBinding } from "../lib/keybindings";
import { PRESET_SECTION_KEYS, PRESET_SECTION_LABELS, type PresetSectionKey } from "../lib/presets";
import { SOFT_PROOF_INTENT_LABELS, SOFT_PROOF_PROFILE_LABELS, type SoftProofIntent, type SoftProofProfile } from "../lib/softProof";
import { pickFilePath } from "../lib/tauri";
import { selectActivePhotos, useAppStore } from "../store";
import { ColorHarmonyWheel } from "./ColorHarmonyWheel";
import { ColorWheel } from "./ColorWheel";
import { CurveEditor } from "./CurveEditor";
import { DevelopSlider } from "./DevelopSlider";
import { LensCalibrationDialog } from "./LensCalibrationDialog";
import { CanvasExtendDialog } from "./CanvasExtendDialog";
import { ContentAwareScaleDialog } from "./ContentAwareScaleDialog";
import type { FrequencyViewMode } from "../lib/frequencySeparation";
import { PaletteFrame } from "./PaletteFrame";
import { SavePresetDialog } from "./SavePresetDialog";
import { SkinSmoothingPanel } from "./SkinSmoothingPanel";
import { SkyReplacePanel } from "./SkyReplacePanel";
import { StyleTransferPanel } from "./StyleTransferPanel";
import { VirtualAperturePanel } from "./VirtualAperturePanel";

// ---- Reparatur (Klonen/Reparieren) — Phase 4 Schritt 12 --------------------
//
// `radius`/`feather` sind im EDL Bruchteile der Bildbreite (0..1, siehe
// `repair.rs`s Moduldoku), hier für eine handlichere Regler-Skala als
// Prozent der Bildbreite dargestellt; `opacity` ist 0..1, hier als
// Prozent. Keine `SliderSpec.key`-basierte generische Feld-Zuordnung wie
// bei den EDL-Reglern nötig, da diese drei nur den *nächsten* Strich
// betreffen (siehe `store/index.ts`s `repairDraft*`-Felder), kein EDL-Feld.
const REPAIR_RADIUS_SPEC: SliderSpec = { key: "radius", label: "Radius (% der Bildbreite)", min: 1, max: 50, fineStep: 0.5, coarseStep: 5, neutral: 5 };
const REPAIR_FEATHER_SPEC: SliderSpec = { key: "feather", label: "Weiche Kante (% der Bildbreite)", min: 0, max: 25, fineStep: 0.5, coarseStep: 2, neutral: 2 };
const REPAIR_OPACITY_SPEC: SliderSpec = { key: "opacity", label: "Deckkraft (%)", min: 0, max: 100, fineStep: 1, coarseStep: 10, neutral: 100 };

// ---- Verflüssigen (Liquify, Phase 15 Schritt 3, siehe DECISIONS.md
// ADR-0042 — Photoshop-exklusiv, Lightroom hat kein Verformungswerkzeug) --
const LIQUIFY_RADIUS_SPEC: SliderSpec = { key: "radius", label: "Radius (% der Bildbreite)", min: 2, max: 50, fineStep: 0.5, coarseStep: 5, neutral: 15 };
const LIQUIFY_STRENGTH_SPEC: SliderSpec = { key: "strength", label: "Stärke (%)", min: 1, max: 100, fineStep: 1, coarseStep: 10, neutral: 50 };

// ---- Mehrfachbelichtung/Layer-Compositing — Phase 14 Schritt 3, siehe
// DECISIONS.md ADR-0041 (Lightroom Classic hat "keine klassischen
// Ebenen-Kompositionsfähigkeiten wie Photoshop") --------------------------
const COMPOSITE_OPACITY_SPEC: SliderSpec = { key: "opacity", label: "Deckkraft (%)", min: 0, max: 100, fineStep: 1, coarseStep: 10, neutral: 100 };
const COMPOSITE_SCALE_SPEC: SliderSpec = { key: "scale", label: "Skalierung (%)", min: 10, max: 300, fineStep: 1, coarseStep: 10, neutral: 100 };
const COMPOSITE_OFFSET_X_SPEC: SliderSpec = { key: "offset_x", label: "Position X (%)", min: 0, max: 100, fineStep: 1, coarseStep: 10, neutral: 50 };
const COMPOSITE_OFFSET_Y_SPEC: SliderSpec = { key: "offset_y", label: "Position Y (%)", min: 0, max: 100, fineStep: 1, coarseStep: 10, neutral: 50 };
const COMPOSITE_BLEND_IF_SHADOW_SPEC: SliderSpec = { key: "blend_if_shadow_cutoff", label: "Blend-If Schatten (%)", min: 0, max: 100, fineStep: 1, coarseStep: 5, neutral: 0 };
const COMPOSITE_BLEND_IF_HIGHLIGHT_SPEC: SliderSpec = { key: "blend_if_highlight_cutoff", label: "Blend-If Lichter (%)", min: 0, max: 100, fineStep: 1, coarseStep: 5, neutral: 100 };

// ---- Node-Editor (Phase 9 Schritt 7, siehe DECISIONS.md ADR-0035) ---------
//
// Kein `@xyflow/react`-Graph-Canvas: die Rendering-Reihenfolge ist fest
// (siehe `develop.rs`s Moduldoku), ein frei zieh-/verbindbarer Knotengraph
// würde also nur Fähigkeiten *vortäuschen*, die es nicht gibt (Umsortieren,
// neue Verbindungen). Diese geordnete Liste zeigt exakt dieselbe
// Information — ein Knoten je Stufe, feste Reihenfolge, Ein/Aus-Schalter,
// „Öffnen" springt zum zugehörigen Regler-Abschnitt — ohne diese
// Erwartung zu wecken (bewusste Vereinfachung gegenüber der ursprünglichen
// PLAN.md-Formulierung).
const STAGE_ANCHOR_IDS: Record<keyof StageEnabled, string> = {
  repair: "stage-repair",
  calibration: "stage-calibration",
  basic: "stage-basic",
  // Textur/Klarheit sind Regler innerhalb desselben Grundeinstellungen-
  // Reglersatzes wie `basic` (siehe `EdlPayload.basic`) — derselbe Anker.
  local_contrast: "stage-basic",
  details: "stage-details",
  hsl_color_mixer: "stage-hsl_color_mixer",
  color_grading: "stage-color_grading",
  lens_corrections: "stage-lens_corrections",
  effects: "stage-effects",
  masks: "stage-masks",
  treatment: "stage-treatment",
  curves: "stage-curves",
  composite: "stage-composite",
  virtual_aperture: "stage-virtual_aperture",
  style_transfer: "stage-style_transfer",
  skin_smoothing: "stage-skin_smoothing",
  sky_replace: "stage-sky_replace",
  liquify: "stage-liquify",
  geometry: "stage-geometry",
};

function openStageAnchor(key: keyof StageEnabled): void {
  document.getElementById(STAGE_ANCHOR_IDS[key])?.scrollIntoView({ behavior: "smooth", block: "start" });
}

const LIQUIFY_MODE_OPTIONS: ReadonlyArray<{ value: LiquifyMode; label: string }> = [
  { value: "Push", label: "Schieben" },
  { value: "Twirl", label: "Verwirbeln" },
  { value: "Pucker", label: "Stauchen" },
  { value: "Bloat", label: "Aufblähen" },
];

const REPAIR_MODE_OPTIONS: ReadonlyArray<{ value: RepairMode; label: string }> = [
  { value: "Clone", label: "Klonen" },
  { value: "Heal", label: "Reparieren" },
  // Inhaltsbasiertes Füllen (Phase 7, ADR-0033 Punkt 4): kein
  // Quellpunkt nötig, siehe RepairOverlay.tsx/store/index.ts's
  // addRepairStroke.
  { value: "ContentAwareFill", label: "Inhaltsbasiert füllen" },
  // KI-Ausfüllen (Phase 13 Schritt 1, ADR-0040): ebenfalls kein
  // Quellpunkt, braucht aber zusätzlich einen expliziten „Anwenden"-Klick
  // (`runAiInpaintForStroke`) — der Strich bleibt bis dahin ein No-Op.
  { value: "AiInpaint", label: "KI-Ausfüllen" },
];

// Frequenztrennung (Phase 14 Schritt 2, ADR-0041): lässt Klonen/
// Reparieren/Inhaltsbasiert-füllen/KI-Ausfüllen gezielt nur auf Ton/
// Farbe (Tieffrequenz) oder Textur/Poren/Kanten (Hochfrequenz) statt
// direkt auf dem vollen Bild wirken.
const REPAIR_LAYER_OPTIONS: ReadonlyArray<{ value: RepairLayer; label: string }> = [
  { value: "Normal", label: "Ganzes Bild" },
  { value: "LowFrequency", label: "Nur Tieffrequenz (Ton/Farbe)" },
  { value: "HighFrequency", label: "Nur Hochfrequenz (Textur/Poren)" },
];

// ---- Preset-Stärke (Phase 5 Schritt 5, siehe SPEC.md §3.5) -----------------
const PRESET_STRENGTH_SPEC: SliderSpec = { key: "strength", label: "Stärke (%)", min: 0, max: 200, fineStep: 1, coarseStep: 10, neutral: 100 };

const WHITE_BALANCE_KEYS = new Set(["temp_shift_kelvin", "tint_shift"]);

/** Die vier numerischen Objektivkorrektur-Regler (Phase 4 Schritt 9,
 * ohne `manual_transform`, `profile_id`, `auto_ca`, `upright_mode`,
 * `guided_lines`). */
type LensNumericKey = keyof Pick<
  LensCorrectionAdjustment,
  "ca_red_cyan" | "ca_blue_yellow" | "vignette_amount" | "distortion_amount"
>;

/** Die vier Zahlenfelder einer Guided-Hilfslinie. */
const GUIDED_LINE_FIELDS: ReadonlyArray<keyof GuidedLine> = ["x1", "y1", "x2", "y2"];

/**
 * Das Entwickeln-Panel: die sieben Grundeinstellungs-Regler (Weißabgleich
 * zählt als einer, hat aber zwei Zahlenwerte — siehe `SPEC.md` §5) plus
 * Rückgängig/Wiederholen. Nur sichtbar, wenn `developPanelOpen` (siehe
 * `store/index.ts`, umgeschaltet über den "Entwickeln"-Knopf in
 * `Header.tsx`).
 *
 * Undo/Redo laufen direkt über `apx-catalog`s `edit_history` (siehe
 * `crates/apx-app/src/commands.rs`), nicht über eine separate
 * Frontend-Bibliothek wie ursprünglich in ADR-0018 vorgesehen — siehe die
 * Korrektur-Notiz dort: ein zusätzliches, lose synchronisiertes
 * Verlaufssystem im Frontend hätte keinen erkennbaren Vorteil gegenüber
 * der ohnehin schon vollständig getesteten Backend-Historie gebracht,
 * und Tauris IPC ist lokal schnell genug, um pro Undo/Redo-Klick einen
 * Roundtrip zu rechtfertigen.
 */
export function DevelopPanel() {
  const open = useAppStore((s) => s.developPanelOpen);
  const basic = useAppStore((s) => s.developEdl.basic);
  const setBasicField = useAppStore((s) => s.setBasicField);
  const commitDevelopEdit = useAppStore((s) => s.commitDevelopEdit);
  const undoDevelop = useAppStore((s) => s.undoDevelop);
  const redoDevelop = useAppStore((s) => s.redoDevelop);
  const toggleHistoryDialog = useAppStore((s) => s.toggleHistoryDialog);
  const selectedPhotoId = useAppStore((s) => s.selectedPhotoId);
  const lastLatencyMs = useAppStore((s) => s.developLastLatencyMs);
  const snapshots = useAppStore((s) => s.snapshots);
  const saveSnapshot = useAppStore((s) => s.saveSnapshot);
  const renameSnapshotAction = useAppStore((s) => s.renameSnapshotAction);
  const removeSnapshot = useAppStore((s) => s.removeSnapshot);
  const restoreSnapshot = useAppStore((s) => s.restoreSnapshot);
  const beforeAfterMode = useAppStore((s) => s.beforeAfterMode);
  const setBeforeAfterMode = useAppStore((s) => s.setBeforeAfterMode);
  const referenceViewActive = useAppStore((s) => s.referenceViewActive);
  const toggleReferenceView = useAppStore((s) => s.toggleReferenceView);
  const referencePhotoId = useAppStore((s) => s.referencePhotoId);
  const setReferencePhotoId = useAppStore((s) => s.setReferencePhotoId);
  const softProofActive = useAppStore((s) => s.softProofActive);
  const toggleSoftProof = useAppStore((s) => s.toggleSoftProof);
  const softProofProfile = useAppStore((s) => s.softProofProfile);
  const setSoftProofProfile = useAppStore((s) => s.setSoftProofProfile);
  const softProofCustomIccPath = useAppStore((s) => s.softProofCustomIccPath);
  const setSoftProofCustomIccPath = useAppStore((s) => s.setSoftProofCustomIccPath);
  const softProofIntent = useAppStore((s) => s.softProofIntent);
  const setSoftProofIntent = useAppStore((s) => s.setSoftProofIntent);
  const softProofGamutWarning = useAppStore((s) => s.softProofGamutWarning);
  const toggleSoftProofGamutWarning = useAppStore((s) => s.toggleSoftProofGamutWarning);
  const softProofPaperWhite = useAppStore((s) => s.softProofPaperWhite);
  const toggleSoftProofPaperWhite = useAppStore((s) => s.toggleSoftProofPaperWhite);

  async function handlePickSoftProofIccFile() {
    const path = await pickFilePath("ICC-Profil", ["icc", "icm"]);
    if (path) setSoftProofCustomIccPath(path);
  }
  const activePhotos = useAppStore(useShallow(selectActivePhotos));
  const otherPhotosForReference = activePhotos.filter((p) => p.id !== selectedPhotoId);
  const compositeLayers = useAppStore((s) => s.developEdl.composite_layers);
  const compositeLayerLoading = useAppStore((s) => s.compositeLayerLoading);
  const addCompositeLayerFromPhoto = useAppStore((s) => s.addCompositeLayerFromPhoto);
  const addCompositeLayerFromTexture = useAppStore((s) => s.addCompositeLayerFromTexture);
  const removeCompositeLayer = useAppStore((s) => s.removeCompositeLayer);
  const setCompositeLayerField = useAppStore((s) => s.setCompositeLayerField);
  const [compositeSourcePhotoId, setCompositeSourcePhotoId] = useState("");
  const copiedEdlSubset = useAppStore((s) => s.copiedEdlSubset);
  const copyDevelopSettings = useAppStore((s) => s.copyDevelopSettings);
  const pasteDevelopSettings = useAppStore((s) => s.pasteDevelopSettings);
  const lastDevelopPhotoId = useAppStore((s) => s.lastDevelopPhotoId);
  const applyPreviousSettings = useAppStore((s) => s.applyPreviousSettings);
  const syncSettingsToSelection = useAppStore((s) => s.syncSettingsToSelection);
  const autoSyncActive = useAppStore((s) => s.autoSyncActive);
  const toggleAutoSync = useAppStore((s) => s.toggleAutoSync);
  const multiSelectedIds = useAppStore((s) => s.multiSelectedIds);
  const [workflowSections, setWorkflowSections] = useState<Set<PresetSectionKey>>(new Set(PRESET_SECTION_KEYS));
  const otherSelectedCount = selectedPhotoId && multiSelectedIds.includes(selectedPhotoId) ? multiSelectedIds.length - 1 : 0;

  function toggleWorkflowSection(key: PresetSectionKey) {
    setWorkflowSections((prev) => {
      const next = new Set(prev);
      if (next.has(key)) next.delete(key);
      else next.add(key);
      return next;
    });
  }
  const wbPickerActive = useAppStore((s) => s.wbPickerActive);
  const toggleWbPicker = useAppStore((s) => s.toggleWbPicker);
  const applyWhiteBalancePreset = useAppStore((s) => s.applyWhiteBalancePreset);
  const curves = useAppStore((s) => s.developEdl.curves);
  const setCurveChannel = useAppStore((s) => s.setCurveChannel);
  const [activeCurveChannel, setActiveCurveChannel] = useState<keyof CurvesAdjustment>("rgb");
  const hsl = useAppStore((s) => s.developEdl.hsl);
  const setHslBandField = useAppStore((s) => s.setHslBandField);
  const [activeHslBand, setActiveHslBand] = useState<keyof HslAdjustment>("red");
  const treatment = useAppStore((s) => s.developEdl.treatment);
  const setTreatment = useAppStore((s) => s.setTreatment);
  const bwMixer = useAppStore((s) => s.developEdl.bw_mixer);
  const setBwMixerField = useAppStore((s) => s.setBwMixerField);
  const [activeBwMixerBand, setActiveBwMixerBand] = useState<keyof BlackAndWhiteMixerAdjustment>("red");
  const stageEnabled = useAppStore((s) => s.developEdl.stage_enabled);
  const toggleStage = useAppStore((s) => s.toggleStage);
  const enhanceRunning = useAppStore((s) => s.enhanceRunning);
  const enhanceStatus = useAppStore((s) => s.enhanceStatus);
  const runDenoise = useAppStore((s) => s.runDenoise);
  const runUpscale = useAppStore((s) => s.runUpscale);
  const runConvertToDng = useAppStore((s) => s.runConvertToDng);
  const colorMixer = useAppStore((s) => s.developEdl.color_mixer);
  const colorMixerPickerActive = useAppStore((s) => s.colorMixerPickerActive);
  const toggleColorMixerPicker = useAppStore((s) => s.toggleColorMixerPicker);
  const removeColorMixerRegion = useAppStore((s) => s.removeColorMixerRegion);
  const updateColorMixerRegion = useAppStore((s) => s.updateColorMixerRegion);
  const [selectedRegionIndex, setSelectedRegionIndex] = useState<number | null>(null);
  const previousRegionCount = useRef(colorMixer.regions.length);
  const [savePresetOpen, setSavePresetOpen] = useState(false);
  const presetStrengthContext = useAppStore((s) => s.presetStrengthContext);
  const setPresetStrength = useAppStore((s) => s.setPresetStrength);
  const commitPresetStrength = useAppStore((s) => s.commitPresetStrength);
  const dismissPresetStrengthContext = useAppStore((s) => s.dismissPresetStrengthContext);
  const colorGrading = useAppStore((s) => s.developEdl.color_grading);
  const setColorGradingWheel = useAppStore((s) => s.setColorGradingWheel);
  const setColorGradingBalance = useAppStore((s) => s.setColorGradingBalance);
  const setColorGradingBlending = useAppStore((s) => s.setColorGradingBlending);
  const calibration = useAppStore((s) => s.developEdl.calibration);
  const setCalibrationPrimaryField = useAppStore((s) => s.setCalibrationPrimaryField);
  const setCalibrationShadowTint = useAppStore((s) => s.setCalibrationShadowTint);
  const setCalibrationCameraProfile = useAppStore((s) => s.setCalibrationCameraProfile);
  const importDcpProfile = useAppStore((s) => s.importDcpProfile);
  const clearDcpProfile = useAppStore((s) => s.clearDcpProfile);
  const dcpProfileImporting = useAppStore((s) => s.dcpProfileImporting);
  const details = useAppStore((s) => s.developEdl.details);
  const setDetailsField = useAppStore((s) => s.setDetailsField);
  const setDetailsUseDeconvolutionSharpen = useAppStore((s) => s.setDetailsUseDeconvolutionSharpen);
  const lensCorrections = useAppStore((s) => s.developEdl.lens_corrections);
  const setLensCorrectionField = useAppStore((s) => s.setLensCorrectionField);
  const setLensCorrectionManualTransformField = useAppStore((s) => s.setLensCorrectionManualTransformField);
  const setLensCorrectionProfile = useAppStore((s) => s.setLensCorrectionProfile);
  const manuallyDetectLensProfile = useAppStore((s) => s.manuallyDetectLensProfile);
  const setLensCorrectionCustomDistortionK1 = useAppStore((s) => s.setLensCorrectionCustomDistortionK1);
  const setLensCalibrationDialogOpen = useAppStore((s) => s.setLensCalibrationDialogOpen);
  const setCanvasExtendDialogOpen = useAppStore((s) => s.setCanvasExtendDialogOpen);
  const setContentAwareScaleDialogOpen = useAppStore((s) => s.setContentAwareScaleDialogOpen);
  const setLensCorrectionAutoCa = useAppStore((s) => s.setLensCorrectionAutoCa);
  const setLensCorrectionUprightMode = useAppStore((s) => s.setLensCorrectionUprightMode);
  const runUprightAutoDetect = useAppStore((s) => s.runUprightAutoDetect);
  const uprightDetectLoading = useAppStore((s) => s.uprightDetectLoading);
  const setLensCorrectionGuidedLineField = useAppStore((s) => s.setLensCorrectionGuidedLineField);
  const effects = useAppStore((s) => s.developEdl.effects);
  const setEffectsField = useAppStore((s) => s.setEffectsField);
  const geometry = useAppStore((s) => s.developEdl.geometry);
  const geometryCropActive = useAppStore((s) => s.geometryCropActive);
  const toggleGeometryCropActive = useAppStore((s) => s.toggleGeometryCropActive);
  const setGeometryAngle = useAppStore((s) => s.setGeometryAngle);
  const setGeometryAspectRatio = useAppStore((s) => s.setGeometryAspectRatio);
  const setGeometryOverlay = useAppStore((s) => s.setGeometryOverlay);
  const setGeometryAutoHorizon = useAppStore((s) => s.setGeometryAutoHorizon);
  const repairStrokes = useAppStore((s) => s.developEdl.repair);
  const repairActive = useAppStore((s) => s.repairActive);
  const toggleRepairActive = useAppStore((s) => s.toggleRepairActive);
  const contentAwareMoveActive = useAppStore((s) => s.contentAwareMoveActive);
  const toggleContentAwareMoveTool = useAppStore((s) => s.toggleContentAwareMoveTool);
  const contentAwareMoveRect = useAppStore((s) => s.contentAwareMoveRect);
  const contentAwareMoveLoading = useAppStore((s) => s.contentAwareMoveLoading);
  const liquifyActive = useAppStore((s) => s.liquifyActive);
  const toggleLiquifyActive = useAppStore((s) => s.toggleLiquifyActive);
  const liquifyDraftMode = useAppStore((s) => s.liquifyDraftMode);
  const setLiquifyDraftMode = useAppStore((s) => s.setLiquifyDraftMode);
  const liquifyDraftRadius = useAppStore((s) => s.liquifyDraftRadius);
  const liquifyDraftStrength = useAppStore((s) => s.liquifyDraftStrength);
  const setLiquifyDraftField = useAppStore((s) => s.setLiquifyDraftField);
  const liquifyStrokes = useAppStore((s) => s.developEdl.liquify_strokes);
  const removeLiquifyStroke = useAppStore((s) => s.removeLiquifyStroke);
  const repairDraftMode = useAppStore((s) => s.repairDraftMode);
  const setRepairDraftMode = useAppStore((s) => s.setRepairDraftMode);
  const repairDraftLayer = useAppStore((s) => s.repairDraftLayer);
  const setRepairDraftLayer = useAppStore((s) => s.setRepairDraftLayer);
  const frequencyViewMode = useAppStore((s) => s.frequencyViewMode);
  const setFrequencyViewMode = useAppStore((s) => s.setFrequencyViewMode);
  const repairDraftRadius = useAppStore((s) => s.repairDraftRadius);
  const repairDraftFeather = useAppStore((s) => s.repairDraftFeather);
  const repairDraftOpacity = useAppStore((s) => s.repairDraftOpacity);
  const setRepairDraftField = useAppStore((s) => s.setRepairDraftField);
  const repairPendingSource = useAppStore((s) => s.repairPendingSource);
  const cancelRepairSource = useAppStore((s) => s.cancelRepairSource);
  const removeRepairStroke = useAppStore((s) => s.removeRepairStroke);
  const runAiInpaintForStroke = useAppStore((s) => s.runAiInpaintForStroke);
  const aiInpaintLoadingIndex = useAppStore((s) => s.aiInpaintLoadingIndex);
  const aiSettings = useAppStore((s) => s.aiSettings);
  const loadAiSettings = useAppStore((s) => s.loadAiSettings);
  const downloadInpaintingModel = useAppStore((s) => s.downloadInpaintingModel);
  const inpaintingModelDownloading = useAppStore((s) => s.inpaintingModelDownloading);
  // Reparatur-Erweiterungen (Phase 7 Schritt 3, siehe DECISIONS.md ADR-0033).
  const autoSourceModeActive = useAppStore((s) => s.autoSourceModeActive);
  const toggleAutoSourceMode = useAppStore((s) => s.toggleAutoSourceMode);
  const repairSourceSuggestionLoading = useAppStore((s) => s.repairSourceSuggestionLoading);
  const sensorSpotCandidates = useAppStore((s) => s.sensorSpotCandidates);
  const sensorSpotsLoading = useAppStore((s) => s.sensorSpotsLoading);
  const detectSensorSpotsForCurrentPhoto = useAppStore((s) => s.detectSensorSpotsForCurrentPhoto);
  const clearSensorSpots = useAppStore((s) => s.clearSensorSpots);
  const applySensorSpotAsRepairStroke = useAppStore((s) => s.applySensorSpotAsRepairStroke);

  // Eine per Bildklick neu angelegte Region wird sofort zur Bearbeitung
  // ausgewählt statt dass der Nutzer sie erst in der Liste anklicken muss.
  useEffect(() => {
    if (colorMixer.regions.length > previousRegionCount.current) {
      setSelectedRegionIndex(colorMixer.regions.length - 1);
    }
    previousRegionCount.current = colorMixer.regions.length;
  }, [colorMixer.regions.length]);

  // `aiSettings` (Phase 13 Schritt 1) wird sonst nur beim Öffnen des
  // Presets-Panels geladen (siehe `PresetsPanel.tsx`s
  // `AiPresetGeneratorSection`) — hier zusätzlich einmalig, sobald der
  // Nutzer den KI-Ausfüllen-Modus wählt, damit der Download-Status ohne
  // Umweg über das Presets-Panel sichtbar ist.
  useEffect(() => {
    if (repairDraftMode === "AiInpaint" && !aiSettings) {
      void loadAiSettings();
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [repairDraftMode]);

  useEffect(() => {
    if (!open) return;

    function handleKeyDown(event: KeyboardEvent) {
      const target = event.target as HTMLElement | null;
      if (target && (target.tagName === "INPUT" || target.tagName === "TEXTAREA")) return;

      // Umbelegbar über `lib/keybindings.ts` (Phase 11 Schritt 11, siehe
      // DECISIONS.md ADR-0038) — dieselben "undo"/"redo"-IDs wie
      // `App.tsx`s Bibliotheks-Metadaten-Undo, weil sich beide Kontexte
      // gegenseitig ausschließen (`App.tsx` reicht Ctrl/Cmd+Z nur weiter,
      // wenn dieses Panel geschlossen ist).
      if (matchesBinding(event, "redo")) {
        event.preventDefault();
        void redoDevelop();
      } else if (matchesBinding(event, "undo")) {
        event.preventDefault();
        void undoDevelop();
      }
    }

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [open, undoDevelop, redoDevelop]);

  if (!open) return null;

  const whiteBalanceSpecs = BASIC_SLIDER_SPECS.filter((spec) => WHITE_BALANCE_KEYS.has(spec.key));
  const toneSpecs = BASIC_SLIDER_SPECS.filter((spec) => !WHITE_BALANCE_KEYS.has(spec.key));

  return (
    <>
    <PaletteFrame id="develop" side="right" defaultWidth={288} label="Entwickeln" className="gap-4 border-l border-border bg-bg-raised p-3">
      <div className="flex items-center justify-between">
        <h2 className="text-sm font-semibold text-text-primary">Entwickeln</h2>
        <div className="flex gap-1">
          <button
            type="button"
            onClick={() => setSavePresetOpen(true)}
            disabled={!selectedPhotoId}
            aria-label="Preset speichern"
            title="Aktuelle Einstellungen als Preset speichern"
            className="rounded px-2 py-1 text-xs hover:bg-bg-panel disabled:cursor-not-allowed disabled:opacity-40"
          >
            Preset speichern
          </button>
          <button
            type="button"
            onClick={() => void undoDevelop()}
            disabled={!selectedPhotoId}
            aria-label="Rückgängig"
            title="Rückgängig (Strg/Cmd+Z)"
            className="rounded px-2 py-1 text-xs hover:bg-bg-panel disabled:cursor-not-allowed disabled:opacity-40"
          >
            ↶
          </button>
          <button
            type="button"
            onClick={() => void redoDevelop()}
            disabled={!selectedPhotoId}
            aria-label="Wiederholen"
            title="Wiederholen (Strg/Cmd+Umschalt+Z)"
            className="rounded px-2 py-1 text-xs hover:bg-bg-panel disabled:cursor-not-allowed disabled:opacity-40"
          >
            ↷
          </button>
          <button
            type="button"
            onClick={toggleHistoryDialog}
            disabled={!selectedPhotoId}
            title="Zeitleiste & Verlaufs-Vergleich öffnen (Phase 9 Schritt 7)"
            className="rounded px-2 py-1 text-xs hover:bg-bg-panel disabled:cursor-not-allowed disabled:opacity-40"
          >
            Verlauf
          </button>
        </div>
      </div>

      {!selectedPhotoId && <p className="text-xs text-text-muted">Kein Foto ausgewählt.</p>}

      {selectedPhotoId && lastLatencyMs !== null && (
        <p className="text-xs text-text-muted" title="Ende-zu-Ende-Antwortzeit der letzten Regler-Vorschau (IPC + Dekodierung/Rendern, ohne Neuzeichnen im Browser) — siehe PLAN.md Phase 2 Schritt 7">
          Letztes Rendering: {Math.round(lastLatencyMs)} ms
        </p>
      )}

      {selectedPhotoId && (
        <fieldset className="flex flex-col gap-1 rounded border border-border p-2" aria-label="Node-Editor">
          <legend className="mb-1 px-1 text-xs font-medium text-text-secondary">Node-Editor (Rendering-Stufen)</legend>
          <p className="mb-1 text-[11px] text-text-muted">
            Feste Reihenfolge, keine frei verschiebbaren Knoten — jede Stufe lässt sich ein-/ausschalten und öffnet per Klick den zugehörigen Regler-Abschnitt.
          </p>
          <ol className="flex flex-col gap-0.5">
            {STAGE_NODE_SPECS.map((stage, index) => (
              <li key={stage.key} className="flex items-center gap-2 rounded px-1 py-0.5 text-xs hover:bg-bg-panel">
                <span className="w-4 text-right text-text-muted">{index + 1}</span>
                <label className="flex flex-1 items-center gap-1.5">
                  <input type="checkbox" checked={stageEnabled[stage.key]} onChange={() => toggleStage(stage.key)} aria-label={`${stage.label} aktiv`} />
                  <span className={stageEnabled[stage.key] ? "text-text-primary" : "text-text-muted line-through"}>{stage.label}</span>
                </label>
                <button type="button" onClick={() => openStageAnchor(stage.key)} className="rounded px-1.5 py-0.5 text-[11px] text-text-secondary hover:bg-bg-panel">
                  Öffnen
                </button>
              </li>
            ))}
          </ol>
        </fieldset>
      )}

      {selectedPhotoId && (
        <fieldset className="flex flex-col gap-2">
          <legend className="mb-1 text-xs font-medium text-text-secondary">Schnappschüsse</legend>
          <button
            type="button"
            onClick={() => {
              const name = window.prompt("Schnappschuss benennen");
              if (name) void saveSnapshot(name);
            }}
            className="rounded border border-border px-2 py-1 text-xs text-text-secondary hover:bg-bg-panel"
          >
            + Schnappschuss vom aktuellen Stand
          </button>
          {snapshots.length === 0 && <p className="text-xs text-text-muted">Noch keine Schnappschüsse.</p>}
          <ul className="flex flex-col gap-1">
            {snapshots.map((snapshot) => (
              <li key={snapshot.id} className="flex items-center gap-1.5 rounded border border-border px-2 py-1 text-xs">
                <button type="button" onClick={() => void restoreSnapshot(snapshot.id)} className="min-w-0 flex-1 truncate text-left text-text-primary hover:underline" title="Wiederherstellen">
                  {snapshot.name}
                </button>
                <span
                  role="button"
                  tabIndex={0}
                  onClick={() => {
                    const name = window.prompt("Schnappschuss umbenennen", snapshot.name);
                    if (name) void renameSnapshotAction(snapshot.id, name);
                  }}
                  className="shrink-0 text-text-muted hover:text-accent"
                  title="Umbenennen"
                >
                  ✎
                </span>
                <button type="button" onClick={() => void removeSnapshot(snapshot.id)} className="shrink-0 text-danger" aria-label={`Schnappschuss ${snapshot.name} löschen`}>
                  ×
                </button>
              </li>
            ))}
          </ul>
        </fieldset>
      )}

      {selectedPhotoId && (
        <fieldset className="flex flex-col gap-1">
          <legend className="mb-1 text-xs font-medium text-text-secondary">Vorher/Nachher</legend>
          <div className="grid grid-cols-2 gap-1">
            <button
              type="button"
              onClick={() => setBeforeAfterMode(beforeAfterMode === "sideBySide" ? "none" : "sideBySide")}
              aria-pressed={beforeAfterMode === "sideBySide"}
              className={`rounded border px-2 py-1 text-xs ${beforeAfterMode === "sideBySide" ? "border-accent bg-accent/10 text-accent" : "border-border bg-bg-panel hover:border-accent"}`}
            >
              Links/Rechts
            </button>
            <button
              type="button"
              onClick={() => setBeforeAfterMode(beforeAfterMode === "stacked" ? "none" : "stacked")}
              aria-pressed={beforeAfterMode === "stacked"}
              className={`rounded border px-2 py-1 text-xs ${beforeAfterMode === "stacked" ? "border-accent bg-accent/10 text-accent" : "border-border bg-bg-panel hover:border-accent"}`}
            >
              Oben/Unten
            </button>
            <button
              type="button"
              onClick={() => setBeforeAfterMode(beforeAfterMode === "splitVertical" ? "none" : "splitVertical")}
              aria-pressed={beforeAfterMode === "splitVertical"}
              className={`rounded border px-2 py-1 text-xs ${beforeAfterMode === "splitVertical" ? "border-accent bg-accent/10 text-accent" : "border-border bg-bg-panel hover:border-accent"}`}
            >
              Geteilt
            </button>
            <button
              type="button"
              onClick={() => setBeforeAfterMode(beforeAfterMode === "splitHorizontal" ? "none" : "splitHorizontal")}
              aria-pressed={beforeAfterMode === "splitHorizontal"}
              className={`rounded border px-2 py-1 text-xs ${beforeAfterMode === "splitHorizontal" ? "border-accent bg-accent/10 text-accent" : "border-border bg-bg-panel hover:border-accent"}`}
            >
              Geteilt vertikal
            </button>
          </div>
        </fieldset>
      )}

      {selectedPhotoId && (
        <fieldset className="flex flex-col gap-1">
          <legend className="mb-1 text-xs font-medium text-text-secondary">Referenzansicht</legend>
          <label className="flex items-center gap-2 text-xs text-text-secondary">
            Referenzfoto
            <select
              aria-label="Referenzfoto"
              value={referencePhotoId ?? ""}
              onChange={(event) => setReferencePhotoId(event.target.value || null)}
              className="flex-1 rounded border border-border bg-bg-panel px-1.5 py-0.5"
            >
              <option value="">Foto wählen…</option>
              {otherPhotosForReference.map((photo) => (
                <option key={photo.id} value={photo.id}>
                  {photo.filename}
                </option>
              ))}
            </select>
          </label>
          <button
            type="button"
            onClick={toggleReferenceView}
            disabled={!referencePhotoId}
            aria-pressed={referenceViewActive}
            className={`rounded border px-2 py-1 text-xs disabled:cursor-not-allowed disabled:opacity-40 ${referenceViewActive ? "border-accent bg-accent/10 text-accent" : "border-border bg-bg-panel hover:border-accent"}`}
          >
            Referenzansicht {referenceViewActive ? "ausblenden" : "anzeigen"}
          </button>
        </fieldset>
      )}

      {selectedPhotoId && (
        <fieldset className="flex flex-col gap-2">
          <legend className="mb-1 text-xs font-medium text-text-secondary">Soft-Proof</legend>
          <button
            type="button"
            onClick={toggleSoftProof}
            aria-pressed={softProofActive}
            className={`rounded border px-2 py-1 text-xs ${softProofActive ? "border-accent bg-accent/10 text-accent" : "border-border bg-bg-panel hover:border-accent"}`}
          >
            Soft-Proof {softProofActive ? "ausschalten" : "einschalten"}
          </button>
          {softProofActive && (
            <>
              <label className="flex items-center gap-2 text-xs text-text-secondary">
                Zielprofil
                <select
                  aria-label="Soft-Proof-Zielprofil"
                  value={softProofProfile}
                  onChange={(event) => setSoftProofProfile(event.target.value as SoftProofProfile)}
                  className="flex-1 rounded border border-border bg-bg-panel px-1.5 py-0.5"
                >
                  {Object.entries(SOFT_PROOF_PROFILE_LABELS).map(([value, label]) => (
                    <option key={value} value={value}>
                      {label}
                    </option>
                  ))}
                </select>
              </label>
              {softProofProfile === "custom" && (
                <div className="flex gap-1">
                  <input
                    type="text"
                    readOnly
                    value={softProofCustomIccPath}
                    placeholder="ICC-Datei wählen…"
                    className="min-w-0 flex-1 rounded border border-border bg-bg-panel px-2 py-1 text-xs"
                  />
                  <button type="button" onClick={() => void handlePickSoftProofIccFile()} className="shrink-0 rounded border border-border px-2 py-1 text-xs hover:border-accent">
                    Wählen…
                  </button>
                </div>
              )}
              <label className="flex items-center gap-2 text-xs text-text-secondary">
                Renderpriorität
                <select
                  aria-label="Soft-Proof-Renderpriorität"
                  value={softProofIntent}
                  onChange={(event) => setSoftProofIntent(event.target.value as SoftProofIntent)}
                  className="flex-1 rounded border border-border bg-bg-panel px-1.5 py-0.5"
                >
                  {Object.entries(SOFT_PROOF_INTENT_LABELS).map(([value, label]) => (
                    <option key={value} value={value}>
                      {label}
                    </option>
                  ))}
                </select>
              </label>
              <label className="flex items-center gap-2 text-xs text-text-secondary">
                <input type="checkbox" checked={softProofGamutWarning} onChange={toggleSoftProofGamutWarning} />
                Farbumfangswarnung
              </label>
              <label className="flex items-center gap-2 text-xs text-text-secondary">
                <input type="checkbox" checked={softProofPaperWhite} onChange={toggleSoftProofPaperWhite} />
                Papierweiß-Simulation
              </label>
            </>
          )}
        </fieldset>
      )}

      {selectedPhotoId && (
        <fieldset className="flex flex-col gap-2">
          <legend className="mb-1 text-xs font-medium text-text-secondary">Einstellungen kopieren/einfügen/synchronisieren</legend>
          <div className="flex flex-col gap-1">
            {PRESET_SECTION_KEYS.map((key) => (
              <label key={key} className="flex items-center gap-2 text-xs text-text-secondary">
                <input type="checkbox" checked={workflowSections.has(key)} onChange={() => toggleWorkflowSection(key)} />
                {PRESET_SECTION_LABELS[key]}
              </label>
            ))}
          </div>
          <div className="grid grid-cols-2 gap-1">
            <button
              type="button"
              onClick={() => copyDevelopSettings([...workflowSections])}
              disabled={workflowSections.size === 0}
              className="rounded border border-border px-2 py-1 text-xs text-text-secondary hover:bg-bg-panel disabled:cursor-not-allowed disabled:opacity-40"
            >
              Kopieren
            </button>
            <button
              type="button"
              onClick={pasteDevelopSettings}
              disabled={!copiedEdlSubset}
              className="rounded border border-border px-2 py-1 text-xs text-text-secondary hover:bg-bg-panel disabled:cursor-not-allowed disabled:opacity-40"
            >
              Einfügen
            </button>
            <button
              type="button"
              onClick={() => void applyPreviousSettings()}
              disabled={!lastDevelopPhotoId}
              className="col-span-2 rounded border border-border px-2 py-1 text-xs text-text-secondary hover:bg-bg-panel disabled:cursor-not-allowed disabled:opacity-40"
              title="Übernimmt den zuletzt gespeicherten Stand des vorher im Entwickeln-Modul geöffneten Fotos"
            >
              Vorherige übernehmen
            </button>
            <button
              type="button"
              onClick={() => void syncSettingsToSelection([...workflowSections])}
              disabled={workflowSections.size === 0 || otherSelectedCount === 0}
              className="col-span-2 rounded border border-border px-2 py-1 text-xs text-text-secondary hover:bg-bg-panel disabled:cursor-not-allowed disabled:opacity-40"
            >
              Auf {otherSelectedCount || ""} weitere ausgewählte Foto{otherSelectedCount === 1 ? "" : "s"} synchronisieren
            </button>
          </div>
          <label className="flex items-center gap-2 text-xs text-text-secondary">
            <input type="checkbox" checked={autoSyncActive} onChange={toggleAutoSync} />
            Auto-Sync (jede Änderung sofort auf die übrige Auswahl übertragen, alle Sektionen)
          </label>
        </fieldset>
      )}

      {presetStrengthContext && (
        <fieldset className="flex flex-col gap-2 rounded border border-accent/40 bg-accent/5 p-2">
          <legend className="px-1 text-xs font-medium text-text-secondary">Preset „{presetStrengthContext.presetName}"</legend>
          <DevelopSlider
            spec={PRESET_STRENGTH_SPEC}
            value={presetStrengthContext.strength}
            onChange={setPresetStrength}
            onCommit={commitPresetStrength}
          />
          <button type="button" onClick={dismissPresetStrengthContext} className="self-end text-xs text-text-muted underline">
            Stärke-Regler schließen
          </button>
        </fieldset>
      )}

      {selectedPhotoId && (
        <>
          <fieldset className="flex flex-col gap-3">
            <legend className="mb-1 text-xs font-medium text-text-secondary">Weißabgleich</legend>

            <div className="flex items-center gap-2">
              <select
                aria-label="Weißabgleich-Preset"
                defaultValue=""
                onChange={(event) => {
                  if (event.target.value) applyWhiteBalancePreset(event.target.value);
                  event.target.value = "";
                }}
                className="flex-1 rounded border border-border bg-bg-panel px-2 py-1 text-xs"
              >
                <option value="" disabled>
                  Preset wählen…
                </option>
                {WHITE_BALANCE_PRESETS.map((preset) => (
                  <option key={preset.key} value={preset.key}>
                    {preset.label}
                  </option>
                ))}
              </select>
              <button
                type="button"
                onClick={toggleWbPicker}
                aria-pressed={wbPickerActive}
                title="Weißabgleich-Pipette: ins Bild klicken, um einen neutralen Punkt zu setzen"
                className={`rounded border px-2 py-1 text-xs ${
                  wbPickerActive ? "border-accent bg-accent/10 text-accent" : "border-border bg-bg-panel hover:border-accent"
                }`}
              >
                Pipette
              </button>
            </div>
            {wbPickerActive && <p className="text-xs text-accent">Klicken Sie in einen neutral-grauen Bildpunkt.</p>}

            {whiteBalanceSpecs.map((spec) => (
              <DevelopSlider
                key={spec.key}
                spec={spec}
                value={readBasicField(basic, spec.key)}
                onChange={(value) => setBasicField(spec.key, value)}
                onCommit={() => void commitDevelopEdit()}
              />
            ))}
          </fieldset>

          <fieldset id="stage-basic" className="flex flex-col gap-3">
            {/* Nur für Assistive Technologien / Tests: gruppiert diese
                Regler unter einem eigenen Namen, damit z. B. "Sättigung"
                hier eindeutig von der gleichnamigen HSL-Band-Regler
                unterscheidbar bleibt (beide Abschnitte sind gleichzeitig
                sichtbar). Trägt außerdem den Anker für den Node-Editor
                (Phase 9 Schritt 7) — Textur/Klarheit leben im selben
                Regler-Satz wie die übrigen Grundeinstellungen, deshalb
                zeigt `local_contrast` auf denselben Anker. */}
            <legend className="sr-only">Grundeinstellungen (Ton)</legend>
            {toneSpecs.map((spec) => (
              <DevelopSlider
                key={spec.key}
                spec={spec}
                value={readBasicField(basic, spec.key)}
                onChange={(value) => setBasicField(spec.key, value)}
                onCommit={() => void commitDevelopEdit()}
              />
            ))}
          </fieldset>

          <fieldset id="stage-curves" className="flex flex-col gap-2">
            <legend className="mb-1 text-xs font-medium text-text-secondary">Kurven</legend>
            <div className="flex flex-wrap gap-1">
              {CURVE_CHANNEL_TABS.map((tab) => (
                <button
                  key={tab.key}
                  type="button"
                  onClick={() => setActiveCurveChannel(tab.key)}
                  aria-pressed={activeCurveChannel === tab.key}
                  className={`rounded border px-2 py-1 text-xs ${
                    activeCurveChannel === tab.key ? "border-accent bg-accent/10 text-accent" : "border-border bg-bg-panel hover:border-accent"
                  }`}
                >
                  {tab.label}
                </button>
              ))}
            </div>
            <CurveEditor
              key={activeCurveChannel}
              channel={curves[activeCurveChannel]}
              onChange={(next) => setCurveChannel(activeCurveChannel, next)}
              onCommit={() => void commitDevelopEdit()}
            />
          </fieldset>

          <fieldset id="stage-hsl_color_mixer" className="flex flex-col gap-2">
            <legend className="mb-1 text-xs font-medium text-text-secondary">HSL</legend>
            <div className="flex flex-wrap gap-1">
              {HSL_BAND_TABS.map((tab) => (
                <button
                  key={tab.key}
                  type="button"
                  onClick={() => setActiveHslBand(tab.key)}
                  aria-pressed={activeHslBand === tab.key}
                  className={`rounded border px-2 py-1 text-xs ${
                    activeHslBand === tab.key ? "border-accent bg-accent/10 text-accent" : "border-border bg-bg-panel hover:border-accent"
                  }`}
                >
                  {tab.label}
                </button>
              ))}
            </div>
            <div className="flex flex-col gap-3">
              {HSL_BAND_SLIDER_SPECS.map((spec) => {
                const field = spec.key as "hue" | "saturation" | "luminance";
                return (
                  <DevelopSlider
                    key={spec.key}
                    spec={spec}
                    value={hsl[activeHslBand][field]}
                    onChange={(value) => setHslBandField(activeHslBand, field, value)}
                    onCommit={() => void commitDevelopEdit()}
                  />
                );
              })}
            </div>
          </fieldset>

          <fieldset className="flex flex-col gap-2">
            <legend className="mb-1 text-xs font-medium text-text-secondary">Farb-Harmonie-Rad</legend>
            <ColorHarmonyWheel />
          </fieldset>

          <fieldset id="stage-treatment" className="flex flex-col gap-2">
            <legend className="mb-1 text-xs font-medium text-text-secondary">Behandlung</legend>
            <div className="flex gap-1" role="group" aria-label="Behandlung">
              <button
                type="button"
                onClick={() => setTreatment("Color")}
                aria-pressed={treatment === "Color"}
                className={`flex-1 rounded border px-2 py-1 text-xs ${treatment === "Color" ? "border-accent bg-accent/10 text-accent" : "border-border bg-bg-panel hover:border-accent"}`}
              >
                Farbe
              </button>
              <button
                type="button"
                onClick={() => setTreatment("BlackAndWhite")}
                aria-pressed={treatment === "BlackAndWhite"}
                className={`flex-1 rounded border px-2 py-1 text-xs ${treatment === "BlackAndWhite" ? "border-accent bg-accent/10 text-accent" : "border-border bg-bg-panel hover:border-accent"}`}
              >
                Schwarzweiß
              </button>
            </div>
            {treatment === "BlackAndWhite" && (
              <>
                <div className="flex flex-wrap gap-1">
                  {BW_MIXER_BAND_TABS.map((tab) => (
                    <button
                      key={tab.key}
                      type="button"
                      onClick={() => setActiveBwMixerBand(tab.key)}
                      aria-pressed={activeBwMixerBand === tab.key}
                      className={`rounded border px-2 py-1 text-xs ${
                        activeBwMixerBand === tab.key ? "border-accent bg-accent/10 text-accent" : "border-border bg-bg-panel hover:border-accent"
                      }`}
                    >
                      {tab.label}
                    </button>
                  ))}
                </div>
                <DevelopSlider
                  spec={{ ...BW_MIXER_SLIDER_SPEC, label: BW_MIXER_BAND_TABS.find((t) => t.key === activeBwMixerBand)?.label ?? "" }}
                  value={bwMixer[activeBwMixerBand]}
                  onChange={(value) => setBwMixerField(activeBwMixerBand, value)}
                  onCommit={() => void commitDevelopEdit()}
                />
              </>
            )}
          </fieldset>

          <fieldset className="flex flex-col gap-2">
            <legend className="mb-1 text-xs font-medium text-text-secondary">Farbmischer</legend>
            <button
              type="button"
              onClick={toggleColorMixerPicker}
              disabled={colorMixer.regions.length >= MAX_COLOR_MIXER_REGIONS && !colorMixerPickerActive}
              aria-pressed={colorMixerPickerActive}
              title="Region hinzufügen: ins Bild klicken, um eine neue Farbmischer-Region an dieser Farbe anzulegen"
              className={`rounded border px-2 py-1 text-xs disabled:cursor-not-allowed disabled:opacity-40 ${
                colorMixerPickerActive ? "border-accent bg-accent/10 text-accent" : "border-border bg-bg-panel hover:border-accent"
              }`}
            >
              Region hinzufügen
            </button>
            {colorMixerPickerActive && <p className="text-xs text-accent">Klicken Sie ins Bild, um eine Region an dieser Farbe anzulegen.</p>}

            {colorMixer.regions.length === 0 && <p className="text-xs text-text-muted">Noch keine Regionen.</p>}

            <div className="flex flex-wrap gap-1">
              {colorMixer.regions.map((region, index) => (
                <span key={index} className="flex items-center gap-1 rounded border border-border bg-bg-panel px-1 py-0.5 text-xs">
                  <button
                    type="button"
                    onClick={() => setSelectedRegionIndex(index)}
                    aria-pressed={selectedRegionIndex === index}
                    className={selectedRegionIndex === index ? "text-accent" : "text-text-secondary hover:text-accent"}
                  >
                    {Math.round(region.target_hue_degrees)}°
                  </button>
                  <button
                    type="button"
                    onClick={() => {
                      removeColorMixerRegion(index);
                      if (selectedRegionIndex === index) setSelectedRegionIndex(null);
                    }}
                    aria-label={`Region bei ${Math.round(region.target_hue_degrees)}° entfernen`}
                    className="text-text-muted hover:text-danger"
                  >
                    ×
                  </button>
                </span>
              ))}
            </div>

            {selectedRegionIndex !== null && colorMixer.regions[selectedRegionIndex] && (
              <div className="flex flex-col gap-3">
                {COLOR_MIXER_REGION_SLIDER_SPECS.map((spec) => {
                  const field = spec.key as keyof ColorMixerRegion;
                  const region = colorMixer.regions[selectedRegionIndex];
                  if (!region) return null;
                  return (
                    <DevelopSlider
                      key={spec.key}
                      spec={spec}
                      value={region[field]}
                      onChange={(value) => updateColorMixerRegion(selectedRegionIndex, { [field]: value })}
                      onCommit={() => void commitDevelopEdit()}
                    />
                  );
                })}
              </div>
            )}
          </fieldset>

          <fieldset id="stage-color_grading" className="flex flex-col gap-2">
            <legend className="mb-1 text-xs font-medium text-text-secondary">Color Grading</legend>
            <div className="flex flex-wrap justify-center gap-3">
              {COLOR_GRADING_WHEEL_TABS.map((tab) => (
                <ColorWheel
                  key={tab.key}
                  label={tab.label}
                  wheel={colorGrading[tab.key]}
                  onChange={(next) => setColorGradingWheel(tab.key, next)}
                  onCommit={() => void commitDevelopEdit()}
                />
              ))}
            </div>
            <div className="flex flex-col gap-3">
              <DevelopSlider
                spec={{ key: "balance", label: "Balance", min: -100, max: 100, fineStep: 1, coarseStep: 10, neutral: 0 }}
                value={colorGrading.balance}
                onChange={setColorGradingBalance}
                onCommit={() => void commitDevelopEdit()}
              />
              <DevelopSlider
                spec={{ key: "blending", label: "Überblendung", min: 0, max: 100, fineStep: 1, coarseStep: 10, neutral: 50 }}
                value={colorGrading.blending}
                onChange={setColorGradingBlending}
                onCommit={() => void commitDevelopEdit()}
              />
            </div>
          </fieldset>

          <fieldset id="stage-calibration" className="flex flex-col gap-3">
            <legend className="mb-1 text-xs font-medium text-text-secondary">Kalibrierung</legend>
            {/* Nur `V1` existiert — reiner Vorwärtskompatibilitäts-Platzhalter
                (siehe `crates/apx-pipeline/src/edl/v2.rs`s Moduldoku),
                deshalb kein Auswahl-Widget, nur eine informative Anzeige. */}
            <p className="text-xs text-text-secondary">Prozessversion: V1</p>

            {CALIBRATION_PRIMARY_ROWS.map((row) => (
              <div key={row.key} className="flex flex-col gap-2">
                <DevelopSlider
                  spec={{ key: "hue", label: `Farbton (${row.label})`, min: -100, max: 100, fineStep: 1, coarseStep: 10, neutral: 0 }}
                  value={calibration[row.key].hue}
                  onChange={(value) => setCalibrationPrimaryField(row.key, "hue", value)}
                  onCommit={() => void commitDevelopEdit()}
                />
                <DevelopSlider
                  spec={{ key: "saturation", label: `Sättigung (${row.label})`, min: -100, max: 100, fineStep: 1, coarseStep: 10, neutral: 0 }}
                  value={calibration[row.key].saturation}
                  onChange={(value) => setCalibrationPrimaryField(row.key, "saturation", value)}
                  onCommit={() => void commitDevelopEdit()}
                />
              </div>
            ))}

            <DevelopSlider
              spec={{ key: "shadow_tint", label: "Schattentönung", min: -100, max: 100, fineStep: 1, coarseStep: 10, neutral: 0 }}
              value={calibration.shadow_tint}
              onChange={setCalibrationShadowTint}
              onCommit={() => void commitDevelopEdit()}
            />

            <label className="flex items-center gap-2 text-xs text-text-secondary">
              Kameraprofil
              <select
                aria-label="Kameraprofil"
                value={calibration.camera_profile ?? ""}
                disabled={Boolean(calibration.dcp_profile)}
                onChange={(event) => setCalibrationCameraProfile(event.target.value || null)}
                className="flex-1 rounded border border-border bg-bg-panel px-2 py-1 text-xs disabled:cursor-not-allowed disabled:opacity-40"
              >
                {CAMERA_PROFILE_OPTIONS.map((option) => (
                  <option key={option.label} value={option.value ?? ""}>
                    {option.label}
                  </option>
                ))}
              </select>
            </label>

            {/* Echter DCP-Import (Phase 13 Schritt 3, ADR-0040-Nachtrag) — hat
                Vorrang vor der Handliste oben, deshalb dort deaktiviert,
                solange ein Profil importiert ist. */}
            <div className="flex items-center justify-between text-xs text-text-secondary">
              {calibration.dcp_profile ? (
                <>
                  <span className="truncate" title={calibration.dcp_profile.name}>
                    DCP: {calibration.dcp_profile.name}
                  </span>
                  <button type="button" onClick={clearDcpProfile} className="shrink-0 text-danger underline">
                    Entfernen
                  </button>
                </>
              ) : (
                <button
                  type="button"
                  disabled={dcpProfileImporting}
                  onClick={() => void importDcpProfile()}
                  className="rounded border border-border px-2 py-1 text-xs hover:border-accent disabled:cursor-not-allowed disabled:opacity-40"
                >
                  {dcpProfileImporting ? "Importiert…" : "Adobe-.dcp-Profil importieren…"}
                </button>
              )}
            </div>
          </fieldset>

          <fieldset id="stage-details" className="flex flex-col gap-3">
            <legend className="mb-1 text-xs font-medium text-text-secondary">Details</legend>
            <div className="flex flex-col gap-2">
              {SHARPEN_SLIDER_SPECS.map((spec) => (
                <DevelopSlider
                  key={spec.key}
                  spec={spec}
                  value={details[spec.key as DetailsSliderKey]}
                  onChange={(value) => setDetailsField(spec.key as DetailsSliderKey, value)}
                  onCommit={() => void commitDevelopEdit()}
                />
              ))}
              <label className="flex items-center gap-2 text-xs text-text-secondary">
                <input
                  type="checkbox"
                  checked={details.use_deconvolution_sharpen}
                  onChange={(event) => setDetailsUseDeconvolutionSharpen(event.target.checked)}
                />
                Deconvolution-Schärfung (Alternativmodus)
              </label>
            </div>
            <div className="flex flex-col gap-2">
              {LUMINANCE_NR_SLIDER_SPECS.map((spec) => (
                <DevelopSlider
                  key={spec.key}
                  spec={spec}
                  value={details[spec.key as DetailsSliderKey]}
                  onChange={(value) => setDetailsField(spec.key as DetailsSliderKey, value)}
                  onCommit={() => void commitDevelopEdit()}
                />
              ))}
            </div>
            <div className="flex flex-col gap-2">
              {COLOR_NR_SLIDER_SPECS.map((spec) => (
                <DevelopSlider
                  key={spec.key}
                  spec={spec}
                  value={details[spec.key as DetailsSliderKey]}
                  onChange={(value) => setDetailsField(spec.key as DetailsSliderKey, value)}
                  onCommit={() => void commitDevelopEdit()}
                />
              ))}
            </div>
          </fieldset>

          <fieldset id="stage-lens_corrections" className="flex flex-col gap-3">
            <legend className="mb-1 text-xs font-medium text-text-secondary">Objektivkorrekturen</legend>

            <div className="flex items-center gap-2">
              <label className="flex flex-1 items-center gap-2 text-xs text-text-secondary">
                Objektivprofil
                <select
                  aria-label="Objektivprofil"
                  value={lensCorrections.profile_id ?? ""}
                  onChange={(event) => setLensCorrectionProfile(event.target.value || null)}
                  className="flex-1 rounded border border-border bg-bg-panel px-2 py-1 text-xs"
                >
                  {LENS_PROFILE_OPTIONS.map((option) => (
                    <option key={option.label} value={option.value ?? ""}>
                      {option.label}
                    </option>
                  ))}
                </select>
              </label>
              <button
                type="button"
                onClick={() => void manuallyDetectLensProfile()}
                title="Objektivprofil aus dem EXIF-Objektivstring des Fotos erkennen (Phase 12 Schritt 3, siehe DECISIONS.md ADR-0039)"
                className="shrink-0 rounded border border-border px-2 py-1 text-xs text-text-secondary hover:border-accent"
              >
                Automatisch erkennen
              </button>
            </div>

            <div className="flex items-center gap-2 text-xs">
              <button
                type="button"
                onClick={() => setLensCalibrationDialogOpen(true)}
                title="Objektiv aus eigenen Kalibrierfotos vermessen (Phase 12 Schritt 3 Teil B, siehe DECISIONS.md ADR-0039)"
                className="rounded border border-border px-2 py-1 text-text-secondary hover:border-accent"
              >
                Objektiv kalibrieren…
              </button>
              {lensCorrections.custom_distortion_k1 !== null && (
                <span className="flex items-center gap-1 text-text-secondary">
                  Eigene Kalibrierung aktiv (k1 = {lensCorrections.custom_distortion_k1.toFixed(4)})
                  <button type="button" onClick={() => setLensCorrectionCustomDistortionK1(null)} className="text-danger underline">
                    Entfernen
                  </button>
                </span>
              )}
            </div>

            <label className="flex items-center gap-2 text-xs text-text-secondary">
              <input
                type="checkbox"
                checked={lensCorrections.auto_ca}
                onChange={(event) => setLensCorrectionAutoCa(event.target.checked)}
              />
              Automatische CA-Korrektur (nutzt Profilwerte)
            </label>

            {!lensCorrections.auto_ca &&
              LENS_CA_SLIDER_SPECS.map((spec) => (
                <DevelopSlider
                  key={spec.key}
                  spec={spec}
                  value={lensCorrections[spec.key as LensNumericKey]}
                  onChange={(value) => setLensCorrectionField(spec.key as LensNumericKey, value)}
                  onCommit={() => void commitDevelopEdit()}
                />
              ))}

            {LENS_SLIDER_SPECS.map((spec) => (
              <DevelopSlider
                key={spec.key}
                spec={spec}
                value={lensCorrections[spec.key as LensNumericKey]}
                onChange={(value) => setLensCorrectionField(spec.key as LensNumericKey, value)}
                onCommit={() => void commitDevelopEdit()}
              />
            ))}

            <label className="flex items-center gap-2 text-xs text-text-secondary">
              Perspektive/Upright
              <select
                aria-label="Perspektive/Upright"
                value={lensCorrections.upright_mode}
                onChange={(event) => setLensCorrectionUprightMode(event.target.value as LensCorrectionAdjustment["upright_mode"])}
                className="flex-1 rounded border border-border bg-bg-panel px-2 py-1 text-xs"
              >
                {UPRIGHT_MODE_OPTIONS.map((option) => (
                  <option key={option.value} value={option.value}>
                    {option.label}
                  </option>
                ))}
              </select>
            </label>

            {(lensCorrections.upright_mode === "Level" ||
              lensCorrections.upright_mode === "Vertical" ||
              lensCorrections.upright_mode === "Auto" ||
              lensCorrections.upright_mode === "Full") && (
              <button
                type="button"
                disabled={uprightDetectLoading}
                onClick={() => void runUprightAutoDetect()}
                className="rounded border border-border px-2 py-1 text-xs hover:border-accent disabled:cursor-not-allowed disabled:opacity-40"
              >
                {uprightDetectLoading ? "Erkennt Kanten…" : "Automatisch erkennen"}
              </button>
            )}

            {lensCorrections.upright_mode === "Guided" && (
              <div className="flex flex-col gap-2">
                {/* Bewusste Vereinfachung (siehe DECISIONS.md ADR-0030):
                    Zahlenfelder statt einer Klick-Interaktion im Viewer —
                    eine echte Linienauswahl per Klick wäre eine eigene,
                    größere UI-Aufgabe (SVG-Overlay, Ziehgriffe). */}
                <p className="text-xs text-text-secondary">Hilfslinien (normierte Bildkoordinaten 0–1)</p>
                {[0, 1].map((lineIndex) => {
                  const line: GuidedLine = lensCorrections.guided_lines[lineIndex] ?? {
                    x1: 0,
                    y1: 0,
                    x2: 0,
                    y2: 0,
                  };
                  return (
                    <div key={lineIndex} className="grid grid-cols-4 gap-1">
                      {GUIDED_LINE_FIELDS.map((field) => (
                        <label key={field} className="flex flex-col text-[10px] text-text-secondary">
                          {`L${lineIndex + 1}.${field}`}
                          <input
                            type="number"
                            step={0.01}
                            aria-label={`Linie ${lineIndex + 1}: ${field}`}
                            value={line[field]}
                            onChange={(event) =>
                              setLensCorrectionGuidedLineField(lineIndex as 0 | 1, field, Number(event.target.value))
                            }
                            onBlur={() => void commitDevelopEdit()}
                            className="w-full rounded border border-border bg-bg-base px-1 py-0.5 text-right text-text-primary"
                          />
                        </label>
                      ))}
                    </div>
                  );
                })}
              </div>
            )}

            <div className="flex flex-col gap-2">
              <p className="text-xs text-text-secondary">Manuelle Transformation</p>
              {MANUAL_TRANSFORM_SLIDER_SPECS.map((spec) => (
                <DevelopSlider
                  key={spec.key}
                  spec={spec}
                  value={lensCorrections.manual_transform[spec.key as keyof ManualTransform]}
                  onChange={(value) => setLensCorrectionManualTransformField(spec.key as keyof ManualTransform, value)}
                  onCommit={() => void commitDevelopEdit()}
                />
              ))}
            </div>
          </fieldset>

          <fieldset id="stage-effects" className="flex flex-col gap-3">
            <legend className="mb-1 text-xs font-medium text-text-secondary">Effekte</legend>
            <div className="flex flex-col gap-2">
              {POST_VIGNETTE_SLIDER_SPECS.map((spec) => (
                <DevelopSlider
                  key={spec.key}
                  spec={spec}
                  value={effects[spec.key as keyof EffectsAdjustment]}
                  onChange={(value) => setEffectsField(spec.key as keyof EffectsAdjustment, value)}
                  onCommit={() => void commitDevelopEdit()}
                />
              ))}
            </div>
            <div className="flex flex-col gap-2">
              {GRAIN_SLIDER_SPECS.map((spec) => (
                <DevelopSlider
                  key={spec.key}
                  spec={spec}
                  value={effects[spec.key as keyof EffectsAdjustment]}
                  onChange={(value) => setEffectsField(spec.key as keyof EffectsAdjustment, value)}
                  onCommit={() => void commitDevelopEdit()}
                />
              ))}
            </div>
            {/* Echte Halation-/Bloom-Simulation (Phase 14 Schritt 4,
                ADR-0041): Lightroom Classic "cannot create true film
                halation, only a soft bloom approximation". */}
            <div className="flex flex-col gap-2 border-t border-border pt-2">
              <p className="text-xs text-text-muted">Halation (Lichter-Ausblutung, z. B. Filmlook)</p>
              {HALATION_SLIDER_SPECS.map((spec) => (
                <DevelopSlider
                  key={spec.key}
                  spec={spec}
                  value={effects[spec.key as keyof EffectsAdjustment]}
                  onChange={(value) => setEffectsField(spec.key as keyof EffectsAdjustment, value)}
                  onCommit={() => void commitDevelopEdit()}
                />
              ))}
            </div>
          </fieldset>

          <fieldset id="stage-composite" className="flex flex-col gap-2">
            <legend className="mb-1 text-xs font-medium text-text-secondary">Compositing</legend>
            <p className="text-xs text-text-muted">Mehrfachbelichtung: legt ein weiteres Foto oder eine Textur (z. B. ein Lichtleck) über das aktuelle Bild.</p>

            <label className="flex items-center gap-2 text-xs text-text-secondary">
              Foto
              <select
                aria-label="Ebenen-Quellfoto"
                value={compositeSourcePhotoId}
                onChange={(event) => setCompositeSourcePhotoId(event.target.value)}
                className="min-w-0 flex-1 rounded border border-border bg-bg-panel px-1.5 py-0.5"
              >
                <option value="">Foto wählen…</option>
                {otherPhotosForReference.map((photo) => (
                  <option key={photo.id} value={photo.id}>
                    {photo.filename}
                  </option>
                ))}
              </select>
            </label>
            <div className="flex gap-1.5">
              <button
                type="button"
                onClick={() => compositeSourcePhotoId && void addCompositeLayerFromPhoto(compositeSourcePhotoId)}
                disabled={!compositeSourcePhotoId || compositeLayerLoading}
                className="rounded border border-border px-2 py-1 text-xs text-text-secondary hover:bg-bg-panel disabled:cursor-not-allowed disabled:opacity-40"
              >
                + Ebene aus Foto
              </button>
              <button
                type="button"
                onClick={() => {
                  void (async () => {
                    const path = await pickFilePath("Bild", ["png", "jpg", "jpeg", "webp", "tiff", "bmp"]);
                    if (path) void addCompositeLayerFromTexture(path);
                  })();
                }}
                disabled={compositeLayerLoading}
                className="rounded border border-border px-2 py-1 text-xs text-text-secondary hover:bg-bg-panel disabled:cursor-not-allowed disabled:opacity-40"
              >
                + Ebene aus Textur…
              </button>
            </div>
            {compositeLayerLoading && <p className="text-xs text-text-muted">Löst Ebenenquelle auf…</p>}

            {compositeLayers.length === 0 && <p className="text-xs text-text-muted">Keine Compositing-Ebenen vorhanden.</p>}

            <ul className="flex flex-col gap-2">
              {compositeLayers.map((layer, index) => (
                <li key={index} className="flex flex-col gap-2 rounded border border-border px-2 py-1.5">
                  <div className="flex items-center gap-1.5">
                    <button
                      type="button"
                      onClick={() => setCompositeLayerField(index, "visible", !layer.visible)}
                      aria-label={layer.visible ? `Ebene ${index + 1} ausblenden` : `Ebene ${index + 1} einblenden`}
                      aria-pressed={layer.visible}
                      className={`shrink-0 ${layer.visible ? "text-accent" : "text-text-muted"}`}
                      title="Sichtbarkeit"
                    >
                      {layer.visible ? "👁" : "🚫"}
                    </button>
                    <span className="min-w-0 flex-1 truncate text-xs text-text-primary">Ebene {index + 1}</span>
                    <button type="button" onClick={() => removeCompositeLayer(index)} className="shrink-0 text-danger" aria-label={`Ebene ${index + 1} löschen`}>
                      ×
                    </button>
                  </div>

                  <label className="flex items-center gap-2 text-xs text-text-secondary">
                    Blend-Modus
                    <select
                      aria-label={`Blend-Modus Ebene ${index + 1}`}
                      value={layer.blend_mode}
                      onChange={(event) => setCompositeLayerField(index, "blend_mode", event.target.value as BlendMode)}
                      className="min-w-0 flex-1 rounded border border-border bg-bg-panel px-1.5 py-0.5"
                    >
                      {BLEND_MODE_OPTIONS.map((option) => (
                        <option key={option.value} value={option.value}>
                          {option.label}
                        </option>
                      ))}
                    </select>
                  </label>

                  <DevelopSlider
                    spec={COMPOSITE_OPACITY_SPEC}
                    value={layer.opacity * 100}
                    onChange={(value) => setCompositeLayerField(index, "opacity", value / 100)}
                    onCommit={() => void commitDevelopEdit()}
                  />
                  <DevelopSlider
                    spec={COMPOSITE_SCALE_SPEC}
                    value={layer.scale * 100}
                    onChange={(value) => setCompositeLayerField(index, "scale", value / 100)}
                    onCommit={() => void commitDevelopEdit()}
                  />
                  <DevelopSlider
                    spec={COMPOSITE_OFFSET_X_SPEC}
                    value={layer.offset_x * 100}
                    onChange={(value) => setCompositeLayerField(index, "offset_x", value / 100)}
                    onCommit={() => void commitDevelopEdit()}
                  />
                  <DevelopSlider
                    spec={COMPOSITE_OFFSET_Y_SPEC}
                    value={layer.offset_y * 100}
                    onChange={(value) => setCompositeLayerField(index, "offset_y", value / 100)}
                    onCommit={() => void commitDevelopEdit()}
                  />
                  <DevelopSlider
                    spec={COMPOSITE_BLEND_IF_SHADOW_SPEC}
                    value={layer.blend_if_shadow_cutoff * 100}
                    onChange={(value) => setCompositeLayerField(index, "blend_if_shadow_cutoff", value / 100)}
                    onCommit={() => void commitDevelopEdit()}
                  />
                  <DevelopSlider
                    spec={COMPOSITE_BLEND_IF_HIGHLIGHT_SPEC}
                    value={layer.blend_if_highlight_cutoff * 100}
                    onChange={(value) => setCompositeLayerField(index, "blend_if_highlight_cutoff", value / 100)}
                    onCommit={() => void commitDevelopEdit()}
                  />
                </li>
              ))}
            </ul>
          </fieldset>

          {/* KI-Tiefenschärfe-Simulator "Virtuelle Blende" (Phase 14
              Schritt 8, ADR-0041 Nachtrag VIII) — läuft nach dem
              Halation-Kurzschluss, vor `masks` (siehe `develop.rs`s
              Moduldoku), in der Anzeige aber neben `composite` platziert
              (dieselbe Vereinfachung wie bei allen übrigen Knoten:
              Anzeigereihenfolge = `STAGE_NODE_SPECS`). */}
          <fieldset id="stage-virtual_aperture" className="flex flex-col gap-2">
            <legend className="mb-1 text-xs font-medium text-text-secondary">Virtuelle Blende</legend>
            <VirtualAperturePanel />
          </fieldset>

          {/* KI-Stiltransfer zwischen Fotos (Phase 14 Schritt 9,
              ADR-0041 Nachtrag IX) — läuft nach `composite`, vor
              `geometry` (siehe `stages::style_transfer`s Moduldoku). */}
          <fieldset id="stage-style_transfer" className="flex flex-col gap-2">
            <legend className="mb-1 text-xs font-medium text-text-secondary">Stiltransfer</legend>
            <StyleTransferPanel />
          </fieldset>

          {/* Photoshop-Funktion: Automatisches Hautglätten (Phase 15
              Schritt 5, ADR-0042) — läuft nach `style_transfer`, vor
              `sky_replace` (siehe `stages::skin_smoothing`s Moduldoku). */}
          <fieldset id="stage-skin_smoothing" className="flex flex-col gap-2">
            <legend className="mb-1 text-xs font-medium text-text-secondary">Hautglätten</legend>
            <SkinSmoothingPanel />
          </fieldset>

          <fieldset id="stage-sky_replace" className="flex flex-col gap-2">
            <legend className="mb-1 text-xs font-medium text-text-secondary">Himmelsaustausch</legend>
            <SkyReplacePanel />
          </fieldset>

          <fieldset id="stage-geometry" className="flex flex-col gap-3">
            <legend className="mb-1 text-xs font-medium text-text-secondary">Geometrie</legend>
            <button
              type="button"
              aria-pressed={geometryCropActive}
              onClick={toggleGeometryCropActive}
              className={`rounded border px-2 py-1 text-xs ${geometryCropActive ? "border-accent bg-accent/20 text-accent" : "border-border text-text-secondary"}`}
            >
              Freistellen {geometryCropActive ? "(aktiv)" : ""}
            </button>

            <DevelopSlider
              spec={{ key: "angle_degrees", label: "Winkel", min: -45, max: 45, fineStep: 0.1, coarseStep: 1, neutral: 0 }}
              value={geometry.angle_degrees}
              onChange={setGeometryAngle}
              onCommit={() => void commitDevelopEdit()}
            />

            <label className="flex items-center gap-2 text-xs text-text-secondary">
              Seitenverhältnis
              <select
                aria-label="Seitenverhältnis"
                value={geometry.aspect_ratio ?? ""}
                onChange={(event) => setGeometryAspectRatio(event.target.value ? Number(event.target.value) : null)}
                className="flex-1 rounded border border-border bg-bg-panel px-2 py-1 text-xs"
              >
                {ASPECT_RATIO_PRESETS.map((option) => (
                  <option key={option.label} value={option.value ?? ""}>
                    {option.label}
                  </option>
                ))}
              </select>
            </label>

            <label className="flex items-center gap-2 text-xs text-text-secondary">
              Raster
              <select
                aria-label="Rasterüberlagerung"
                value={geometry.overlay}
                onChange={(event) => setGeometryOverlay(event.target.value as GridOverlay)}
                className="flex-1 rounded border border-border bg-bg-panel px-2 py-1 text-xs"
              >
                {GRID_OVERLAY_OPTIONS.map((option) => (
                  <option key={option.value} value={option.value}>
                    {option.label}
                  </option>
                ))}
              </select>
            </label>

            <label className="flex items-center gap-2 text-xs text-text-secondary">
              <input
                type="checkbox"
                checked={geometry.auto_horizon}
                onChange={(event) => setGeometryAutoHorizon(event.target.checked)}
              />
              Automatische Ausrichtung (nur EXIF-Ausrichtung, siehe ADR-0028)
            </label>

            <button
              type="button"
              onClick={() => setCanvasExtendDialogOpen(true)}
              title="Leinwand per KI-Ausfüllen über den Bildrand hinaus erweitern (Phase 14 Schritt 1, siehe DECISIONS.md ADR-0041)"
              className="rounded border border-border px-2 py-1 text-xs text-text-secondary hover:border-accent"
            >
              Leinwand erweitern (KI)…
            </button>

            {/* Photoshop-Funktion: Content-Aware Scale / Seam Carving
                (Phase 15 Schritt 4, ADR-0042) — klassischer Algorithmus,
                kein Modell-Download nötig. */}
            <button
              type="button"
              onClick={() => setContentAwareScaleDialogOpen(true)}
              title="Breite/Höhe unabhängig ändern, ohne wichtige Bildinhalte zu verzerren (Phase 15 Schritt 4, siehe DECISIONS.md ADR-0042)"
              className="rounded border border-border px-2 py-1 text-xs text-text-secondary hover:border-accent"
            >
              Inhaltssensitiv skalieren…
            </button>
          </fieldset>

          <fieldset id="stage-repair" className="flex flex-col gap-3">
            <legend className="mb-1 text-xs font-medium text-text-secondary">Reparatur (Klonen/Reparieren)</legend>

            {/* Frequenztrennungs-Ansichtsmodus (Phase 14 Schritt 2,
                ADR-0041): zeigt Tieffrequenz/Hochfrequenz statt des
                normalen Bilds im Viewer — reine Anzeige, verändert
                developEdl nicht. */}
            <label className="flex items-center gap-2 text-xs text-text-secondary">
              Ansicht
              <select
                aria-label="Frequenztrennungs-Ansicht"
                value={frequencyViewMode}
                onChange={(event) => setFrequencyViewMode(event.target.value as FrequencyViewMode)}
                className="flex-1 rounded border border-border bg-bg-panel px-2 py-1 text-xs"
              >
                <option value="Normal">Normal</option>
                <option value="LowFrequency">Tieffrequenz (Ton/Farbe)</option>
                <option value="HighFrequency">Hochfrequenz (Textur/Poren)</option>
              </select>
            </label>

            <button
              type="button"
              aria-pressed={repairActive}
              onClick={toggleRepairActive}
              className={`rounded border px-2 py-1 text-xs ${repairActive ? "border-accent bg-accent/20 text-accent" : "border-border text-text-secondary"}`}
            >
              Reparatur-Pinsel {repairActive ? "(aktiv)" : ""}
            </button>

            {/* Photoshop-Funktion: Content-Aware Move (Phase 15 Schritt 1,
                ADR-0042) — nutzt dieselbe LaMa-Session wie das
                KI-Ausfüllen oben, aber als eigenständiges Werkzeug (kein
                `RepairMode`, siehe `content_aware_move`s Moduldoku). */}
            <button
              type="button"
              aria-pressed={contentAwareMoveActive}
              onClick={toggleContentAwareMoveTool}
              disabled={!aiSettings?.inpainting_model_path}
              title={!aiSettings?.inpainting_model_path ? "Braucht das KI-Ausfüllen-Modell (siehe oben)" : undefined}
              className={`rounded border px-2 py-1 text-xs disabled:cursor-not-allowed disabled:opacity-40 ${contentAwareMoveActive ? "border-accent bg-accent/20 text-accent" : "border-border text-text-secondary"}`}
            >
              Objekt verschieben (Content-Aware Move) {contentAwareMoveActive ? "(aktiv)" : ""}
            </button>
            {contentAwareMoveActive && (
              <p className="text-xs text-text-muted">
                {contentAwareMoveRect
                  ? "Auswahl an die Zielposition ziehen und loslassen."
                  : "Rechteck um das zu verschiebende Objekt aufziehen."}
                {contentAwareMoveLoading && " Berechnet…"}
              </p>
            )}

            {repairActive && (
              <p className="text-xs text-text-muted">
                {repairDraftMode === "ContentAwareFill"
                  ? "Ziel im Bild malen (Ziehen) — kein Quellpunkt nötig, der Füllinhalt kommt aus der Umgebung."
                  : repairDraftMode === "AiInpaint"
                    ? "Ziel im Bild malen (Ziehen) — kein Quellpunkt nötig. Danach unten „Anwenden“ klicken, um die KI-Inferenz auszulösen."
                    : repairPendingSource
                      ? "Ziel im Bild malen (Ziehen), um den Strich abzuschließen."
                      : "Quellpunkt im Bild anklicken."}
                {repairDraftMode !== "ContentAwareFill" && repairDraftMode !== "AiInpaint" && repairPendingSource && (
                  <button type="button" onClick={cancelRepairSource} className="ml-2 underline">
                    Quellpunkt verwerfen
                  </button>
                )}
              </p>
            )}

            <label className="flex items-center gap-2 text-xs text-text-secondary">
              Modus
              <select
                aria-label="Reparatur-Modus"
                value={repairDraftMode}
                onChange={(event) => setRepairDraftMode(event.target.value as RepairMode)}
                className="flex-1 rounded border border-border bg-bg-panel px-2 py-1 text-xs"
              >
                {REPAIR_MODE_OPTIONS.map((option) => (
                  <option key={option.value} value={option.value}>
                    {option.label}
                  </option>
                ))}
              </select>
            </label>

            {/* Frequenztrennung (Phase 14 Schritt 2, ADR-0041): Lightroom
                hat kein eingebautes Frequenztrennungs-Werkzeug wie
                Photoshop — lässt den Strich gezielt nur auf Ton/Farbe
                oder nur auf Textur wirken, siehe stages::repair. */}
            <label className="flex items-center gap-2 text-xs text-text-secondary">
              Ebene
              <select
                aria-label="Frequenz-Ebene"
                value={repairDraftLayer}
                onChange={(event) => setRepairDraftLayer(event.target.value as RepairLayer)}
                className="flex-1 rounded border border-border bg-bg-panel px-2 py-1 text-xs"
              >
                {REPAIR_LAYER_OPTIONS.map((option) => (
                  <option key={option.value} value={option.value}>
                    {option.label}
                  </option>
                ))}
              </select>
            </label>

            {/* KI-Ausfüllen (Phase 13 Schritt 1, ADR-0040): echtes
                LaMa-Modell, opt-in-Download (~208 MB, Apache-2.0,
                `Carve/LaMa-ONNX`, lokal — kein Text-Prompt, kein
                Cloud-Aufruf). Ohne heruntergeladenes Modell schlägt
                „Anwenden" oben mit einer klaren Fehlermeldung fehl. */}
            {repairDraftMode === "AiInpaint" && (
              <p className="rounded border border-border px-2 py-1 text-xs text-text-secondary">
                {aiSettings?.inpainting_model_path ? (
                  "KI-Ausfüllen-Modell installiert."
                ) : (
                  <>
                    Kein Modell installiert — LaMa-Inpainting (Apache-2.0, ~208 MB, lokal, kein Cloud-Aufruf).{" "}
                    <button
                      type="button"
                      disabled={inpaintingModelDownloading}
                      onClick={() => void downloadInpaintingModel()}
                      className="text-accent underline disabled:cursor-not-allowed disabled:opacity-40"
                    >
                      {inpaintingModelDownloading ? "Lädt herunter…" : "Herunterladen"}
                    </button>
                  </>
                )}
              </p>
            )}

            {/* Auto-Quellenfindung (Phase 7 Schritt 3, ADR-0033) — nur für
                Klonen/Reparieren sinnvoll, ContentAwareFill/AiInpaint
                brauchen ohnehin keinen Quellpunkt. */}
            {repairDraftMode !== "ContentAwareFill" && repairDraftMode !== "AiInpaint" && (
              <label className="flex items-center gap-2 text-xs text-text-secondary">
                <input type="checkbox" checked={autoSourceModeActive} onChange={toggleAutoSourceMode} />
                Quelle automatisch vorschlagen
                {repairSourceSuggestionLoading && <span className="text-text-muted">(sucht…)</span>}
              </label>
            )}

            <DevelopSlider
              spec={REPAIR_RADIUS_SPEC}
              value={repairDraftRadius * 100}
              onChange={(value) => setRepairDraftField("radius", value / 100)}
              onCommit={() => {}}
            />
            <DevelopSlider
              spec={REPAIR_FEATHER_SPEC}
              value={repairDraftFeather * 100}
              onChange={(value) => setRepairDraftField("feather", value / 100)}
              onCommit={() => {}}
            />
            <DevelopSlider
              spec={REPAIR_OPACITY_SPEC}
              value={repairDraftOpacity * 100}
              onChange={(value) => setRepairDraftField("opacity", value / 100)}
              onCommit={() => {}}
            />

            {repairStrokes.length > 0 && (
              <ul className="flex flex-col gap-1 text-xs text-text-secondary">
                {repairStrokes.map((stroke, index) => (
                  <li key={index} className="flex items-center justify-between rounded border border-border px-2 py-1">
                    <span>
                      {index + 1}.{" "}
                      {stroke.mode === "Heal"
                        ? "Reparieren"
                        : stroke.mode === "ContentAwareFill"
                          ? "Inhaltsbasiert gefüllt"
                          : stroke.mode === "AiInpaint"
                            ? stroke.ai_fill
                              ? "KI-ausgefüllt"
                              : "KI-Ausfüllen (noch nicht angewendet)"
                            : "Klonen"}
                    </span>
                    <span className="flex items-center gap-2">
                      {/* KI-Ausfüllen läuft nicht automatisch (siehe
                          Moduldoku `runAiInpaintForStroke`) — ein
                          gemalter, noch nicht angewendeter Strich zeigt
                          hier den expliziten Auslöser. */}
                      {stroke.mode === "AiInpaint" && !stroke.ai_fill && (
                        <button
                          type="button"
                          disabled={!selectedPhotoId || aiInpaintLoadingIndex !== null}
                          onClick={() => void runAiInpaintForStroke(index)}
                          className="text-accent underline disabled:cursor-not-allowed disabled:opacity-40"
                        >
                          {aiInpaintLoadingIndex === index ? "Berechnet…" : "Anwenden"}
                        </button>
                      )}
                      <button type="button" onClick={() => removeRepairStroke(index)} className="text-danger underline">
                        Entfernen
                      </button>
                    </span>
                  </li>
                ))}
              </ul>
            )}

            {/* Sensorflecken-Visualisierung (Phase 7 Schritt 3, ADR-0033)
                — reine Analyse, legt selbst keine Striche an; die
                orangen Kreise im Bild (`RepairOverlay.tsx`) markieren die
                Fundstellen. */}
            <div className="flex items-center gap-1 border-t border-border pt-2">
              <button
                type="button"
                disabled={!selectedPhotoId || sensorSpotsLoading}
                onClick={() => void detectSensorSpotsForCurrentPhoto(0.5)}
                className="rounded border border-border px-2 py-1 text-xs text-text-secondary hover:bg-bg-panel disabled:cursor-not-allowed disabled:opacity-40"
              >
                {sensorSpotsLoading ? "Suche…" : "Sensorflecken suchen"}
              </button>
              {sensorSpotCandidates.length > 0 && (
                <button type="button" onClick={clearSensorSpots} className="text-xs text-text-muted hover:text-danger">
                  Verwerfen
                </button>
              )}
            </div>
            {sensorSpotCandidates.length > 0 && (
              <ul className="flex flex-col gap-1 text-xs text-text-secondary">
                {sensorSpotCandidates.map((spot, index) => (
                  <li key={index} className="flex items-center justify-between rounded border border-border px-2 py-1">
                    <span>
                      Fleck {index + 1} ({Math.round(spot.strength * 100)} %)
                    </span>
                    <button type="button" onClick={() => applySensorSpotAsRepairStroke(spot)} className="text-accent underline">
                      Reparieren
                    </button>
                  </li>
                ))}
              </ul>
            )}
          </fieldset>

          {/* Photoshop-Funktion: Verflüssigen (Liquify, Phase 15 Schritt 3,
              ADR-0042) — Lightroom hat kein Verformungswerkzeug. Rein
              deterministische CPU-Verzerrung, kein separates „Anwenden"
              nötig (siehe `stages::liquify`s Moduldoku, `LiquifyOverlay`). */}
          <fieldset id="stage-liquify" className="flex flex-col gap-3">
            <legend className="mb-1 text-xs font-medium text-text-secondary">Verflüssigen</legend>

            <button
              type="button"
              aria-pressed={liquifyActive}
              onClick={toggleLiquifyActive}
              className={`rounded border px-2 py-1 text-xs ${liquifyActive ? "border-accent bg-accent/20 text-accent" : "border-border text-text-secondary"}`}
            >
              Verflüssigen-Pinsel {liquifyActive ? "(aktiv)" : ""}
            </button>
            {liquifyActive && <p className="text-xs text-text-muted">Strich im Bild ziehen, um den gewählten Verformungsmodus anzuwenden.</p>}

            <div className="flex gap-1">
              {LIQUIFY_MODE_OPTIONS.map((option) => (
                <button
                  key={option.value}
                  type="button"
                  aria-pressed={liquifyDraftMode === option.value}
                  onClick={() => setLiquifyDraftMode(option.value)}
                  className={`flex-1 rounded border px-2 py-1 text-xs ${liquifyDraftMode === option.value ? "border-accent bg-accent/20 text-accent" : "border-border text-text-secondary"}`}
                >
                  {option.label}
                </button>
              ))}
            </div>

            <DevelopSlider
              spec={LIQUIFY_RADIUS_SPEC}
              value={liquifyDraftRadius * 100}
              onChange={(value) => setLiquifyDraftField("radius", value / 100)}
              onCommit={() => {}}
            />
            <DevelopSlider
              spec={LIQUIFY_STRENGTH_SPEC}
              value={liquifyDraftStrength * 100}
              onChange={(value) => setLiquifyDraftField("strength", value / 100)}
              onCommit={() => {}}
            />

            {liquifyStrokes.length > 0 && (
              <ul className="flex flex-col gap-1 text-xs text-text-secondary">
                {liquifyStrokes.map((stroke, index) => (
                  <li key={index} className="flex items-center justify-between rounded border border-border px-2 py-1">
                    <span>
                      {index + 1}. {LIQUIFY_MODE_OPTIONS.find((option) => option.value === stroke.mode)?.label ?? stroke.mode}
                    </span>
                    <button type="button" onClick={() => removeLiquifyStroke(index)} className="text-danger underline">
                      Entfernen
                    </button>
                  </li>
                ))}
              </ul>
            )}
          </fieldset>

          <fieldset className="flex flex-col gap-2">
            <legend className="mb-1 text-xs font-medium text-text-secondary">Entrauschung &amp; Hochskalierung</legend>
            <p className="text-xs text-text-muted">Klassische Algorithmen (Bilateral-Filter, kantengerichtete Interpolation), keine Modellinferenz — schreiben eine neue Datei neben dem Original, ändern die Bearbeitung nicht.</p>
            <div className="flex gap-1">
              <button
                type="button"
                disabled={!selectedPhotoId || enhanceRunning !== null}
                onClick={() => selectedPhotoId && void runDenoise(selectedPhotoId)}
                className="flex-1 rounded border border-border px-2 py-1 text-xs hover:border-accent disabled:cursor-not-allowed disabled:opacity-40"
              >
                {enhanceRunning === "denoise" ? "Entrauscht…" : "Entrauschen"}
              </button>
              <button
                type="button"
                disabled={!selectedPhotoId || enhanceRunning !== null}
                onClick={() => selectedPhotoId && void runUpscale(selectedPhotoId)}
                className="flex-1 rounded border border-border px-2 py-1 text-xs hover:border-accent disabled:cursor-not-allowed disabled:opacity-40"
              >
                {enhanceRunning === "upscale" ? "Skaliert…" : "2× hochskalieren"}
              </button>
            </div>
            {enhanceStatus && <p className="text-xs text-text-muted">{enhanceStatus}</p>}
          </fieldset>

          <fieldset className="flex flex-col gap-2">
            <legend className="mb-1 text-xs font-medium text-text-secondary">DNG-Konvertierung</legend>
            <p className="text-xs text-text-muted">
              Schreibt eine „Linear DNG" aus den unveränderten, kamera-nativen RAW-Daten (nicht dem entwickelten
              Rendering) neben das Original — ein Rohdatenformat mit demosaicten statt der ursprünglichen
              Bayer-Mosaik-Daten, siehe Dokumentation.
            </p>
            <button
              type="button"
              disabled={!selectedPhotoId || enhanceRunning !== null}
              onClick={() => selectedPhotoId && void runConvertToDng(selectedPhotoId)}
              className="rounded border border-border px-2 py-1 text-xs hover:border-accent disabled:cursor-not-allowed disabled:opacity-40"
            >
              {enhanceRunning === "dng" ? "Konvertiert…" : "Als DNG konvertieren"}
            </button>
          </fieldset>
        </>
      )}
    </PaletteFrame>
    <SavePresetDialog open={savePresetOpen} onClose={() => setSavePresetOpen(false)} />
    <LensCalibrationDialog />
    <CanvasExtendDialog />
    <ContentAwareScaleDialog />
    </>
  );
}
