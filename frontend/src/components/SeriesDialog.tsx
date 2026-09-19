import { Camera, Crosshair, Layers, Loader2, SlidersHorizontal, Sparkles } from "lucide-react";
import { useEffect } from "react";

import { previewUrl } from "../lib/media";
import type { DetectedSeriesDto, SeriesKind } from "../lib/tauri";
import { useAppStore } from "../store";
import { Dialog } from "./ui/Dialog";

/**
 * Serien- und Belichtungsreihen-Erkennung (Phase 32 F7).
 *
 * **Was das über „Automatisch stapeln nach Zeit" (Phase 9 Schritt 1)
 * hinaus kann.** Jenes fasst alles zusammen, was zeitlich nah
 * beieinanderliegt, und sagt nicht, *was* da zusammenliegt. Genau das
 * ist aber der Unterschied: zwanzig Aufnahmen einer Vogelserie — davon
 * will man eine. Drei Aufnahmen einer Belichtungsreihe — die will man
 * alle drei, und zwar zusammen für HDR.
 *
 * Die Unterscheidung passiert in Rust (`apx_catalog::series`) über die
 * Belichtungswerte, nicht hier: dieselbe Trennung wie bei der
 * Stapel-Umbenennung — Entscheidungen im Backend, Darstellung im
 * Frontend.
 */

const KIND_LABEL: Record<SeriesKind, string> = {
  burst: "Reihenaufnahme",
  exposure_bracket: "Belichtungsreihe",
  mixed: "Gemischt",
};

const KIND_HINT: Record<SeriesKind, string> = {
  burst: "Gleiche Belichtung in schneller Folge — meist will man hier eine Aufnahme behalten.",
  exposure_bracket: "Gestufte Belichtung — Grundlage für HDR, alle Aufnahmen gehören zusammen.",
  mixed: "Zeitlich zusammen, aber weder gleich belichtet noch sauber gestuft.",
};

const KIND_ICON: Record<SeriesKind, typeof Camera> = {
  burst: Camera,
  exposure_bracket: SlidersHorizontal,
  mixed: Sparkles,
};

interface SeriesDialogProps {
  open: boolean;
  onClose: () => void;
}

export function SeriesDialog({ open, onClose }: SeriesDialogProps) {
  const series = useAppStore((s) => s.detectedSeries);
  const running = useAppStore((s) => s.seriesDetectionRunning);
  const gapSeconds = useAppStore((s) => s.seriesGapSeconds);
  const setGapSeconds = useAppStore((s) => s.setSeriesGapSeconds);
  const runDetection = useAppStore((s) => s.runSeriesDetection);
  const stackSeries = useAppStore((s) => s.stackDetectedSeries);
  const setMultiSelection = useAppStore((s) => s.setMultiSelection);
  const openCompareView = useAppStore((s) => s.openCompareView);
  const selectedFolderId = useAppStore((s) => s.selectedFolderId);
  const requestCommand = useAppStore((s) => s.requestCommand);

  useEffect(() => {
    if (open) void runDetection();
  }, [open, runDetection, gapSeconds]);

  const byKind = (kind: SeriesKind) => series.filter((entry) => entry.kind === kind).length;

  return (
    <Dialog open={open} onClose={onClose} label="Serien und Belichtungsreihen" className="flex max-h-[85vh] w-[46rem] max-w-[92vw] flex-col">
      <div className="flex min-h-0 flex-col gap-3 p-4">
        <div>
          <h2 className="text-sm font-semibold text-text-primary">Serien und Belichtungsreihen</h2>
          <p className="mt-0.5 text-xs text-text-muted" data-testid="series-summary">
            {selectedFolderId
              ? `${series.length} ${series.length === 1 ? "Serie" : "Serien"} gefunden · ${byKind("burst")} Reihenaufnahmen · ${byKind("exposure_bracket")} Belichtungsreihen`
              : "Kein Ordner gewählt."}
          </p>
        </div>

        <div className="flex flex-wrap items-center gap-2">
          <label className="flex items-center gap-2 text-xs text-text-secondary">
            Höchstabstand
            <input
              type="number"
              min={1}
              max={60}
              value={gapSeconds}
              onChange={(event) => setGapSeconds(Number(event.target.value) || 1)}
              aria-label="Höchstabstand in Sekunden"
              className="w-16 rounded border border-border bg-bg-base px-2 py-1 text-xs text-text-primary"
            />
            Sekunden
          </label>
          <button
            type="button"
            onClick={() => void runDetection()}
            disabled={running || !selectedFolderId}
            className="apx-btn-liquid rounded border border-border px-2 py-1 text-xs text-text-secondary disabled:opacity-40"
          >
            {running ? "Sucht…" : "Neu suchen"}
          </button>
          {running && <Loader2 aria-hidden="true" className="size-3.5 animate-spin text-text-muted" />}
        </div>

        <div className="min-h-0 flex-1 overflow-y-auto" data-testid="series-list">
          {series.length === 0 && !running && (
            <p className="text-xs text-text-muted">
              Keine Serie gefunden. Entweder liegt zwischen den Aufnahmen mehr Zeit als eingestellt — oder dieser Ordner enthält
              keine.
            </p>
          )}

          <ul className="flex flex-col gap-2">
            {series.map((entry, index) => (
              <SeriesRow
                key={`${entry.photo_ids[0]}-${index}`}
                entry={entry}
                onStack={() => void stackSeries(entry.photo_ids)}
                onSelect={() => {
                  setMultiSelection(entry.photo_ids);
                  onClose();
                }}
                onCompare={() => {
                  openCompareView(entry.photo_ids.slice(0, 9));
                  onClose();
                }}
                onRateSharpness={() => {
                  // Die Bewertung arbeitet auf der Auswahl — die Serie
                  // wird also erst ausgewählt, dann der Dialog geöffnet.
                  setMultiSelection(entry.photo_ids);
                  onClose();
                  requestCommand("sharpness");
                }}
              />
            ))}
          </ul>
        </div>

        <p className="text-[11px] text-text-muted">
          Fokusreihen und Panoramen sehen in den EXIF-Daten aus wie eine Reihenaufnahme und werden auch so gemeldet — sie ließen
          sich nur über die Fokusdistanz oder den Bildinhalt unterscheiden.
        </p>

        <div className="flex justify-end">
          <button type="button" onClick={onClose} className="apx-btn-liquid rounded border border-border px-3 py-1 text-xs text-text-secondary">
            Schließen
          </button>
        </div>
      </div>
    </Dialog>
  );
}

