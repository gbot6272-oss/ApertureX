import { useState } from "react";

import {
  CREATIVE_TOOL_SPECS,
  NEUTRAL_CREATIVE,
  type CreativeAdjustments,
  type FilmLabProcess,
} from "../lib/edl";
import { useAppStore } from "../store";
import { DevelopSlider } from "./DevelopSlider";
import { ToolTile, ToolToolbar, matchesToolQuery } from "./ToolTiles";

/**
 * Die zehn Kreativ-Werkzeuge aus Phase 27 (siehe `DECISIONS.md`
 * ADR-0057) als eigener Abschnitt der Entwickeln-Registerkarte
 * "Kreativ".
 *
 * **Aufbau bewusst flach und kurz** (Nutzervorgabe "wenig Subtext,
 * einfach zu navigieren"): je Werkzeug eine Glaskachel mit Titel, EINEM
 * Halbsatz Wirkung und den Reglern — keine Erklärabsätze, keine
 * verschachtelten Unterabschnitte. Die Reihenfolge ist dieselbe, in der
 * die Pipeline die Werkzeuge anwendet (`stages/creative.rs`), wer also
 * von oben nach unten liest, sieht die echte Verarbeitungskette.
 *
 * Die beiden Ein-Klick-Vorbereitungen (Motiv freistellen, Tiefenkarte)
 * stehen direkt in der Kachel, zu der sie gehören — nicht in einem
 * separaten KI-Bereich, den man erst suchen müsste.
 *
 * **Phase 28:** Kopf (Suche, „Nur aktive") und Kachel kommen jetzt aus
 * `ToolTiles`, gemeinsam mit dem Licht-&-Optik-Panel — mit 22 Kacheln in
 * zwei Panels braucht es beides, und zwei Fassungen desselben Kopfes
 * würden auseinanderlaufen.
 */
