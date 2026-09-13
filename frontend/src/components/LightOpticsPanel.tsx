import { useMemo, useState } from "react";

import {
  CHANNEL_MATRIX_PRESETS,
  LIGHT_OPTICS_TOOL_SPECS,
  MOTION_BLUR_KINDS,
  NEUTRAL_LIGHT_OPTICS,
  type LightOpticsAdjustments,
} from "../lib/edl";
import { useAppStore } from "../store";
import { DevelopSlider } from "./DevelopSlider";
import { ToolTile, ToolToolbar, matchesToolQuery } from "./ToolTiles";

/**
 * Die zwölf Licht-&-Optik-Werkzeuge aus Phase 28 (siehe `DECISIONS.md`
 * ADR-0058) als eigene Entwickeln-Registerkarte.
 *
 * **Eigene Karte statt Anbau an „Licht"**: die zwölf hätten die
 * bestehende Karte verdoppelt. Aufbau wie beim Kreativ-Panel — je
 * Werkzeug eine Glaskachel mit Titel, EINEM Halbsatz Wirkung und den
 * Reglern, in genau der Reihenfolge, in der die Pipeline sie anwendet
 * (`stages/light_optics.rs`).
 *
 * Die Ein-Klick-Vorbereitungen (Tiefenkarte, Himmelsmaske, Motivmaske,
 * Referenzfoto) stehen in der Kachel, zu der sie gehören.
 */