interface SeriesRowProps {
  entry: DetectedSeriesDto;
  onStack: () => void;
  onSelect: () => void;
  onCompare: () => void;
  /** Phase 33 F3: die naheliegendste Anschlussfrage an eine erkannte
   * Reihenaufnahme ist „und welche davon ist die schärfste?". */
  onRateSharpness: () => void;
}

function SeriesRow({ entry, onStack, onSelect, onCompare, onRateSharpness }: SeriesRowProps) {
  const Icon = KIND_ICON[entry.kind];
  return (
    <li className="rounded border border-border bg-bg-panel p-2" data-testid="series-row" data-kind={entry.kind}>
      <div className="mb-1.5 flex flex-wrap items-center gap-2">
        <span
          title={KIND_HINT[entry.kind]}
          className={`flex items-center gap-1 rounded px-1.5 py-0.5 text-[11px] ${
            entry.kind === "exposure_bracket" ? "bg-accent/15 text-accent" : "bg-bg-raised text-text-secondary"
          }`}
        >
          <Icon aria-hidden="true" className="size-3" />
          {KIND_LABEL[entry.kind]}
        </span>
        <span className="text-xs text-text-secondary">
          {entry.photo_ids.length} Aufnahmen · {entry.span_seconds} s
          {entry.ev_spread !== null ? ` · ${entry.ev_spread.toFixed(1)} EV Spanne` : " · keine Belichtungsdaten"}
        </span>
        <span className="flex-1" />
        <button type="button" onClick={onSelect} className="apx-btn-liquid rounded border border-border px-2 py-0.5 text-[11px] text-text-secondary">
          Auswählen
        </button>
        <button type="button" onClick={onCompare} className="apx-btn-liquid rounded border border-border px-2 py-0.5 text-[11px] text-text-secondary">
          Vergleichen
        </button>
        <button
          type="button"
          onClick={onRateSharpness}
          className="apx-btn-liquid flex items-center gap-1 rounded border border-border px-2 py-0.5 text-[11px] text-text-secondary"
        >
          <Crosshair aria-hidden="true" className="size-3" />
          Schärfe
        </button>
        <button
          type="button"
          onClick={onStack}
          className="apx-btn-liquid flex items-center gap-1 rounded border border-accent bg-accent/10 px-2 py-0.5 text-[11px] text-accent"
        >
          <Layers aria-hidden="true" className="size-3" />
          Stapeln
        </button>
      </div>

      <ul className="flex gap-1.5 overflow-x-auto pb-1">
        {entry.photo_ids.map((photoId, index) => (
          <li key={photoId} className="shrink-0">
            <span className="block size-16 overflow-hidden rounded border border-border">
              <img src={previewUrl(photoId)} alt="" className="size-full object-cover" />
            </span>
            <span className="mt-0.5 block text-center text-[10px] tabular-nums text-text-muted">
              {entry.ev_values[index] !== null && entry.ev_values[index] !== undefined
                ? `${entry.ev_values[index]!.toFixed(1)} EV`
                : "—"}
            </span>
          </li>
        ))}
      </ul>
    </li>
  );
}
