import { useState } from "react";
import { useShallow } from "zustand/react/shallow";

import {
  AI_MASK_KIND_LABELS,
  BASIC_SLIDER_SPECS,
  BLEND_MODE_OPTIONS,
  COLOR_GRADING_WHEEL_TABS,
  COLOR_MIXER_REGION_SLIDER_SPECS,
  COLOR_NR_SLIDER_SPECS,
  CURVE_CHANNEL_TABS,
  HSL_BAND_SLIDER_SPECS,
  HSL_BAND_TABS,
  LUMINANCE_NR_SLIDER_SPECS,
  MASK_SLIDER_SPECS,
  MAX_COLOR_MIXER_REGIONS,
  readBasicField,
  SHARPEN_SLIDER_SPECS,
  type ColorMixerRegion,
  type CurvesAdjustment,
  type DetailsSliderKey,
  type HslAdjustment,
  type SliderSpec,
} from "../lib/edl";
import { AI_MASK_KINDS, selectActivePhotos, type MaskKind } from "../store";
import { MASK_KIND_LABEL, useAppStore } from "../store";
import { ColorWheel } from "./ColorWheel";
import { CurveEditor } from "./CurveEditor";
import { DevelopSlider } from "./DevelopSlider";
import { PaletteFrame } from "./PaletteFrame";

/** Die sechs Maskentypen, in derselben Reihenfolge wie die „+ …"-Knöpfe
 * oben im Panel — wiederverwendet für „+ Komponente hinzufügen". */
const MASK_KINDS: readonly MaskKind[] = ["LinearGradient", "RadialGradient", "Brush", "ColorRange", "LuminanceRange", "BlurDepthApprox"];

/** Entwurfsregler für den *nächsten* im Viewer gemalten Pinselstrich
 * (Phase 6 Schritt 4) — analog zu `DevelopPanel.tsx`s
 * `REPAIR_RADIUS_SPEC`/`REPAIR_FEATHER_SPEC`. */
const BRUSH_RADIUS_SPEC: SliderSpec = { key: "radius", label: "Pinsel: Radius (% der Bildbreite)", min: 1, max: 50, fineStep: 0.5, coarseStep: 5, neutral: 5 };
const BRUSH_FEATHER_SPEC: SliderSpec = { key: "feather", label: "Pinsel: Weiche Kante (% der Bildbreite)", min: 0, max: 25, fineStep: 0.5, coarseStep: 2, neutral: 2 };

/** Regler für Farbbereich-/Luminanzbereich-Masken (Phase 6 Schritt 5) —
 * `tolerance`/`feather`/`range_min`/`range_max` sind im EDL `0.0..=1.0`,
 * die Regler zeigen sie wie überall sonst als Prozent an. */
const COLOR_RANGE_TOLERANCE_SPEC: SliderSpec = { key: "tolerance", label: "Toleranz (%)", min: 0, max: 100, fineStep: 1, coarseStep: 5, neutral: 15 };
const COLOR_RANGE_FEATHER_SPEC: SliderSpec = { key: "feather", label: "Weiche Kante (%)", min: 0, max: 100, fineStep: 1, coarseStep: 5, neutral: 10 };
const LUMINANCE_RANGE_MIN_SPEC: SliderSpec = { key: "range_min", label: "Untere Grenze (%)", min: 0, max: 100, fineStep: 1, coarseStep: 5, neutral: 50 };
const LUMINANCE_RANGE_MAX_SPEC: SliderSpec = { key: "range_max", label: "Obere Grenze (%)", min: 0, max: 100, fineStep: 1, coarseStep: 5, neutral: 100 };
const LUMINANCE_RANGE_FEATHER_SPEC: SliderSpec = { key: "feather", label: "Weiche Kante (%)", min: 0, max: 100, fineStep: 1, coarseStep: 5, neutral: 10 };
/** Phase 11 Schritt 7 (siehe DECISIONS.md ADR-0038) — `threshold` ist im
 * EDL `0.0..=1.0`, wie oben als Prozent angezeigt. */
const BLUR_DEPTH_APPROX_THRESHOLD_SPEC: SliderSpec = { key: "threshold", label: "Schärfe-Schwellwert (%)", min: 0, max: 100, fineStep: 1, coarseStep: 5, neutral: 50 };

