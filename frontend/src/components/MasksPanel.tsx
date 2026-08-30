import type { SliderSpec } from "../lib/edl";
import { BASIC_SLIDER_SPECS, MASK_SLIDER_SPECS, readBasicField } from "../lib/edl";
import { useAppStore } from "../store";
import { DevelopSlider } from "./DevelopSlider";

/** Die für Schritt 3 sichtbaren Grundeinstellungs-Regler pro Maske — eine
 * kleine, repräsentative Auswahl (Belichtung/Kontrast) statt aller zwölf,
 * um zu belegen, dass eine Maske ihre Werkzeuge tatsächlich anwendet. Die
 * vollständige Reglerabdeckung (alle sechs Werkzeugsektionen wie bei
 * einem Preset) ist Politur für Schritt 7. */
const MASK_BASIC_SLIDER_KEYS = ["exposure_ev", "contrast"];
const MASK_BASIC_SLIDER_SPECS = BASIC_SLIDER_SPECS.filter((spec) => MASK_BASIC_SLIDER_KEYS.includes(spec.key));

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

/**
 * Maskenverwaltung (Phase 6 Schritt 3-5, siehe `DECISIONS.md` ADR-0032) —
 * Liste vorhandener Masken, Anlegen neuer Masken (Linearer/Radialer
 * Verlauf, Pinsel, Farbbereich, Luminanzbereich), Auswahl zum Bearbeiten,
 * kleine Reglerauswahl für die ausgewählte Maske. Wie `DevelopPanel` nur
 * sichtbar, während das Entwickeln-Panel offen ist.
 *
 * Die Farbbereich-Zielfarbe wird per Bildklick aufgenommen
 * (`maskColorRangePickerActive`/`toggleMaskColorRangePicker`) — derselbe
 * Viewer-Sampling-Code wie die Weißabgleich-Pipette/der Farbmischer
 * (siehe `Viewer.tsx`). **Bewusste Vereinfachung:** `masks.rs`s
 * `ColorRange` vergleicht im linearen Arbeitsraum (siehe dessen
 * Moduldoku), der Bildklick liefert aber den bereits gerenderten,
 * display-referred Vorschau-Frame — dieselbe Näherung, die die
 * Weißabgleich-Pipette/der Farbmischer schon seit Phase 4 verwenden.
 */
export function MasksPanel() {
  const open = useAppStore((s) => s.developPanelOpen);
  const masks = useAppStore((s) => s.developEdl.masks);
  const selectedMaskId = useAppStore((s) => s.selectedMaskId);
  const selectMask = useAppStore((s) => s.selectMask);
  const addMask = useAppStore((s) => s.addMask);
  const removeMask = useAppStore((s) => s.removeMask);
  const setMaskVisible = useAppStore((s) => s.setMaskVisible);
  const renameMask = useAppStore((s) => s.renameMask);
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

  if (!open) return null;

  const selectedMask = masks.find((m) => m.id === selectedMaskId) ?? null;
  const selectedMaskGeometry = selectedMask?.components[0]?.geometry;

  function handleRename(maskId: string, currentName: string, event: React.MouseEvent) {
    event.stopPropagation();
    const name = window.prompt("Maske umbenennen", currentName);
    if (name) renameMask(maskId, name);
  }

  return (
    <aside className="flex w-64 shrink-0 flex-col gap-3 overflow-y-auto border-l border-border bg-bg-raised p-3" aria-label="Masken">
      <h2 className="text-sm font-semibold text-text-primary">Masken</h2>

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
          className="col-span-2 rounded border border-border px-2 py-1 text-xs text-text-secondary hover:bg-bg-panel disabled:cursor-not-allowed disabled:opacity-40"
        >
          + Luminanzbereich
        </button>
      </div>

      <ul className="flex flex-col gap-1">
        {masks.map((mask) => (
          <li
            key={mask.id}
            className={`flex items-center gap-1.5 rounded border px-2 py-1.5 text-sm ${
              mask.id === selectedMaskId ? "border-accent bg-accent/10" : "border-border"
            }`}
          >
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
            <button
              type="button"
              onClick={() => removeMask(mask.id)}
              className="shrink-0 text-danger"
              aria-label={`${mask.name} löschen`}
            >
              ×
            </button>
          </li>
        ))}
        {masks.length === 0 && <li className="text-xs text-text-muted">Keine Masken vorhanden.</li>}
      </ul>

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
          <h4 className="text-xs font-medium text-text-secondary">Grundeinstellungen (Auswahl)</h4>
          {MASK_BASIC_SLIDER_SPECS.map((spec) => (
            <DevelopSlider
              key={spec.key}
              spec={spec}
              value={readBasicField(selectedMask.adjustments.basic, spec.key)}
              onChange={(value) => setMaskBasicField(selectedMask.id, spec.key, value)}
              onCommit={commitMaskDrag}
            />
          ))}
        </div>
      )}
    </aside>
  );
}
