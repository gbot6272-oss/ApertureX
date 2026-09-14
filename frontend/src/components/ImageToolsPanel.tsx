import { useState } from "react";

import { CHANNEL_MATRIX_PRESETS, type InteractiveAdjustments } from "../lib/edl";
import { ZONE_COLORS } from "../lib/zoneOverlay";
import { useAppStore, type ImageToolMode } from "../store";
import { ApertureShapePreview } from "./ApertureShapePreview";
import { ChannelMatrixGrid } from "./ChannelMatrixGrid";
import { DevelopSlider } from "./DevelopSlider";
import { GradientRampEditor } from "./GradientRampEditor";
import { ToolTile, ToolToolbar, matchesToolQuery } from "./ToolTiles";

/**
 * Die Registerkarte „Am Bild" (Phase 30, siehe `DECISIONS.md`
 * ADR-0060).
 *
 * Anders als die Kachel-Panels aus Phase 27/28 ist hier die Kachel
 * nicht der Ort der Bedienung, sondern ihr *Schalter*: jedes der
 * sieben Werkzeuge hat einen „Am Bild"-Knopf, der den Viewer in den
 * passenden Modus versetzt. Die eigentliche Arbeit passiert dann im
 * Foto — Lichter ziehen, eine Horizontlinie legen, Punkte setzen.
 *
 * Drei weitere Kacheln bedienen bestehende Funktionen, die bisher kein
 * Bedienelement hatten: das 3×3-Kanalmatrix-Gitter (Phase 28), die
 * Blendenform-Vorschau (Phase 28) und die Zonen-Überlagerung
 * (Phase 28).
 */

/** Ein Halbsatz je Werkzeug — dieselbe „wenig Subtext"-Regel wie in den
 * Panels aus Phase 27/28. */
const TOOLS: readonly {
  group: keyof InteractiveAdjustments;
  title: string;
  hint: string;
  mode: ImageToolMode | null;
  modeLabel: string;
}[] = [
  {
    group: "point_lights",
    title: "Lichtquellen",
    hint: "Beliebig viele Lichter frei im Bild setzen",
    mode: "lights",
    modeLabel: "Lichter setzen",
  },
  {
    group: "spotlight",
    title: "Lichtkegel",
    hint: "Innen aufhellen, außen abdunkeln",
    mode: "spotlight",
    modeLabel: "Kegel verschieben",
  },
  {
    group: "dodge_burn",
    title: "Abwedeln & Nachbelichten",
    hint: "Punkte, die örtlich aufhellen oder abdunkeln",
    mode: "dodgeBurn",
    modeLabel: "Punkte setzen",
  },
  {
    group: "split_light",
    title: "Split-Lighting",
    hint: "Zwei Lichtfarben entlang einer Achse",
    mode: "splitLight",
    modeLabel: "Achse ziehen",
  },
  {
    group: "color_replace",
    title: "Farbe ersetzen",
    hint: "Eine Farbe im Bild greifen und austauschen",
    mode: "colorReplacePick",
    modeLabel: "Farbe greifen",
  },
  {
    group: "gradient_ramp",
    title: "Verlaufsband",
    hint: "Helligkeit auf einen selbst gebauten Verlauf abbilden",
    mode: null,
    modeLabel: "",
  },
  {
    group: "horizon_grad",
    title: "Horizont-Verlaufsfilter",
    hint: "Grauverlauf entlang einer frei gezogenen Linie",
    mode: "horizon",
    modeLabel: "Linie ziehen",
  },
];

function toHex(rgb: readonly number[]): string {
  const part = (v: number) =>
    Math.round(Math.min(1, Math.max(0, v)) * 255)
      .toString(16)
      .padStart(2, "0");
  return `#${part(rgb[0] ?? 0)}${part(rgb[1] ?? 0)}${part(rgb[2] ?? 0)}`;
}

function fromHex(hex: string): number[] {
  const clean = hex.replace("#", "");
  return [0, 2, 4].map((offset) => parseInt(clean.slice(offset, offset + 2), 16) / 255);
}

const UNIT = { min: 0, max: 1, fineStep: 0.01, coarseStep: 0.1 } as const;