export function CreativePanel() {
  const creative = useAppStore((s) => s.developEdl.creative);
  const setCreativeField = useAppStore((s) => s.setCreativeField);
  const setFilmLabProcess = useAppStore((s) => s.setFilmLabProcess);
  const resetCreative = useAppStore((s) => s.resetCreative);
  const commitDevelopEdit = useAppStore((s) => s.commitDevelopEdit);
  const segmentSubject = useAppStore((s) => s.segmentSubjectForCurrentPhoto);
  const subjectSegmenting = useAppStore((s) => s.subjectSegmenting);
  const useDepthMapForHaze = useAppStore((s) => s.useDepthMapForHaze);
  const depthEstimating = useAppStore((s) => s.depthEstimating);
  const setColorMatchReference = useAppStore((s) => s.setColorMatchReference);
  const colorMatchLoading = useAppStore((s) => s.colorMatchLoading);
  const selectedPhotoId = useAppStore((s) => s.selectedPhotoId);
  const multiSelectedIds = useAppStore((s) => s.multiSelectedIds);

  const [query, setQuery] = useState("");
  const [onlyActive, setOnlyActive] = useState(false);

  function readField(group: keyof CreativeAdjustments, field: string): number {
    return (creative[group] as unknown as Record<string, number>)[field] ?? 0;
  }

  /** Das Referenzfoto für den Farbabgleich: das zweite Foto der
   * Mehrfachauswahl, sonst das aktuell ausgewählte. So braucht es
   * keinen eigenen Auswahldialog — "zwei Fotos markieren, Knopf
   * drücken" ist der kürzeste Weg zum Ziel. */
  const referenceId = multiSelectedIds.find((id) => id !== selectedPhotoId) ?? selectedPhotoId;

  function isActive(tool: (typeof CREATIVE_TOOL_SPECS)[number]): boolean {
    return tool.sliders.some((spec) => readField(tool.group, spec.key) !== spec.neutral);
  }

  function resetTool(group: keyof CreativeAdjustments, title: string) {
    const neutral = NEUTRAL_CREATIVE[group] as unknown as Record<string, unknown>;
    for (const [field, value] of Object.entries(neutral)) {
      if (typeof value === "number") setCreativeField(group, field, value);
    }
    void commitDevelopEdit(`${title} zurückgesetzt`);
  }

  const visible = CREATIVE_TOOL_SPECS.filter(
    (tool) => matchesToolQuery(tool.title, tool.hint, query) && (!onlyActive || isActive(tool)),
  );

  return (
    <div className="flex flex-col gap-3" data-testid="creative-panel">
      <ToolToolbar
        title="Kreativ-Werkzeuge"
        query={query}
        onQueryChange={setQuery}
        onlyActive={onlyActive}
        onOnlyActiveChange={setOnlyActive}
        onReset={resetCreative}
        visibleCount={visible.length}
        totalCount={CREATIVE_TOOL_SPECS.length}
        idPrefix="creative"
      />

      {visible.length === 0 && (
        <p className="text-xs text-text-muted">Kein Werkzeug passt zur Suche.</p>
      )}

      {visible.map((tool) => {
        return (
          <ToolTile
            key={tool.group}
            title={tool.title}
            hint={tool.hint}
            active={isActive(tool)}
            onReset={() => resetTool(tool.group, tool.title)}
          >

            {tool.group === "color_match" && (
              <button
                type="button"
                disabled={colorMatchLoading || !referenceId}
                onClick={() => referenceId && void setColorMatchReference(referenceId)}
                className="apx-btn-liquid self-start rounded border border-accent bg-accent/10 px-3 py-1 text-xs font-medium text-accent transition-[background-color,box-shadow] duration-[var(--duration-fast)] hover:bg-accent/20 disabled:cursor-not-allowed disabled:opacity-50"
              >
                {colorMatchLoading ? "Lese Referenz…" : creative.color_match.has_target ? "Referenz neu lesen" : "Referenzfoto übernehmen"}
              </button>
            )}

            {tool.group === "depth_haze" && (
              <button
                type="button"
                disabled={depthEstimating}
                onClick={() => void useDepthMapForHaze()}
                className="apx-btn-liquid self-start rounded border border-accent bg-accent/10 px-3 py-1 text-xs font-medium text-accent transition-[background-color,box-shadow] duration-[var(--duration-fast)] hover:bg-accent/20 disabled:cursor-not-allowed disabled:opacity-50"
              >
                {depthEstimating ? "Berechne Tiefe…" : creative.depth_haze.depth_map ? "Tiefe für Nebel erneuern" : "Tiefe für Nebel berechnen"}
              </button>
            )}

            {tool.group === "subject_focus" && (
              <button
                type="button"
                disabled={subjectSegmenting}
                onClick={() => void segmentSubject()}
                className="apx-btn-liquid self-start rounded border border-accent bg-accent/10 px-3 py-1 text-xs font-medium text-accent transition-[background-color,box-shadow] duration-[var(--duration-fast)] hover:bg-accent/20 disabled:cursor-not-allowed disabled:opacity-50"
              >
                {subjectSegmenting ? "Stelle frei…" : creative.subject_focus.mask ? "Motiv neu freistellen" : "Motiv freistellen"}
              </button>
            )}

            {tool.group === "film_lab" && (
              <div className="flex gap-1" role="group" aria-label="Filmlabor-Prozess">
                {(
                  [
                    ["BleachBypass", "Bleach Bypass"],
                    ["CrossProcess", "Cross-Processing"],
                  ] as [FilmLabProcess, string][]
                ).map(([value, label]) => (
                  <button
                    key={value}
                    type="button"
                    aria-pressed={creative.film_lab.process === value}
                    onClick={() => {
                      setFilmLabProcess(value);
                      void commitDevelopEdit(`Filmlabor: ${label}`);
                    }}
                    className={`apx-btn-liquid flex-1 rounded border px-2 py-1 text-xs transition-colors duration-[var(--duration-fast)] ${
                      creative.film_lab.process === value
                        ? "apx-btn-liquid-active border-accent bg-accent/10 text-accent"
                        : "border-border text-text-secondary hover:border-accent hover:text-text-primary"
                    }`}
                  >
                    {label}
                  </button>
                ))}
              </div>
            )}

            {tool.sliders.map((spec) => (
              <DevelopSlider
                key={spec.key}
                spec={spec}
                value={readField(tool.group, spec.key)}
                onChange={(value) => setCreativeField(tool.group, spec.key, value)}
                onCommit={() => void commitDevelopEdit(`${tool.title}: ${spec.label}`)}
              />
            ))}
          </ToolTile>
        );
      })}
    </div>
  );
}