/**
 * Maskenverwaltung (Phase 6 Schritt 3-7, siehe `DECISIONS.md` ADR-0032) —
 * Liste vorhandener Masken (mit Drag-&-Drop-Umsortierung, Duplizieren,
 * Übertragen auf ein anderes Foto), Gruppen, wiederverwendbare Bausteine,
 * Anlegen neuer Masken (Linearer/Radialer Verlauf, Pinsel, Farbbereich,
 * Luminanzbereich), Auswahl zum Bearbeiten, volle Sechs-Sektionen-
 * Reglerabdeckung für die ausgewählte Maske (Grundeinstellungen, Kurven,
 * HSL, Farbmischer, Color Grading, Details — dieselben Widgets wie
 * `DevelopPanel.tsx`, hier auf `mask.adjustments` statt `developEdl`
 * gerichtet). Wie `DevelopPanel` nur sichtbar, während das
 * Entwickeln-Panel offen ist.
 *
 * Die Farbbereich-Zielfarbe und Farbmischer-Regionen werden per Bildklick
 * aufgenommen (`maskColorRangePickerActive`/`maskColorMixerPickerActive`)
 * — derselbe Viewer-Sampling-Code wie die Weißabgleich-Pipette/der
 * globale Farbmischer (siehe `Viewer.tsx`). **Bewusste Vereinfachung:**
 * `masks.rs`s `ColorRange` vergleicht im linearen Arbeitsraum (siehe
 * dessen Moduldoku), der Bildklick liefert aber den bereits gerenderten,
 * display-referred Vorschau-Frame — dieselbe Näherung, die die
 * Weißabgleich-Pipette/der Farbmischer schon seit Phase 4 verwenden.
 *
 * **Maskenkombination (Schritt 6, `SPEC.md` §5):** eine Maske kann aus
 * mehreren Komponenten bestehen, jede mit ihrer eigenen Geometrie und
 * `combine`-Verrechnung (Hinzufügen/Subtrahieren/Schneiden) gegen die
 * vorangehenden Komponenten derselben Maske. Die „Komponenten"-Liste
 * unten wählt aus, welche Komponente gerade im Viewer bearbeitet wird
 * (`selectedMaskComponentIndex`) — dieselbe Maske kann so z. B. einen
 * Pinselstrich UND einen Farbbereich kombinieren.
 *
 * **Bausteine (Schritt 7) sind bewusst nur clientseitig für diese
 * Sitzung gehalten** (kein Backend-Katalog-Eintrag wie bei Presets aus
 * Phase 5) — siehe `store/index.ts`s `maskBuildingBlocks`-Moduldoku für
 * die Begründung.
 */