export function ImageToolsPanel() {
  const interactive = useAppStore((s) => s.developEdl.interactive);
  const virtualAperture = useAppStore((s) => s.developEdl.virtual_aperture);
  const lightOptics = useAppStore((s) => s.developEdl.light_optics);
  const imageToolMode = useAppStore((s) => s.imageToolMode);
  const setImageToolMode = useAppStore((s) => s.setImageToolMode);
  const setField = useAppStore((s) => s.setInteractiveField);
  const setFlag = useAppStore((s) => s.setInteractiveFlag);
  const setColor = useAppStore((s) => s.setInteractiveColor);
  const resetInteractive = useAppStore((s) => s.resetInteractive);
  const commitDevelopEdit = useAppStore((s) => s.commitDevelopEdit);

  const selectedLightIndex = useAppStore((s) => s.selectedLightIndex);
  const selectLightIndex = useAppStore((s) => s.selectLightIndex);
  const addPointLight = useAppStore((s) => s.addPointLight);
  const updatePointLight = useAppStore((s) => s.updatePointLight);
  const removePointLight = useAppStore((s) => s.removePointLight);

  const selectedDodgeBurnIndex = useAppStore((s) => s.selectedDodgeBurnIndex);
  const selectDodgeBurnIndex = useAppStore((s) => s.selectDodgeBurnIndex);
  const addDodgeBurnPoint = useAppStore((s) => s.addDodgeBurnPoint);
  const updateDodgeBurnPoint = useAppStore((s) => s.updateDodgeBurnPoint);
  const removeDodgeBurnPoint = useAppStore((s) => s.removeDodgeBurnPoint);

  const selectedGradientStopIndex = useAppStore((s) => s.selectedGradientStopIndex);
  const selectGradientStopIndex = useAppStore((s) => s.selectGradientStopIndex);
  const addGradientStop = useAppStore((s) => s.addGradientStop);
  const updateGradientStop = useAppStore((s) => s.updateGradientStop);
  const removeGradientStop = useAppStore((s) => s.removeGradientStop);

  const setVirtualApertureField = useAppStore((s) => s.setVirtualApertureField);
  const setLightOpticsField = useAppStore((s) => s.setLightOpticsField);
  const applyChannelMatrixPreset = useAppStore((s) => s.applyChannelMatrixPreset);
  const setZoneValue = useAppStore((s) => s.setZoneValue);
  const zoneOverlayEnabled = useAppStore((s) => s.zoneOverlayEnabled);
  const toggleZoneOverlay = useAppStore((s) => s.toggleZoneOverlay);
  const zoneOverlayHighlight = useAppStore((s) => s.zoneOverlayHighlight);
  const setZoneOverlayHighlight = useAppStore((s) => s.setZoneOverlayHighlight);

  const [query, setQuery] = useState("");
  const [onlyActive, setOnlyActive] = useState(false);

  function read(group: keyof InteractiveAdjustments, field: string): number {
    return (interactive[group] as unknown as Record<string, number>)[field] ?? 0;
  }

  function isActive(group: keyof InteractiveAdjustments): boolean {
    const amount = read(group, "amount");
    if (amount <= 0) return false;
    if (group === "point_lights") return interactive.point_lights.lights.length > 0;
    if (group === "dodge_burn") return interactive.dodge_burn.points.length > 0;
    if (group === "gradient_ramp") return interactive.gradient_ramp.stops.length >= 2;
    if (group === "color_replace") return interactive.color_replace.has_source;
    return true;
  }

  const visible = TOOLS.filter(
    (tool) => matchesToolQuery(tool.title, tool.hint, query) && (!onlyActive || isActive(tool.group)),
  );

  const actionButton =
    "apx-btn-liquid self-start rounded border px-3 py-1 text-xs font-medium transition-[background-color,box-shadow] duration-[var(--duration-fast)]";

  function amountSlider(group: keyof InteractiveAdjustments, label = "Stärke") {
    return (
      <DevelopSlider
        spec={{ key: `${group}-amount`, label, ...UNIT, neutral: 0 }}
        value={read(group, "amount")}
        onChange={(value) => setField(group, "amount", value)}
        onCommit={() => void commitDevelopEdit(`${label} geändert`)}
      />
    );
  }

  const selectedLight = interactive.point_lights.lights[selectedLightIndex];
  const selectedPoint = interactive.dodge_burn.points[selectedDodgeBurnIndex];

  return (
    <div className="flex flex-col gap-3" data-testid="image-tools-panel">
      <ToolToolbar
        title="Am Bild"
        query={query}
        onQueryChange={setQuery}
        onlyActive={onlyActive}
        onOnlyActiveChange={setOnlyActive}
        onReset={resetInteractive}
        visibleCount={visible.length}
        totalCount={TOOLS.length}
        idPrefix="image-tools"
      />

      {visible.length === 0 && (
        <p className="text-xs text-text-muted">Kein Werkzeug passt zur Suche.</p>
      )}

      {visible.map((tool) => (
        <ToolTile
          key={tool.group}
          title={tool.title}
          hint={tool.hint}
          active={isActive(tool.group)}
          onReset={() => {
            setField(tool.group, "amount", 0);
            void commitDevelopEdit(`${tool.title} zurückgesetzt`);
          }}
        >
          {tool.mode && (
            <button
              type="button"
              aria-pressed={imageToolMode === tool.mode}
              onClick={() => setImageToolMode(imageToolMode === tool.mode ? "off" : tool.mode!)}
              className={`${actionButton} ${
                imageToolMode === tool.mode
                  ? "apx-btn-liquid-active border-accent bg-accent/20 text-accent"
                  : "border-accent bg-accent/10 text-accent hover:bg-accent/20"
              }`}
            >
              {imageToolMode === tool.mode ? "Fertig" : tool.modeLabel}
            </button>
          )}

          {tool.group === "point_lights" && (
            <div className="flex flex-col gap-2">
              <div className="flex flex-wrap gap-1" role="group" aria-label="Gesetzte Lichter">
                {interactive.point_lights.lights.map((light, index) => (
                  <button
                    key={index}
                    type="button"
                    aria-pressed={index === selectedLightIndex}
                    onClick={() => selectLightIndex(index)}
                    className={`h-6 w-6 rounded-full border-2 transition-transform duration-[var(--duration-fast)] hover:scale-110 ${
                      index === selectedLightIndex ? "scale-110 border-accent" : "border-border"
                    }`}
                    style={{ backgroundColor: toHex(light.color_rgb) }}
                    title={`Licht ${index + 1}`}
                  />
                ))}
                <button
                  type="button"
                  onClick={() => addPointLight()}
                  className="apx-btn-liquid rounded border border-border px-2 text-xs text-text-secondary transition-colors duration-[var(--duration-fast)] hover:border-accent hover:text-text-primary"
                >
                  + Licht
                </button>
              </div>
              {selectedLight && (
                <>
                  <div className="flex items-center gap-2">
                    <input
                      type="color"
                      aria-label="Lichtfarbe"
                      value={toHex(selectedLight.color_rgb)}
                      onChange={(event) =>
                        updatePointLight(selectedLightIndex, { color_rgb: fromHex(event.target.value) })
                      }
                      onBlur={() => void commitDevelopEdit("Lichtfarbe geändert")}
                      className="h-7 w-10 cursor-pointer rounded border border-[var(--glass-border)] bg-transparent"
                    />
                    <button
                      type="button"
                      onClick={() => removePointLight(selectedLightIndex)}
                      className="apx-btn-liquid rounded border border-border px-2 py-1 text-xs text-text-secondary transition-colors duration-[var(--duration-fast)] hover:border-accent hover:text-text-primary"
                    >
                      Licht entfernen
                    </button>
                  </div>
                  <DevelopSlider
                    spec={{ key: "light-radius", label: "Radius", ...UNIT, neutral: 0.3 }}
                    value={selectedLight.radius}
                    onChange={(value) => updatePointLight(selectedLightIndex, { radius: value })}
                    onCommit={() => void commitDevelopEdit("Lichtradius")}
                  />
                  <DevelopSlider
                    spec={{ key: "light-intensity", label: "Stärke", min: -1, max: 1, fineStep: 0.01, coarseStep: 0.1, neutral: 0.5 }}
                    value={selectedLight.intensity}
                    onChange={(value) => updatePointLight(selectedLightIndex, { intensity: value })}
                    onCommit={() => void commitDevelopEdit("Lichtstärke")}
                  />
                  <DevelopSlider
                    spec={{ key: "light-falloff", label: "Abfall", min: 0.5, max: 4, fineStep: 0.1, coarseStep: 0.5, neutral: 2 }}
                    value={selectedLight.falloff}
                    onChange={(value) => updatePointLight(selectedLightIndex, { falloff: value })}
                    onCommit={() => void commitDevelopEdit("Lichtabfall")}
                  />
                </>
              )}
            </div>
          )}

          {tool.group === "spotlight" && (
            <>
              <input
                type="color"
                aria-label="Kegelfarbe"
                value={toHex(interactive.spotlight.color_rgb)}
                onChange={(event) => setColor("spotlight", "color_rgb", fromHex(event.target.value))}
                onBlur={() => void commitDevelopEdit("Kegelfarbe")}
                className="h-7 w-10 cursor-pointer self-start rounded border border-[var(--glass-border)] bg-transparent"
              />
              {(
                [
                  ["rx", "Breite", 0.3],
                  ["ry", "Höhe", 0.22],
                  ["feather", "Weichheit", 0.6],
                  ["inner_gain", "Innen heller", 0.45],
                  ["outer_gain", "Außen dunkler", 0.5],
                ] as [string, string, number][]
              ).map(([key, label, neutral]) => (
                <DevelopSlider
                  key={key}
                  spec={{ key, label, ...UNIT, neutral }}
                  value={read("spotlight", key)}
                  onChange={(value) => setField("spotlight", key, value)}
                  onCommit={() => void commitDevelopEdit(`Lichtkegel: ${label}`)}
                />
              ))}
              <DevelopSlider
                spec={{ key: "spot-angle", label: "Drehung", min: 0, max: 360, fineStep: 1, coarseStep: 15, neutral: 0 }}
                value={read("spotlight", "angle_deg")}
                onChange={(value) => setField("spotlight", "angle_deg", value)}
                onCommit={() => void commitDevelopEdit("Lichtkegel: Drehung")}
              />
            </>
          )}

          {tool.group === "dodge_burn" && (
            <div className="flex flex-col gap-2">
              <div className="flex flex-wrap gap-1" role="group" aria-label="Gesetzte Punkte">
                {interactive.dodge_burn.points.map((point, index) => (
                  <button
                    key={index}
                    type="button"
                    aria-pressed={index === selectedDodgeBurnIndex}
                    onClick={() => selectDodgeBurnIndex(index)}
                    className={`h-6 w-6 rounded-full border-2 text-[10px] transition-transform duration-[var(--duration-fast)] hover:scale-110 ${
                      index === selectedDodgeBurnIndex ? "scale-110 border-accent" : "border-border"
                    }`}
                    style={{ backgroundColor: point.amount >= 0 ? "#ffffff" : "#141414" }}
                    title={point.amount >= 0 ? "Aufhellen" : "Abdunkeln"}
                  />
                ))}
                <button
                  type="button"
                  onClick={() => addDodgeBurnPoint(0.5, 0.5, 0.4)}
                  className="apx-btn-liquid rounded border border-border px-2 text-xs text-text-secondary transition-colors duration-[var(--duration-fast)] hover:border-accent hover:text-text-primary"
                >
                  + Aufhellen
                </button>
                <button
                  type="button"
                  onClick={() => addDodgeBurnPoint(0.5, 0.5, -0.4)}
                  className="apx-btn-liquid rounded border border-border px-2 text-xs text-text-secondary transition-colors duration-[var(--duration-fast)] hover:border-accent hover:text-text-primary"
                >
                  + Abdunkeln
                </button>
              </div>
              {selectedPoint && (
                <>
                  <DevelopSlider
                    spec={{ key: "db-radius", label: "Radius", ...UNIT, neutral: 0.15 }}
                    value={selectedPoint.radius}
                    onChange={(value) => updateDodgeBurnPoint(selectedDodgeBurnIndex, { radius: value })}
                    onCommit={() => void commitDevelopEdit("Punktradius")}
                  />
                  <DevelopSlider
                    spec={{ key: "db-amount", label: "Wirkung", min: -1.5, max: 1.5, fineStep: 0.05, coarseStep: 0.25, neutral: 0.4 }}
                    value={selectedPoint.amount}
                    onChange={(value) => updateDodgeBurnPoint(selectedDodgeBurnIndex, { amount: value })}
                    onCommit={() => void commitDevelopEdit("Punktwirkung")}
                  />
                  <button
                    type="button"
                    onClick={() => removeDodgeBurnPoint(selectedDodgeBurnIndex)}
                    className="apx-btn-liquid self-start rounded border border-border px-2 py-1 text-xs text-text-secondary transition-colors duration-[var(--duration-fast)] hover:border-accent hover:text-text-primary"
                  >
                    Punkt entfernen
                  </button>
                </>
              )}
            </div>
          )}

          {tool.group === "split_light" && (
            <>
              <div className="flex items-center gap-2 text-xs text-text-muted">
                <input
                  type="color"
                  aria-label="Lichtfarbe A"
                  value={toHex(interactive.split_light.color_a)}
                  onChange={(event) => setColor("split_light", "color_a", fromHex(event.target.value))}
                  onBlur={() => void commitDevelopEdit("Lichtfarbe A")}
                  className="h-7 w-10 cursor-pointer rounded border border-[var(--glass-border)] bg-transparent"
                />
                <span>→</span>
                <input
                  type="color"
                  aria-label="Lichtfarbe B"
                  value={toHex(interactive.split_light.color_b)}
                  onChange={(event) => setColor("split_light", "color_b", fromHex(event.target.value))}
                  onBlur={() => void commitDevelopEdit("Lichtfarbe B")}
                  className="h-7 w-10 cursor-pointer rounded border border-[var(--glass-border)] bg-transparent"
                />
              </div>
              <DevelopSlider
                spec={{ key: "split-bias", label: "Nur Lichter", ...UNIT, neutral: 0.35 }}
                value={read("split_light", "luma_bias")}
                onChange={(value) => setField("split_light", "luma_bias", value)}
                onCommit={() => void commitDevelopEdit("Split-Lighting")}
              />
            </>
          )}

          {tool.group === "color_replace" && (
            <>
              <div className="flex items-center gap-2 text-xs text-text-muted">
                <span
                  aria-label="Gegriffene Quellfarbe"
                  data-testid="color-replace-source"
                  className="h-7 w-10 rounded border border-[var(--glass-border)]"
                  style={{ backgroundColor: toHex(interactive.color_replace.from_rgb) }}
                />
                <span>→</span>
                <input
                  type="color"
                  aria-label="Zielfarbe"
                  value={toHex(interactive.color_replace.to_rgb)}
                  onChange={(event) => setColor("color_replace", "to_rgb", fromHex(event.target.value))}
                  onBlur={() => void commitDevelopEdit("Zielfarbe")}
                  className="h-7 w-10 cursor-pointer rounded border border-[var(--glass-border)] bg-transparent"
                />
                {!interactive.color_replace.has_source && <span>noch keine Farbe gegriffen</span>}
              </div>
              <DevelopSlider
                spec={{ key: "cr-tolerance", label: "Toleranz", ...UNIT, neutral: 0.25 }}
                value={read("color_replace", "tolerance")}
                onChange={(value) => setField("color_replace", "tolerance", value)}
                onCommit={() => void commitDevelopEdit("Toleranz")}
              />
              <DevelopSlider
                spec={{ key: "cr-softness", label: "Weichheit", ...UNIT, neutral: 0.15 }}
                value={read("color_replace", "softness")}
                onChange={(value) => setField("color_replace", "softness", value)}
                onCommit={() => void commitDevelopEdit("Weichheit")}
              />
              <label className="flex items-center gap-2 text-xs text-text-secondary">
                <input
                  type="checkbox"
                  checked={interactive.color_replace.preserve_luma}
                  onChange={(event) => setFlag("color_replace", "preserve_luma", event.target.checked)}
                />
                Helligkeit behalten
              </label>
            </>
          )}

          {tool.group === "gradient_ramp" && (
            <>
              <GradientRampEditor
                stops={interactive.gradient_ramp.stops}
                selectedIndex={selectedGradientStopIndex}
                onSelect={selectGradientStopIndex}
                onMove={(index, position) => updateGradientStop(index, { position })}
                onMoveEnd={() => void commitDevelopEdit("Stützstelle verschoben")}
                onColorChange={(index, rgb) => {
                  updateGradientStop(index, { color_rgb: rgb });
                  void commitDevelopEdit("Stützstellenfarbe");
                }}
                onAdd={addGradientStop}
                onRemove={removeGradientStop}
              />
              <label className="flex items-center gap-2 text-xs text-text-secondary">
                <input
                  type="checkbox"
                  checked={interactive.gradient_ramp.preserve_luma}
                  onChange={(event) => setFlag("gradient_ramp", "preserve_luma", event.target.checked)}
                />
                Helligkeit behalten
              </label>
            </>
          )}

          {tool.group === "horizon_grad" && (
            <>
              <input
                type="color"
                aria-label="Filterfarbe"
                value={toHex(interactive.horizon_grad.color_rgb)}
                onChange={(event) => setColor("horizon_grad", "color_rgb", fromHex(event.target.value))}
                onBlur={() => void commitDevelopEdit("Filterfarbe")}
                className="h-7 w-10 cursor-pointer self-start rounded border border-[var(--glass-border)] bg-transparent"
              />
              {(
                [
                  ["density", "Dichte", 0.55],
                  ["softness", "Weichheit", 0.25],
                  ["tint", "Einfärbung", 0.25],
                ] as [string, string, number][]
              ).map(([key, label, neutral]) => (
                <DevelopSlider
                  key={key}
                  spec={{ key, label, ...UNIT, neutral }}
                  value={read("horizon_grad", key)}
                  onChange={(value) => setField("horizon_grad", key, value)}
                  onCommit={() => void commitDevelopEdit(`Verlaufsfilter: ${label}`)}
                />
              ))}
              <label className="flex items-center gap-2 text-xs text-text-secondary">
                <input
                  type="checkbox"
                  checked={interactive.horizon_grad.flipped}
                  onChange={(event) => setFlag("horizon_grad", "flipped", event.target.checked)}
                />
                Andere Seite
              </label>
            </>
          )}

          {amountSlider(tool.group)}
        </ToolTile>
      ))}

      {/* ---- Drei Bedienelemente für bestehende Funktionen ------------- */}

      <ToolTile
        title="Kanalmatrix"
        hint="Die neun Zahlen der Matrix aus Optik"
        active={lightOptics.channel_matrix.amount > 0}
        onReset={() => {
          setLightOpticsField("channel_matrix", "amount", 0);
          void commitDevelopEdit("Kanalmatrix zurückgesetzt");
        }}
      >
        <ChannelMatrixGrid
          matrix={lightOptics.channel_matrix.matrix}
          onChange={(index, value) => {
            const next = [...lightOptics.channel_matrix.matrix];
            next[index] = value;
            applyChannelMatrixPreset(next);
          }}
          onCommit={() => void commitDevelopEdit("Kanalmatrix")}
        />
        <div className="flex flex-wrap gap-1">
          {CHANNEL_MATRIX_PRESETS.map((preset) => (
            <button
              key={preset.id}
              type="button"
              onClick={() => applyChannelMatrixPreset(preset.matrix)}
              className="apx-btn-liquid rounded border border-border px-2 py-1 text-xs text-text-secondary transition-colors duration-[var(--duration-fast)] hover:border-accent hover:text-text-primary"
            >
              {preset.label}
            </button>
          ))}
        </div>
        <DevelopSlider
          spec={{ key: "cm-amount", label: "Stärke", ...UNIT, neutral: 0 }}
          value={lightOptics.channel_matrix.amount}
          onChange={(value) => setLightOpticsField("channel_matrix", "amount", value)}
          onCommit={() => void commitDevelopEdit("Kanalmatrix: Stärke")}
        />
      </ToolTile>

      <ToolTile
        title="Blendenform"
        hint="Zeigt den Bokeh-Kern der Virtuellen Blende"
        active={virtualAperture.blades >= 3 || virtualAperture.anamorphic > 0 || virtualAperture.swirl > 0}
        onReset={() => {
          setVirtualApertureField("blades", 0);
          setVirtualApertureField("anamorphic", 0);
          setVirtualApertureField("swirl", 0);
          void commitDevelopEdit("Blendenform zurückgesetzt");
        }}
      >
        <ApertureShapePreview
          blades={virtualAperture.blades}
          rotationDeg={virtualAperture.rotation}
          anamorphic={virtualAperture.anamorphic}
          swirl={virtualAperture.swirl}
        />
        <DevelopSlider
          spec={{ key: "blades", label: "Lamellen", min: 0, max: 11, fineStep: 1, coarseStep: 2, neutral: 0 }}
          value={virtualAperture.blades}
          onChange={(value) => setVirtualApertureField("blades", value)}
          onCommit={() => void commitDevelopEdit("Blendenlamellen")}
        />
        <DevelopSlider
          spec={{ key: "rotation", label: "Drehung", min: 0, max: 360, fineStep: 1, coarseStep: 15, neutral: 0 }}
          value={virtualAperture.rotation}
          onChange={(value) => setVirtualApertureField("rotation", value)}
          onCommit={() => void commitDevelopEdit("Blendendrehung")}
        />
        <DevelopSlider
          spec={{ key: "anamorphic", label: "Anamorph", ...UNIT, neutral: 0 }}
          value={virtualAperture.anamorphic}
          onChange={(value) => setVirtualApertureField("anamorphic", value)}
          onCommit={() => void commitDevelopEdit("Anamorphe Streckung")}
        />
        <DevelopSlider
          spec={{ key: "swirl", label: "Wirbel", ...UNIT, neutral: 0 }}
          value={virtualAperture.swirl}
          onChange={(value) => setVirtualApertureField("swirl", value)}
          onCommit={() => void commitDevelopEdit("Wirbel")}
        />
        <DevelopSlider
          spec={{ key: "hl-boost", label: "Spitzlichter", ...UNIT, neutral: 0 }}
          value={virtualAperture.highlight_boost}
          onChange={(value) => setVirtualApertureField("highlight_boost", value)}
          onCommit={() => void commitDevelopEdit("Spitzlicht-Anhebung")}
        />
      </ToolTile>

      <ToolTile
        title="Zonen anzeigen"
        hint="Färbt die zehn Helligkeitszonen im Foto ein"
        active={zoneOverlayEnabled}
        onReset={() => setZoneOverlayHighlight(null)}
      >
        <button
          type="button"
          aria-pressed={zoneOverlayEnabled}
          onClick={toggleZoneOverlay}
          data-testid="zone-overlay-toggle"
          className={`${actionButton} ${
            zoneOverlayEnabled
              ? "apx-btn-liquid-active border-accent bg-accent/20 text-accent"
              : "border-accent bg-accent/10 text-accent hover:bg-accent/20"
          }`}
        >
          {zoneOverlayEnabled ? "Überlagerung aus" : "Überlagerung an"}
        </button>
        <div className="flex" role="group" aria-label="Zonenstreifen">
          {ZONE_COLORS.map((color, zone) => (
            <button
              key={zone}
              type="button"
              aria-label={`Zone ${zone}`}
              aria-pressed={zoneOverlayHighlight === zone}
              onClick={() => setZoneOverlayHighlight(zoneOverlayHighlight === zone ? null : zone)}
              className={`h-7 flex-1 border-y-2 first:rounded-l first:border-l-2 last:rounded-r last:border-r-2 transition-transform duration-[var(--duration-fast)] hover:scale-y-125 ${
                zoneOverlayHighlight === zone ? "scale-y-125 border-accent" : "border-transparent"
              }`}
              style={{ backgroundColor: `rgb(${color.join(" ")})` }}
            />
          ))}
        </div>
        {/* Der Regler der gewählten Zone steht direkt darunter — Zone
            anklicken, Regler ziehen, Wirkung im Bild sehen. */}
        {zoneOverlayHighlight !== null && (
          <DevelopSlider
            spec={{
              key: `zone-${zoneOverlayHighlight}`,
              label: `Zone ${zoneOverlayHighlight}`,
              min: -1,
              max: 1,
              fineStep: 0.02,
              coarseStep: 0.2,
              neutral: 0,
            }}
            value={lightOptics.zone_system.zones[zoneOverlayHighlight] ?? 0}
            onChange={(value) => setZoneValue(zoneOverlayHighlight, value)}
            onCommit={() => void commitDevelopEdit(`Zone ${zoneOverlayHighlight}`)}
          />
        )}
      </ToolTile>
    </div>
  );
}
