import { useEffect, useRef, useState } from "react";

import {
  BASIC_SLIDER_SPECS,
  CALIBRATION_PRIMARY_ROWS,
  CAMERA_PROFILE_OPTIONS,
  COLOR_MIXER_REGION_SLIDER_SPECS,
  HSL_BAND_SLIDER_SPECS,
  HSL_BAND_TABS,
  MAX_COLOR_MIXER_REGIONS,
  readBasicField,
  WHITE_BALANCE_PRESETS,
  type ColorGradingAdjustment,
  type ColorMixerRegion,
  type CurvesAdjustment,
  type HslAdjustment,
} from "../lib/edl";
import { useAppStore } from "../store";
import { ColorWheel } from "./ColorWheel";
import { CurveEditor } from "./CurveEditor";
import { DevelopSlider } from "./DevelopSlider";

const WHITE_BALANCE_KEYS = new Set(["temp_shift_kelvin", "tint_shift"]);

/** Die fünf Kurven-Kanäle (Phase 4 Schritt 4) — Anzeigereihenfolge wie in
 * `SPEC.md` §3.2 („RGB-Verbundkurve, R/G/B einzeln, Luminanz-Kurve"). */
const CURVE_CHANNEL_TABS: ReadonlyArray<{ key: keyof CurvesAdjustment; label: string }> = [
  { key: "rgb", label: "RGB" },
  { key: "red", label: "Rot" },
  { key: "green", label: "Grün" },
  { key: "blue", label: "Blau" },
  { key: "luminance", label: "Luminanz" },
];

/** Die vier Color-Grading-Farbräder (Phase 4 Schritt 6). */
const COLOR_GRADING_WHEEL_TABS: ReadonlyArray<{ key: keyof Pick<ColorGradingAdjustment, "shadows" | "midtones" | "highlights" | "global">; label: string }> = [
  { key: "shadows", label: "Schatten" },
  { key: "midtones", label: "Mitteltöne" },
  { key: "highlights", label: "Lichter" },
  { key: "global", label: "Global" },
];

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
  const selectedPhotoId = useAppStore((s) => s.selectedPhotoId);
  const lastLatencyMs = useAppStore((s) => s.developLastLatencyMs);
  const wbPickerActive = useAppStore((s) => s.wbPickerActive);
  const toggleWbPicker = useAppStore((s) => s.toggleWbPicker);
  const applyWhiteBalancePreset = useAppStore((s) => s.applyWhiteBalancePreset);
  const curves = useAppStore((s) => s.developEdl.curves);
  const setCurveChannel = useAppStore((s) => s.setCurveChannel);
  const [activeCurveChannel, setActiveCurveChannel] = useState<keyof CurvesAdjustment>("rgb");
  const hsl = useAppStore((s) => s.developEdl.hsl);
  const setHslBandField = useAppStore((s) => s.setHslBandField);
  const [activeHslBand, setActiveHslBand] = useState<keyof HslAdjustment>("red");
  const colorMixer = useAppStore((s) => s.developEdl.color_mixer);
  const colorMixerPickerActive = useAppStore((s) => s.colorMixerPickerActive);
  const toggleColorMixerPicker = useAppStore((s) => s.toggleColorMixerPicker);
  const removeColorMixerRegion = useAppStore((s) => s.removeColorMixerRegion);
  const updateColorMixerRegion = useAppStore((s) => s.updateColorMixerRegion);
  const [selectedRegionIndex, setSelectedRegionIndex] = useState<number | null>(null);
  const previousRegionCount = useRef(colorMixer.regions.length);
  const colorGrading = useAppStore((s) => s.developEdl.color_grading);
  const setColorGradingWheel = useAppStore((s) => s.setColorGradingWheel);
  const setColorGradingBalance = useAppStore((s) => s.setColorGradingBalance);
  const setColorGradingBlending = useAppStore((s) => s.setColorGradingBlending);
  const calibration = useAppStore((s) => s.developEdl.calibration);
  const setCalibrationPrimaryField = useAppStore((s) => s.setCalibrationPrimaryField);
  const setCalibrationShadowTint = useAppStore((s) => s.setCalibrationShadowTint);
  const setCalibrationCameraProfile = useAppStore((s) => s.setCalibrationCameraProfile);

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
    <aside className="flex w-72 shrink-0 flex-col gap-4 overflow-y-auto border-l border-border bg-bg-raised p-3" aria-label="Entwickeln">
      <div className="flex items-center justify-between">
        <h2 className="text-sm font-semibold text-text-primary">Entwickeln</h2>
        <div className="flex gap-1">
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
        </div>
      </div>

      {!selectedPhotoId && <p className="text-xs text-text-muted">Kein Foto ausgewählt.</p>}

      {selectedPhotoId && lastLatencyMs !== null && (
        <p className="text-xs text-text-muted" title="Ende-zu-Ende-Antwortzeit der letzten Regler-Vorschau (IPC + Dekodierung/Rendern, ohne Neuzeichnen im Browser) — siehe PLAN.md Phase 2 Schritt 7">
          Letztes Rendering: {Math.round(lastLatencyMs)} ms
        </p>
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

          <fieldset className="flex flex-col gap-3">
            {/* Nur für Assistive Technologien / Tests: gruppiert diese
                Regler unter einem eigenen Namen, damit z. B. "Sättigung"
                hier eindeutig von der gleichnamigen HSL-Band-Regler
                unterscheidbar bleibt (beide Abschnitte sind gleichzeitig
                sichtbar). */}
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

          <fieldset className="flex flex-col gap-2">
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

          <fieldset className="flex flex-col gap-2">
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

          <fieldset className="flex flex-col gap-2">
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

          <fieldset className="flex flex-col gap-3">
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
        </>
      )}
    </aside>
  );
}
