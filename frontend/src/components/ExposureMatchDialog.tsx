import { AlertTriangle, Check, Loader2, SunMedium } from "lucide-react";
import { useEffect } from "react";

import { previewUrl } from "../lib/media";
import { useAppStore } from "../store";
import { Dialog } from "./ui/Dialog";

/**
 * Belichtung an ein Referenzfoto angleichen (Phase 34 F2, siehe
 * `DECISIONS.md` ADR-0070).
 *
 * **Wofür das da ist.** In einer Serie aus demselben Licht schwanken die
 * Belichtungen um Bruchteile einer Blendenstufe, weil die Automatik auf
 * unterschiedlich helle Motive reagiert hat. Von Hand gleicht man das
 * Foto für Foto am Regler an — und trifft es nie genau.
 *
 * **Das aktuell gewählte Foto ist die Referenz**, die Mehrfachauswahl
 * sind die Ziele. Die Alternative — Referenz im Dialog auswählen — wäre
 * ein zweiter Auswahlmechanismus neben dem, den die App überall sonst
 * benutzt; hier gilt dieselbe Auswahl wie bei Export, Stapelbewertung
 * und Schärfe.
 *
 * **Warum die Korrektur relativ ist.** Angerechnet wird auf den
 * bisherigen Belichtungswert jedes Zielfotos, nicht ersetzt: ein Foto,
 * an dem schon +0,3 EV standen, soll seine Bearbeitung behalten und
 * zusätzlich angeglichen werden.
 */

interface ExposureMatchDialogProps {
  open: boolean;
  onClose: () => void;
}

/** Ein EV-Wert mit Vorzeichen, wie ihn der Belichtungsregler zeigt. */
function formatEv(value: number): string {
  const rounded = Math.round(value * 100) / 100;
  return `${rounded > 0 ? "+" : ""}${rounded.toFixed(2)} EV`;
}

export function ExposureMatchDialog({ open, onClose }: ExposureMatchDialogProps) {
  const results = useAppStore((s) => s.exposureMatchResults);
  const running = useAppStore((s) => s.exposureMatchRunning);
  const applied = useAppStore((s) => s.exposureMatchApplied);
  const error = useAppStore((s) => s.exposureMatchError);
  const measure = useAppStore((s) => s.measureExposureMatch);
  const apply = useAppStore((s) => s.applyExposureMatch);
  const selectedPhotoId = useAppStore((s) => s.selectedPhotoId);
  const multiSelectedIds = useAppStore((s) => s.multiSelectedIds);

  useEffect(() => {
    if (open) void measure();
  }, [open, measure]);

  const measurable = results.filter((entry) => entry.measurable);
  const targetCount = multiSelectedIds.filter((id) => id !== selectedPhotoId).length;
  const largest = measurable.reduce((max, entry) => Math.max(max, Math.abs(entry.delta_ev)), 0);

  return (
    <Dialog
      open={open}
      onClose={onClose}
      label="Belichtung angleichen"
      className="flex max-h-[85vh] w-[40rem] max-w-[92vw] flex-col"
    >
      <div className="flex min-h-0 flex-col gap-3 p-4">
        <div>
          <h2 className="flex items-center gap-2 text-sm font-semibold text-text-primary">
            <SunMedium aria-hidden="true" className="size-4" />
            Belichtung angleichen
          </h2>
          <p className="mt-0.5 text-xs text-text-muted" data-testid="exposure-match-summary">
            {!selectedPhotoId
              ? "Kein Referenzfoto — das aktuell gewählte Foto ist die Referenz."
              : targetCount === 0
                ? "Keine Zielfotos — mehrere Fotos auswählen, das aktive davon ist die Referenz."
                : running
                  ? "Misst…"
                  : `${measurable.length} von ${results.length} ${results.length === 1 ? "Foto" : "Fotos"} messbar · größte Abweichung ${formatEv(largest)}`}
          </p>
        </div>

        {error && (
          <p className="flex items-start gap-2 rounded border border-danger/40 bg-danger/10 px-2 py-1.5 text-xs text-danger" role="alert">
            <AlertTriangle aria-hidden="true" className="mt-0.5 size-3.5 shrink-0" />
            {error}
          </p>
        )}

        {applied !== null && !error && (
          <p className="flex items-center gap-2 rounded border border-success/40 bg-success/10 px-2 py-1.5 text-xs text-success" role="status">
            <Check aria-hidden="true" className="size-3.5 shrink-0" />
            {applied} {applied === 1 ? "Foto angeglichen" : "Fotos angeglichen"}.
          </p>
        )}

        <ul className="min-h-0 flex-1 overflow-y-auto" data-testid="exposure-match-list">
          {results.map((entry) => (
            <li
              key={entry.photo_id}
              className="flex items-center gap-3 border-b border-border py-1.5 last:border-b-0"
            >
              <img
                src={previewUrl(entry.photo_id)}
                alt={entry.filename}
                className="size-10 shrink-0 rounded object-cover"
              />
              <span className="min-w-0 flex-1 truncate text-xs text-text-secondary">{entry.filename}</span>
              {entry.measurable ? (
                <span
                  className={`tabular-nums text-xs ${
                    Math.abs(entry.delta_ev) < 0.05 ? "text-text-muted" : "text-text-primary"
                  }`}
                >
                  {formatEv(entry.delta_ev)}
                </span>
              ) : (
                <span className="text-xs text-text-muted" title="Keine Vorschau oder vollständig über-/unterbelichtet">
                  nicht messbar
                </span>
              )}
            </li>
          ))}
        </ul>

        <div className="flex flex-wrap items-center gap-2">
          <button
            type="button"
            onClick={() => void measure()}
            disabled={running || !selectedPhotoId || targetCount === 0}
            className="apx-btn-liquid rounded border border-border px-3 py-1 text-xs text-text-secondary transition-colors duration-[var(--duration-fast)] hover:border-accent hover:text-text-primary disabled:opacity-50"
          >
            Erneut messen
          </button>
          <button
            type="button"
            data-testid="exposure-match-apply"
            onClick={() => void apply()}
            disabled={running || measurable.length === 0}
            className="apx-btn-liquid rounded border border-accent bg-accent/10 px-3 py-1 text-xs text-accent transition-colors duration-[var(--duration-fast)] hover:bg-accent/20 disabled:opacity-50"
          >
            {running ? <Loader2 aria-hidden="true" className="mr-1 inline size-3 animate-spin" /> : null}
            Auf {measurable.length} {measurable.length === 1 ? "Foto" : "Fotos"} anwenden
          </button>
          <p className="text-[11px] text-text-muted">
            Wird auf die bisherige Belichtung angerechnet, nicht ersetzt.
          </p>
        </div>
      </div>
    </Dialog>
  );
}