export function MasksPanel() {
  const open = useAppStore((s) => s.developPanelOpen);
  const masks = useAppStore((s) => s.developEdl.masks);
  const maskGroups = useAppStore((s) => s.developEdl.mask_groups);
  const selectedMaskId = useAppStore((s) => s.selectedMaskId);
  const selectMask = useAppStore((s) => s.selectMask);
  const maskOverlayVisible = useAppStore((s) => s.maskOverlayVisible);
  const toggleMaskOverlay = useAppStore((s) => s.toggleMaskOverlay);
  const addMask = useAppStore((s) => s.addMask);
  const removeMask = useAppStore((s) => s.removeMask);
  const setMaskVisible = useAppStore((s) => s.setMaskVisible);
  const renameMask = useAppStore((s) => s.renameMask);
  const setMaskBlendMode = useAppStore((s) => s.setMaskBlendMode);
  const setMaskOpacity = useAppStore((s) => s.setMaskOpacity);
  const setMaskFeather = useAppStore((s) => s.setMaskFeather);
  const commitMaskDrag = useAppStore((s) => s.commitMaskDrag);
  const setMaskBasicField = useAppStore((s) => s.setMaskBasicField);
  const selectedPhotoId = useAppStore((s) => s.selectedPhotoId);
  const maskBrushDraftRadius = useAppStore((s) => s.maskBrushDraftRadius);
  const maskBrushDraftFeather = useAppStore((s) => s.maskBrushDraftFeather);
  const setMaskBrushDraftField = useAppStore((s) => s.setMaskBrushDraftField);
  const removeMaskBrushStroke = useAppStore((s) => s.removeMaskBrushStroke);
  const updateMaskGeometry = useAppStore((s) => s.updateMaskGeometry);
  const maskColorRangePickerActive = useAppStore((s) => s.maskColorRangePickerActive);
  const toggleMaskColorRangePicker = useAppStore((s) => s.toggleMaskColorRangePicker);
  const selectedMaskComponentIndex = useAppStore((s) => s.selectedMaskComponentIndex);
  const selectMaskComponent = useAppStore((s) => s.selectMaskComponent);
  const addMaskComponent = useAppStore((s) => s.addMaskComponent);
  const removeMaskComponent = useAppStore((s) => s.removeMaskComponent);
  const setMaskComponentCombine = useAppStore((s) => s.setMaskComponentCombine);
  const setMaskComponentInvert = useAppStore((s) => s.setMaskComponentInvert);

  // Sechs-Sektionen-Regler (Schritt 7)
  const setMaskCurveChannel = useAppStore((s) => s.setMaskCurveChannel);
  const setMaskHslBandField = useAppStore((s) => s.setMaskHslBandField);
  const maskColorMixerPickerActive = useAppStore((s) => s.maskColorMixerPickerActive);
  const toggleMaskColorMixerPicker = useAppStore((s) => s.toggleMaskColorMixerPicker);
  const removeMaskColorMixerRegion = useAppStore((s) => s.removeMaskColorMixerRegion);
  const updateMaskColorMixerRegion = useAppStore((s) => s.updateMaskColorMixerRegion);
  const setMaskColorGradingWheel = useAppStore((s) => s.setMaskColorGradingWheel);
  const setMaskColorGradingBalance = useAppStore((s) => s.setMaskColorGradingBalance);
  const setMaskColorGradingBlending = useAppStore((s) => s.setMaskColorGradingBlending);
  const setMaskDetailsField = useAppStore((s) => s.setMaskDetailsField);
  const setMaskDetailsUseDeconvolutionSharpen = useAppStore((s) => s.setMaskDetailsUseDeconvolutionSharpen);
  const [activeCurveChannel, setActiveCurveChannel] = useState<keyof CurvesAdjustment>("rgb");
  const [activeHslBand, setActiveHslBand] = useState<keyof HslAdjustment>("red");
  const [selectedColorMixerRegionIndex, setSelectedColorMixerRegionIndex] = useState<number | null>(null);

  // Gruppen (Schritt 7)
  const addMaskGroup = useAppStore((s) => s.addMaskGroup);
  const renameMaskGroup = useAppStore((s) => s.renameMaskGroup);
  const removeMaskGroup = useAppStore((s) => s.removeMaskGroup);
  const setMaskGroupVisible = useAppStore((s) => s.setMaskGroupVisible);
  const setMaskGroup = useAppStore((s) => s.setMaskGroup);

  // Duplizieren/Sortieren/Übertragen/Bausteine (Schritt 7)
  const duplicateMask = useAppStore((s) => s.duplicateMask);
  const reorderMask = useAppStore((s) => s.reorderMask);
  const transferMaskToPhoto = useAppStore((s) => s.transferMaskToPhoto);
  const maskBuildingBlocks = useAppStore((s) => s.maskBuildingBlocks);
  const saveMaskAsBuildingBlock = useAppStore((s) => s.saveMaskAsBuildingBlock);
  const applyMaskBuildingBlock = useAppStore((s) => s.applyMaskBuildingBlock);
  const removeMaskBuildingBlock = useAppStore((s) => s.removeMaskBuildingBlock);
  const activePhotos = useAppStore(useShallow(selectActivePhotos));
  const [transferTargetPhotoId, setTransferTargetPhotoId] = useState<string | null>(null);
  const [dragMaskIndex, setDragMaskIndex] = useState<number | null>(null);

  // Die fünf KI-Masken (Phase 7 Schritt 2, siehe DECISIONS.md ADR-0033).
  const aiMaskClickPickerActive = useAppStore((s) => s.aiMaskClickPickerActive);
  const toggleAiMaskClickPicker = useAppStore((s) => s.toggleAiMaskClickPicker);
  const aiMaskLoading = useAppStore((s) => s.aiMaskLoading);
  const addAiMask = useAppStore((s) => s.addAiMask);

  if (!open) return null;

  const selectedMask = masks.find((m) => m.id === selectedMaskId) ?? null;
  const selectedMaskGeometry = selectedMask?.components[selectedMaskComponentIndex]?.geometry;
  const otherPhotos = activePhotos.filter((p) => p.id !== selectedPhotoId);

  function handleRename(maskId: string, currentName: string, event: React.MouseEvent) {
    event.stopPropagation();
    const name = window.prompt("Maske umbenennen", currentName);
    if (name) renameMask(maskId, name);
  }

  function handleAddGroup() {
    const name = window.prompt("Neue Maskengruppe benennen");
    if (name) addMaskGroup(name);
  }

  function handleRenameGroup(groupId: string, currentName: string) {
    const name = window.prompt("Maskengruppe umbenennen", currentName);
    if (name) renameMaskGroup(groupId, name);
  }

  function handleSaveBuildingBlock(maskId: string) {
    const name = window.prompt("Baustein benennen");
    if (name) saveMaskAsBuildingBlock(maskId, name);
  }

  return (
    <PaletteFrame id="masks" side="right" defaultWidth={256} label="Masken" className="gap-3 border-l border-border bg-bg-raised p-3">
      <div className="flex items-center justify-between">
        <h2 id="stage-masks" className="text-sm font-semibold text-text-primary">Masken</h2>
        <button
          type="button"
          onClick={toggleMaskOverlay}
          aria-pressed={maskOverlayVisible}
          title="Masken-Farbüberlagerung im Viewer ein-/ausblenden (Taste O)"
          className={`rounded border px-2 py-0.5 text-xs ${maskOverlayVisible ? "border-accent bg-accent/10 text-accent" : "border-border text-text-secondary hover:border-accent"}`}
        >
          Überlagerung (O)
        </button>
      </div>

      <div className="grid grid-cols-2 gap-1">
        <button
          type="button"
          onClick={() => addMask("LinearGradient")}
          disabled={!selectedPhotoId}
          className="rounded border border-border px-2 py-1 text-xs text-text-secondary hover:bg-bg-panel disabled:cursor-not-allowed disabled:opacity-40"
        >
          + Linearer Verlauf
        </button>
        <button
          type="button"
          onClick={() => addMask("RadialGradient")}
          disabled={!selectedPhotoId}
          className="rounded border border-border px-2 py-1 text-xs text-text-secondary hover:bg-bg-panel disabled:cursor-not-allowed disabled:opacity-40"
        >
          + Radialer Verlauf
        </button>
        <button
          type="button"
          onClick={() => addMask("Brush")}
          disabled={!selectedPhotoId}
          className="rounded border border-border px-2 py-1 text-xs text-text-secondary hover:bg-bg-panel disabled:cursor-not-allowed disabled:opacity-40"
        >
          + Pinsel
        </button>
        <button
          type="button"
          onClick={() => addMask("ColorRange")}
          disabled={!selectedPhotoId}
          className="rounded border border-border px-2 py-1 text-xs text-text-secondary hover:bg-bg-panel disabled:cursor-not-allowed disabled:opacity-40"
        >
          + Farbbereich
        </button>
        <button
          type="button"
          onClick={() => addMask("LuminanceRange")}
          disabled={!selectedPhotoId}
          className="rounded border border-border px-2 py-1 text-xs text-text-secondary hover:bg-bg-panel disabled:cursor-not-allowed disabled:opacity-40"
        >
          + Luminanzbereich
        </button>
        <button
          type="button"
          onClick={() => addMask("BlurDepthApprox")}
          disabled={!selectedPhotoId}
          title="Keine echte Tiefenkarte — eine Laplace-Varianz-Schärfeheuristik, funktioniert nur bei echtem Schärfentiefe-Effekt (siehe DECISIONS.md ADR-0038)"
          className="col-span-2 rounded border border-border px-2 py-1 text-xs text-text-secondary hover:bg-bg-panel disabled:cursor-not-allowed disabled:opacity-40"
        >
          + Unschärfe-basierte Tiefennäherung
        </button>
      </div>

      {/* Die fünf KI-Masken (Phase 7 Schritt 2, siehe DECISIONS.md
          ADR-0033) — klassische Bildverarbeitungsheuristiken statt echter
          ONNX-Modelle, siehe `apx-ai::segmentation`s Moduldoku. "Objekte"
          braucht einen Klickpunkt im Bild statt sofort zu erzeugen. */}
      <div className="flex flex-col gap-1 border-t border-border pt-2">
        <h3 className="text-xs font-medium text-text-secondary">KI-Maske hinzufügen</h3>
        <div className="grid grid-cols-2 gap-1">
          {AI_MASK_KINDS.map((kind) =>
            kind === "ClickRegion" ? (
              <button
                key={kind}
                type="button"
                onClick={toggleAiMaskClickPicker}
                disabled={!selectedPhotoId || aiMaskLoading !== null}
                aria-pressed={aiMaskClickPickerActive}
                className={`rounded border px-2 py-1 text-xs disabled:cursor-not-allowed disabled:opacity-40 ${
                  aiMaskClickPickerActive ? "border-accent bg-accent/10 text-accent" : "border-border text-text-secondary hover:bg-bg-panel"
                }`}
              >
                {AI_MASK_KIND_LABELS[kind]}…
              </button>
            ) : (
              <button
                key={kind}
                type="button"
                onClick={() => void addAiMask(kind)}
                disabled={!selectedPhotoId || aiMaskLoading !== null}
                className="rounded border border-border px-2 py-1 text-xs text-text-secondary hover:bg-bg-panel disabled:cursor-not-allowed disabled:opacity-40"
              >
                {aiMaskLoading === kind ? "…" : AI_MASK_KIND_LABELS[kind]}
              </button>
            ),
          )}
        </div>
        {aiMaskClickPickerActive && <p className="text-xs text-accent">Klicken Sie ins Bild, um den Objektbereich auszuwählen.</p>}
      </div>

      <ul className="flex flex-col gap-1">
        {masks.map((mask, index) => (
          <li
            key={mask.id}
            draggable
            onDragStart={() => setDragMaskIndex(index)}
            onDragOver={(event) => event.preventDefault()}
            onDrop={(event) => {
              event.preventDefault();
              if (dragMaskIndex !== null) reorderMask(dragMaskIndex, index);
              setDragMaskIndex(null);
            }}
            className={`flex cursor-grab flex-col gap-1 rounded border px-2 py-1.5 text-sm ${
              mask.id === selectedMaskId ? "border-accent bg-accent/10" : "border-border"
            }`}
          >
            <div className="flex items-center gap-1.5">
              <button
                type="button"
                onClick={() => setMaskVisible(mask.id, !mask.visible)}
                aria-label={mask.visible ? `${mask.name} ausblenden` : `${mask.name} einblenden`}
                aria-pressed={mask.visible}
                className={`shrink-0 ${mask.visible ? "text-accent" : "text-text-muted"}`}
                title="Sichtbarkeit"
              >
                {mask.visible ? "👁" : "🚫"}
              </button>
              <button
                type="button"
                onClick={() => selectMask(mask.id === selectedMaskId ? null : mask.id)}
                className="min-w-0 flex-1 truncate text-left text-text-primary hover:underline"
              >
                {mask.name}
              </button>
              <span role="button" tabIndex={0} onClick={(event) => handleRename(mask.id, mask.name, event)} className="shrink-0 text-text-muted hover:text-accent" title="Umbenennen">
                ✎
              </span>
              <button type="button" onClick={() => duplicateMask(mask.id)} className="shrink-0 text-text-muted hover:text-accent" title="Duplizieren">
                ⧉
              </button>
              <button
                type="button"
                onClick={() => removeMask(mask.id)}
                className="shrink-0 text-danger"
                aria-label={`${mask.name} löschen`}
              >
                ×
              </button>
            </div>
            {maskGroups.length > 0 && (
              <label className="flex items-center gap-2 text-xs text-text-secondary">
                Gruppe
                <select
                  aria-label={`${mask.name}: Gruppe`}
                  value={mask.group_id ?? ""}
                  onChange={(event) => setMaskGroup(mask.id, event.target.value || null)}
                  className="flex-1 rounded border border-border bg-bg-panel px-1.5 py-0.5"
                >
                  <option value="">Keine</option>
                  {maskGroups.map((group) => (
                    <option key={group.id} value={group.id}>
                      {group.name}
                    </option>
                  ))}
                </select>
              </label>
            )}
          </li>
        ))}
        {masks.length === 0 && <li className="text-xs text-text-muted">Keine Masken vorhanden.</li>}
      </ul>

      <div className="flex flex-col gap-1 border-t border-border pt-2">
        <div className="flex items-center justify-between">
          <h3 className="text-xs font-medium text-text-secondary">Gruppen</h3>
          <button type="button" onClick={handleAddGroup} className="text-xs text-accent hover:underline">
            + Neue Gruppe
          </button>
        </div>
        {maskGroups.length === 0 && <p className="text-xs text-text-muted">Keine Gruppen angelegt.</p>}
        <ul className="flex flex-col gap-1">
          {maskGroups.map((group) => (
            <li key={group.id} className="flex items-center gap-1.5 rounded border border-border px-2 py-1 text-xs">
              <button
                type="button"
                onClick={() => setMaskGroupVisible(group.id, !group.visible)}
                aria-label={group.visible ? `Gruppe ${group.name} ausblenden` : `Gruppe ${group.name} einblenden`}
                aria-pressed={group.visible}
                className={`shrink-0 ${group.visible ? "text-accent" : "text-text-muted"}`}
              >
                {group.visible ? "👁" : "🚫"}
              </button>
              <button type="button" onClick={() => handleRenameGroup(group.id, group.name)} className="min-w-0 flex-1 truncate text-left text-text-primary hover:underline">
                {group.name}
              </button>
              <button type="button" onClick={() => removeMaskGroup(group.id)} className="shrink-0 text-danger" aria-label={`Gruppe ${group.name} entfernen`}>
                ×
              </button>
            </li>
          ))}
        </ul>
      </div>

      <div className="flex flex-col gap-1 border-t border-border pt-2">
        <h3 className="text-xs font-medium text-text-secondary">Bausteine</h3>
        {maskBuildingBlocks.length === 0 && <p className="text-xs text-text-muted">Noch keine Bausteine gespeichert.</p>}
        <ul className="flex flex-col gap-1">
          {maskBuildingBlocks.map((block) => (
            <li key={block.id} className="flex items-center gap-1.5 rounded border border-border px-2 py-1 text-xs">
              <button type="button" onClick={() => applyMaskBuildingBlock(block.id)} className="min-w-0 flex-1 truncate text-left text-text-primary hover:underline" title="Als neue Maske anwenden">
                {block.name}
              </button>
              <button type="button" onClick={() => removeMaskBuildingBlock(block.id)} className="shrink-0 text-danger" aria-label={`Baustein ${block.name} entfernen`}>
                ×
              </button>
            </li>
          ))}
        </ul>
        {selectedMask && (
          <button type="button" onClick={() => handleSaveBuildingBlock(selectedMask.id)} className="rounded border border-border px-2 py-1 text-xs text-text-secondary hover:bg-bg-panel">
            Aktuelle Maske als Baustein speichern
          </button>
        )}
      </div>

      {selectedMask && otherPhotos.length > 0 && (
        <div className="flex flex-col gap-1 border-t border-border pt-2">
          <label className="flex items-center gap-2 text-xs text-text-secondary">
            Auf anderes Foto übertragen
            <select
              aria-label="Zielfoto für Maskenübertragung"
              value={transferTargetPhotoId ?? ""}
              onChange={(event) => setTransferTargetPhotoId(event.target.value || null)}
              className="flex-1 rounded border border-border bg-bg-panel px-1.5 py-0.5"
            >
              <option value="">Foto wählen…</option>
              {otherPhotos.map((photo) => (
                <option key={photo.id} value={photo.id}>
                  {photo.filename}
                </option>
              ))}
            </select>
          </label>
          <button
            type="button"
            disabled={!transferTargetPhotoId}
            onClick={() => transferTargetPhotoId && void transferMaskToPhoto(selectedMask.id, transferTargetPhotoId)}
            className="rounded border border-border px-2 py-1 text-xs text-text-secondary hover:bg-bg-panel disabled:cursor-not-allowed disabled:opacity-40"
          >
            Übertragen
          </button>
        </div>
      )}

      {selectedMask && (
        <div className="flex flex-col gap-2 border-t border-border pt-2">
          <label className="flex items-center gap-2 text-xs text-text-secondary">
            Mischmodus
            <select
              aria-label="Mischmodus"
              value={selectedMask.blend_mode}
              onChange={(event) => setMaskBlendMode(selectedMask.id, event.target.value as (typeof BLEND_MODE_OPTIONS)[number]["value"])}
              className="flex-1 rounded border border-border bg-bg-panel px-2 py-1 text-xs"
            >
              {BLEND_MODE_OPTIONS.map((option) => (
                <option key={option.value} value={option.value}>
                  {option.label}
                </option>
              ))}
            </select>
          </label>

          <h4 className="text-xs font-medium text-text-secondary">Komponenten</h4>
          <ul className="flex flex-col gap-1">
            {selectedMask.components.map((component, index) => (
              <li
                key={index}
                className={`flex flex-col gap-1 rounded border px-2 py-1.5 text-xs ${
                  index === selectedMaskComponentIndex ? "border-accent bg-accent/10" : "border-border"
                }`}
              >
                <div className="flex items-center gap-1.5">
                  <button type="button" onClick={() => selectMaskComponent(index)} className="min-w-0 flex-1 truncate text-left text-text-primary hover:underline">
                    {index + 1}. {MASK_KIND_LABEL[component.geometry.kind as MaskKind]}
                  </button>
                  {selectedMask.components.length > 1 && (
                    <button
                      type="button"
                      onClick={() => removeMaskComponent(selectedMask.id, index)}
                      className="shrink-0 text-danger"
                      aria-label={`Komponente ${index + 1} entfernen`}
                    >
                      ×
                    </button>
                  )}
                </div>
                {index > 0 && (
                  <label className="flex items-center gap-2 text-text-secondary">
                    Verrechnung
                    <select
                      aria-label={`Komponente ${index + 1}: Verrechnung`}
                      value={component.combine}
                      onChange={(event) => setMaskComponentCombine(selectedMask.id, index, event.target.value as typeof component.combine)}
                      className="flex-1 rounded border border-border bg-bg-panel px-1.5 py-0.5"
                    >
                      <option value="Add">Hinzufügen</option>
                      <option value="Subtract">Subtrahieren</option>
                      <option value="Intersect">Schneiden</option>
                    </select>
                  </label>
                )}
                <label className="flex items-center gap-2 text-text-secondary">
                  <input
                    type="checkbox"
                    aria-label={`Komponente ${index + 1}: Invertieren`}
                    checked={component.invert}
                    onChange={(event) => setMaskComponentInvert(selectedMask.id, index, event.target.checked)}
                  />
                  Invertieren
                </label>
              </li>
            ))}
          </ul>
          <div className="grid grid-cols-2 gap-1">
            {MASK_KINDS.map((kind) => (
              <button
                key={kind}
                type="button"
                onClick={() => addMaskComponent(selectedMask.id, kind)}
                className="rounded border border-border px-2 py-1 text-xs text-text-secondary hover:bg-bg-panel"
              >
                + Komponente: {MASK_KIND_LABEL[kind]}
              </button>
            ))}
          </div>
        </div>
      )}

      {selectedMask && selectedMaskGeometry?.kind === "Brush" && (
        <div className="flex flex-col gap-2 border-t border-border pt-2">
          <p className="text-xs text-text-muted">Ins Bild klicken und ziehen, um zu malen.</p>
          <DevelopSlider
            spec={BRUSH_RADIUS_SPEC}
            value={maskBrushDraftRadius * 100}
            onChange={(value) => setMaskBrushDraftField("radius", value / 100)}
            onCommit={() => {}}
          />
          <DevelopSlider
            spec={BRUSH_FEATHER_SPEC}
            value={maskBrushDraftFeather * 100}
            onChange={(value) => setMaskBrushDraftField("feather", value / 100)}
            onCommit={() => {}}
          />
          {selectedMaskGeometry.strokes.length > 0 && (
            <ul className="flex flex-col gap-1 text-xs text-text-secondary">
              {selectedMaskGeometry.strokes.map((_, index) => (
                <li key={index} className="flex items-center justify-between rounded border border-border px-2 py-1">
                  <span>Pinselstrich {index + 1}</span>
                  <button type="button" onClick={() => removeMaskBrushStroke(selectedMask.id, index)} className="text-danger underline">
                    Entfernen
                  </button>
                </li>
              ))}
            </ul>
          )}
        </div>
      )}

      {selectedMask && selectedMaskGeometry?.kind === "ColorRange" && (
        <div className="flex flex-col gap-2 border-t border-border pt-2">
          <div className="flex items-center gap-2">
            <span
              className="h-5 w-5 shrink-0 rounded border border-border"
              style={{
                backgroundColor: `rgb(${Math.round(selectedMaskGeometry.target_r * 255)}, ${Math.round(selectedMaskGeometry.target_g * 255)}, ${Math.round(
                  selectedMaskGeometry.target_b * 255,
                )})`,
              }}
              title="Aktuelle Zielfarbe"
            />
            <button
              type="button"
              onClick={toggleMaskColorRangePicker}
              aria-pressed={maskColorRangePickerActive}
              className={`flex-1 rounded border px-2 py-1 text-xs ${
                maskColorRangePickerActive ? "border-accent bg-accent/10 text-accent" : "border-border bg-bg-panel text-text-secondary hover:border-accent"
              }`}
            >
              Farbe aufnehmen
            </button>
          </div>
          {maskColorRangePickerActive && <p className="text-xs text-accent">Klicken Sie ins Bild, um die Zielfarbe zu setzen.</p>}
          <DevelopSlider
            spec={COLOR_RANGE_TOLERANCE_SPEC}
            value={selectedMaskGeometry.tolerance * 100}
            onChange={(value) => updateMaskGeometry(selectedMask.id, { ...selectedMaskGeometry, tolerance: value / 100 })}
            onCommit={commitMaskDrag}
          />
          <DevelopSlider
            spec={COLOR_RANGE_FEATHER_SPEC}
            value={selectedMaskGeometry.feather * 100}
            onChange={(value) => updateMaskGeometry(selectedMask.id, { ...selectedMaskGeometry, feather: value / 100 })}
            onCommit={commitMaskDrag}
          />
        </div>
      )}

      {selectedMask && selectedMaskGeometry?.kind === "LuminanceRange" && (
        <div className="flex flex-col gap-2 border-t border-border pt-2">
          <DevelopSlider
            spec={LUMINANCE_RANGE_MIN_SPEC}
            value={selectedMaskGeometry.range_min * 100}
            onChange={(value) => updateMaskGeometry(selectedMask.id, { ...selectedMaskGeometry, range_min: value / 100 })}
            onCommit={commitMaskDrag}
          />
          <DevelopSlider
            spec={LUMINANCE_RANGE_MAX_SPEC}
            value={selectedMaskGeometry.range_max * 100}
            onChange={(value) => updateMaskGeometry(selectedMask.id, { ...selectedMaskGeometry, range_max: value / 100 })}
            onCommit={commitMaskDrag}
          />
          <DevelopSlider
            spec={LUMINANCE_RANGE_FEATHER_SPEC}
            value={selectedMaskGeometry.feather * 100}
            onChange={(value) => updateMaskGeometry(selectedMask.id, { ...selectedMaskGeometry, feather: value / 100 })}
            onCommit={commitMaskDrag}
          />
        </div>
      )}

      {selectedMask && selectedMaskGeometry?.kind === "BlurDepthApprox" && (
        <div className="flex flex-col gap-2 border-t border-border pt-2">
          <p className="text-xs text-text-muted">
            Keine echte Tiefenkarte — eine Unschärfe-Heuristik, die nur bei echtem Schärfentiefe-Effekt (z. B. offene Blende) eine
            sinnvolle Trennung liefert.
          </p>
          <DevelopSlider
            spec={BLUR_DEPTH_APPROX_THRESHOLD_SPEC}
            value={selectedMaskGeometry.threshold * 100}
            onChange={(value) => updateMaskGeometry(selectedMask.id, { ...selectedMaskGeometry, threshold: value / 100 })}
            onCommit={commitMaskDrag}
          />
        </div>
      )}

      {selectedMask && (
        <div className="flex flex-col gap-2 border-t border-border pt-2">
          <h3 className="text-xs font-medium text-text-secondary">{selectedMask.name}</h3>
          {MASK_SLIDER_SPECS.map((spec) => (
            <DevelopSlider
              key={spec.key}
              spec={spec}
              value={selectedMask[spec.key as "opacity" | "feather"]}
              onChange={(value) => (spec.key === "opacity" ? setMaskOpacity(selectedMask.id, value) : setMaskFeather(selectedMask.id, value))}
              onCommit={commitMaskDrag}
            />
          ))}
        </div>
      )}

      {selectedMask && (
        <fieldset className="flex flex-col gap-2 border-t border-border pt-2">
          <legend className="mb-1 text-xs font-medium text-text-secondary">Grundeinstellungen</legend>
          {BASIC_SLIDER_SPECS.map((spec) => (
            <DevelopSlider
              key={spec.key}
              spec={spec}
              value={readBasicField(selectedMask.adjustments.basic, spec.key)}
              onChange={(value) => setMaskBasicField(selectedMask.id, spec.key, value)}
              onCommit={commitMaskDrag}
            />
          ))}
        </fieldset>
      )}

      {selectedMask && (
        <fieldset className="flex flex-col gap-2 border-t border-border pt-2">
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
            channel={selectedMask.adjustments.curves[activeCurveChannel]}
            onChange={(next) => setMaskCurveChannel(selectedMask.id, activeCurveChannel, next)}
            onCommit={commitMaskDrag}
          />
        </fieldset>
      )}

      {selectedMask && (
        <fieldset className="flex flex-col gap-2 border-t border-border pt-2">
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
                  value={selectedMask.adjustments.hsl[activeHslBand][field]}
                  onChange={(value) => setMaskHslBandField(selectedMask.id, activeHslBand, field, value)}
                  onCommit={commitMaskDrag}
                />
              );
            })}
          </div>
        </fieldset>
      )}

      {selectedMask && (
        <fieldset className="flex flex-col gap-2 border-t border-border pt-2">
          <legend className="mb-1 text-xs font-medium text-text-secondary">Farbmischer</legend>
          <button
            type="button"
            onClick={toggleMaskColorMixerPicker}
            disabled={selectedMask.adjustments.color_mixer.regions.length >= MAX_COLOR_MIXER_REGIONS && !maskColorMixerPickerActive}
            aria-pressed={maskColorMixerPickerActive}
            title="Region hinzufügen: ins Bild klicken, um eine neue Farbmischer-Region an dieser Farbe anzulegen"
            className={`rounded border px-2 py-1 text-xs disabled:cursor-not-allowed disabled:opacity-40 ${
              maskColorMixerPickerActive ? "border-accent bg-accent/10 text-accent" : "border-border bg-bg-panel hover:border-accent"
            }`}
          >
            Region hinzufügen
          </button>
          {maskColorMixerPickerActive && <p className="text-xs text-accent">Klicken Sie ins Bild, um eine Region an dieser Farbe anzulegen.</p>}
          {selectedMask.adjustments.color_mixer.regions.length === 0 && <p className="text-xs text-text-muted">Noch keine Regionen.</p>}
          <div className="flex flex-wrap gap-1">
            {selectedMask.adjustments.color_mixer.regions.map((region, index) => (
              <span key={index} className="flex items-center gap-1 rounded border border-border bg-bg-panel px-1 py-0.5 text-xs">
                <button
                  type="button"
                  onClick={() => setSelectedColorMixerRegionIndex(index)}
                  aria-pressed={selectedColorMixerRegionIndex === index}
                  className={selectedColorMixerRegionIndex === index ? "text-accent" : "text-text-secondary hover:text-accent"}
                >
                  {Math.round(region.target_hue_degrees)}°
                </button>
                <button
                  type="button"
                  onClick={() => {
                    removeMaskColorMixerRegion(selectedMask.id, index);
                    if (selectedColorMixerRegionIndex === index) setSelectedColorMixerRegionIndex(null);
                  }}
                  aria-label={`Region bei ${Math.round(region.target_hue_degrees)}° entfernen`}
                  className="text-text-muted hover:text-danger"
                >
                  ×
                </button>
              </span>
            ))}
          </div>
          {selectedColorMixerRegionIndex !== null && selectedMask.adjustments.color_mixer.regions[selectedColorMixerRegionIndex] && (
            <div className="flex flex-col gap-3">
              {COLOR_MIXER_REGION_SLIDER_SPECS.map((spec) => {
                const field = spec.key as keyof ColorMixerRegion;
                const region = selectedMask.adjustments.color_mixer.regions[selectedColorMixerRegionIndex];
                if (!region) return null;
                return (
                  <DevelopSlider
                    key={spec.key}
                    spec={spec}
                    value={region[field]}
                    onChange={(value) => updateMaskColorMixerRegion(selectedMask.id, selectedColorMixerRegionIndex, { [field]: value })}
                    onCommit={commitMaskDrag}
                  />
                );
              })}
            </div>
          )}
        </fieldset>
      )}

      {selectedMask && (
        <fieldset className="flex flex-col gap-2 border-t border-border pt-2">
          <legend className="mb-1 text-xs font-medium text-text-secondary">Color Grading</legend>
          <div className="flex flex-wrap justify-center gap-3">
            {COLOR_GRADING_WHEEL_TABS.map((tab) => (
              <ColorWheel
                key={tab.key}
                label={tab.label}
                wheel={selectedMask.adjustments.color_grading[tab.key]}
                onChange={(next) => setMaskColorGradingWheel(selectedMask.id, tab.key, next)}
                onCommit={commitMaskDrag}
              />
            ))}
          </div>
          <div className="flex flex-col gap-3">
            <DevelopSlider
              spec={{ key: "balance", label: "Balance", min: -100, max: 100, fineStep: 1, coarseStep: 10, neutral: 0 }}
              value={selectedMask.adjustments.color_grading.balance}
              onChange={(value) => setMaskColorGradingBalance(selectedMask.id, value)}
              onCommit={commitMaskDrag}
            />
            <DevelopSlider
              spec={{ key: "blending", label: "Überblendung", min: 0, max: 100, fineStep: 1, coarseStep: 10, neutral: 50 }}
              value={selectedMask.adjustments.color_grading.blending}
              onChange={(value) => setMaskColorGradingBlending(selectedMask.id, value)}
              onCommit={commitMaskDrag}
            />
          </div>
        </fieldset>
      )}

      {selectedMask && (
        <fieldset className="flex flex-col gap-3 border-t border-border pt-2">
          <legend className="mb-1 text-xs font-medium text-text-secondary">Details</legend>
          <div className="flex flex-col gap-2">
            {SHARPEN_SLIDER_SPECS.map((spec) => (
              <DevelopSlider
                key={spec.key}
                spec={spec}
                value={selectedMask.adjustments.details[spec.key as DetailsSliderKey]}
                onChange={(value) => setMaskDetailsField(selectedMask.id, spec.key as DetailsSliderKey, value)}
                onCommit={commitMaskDrag}
              />
            ))}
            <label className="flex items-center gap-2 text-xs text-text-secondary">
              <input
                type="checkbox"
                checked={selectedMask.adjustments.details.use_deconvolution_sharpen}
                onChange={(event) => setMaskDetailsUseDeconvolutionSharpen(selectedMask.id, event.target.checked)}
              />
              Deconvolution-Schärfung (Alternativmodus)
            </label>
          </div>
          <div className="flex flex-col gap-2">
            {LUMINANCE_NR_SLIDER_SPECS.map((spec) => (
              <DevelopSlider
                key={spec.key}
                spec={spec}
                value={selectedMask.adjustments.details[spec.key as DetailsSliderKey]}
                onChange={(value) => setMaskDetailsField(selectedMask.id, spec.key as DetailsSliderKey, value)}
                onCommit={commitMaskDrag}
              />
            ))}
          </div>
          <div className="flex flex-col gap-2">
            {COLOR_NR_SLIDER_SPECS.map((spec) => (
              <DevelopSlider
                key={spec.key}
                spec={spec}
                value={selectedMask.adjustments.details[spec.key as DetailsSliderKey]}
                onChange={(value) => setMaskDetailsField(selectedMask.id, spec.key as DetailsSliderKey, value)}
                onCommit={commitMaskDrag}
              />
            ))}
          </div>
        </fieldset>
      )}
    </PaletteFrame>
  );
}
