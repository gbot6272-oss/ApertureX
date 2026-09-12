import { CREATIVE_TOOL_SPECS, type CreativeAdjustments, type FilmLabProcess } from "../lib/edl";
import { useAppStore } from "../store";
import { DevelopSlider } from "./DevelopSlider";

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

  function readField(group: keyof CreativeAdjustments, field: string): number {
    return (creative[group] as unknown as Record<string, number>)[field] ?? 0;
  }

  /** Das Referenzfoto für den Farbabgleich: das zweite Foto der
   * Mehrfachauswahl, sonst das aktuell ausgewählte. So braucht es
   * keinen eigenen Auswahldialog — "zwei Fotos markieren, Knopf
   * drücken" ist der kürzeste Weg zum Ziel. */
  const referenceId = multiSelectedIds.find((id) => id !== selectedPhotoId) ?? selectedPhotoId;

  return (
    <div className="flex flex-col gap-3" data-testid="creative-panel">
      <div className="flex items-center justify-between">
        <h3 className="text-sm font-semibold text-text-primary">Kreativ-Werkzeuge</h3>
        <button
          type="button"
          onClick={resetCreative}
          className="rounded px-2 py-1 text-xs text-text-muted transition-colors duration-[var(--duration-fast)] hover:text-accent"
        >
          Alles zurücksetzen
        </button>
      </div>

      {CREATIVE_TOOL_SPECS.map((tool) => {
        const active = tool.sliders.some((spec) => readField(tool.group, spec.key) !== spec.neutral);
        return (
          <section
            key={tool.group}
            aria-label={tool.title}
            data-active={active ? "true" : "false"}
            className={`apx-glass flex flex-col gap-2 rounded-lg border p-3 transition-[border-color,box-shadow] duration-[var(--duration-base)] hover:border-accent/50 hover:shadow-[var(--shadow-md)] ${
              active ? "border-accent/60" : "border-[var(--glass-border)]"
            }`}
          >
            <div className="flex items-baseline justify-between gap-2">
              <span className="text-sm font-medium text-text-primary">{tool.title}</span>
              {active && <span className="shrink-0 text-[10px] font-medium tracking-wide text-accent uppercase">aktiv</span>}
            </div>
            <p className="text-xs text-text-muted">{tool.hint}</p>

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
          </section>
        );
      })}
    </div>
  );
}
