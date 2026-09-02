import { useEffect, useRef, useState } from "react";
import { useShallow } from "zustand/react/shallow";

import {
  ASPECT_RATIO_PRESETS,
  BASIC_SLIDER_SPECS,
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
  type ColorMixerRegion,
  type CurvesAdjustment,
  type DetailsSliderKey,
  type EffectsAdjustment,
  type GridOverlay,
  type GuidedLine,
  type HslAdjustment,
  type LensCorrectionAdjustment,
  type ManualTransform,
  type RepairMode,
  type SliderSpec,
  type StageEnabled,
} from "../lib/edl";
import { PRESET_SECTION_KEYS, PRESET_SECTION_LABELS, type PresetSectionKey } from "../lib/presets";
import { SOFT_PROOF_INTENT_LABELS, SOFT_PROOF_PROFILE_LABELS, type SoftProofIntent, type SoftProofProfile } from "../lib/softProof";
import { selectActivePhotos, useAppStore } from "../store";
import { ColorWheel } from "./ColorWheel";
import { CurveEditor } from "./CurveEditor";
import { DevelopSlider } from "./DevelopSlider";
import { SavePresetDialog } from "./SavePresetDialog";

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
  geometry: "stage-geometry",
};

function openStageAnchor(key: keyof StageEnabled): void {
  document.getElementById(STAGE_ANCHOR_IDS[key])?.scrollIntoView({ behavior: "smooth", block: "start" });
}

const REPAIR_MODE_OPTIONS: ReadonlyArray<{ value: RepairMode; label: string }> = [
  { value: "Clone", label: "Klonen" },
  { value: "Heal", label: "Reparieren" },
  // Inhaltsbasiertes Füllen (Phase 7, ADR-0033 Punkt 4): kein
  // Quellpunkt nötig, siehe RepairOverlay.tsx/store/index.ts's
  // addRepairStroke.
  { value: "ContentAwareFill", label: "Inhaltsbasiert füllen" },
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
  const softProofIntent = useAppStore((s) => s.softProofIntent);
  const setSoftProofIntent = useAppStore((s) => s.setSoftProofIntent);
  const softProofGamutWarning = useAppStore((s) => s.softProofGamutWarning);
  const toggleSoftProofGamutWarning = useAppStore((s) => s.toggleSoftProofGamutWarning);
  const softProofPaperWhite = useAppStore((s) => s.softProofPaperWhite);
  const toggleSoftProofPaperWhite = useAppStore((s) => s.toggleSoftProofPaperWhite);
  const activePhotos = useAppStore(useShallow(selectActivePhotos));
  const otherPhotosForReference = activePhotos.filter((p) => p.id !== selectedPhotoId);
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
  const details = useAppStore((s) => s.developEdl.details);
  const setDetailsField = useAppStore((s) => s.setDetailsField);
  const setDetailsUseDeconvolutionSharpen = useAppStore((s) => s.setDetailsUseDeconvolutionSharpen);
  const lensCorrections = useAppStore((s) => s.developEdl.lens_corrections);
  const setLensCorrectionField = useAppStore((s) => s.setLensCorrectionField);
  const setLensCorrectionManualTransformField = useAppStore((s) => s.setLensCorrectionManualTransformField);
  const setLensCorrectionProfile = useAppStore((s) => s.setLensCorrectionProfile);
  const setLensCorrectionAutoCa = useAppStore((s) => s.setLensCorrectionAutoCa);
  const setLensCorrectionUprightMode = useAppStore((s) => s.setLensCorrectionUprightMode);
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
  const repairDraftMode = useAppStore((s) => s.repairDraftMode);
  const setRepairDraftMode = useAppStore((s) => s.setRepairDraftMode);
  const repairDraftRadius = useAppStore((s) => s.repairDraftRadius);
  const repairDraftFeather = useAppStore((s) => s.repairDraftFeather);
  const repairDraftOpacity = useAppStore((s) => s.repairDraftOpacity);
  const setRepairDraftField = useAppStore((s) => s.setRepairDraftField);
  const repairPendingSource = useAppStore((s) => s.repairPendingSource);
  const cancelRepairSource = useAppStore((s) => s.cancelRepairSource);
  const removeRepairStroke = useAppStore((s) => s.removeRepairStroke);
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

  useEffect(() => {
    if (!open) return;

    function handleKeyDown(event: KeyboardEvent) {
      const target = event.target as HTMLElement | null;
      if (target && (target.tagName === "INPUT" || target.tagName === "TEXTAREA")) return;
      if (!(event.metaKey || event.ctrlKey) || event.key.toLowerCase() !== "z") return;

      event.preventDefault();
      if (event.shiftKey) {
        void redoDevelop();
      } else {
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
    <aside className="flex w-72 shrink-0 flex-col gap-4 overflow-y-auto border-l border-border bg-bg-raised p-3" aria-label="Entwickeln">
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
                onChange={(event) => setCalibrationCameraProfile(event.target.value || null)}
                className="flex-1 rounded border border-border bg-bg-panel px-2 py-1 text-xs"
              >
                {CAMERA_PROFILE_OPTIONS.map((option) => (
                  <option key={option.label} value={option.value ?? ""}>
                    {option.label}
                  </option>
                ))}
              </select>
            </label>
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

            <label className="flex items-center gap-2 text-xs text-text-secondary">
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
          </fieldset>

          <fieldset id="stage-repair" className="flex flex-col gap-3">
            <legend className="mb-1 text-xs font-medium text-text-secondary">Reparatur (Klonen/Reparieren)</legend>
            <button
              type="button"
              aria-pressed={repairActive}
              onClick={toggleRepairActive}
              className={`rounded border px-2 py-1 text-xs ${repairActive ? "border-accent bg-accent/20 text-accent" : "border-border text-text-secondary"}`}
            >
              Reparatur-Pinsel {repairActive ? "(aktiv)" : ""}
            </button>

            {repairActive && (
              <p className="text-xs text-text-muted">
                {repairDraftMode === "ContentAwareFill"
                  ? "Ziel im Bild malen (Ziehen) — kein Quellpunkt nötig, der Füllinhalt kommt aus der Umgebung."
                  : repairPendingSource
                    ? "Ziel im Bild malen (Ziehen), um den Strich abzuschließen."
                    : "Quellpunkt im Bild anklicken."}
                {repairDraftMode !== "ContentAwareFill" && repairPendingSource && (
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

            {/* Auto-Quellenfindung (Phase 7 Schritt 3, ADR-0033) — nur für
                Klonen/Reparieren sinnvoll, ContentAwareFill braucht ohnehin
                keinen Quellpunkt. */}
            {repairDraftMode !== "ContentAwareFill" && (
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
                          : "Klonen"}
                    </span>
                    <button type="button" onClick={() => removeRepairStroke(index)} className="text-danger underline">
                      Entfernen
                    </button>
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
    </aside>
    <SavePresetDialog open={savePresetOpen} onClose={() => setSavePresetOpen(false)} />
    </>
  );
}