export function LightOpticsPanel() {
  const lightOptics = useAppStore((s) => s.developEdl.light_optics);
  const setField = useAppStore((s) => s.setLightOpticsField);
  const setZoneValue = useAppStore((s) => s.setZoneValue);
  const setMotionBlurKind = useAppStore((s) => s.setMotionBlurKind);
  const applyChannelMatrixPreset = useAppStore((s) => s.applyChannelMatrixPreset);
  const resetLightOptics = useAppStore((s) => s.resetLightOptics);
  const commitDevelopEdit = useAppStore((s) => s.commitDevelopEdit);
  const segmentSky = useAppStore((s) => s.segmentSkyForCurrentPhoto);
  const skySegmenting = useAppStore((s) => s.skySegmenting);
  const useDepthMap = useAppStore((s) => s.useDepthMapForLightOptics);
  const useSubjectMask = useAppStore((s) => s.useSubjectMaskForMotionBlur);
  const subjectSegmenting = useAppStore((s) => s.subjectSegmenting);
  const depthEstimating = useAppStore((s) => s.depthEstimating);
  const setToneMatchReference = useAppStore((s) => s.setToneMatchReference);
  const toneMatchLoading = useAppStore((s) => s.toneMatchLoading);
  const selectedPhotoId = useAppStore((s) => s.selectedPhotoId);
  const multiSelectedIds = useAppStore((s) => s.multiSelectedIds);

  const [query, setQuery] = useState("");
  const [onlyActive, setOnlyActive] = useState(false);

  function readField(group: keyof LightOpticsAdjustments, field: string): number {
    return (lightOptics[group] as unknown as Record<string, number>)[field] ?? 0;
  }

  /** Ein Werkzeug gilt als aktiv, sobald irgendein Regler vom
   * Neutralwert abweicht — bei den drei Werkzeugen mit Sonderfeldern
   * (Zonen, Kanalmatrix) zählt zusätzlich deren eigener Zustand, sonst
   * würde „Nur aktive" sie fälschlich ausblenden. */
  const activeByGroup = useMemo(() => {
    const map = new Map<string, boolean>();
    for (const tool of LIGHT_OPTICS_TOOL_SPECS) {
      let active = tool.sliders.some((spec) => readField(tool.group, spec.key) !== spec.neutral);
      if (tool.group === "zone_system") {
        active = active || lightOptics.zone_system.zones.some((zone) => zone !== 0);
      }
      if (tool.group === "channel_matrix") {
        active =
          active ||
          lightOptics.channel_matrix.matrix.some(
            (value, index) => value !== NEUTRAL_LIGHT_OPTICS.channel_matrix.matrix[index],
          );
      }
      map.set(tool.group, active);
    }
    return map;
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [lightOptics]);

  const visible = LIGHT_OPTICS_TOOL_SPECS.filter(
    (tool) =>
      matchesToolQuery(tool.title, tool.hint, query) &&
      (!onlyActive || activeByGroup.get(tool.group) === true),
  );

  /** Referenzfoto für den Tonwert-Angleich: dasselbe „zweites Foto der
   * Mehrfachauswahl"-Muster wie beim Farbabgleich aus Phase 27 — kein
   * eigener Auswahldialog nötig. */
  const referenceId = multiSelectedIds.find((id) => id !== selectedPhotoId) ?? selectedPhotoId;

  /** Die drei Werkzeuge, die eine Tiefenkarte brauchen — als Funktion
   * statt als Inline-Bedingung, damit TypeScript den Gruppennamen
   * wirklich auf diese drei einengt und `useDepthMap` typsicher bleibt. */
  function depthTool(
    group: keyof LightOpticsAdjustments,
  ): "depth_dehaze" | "depth_sharpen" | "relight" | null {
    return group === "depth_dehaze" || group === "depth_sharpen" || group === "relight"
      ? group
      : null;
  }

  function depthMapPresent(group: keyof LightOpticsAdjustments): boolean {
    const target = depthTool(group);
    return target !== null && lightOptics[target].depth_map !== null;
  }

  function resetTool(group: keyof LightOpticsAdjustments, title: string) {
    const neutral = NEUTRAL_LIGHT_OPTICS[group] as unknown as Record<string, unknown>;
    for (const [field, value] of Object.entries(neutral)) {
      if (typeof value === "number") setField(group, field, value);
    }
    if (group === "zone_system") {
      NEUTRAL_LIGHT_OPTICS.zone_system.zones.forEach((_, index) => setZoneValue(index, 0));
      setField("zone_system", "amount", 0);
    }
    if (group === "channel_matrix") {
      applyChannelMatrixPreset([...NEUTRAL_LIGHT_OPTICS.channel_matrix.matrix]);
      setField("channel_matrix", "amount", 0);
    }
    void commitDevelopEdit(`${title} zurückgesetzt`);
  }

  const actionButton = "apx-btn-liquid self-start rounded border border-accent bg-accent/10 px-3 py-1 text-xs font-medium text-accent transition-[background-color,box-shadow] duration-[var(--duration-fast)] hover:bg-accent/20 disabled:cursor-not-allowed disabled:opacity-50";

  return (
    <div className="flex flex-col gap-3" data-testid="light-optics-panel">
      <ToolToolbar
        title="Licht & Optik"
        query={query}
        onQueryChange={setQuery}
        onlyActive={onlyActive}
        onOnlyActiveChange={setOnlyActive}
        onReset={resetLightOptics}
        visibleCount={visible.length}
        totalCount={LIGHT_OPTICS_TOOL_SPECS.length}
        idPrefix="light-optics"
      />

      {visible.length === 0 && (
        <p className="text-xs text-text-muted">Kein Werkzeug passt zur Suche.</p>
      )}

      {visible.map((tool) => (
        <ToolTile
          key={tool.group}
          title={tool.title}
          hint={tool.hint}
          active={activeByGroup.get(tool.group) === true}
          onReset={() => resetTool(tool.group, tool.title)}
        >
          {tool.group === "tone_match" && (
            <button
              type="button"
              disabled={toneMatchLoading || !referenceId}
              onClick={() => referenceId && void setToneMatchReference(referenceId)}
              className={actionButton}
            >
              {toneMatchLoading
                ? "Lese Referenz…"
                : lightOptics.tone_match.has_target
                  ? "Tonwerte neu lesen"
                  : "Tonwerte übernehmen"}
            </button>
          )}

          {tool.group === "zone_system" && (
            <div className="flex flex-col gap-1" role="group" aria-label="Zehn Helligkeitszonen">
              {lightOptics.zone_system.zones.map((zone, index) => (
                <DevelopSlider
                  key={index}
                  spec={{
                    key: `zone-${index}`,
                    label: `Zone ${index}`,
                    min: -1,
                    max: 1,
                    fineStep: 0.02,
                    coarseStep: 0.2,
                    neutral: 0,
                  }}
                  value={zone}
                  onChange={(value) => setZoneValue(index, value)}
                  onCommit={() => void commitDevelopEdit(`Zonensystem: Zone ${index}`)}
                />
              ))}
            </div>
          )}

          {depthTool(tool.group) && (
            <button
              type="button"
              disabled={depthEstimating}
              onClick={() => {
                const target = depthTool(tool.group);
                if (target) void useDepthMap(target);
              }}
              className={actionButton}
            >
              {depthEstimating
                ? "Berechne Tiefe…"
                : depthMapPresent(tool.group)
                  ? "Tiefenkarte erneuern"
                  : "Tiefenkarte berechnen"}
            </button>
          )}

          {tool.group === "sky_drama" && (
            <button
              type="button"
              disabled={skySegmenting}
              onClick={() => void segmentSky()}
              className={actionButton}
            >
              {skySegmenting
                ? "Erkenne Himmel…"
                : lightOptics.sky_drama.mask
                  ? "Himmel neu erkennen"
                  : "Himmel erkennen"}
            </button>
          )}

          {tool.group === "motion_blur" && (
            <>
              <div className="flex gap-1" role="group" aria-label="Art der Bewegung">
                {MOTION_BLUR_KINDS.map(({ id, label }) => (
                  <button
                    key={id}
                    type="button"
                    aria-pressed={lightOptics.motion_blur.kind === id}
                    onClick={() => setMotionBlurKind(id)}
                    className={`apx-btn-liquid flex-1 rounded border px-2 py-1 text-xs transition-colors duration-[var(--duration-fast)] ${
                      lightOptics.motion_blur.kind === id
                        ? "apx-btn-liquid-active border-accent bg-accent/10 text-accent"
                        : "border-border text-text-secondary hover:border-accent hover:text-text-primary"
                    }`}
                  >
                    {label}
                  </button>
                ))}
              </div>
              <button
                type="button"
                disabled={subjectSegmenting}
                onClick={() => void useSubjectMask()}
                className={actionButton}
              >
                {subjectSegmenting
                  ? "Stelle frei…"
                  : lightOptics.motion_blur.mask
                    ? "Motivschutz erneuern"
                    : "Motiv schützen"}
              </button>
            </>
          )}

          {tool.group === "channel_matrix" && (
            <div className="flex flex-wrap gap-1" role="group" aria-label="Kanalmatrix-Vorgaben">
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
          )}

          {tool.sliders.map((spec) => (
            <DevelopSlider
              key={spec.key}
              spec={spec}
              value={readField(tool.group, spec.key)}
              onChange={(value) => setField(tool.group, spec.key, value)}
              onCommit={() => void commitDevelopEdit(`${tool.title}: ${spec.label}`)}
            />
          ))}
        </ToolTile>
      ))}
    </div>
  );
}
