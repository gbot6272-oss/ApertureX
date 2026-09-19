import { AlertTriangle, Loader2, Search } from "lucide-react";
import { useEffect } from "react";

import { previewUrl } from "../lib/media";
import { useAppStore } from "../store";
import { Dialog } from "./ui/Dialog";

/**
 * Ähnliche Fotos zu einem Referenzfoto (Phase 33 F9).
 *
 * **Warum das nicht die Duplikatsuche ist.** Die beantwortet „welche
 * Fotos sind dasselbe Bild?" — eine Gruppierung mit fester Schwelle,
 * ohne Referenz und damit ohne Rangfolge. Hier ist ein Foto der
 * Bezugspunkt, und die Treffer sind sortiert.
 *
 * **Der Regler ist der eigentliche Inhalt.** Ein Perceptual Hash ist
 * fast farbenblind: für ihn sind ein Foto und seine Schwarzweiß-Fassung
 * nahezu dasselbe Bild. Wer „ähnliche" sagt, meint aber oft genau das
 * Gegenteil — nicht dasselbe Motiv, sondern denselben Look. Deshalb
 * rechnet das Backend beides, Struktur und Farbe, und dieser Regler
 * entscheidet, was zählt. Ein festes Mischverhältnis hätte die halbe
 * Frage nicht beantworten können.
 */

interface SimilarPhotosDialogProps {
  open: boolean;
  onClose: () => void;
}

export function SimilarPhotosDialog({ open, onClose }: SimilarPhotosDialogProps) {
  const results = useAppStore((s) => s.similarPhotos);
  const running = useAppStore((s) => s.similarPhotosRunning);
  const error = useAppStore((s) => s.similarPhotosError);
  const colorWeight = useAppStore((s) => s.similarityColorWeight);
  const threshold = useAppStore((s) => s.similarityThreshold);
  const setColorWeight = useAppStore((s) => s.setSimilarityColorWeight);
  const setThreshold = useAppStore((s) => s.setSimilarityThreshold);
  const search = useAppStore((s) => s.findSimilarToSelected);
  const selectedPhotoId = useAppStore((s) => s.selectedPhotoId);
  const setMultiSelection = useAppStore((s) => s.setMultiSelection);
  const selectPhoto = useAppStore((s) => s.selectPhoto);

  useEffect(() => {
    if (open) void search();
  }, [open, search]);

  return (
    <Dialog
      open={open}
      onClose={onClose}
      label="Ähnliche Fotos"
      className="flex max-h-[85vh] w-[48rem] max-w-[92vw] flex-col"
    >
      <div className="flex min-h-0 flex-col gap-3 p-4">
        <div className="flex items-start gap-3">
          {selectedPhotoId && (
            <img
              src={previewUrl(selectedPhotoId)}
              alt=""
              className="size-16 shrink-0 rounded border border-accent object-cover"
            />
          )}
          <div>
            <h2 className="text-sm font-semibold text-text-primary">Ähnliche Fotos</h2>
            <p className="mt-0.5 text-xs text-text-muted" data-testid="similar-summary">
              {!selectedPhotoId
                ? "Kein Referenzfoto gewählt."
                : running
                  ? "Sucht…"
                  : results.length === 0
                    ? "Nichts über der Schwelle gefunden. Schwelle senken oder den Regler verschieben."
                    : `${results.length} ${results.length === 1 ? "Treffer" : "Treffer"}`}
            </p>
          </div>
        </div>

        {error && (
          <p className="flex items-start gap-2 rounded border border-danger/40 bg-danger/10 px-2 py-1.5 text-xs text-danger" role="alert">
            <AlertTriangle aria-hidden="true" className="mt-0.5 size-3.5 shrink-0" />
            {error}
          </p>
        )}

        <div className="flex flex-col gap-2">
          <label className="flex items-center gap-2 text-xs text-text-secondary">
            <span className="w-24 shrink-0">Motiv ↔ Farbe</span>
            <input
              type="range"
              min={0}
              max={1}
              step={0.05}
              value={colorWeight}
              onChange={(event) => setColorWeight(Number(event.target.value))}
              onPointerUp={() => void search()}
              onKeyUp={() => void search()}
              aria-label="Gewichtung zwischen Motiv und Farbe"
              className="flex-1"
            />
            <span className="w-28 shrink-0 text-right tabular-nums text-text-muted">
              {Math.round((1 - colorWeight) * 100)} % Motiv
            </span>
          </label>
          <label className="flex items-center gap-2 text-xs text-text-secondary">
            <span className="w-24 shrink-0">Mindestens</span>
            <input
              type="range"
              min={0}
              max={1}
              step={0.05}
              value={threshold}
              onChange={(event) => setThreshold(Number(event.target.value))}
              onPointerUp={() => void search()}
              onKeyUp={() => void search()}
              aria-label="Mindestähnlichkeit"
              className="flex-1"
            />
            <span className="w-28 shrink-0 text-right tabular-nums text-text-muted">
              {Math.round(threshold * 100)} % Ähnlichkeit
            </span>
          </label>
          <div className="flex items-center gap-2">
            <button
              type="button"
              onClick={() => void search()}
              disabled={running || !selectedPhotoId}
              className="apx-btn-liquid flex items-center gap-1.5 rounded border border-border px-2 py-1 text-xs text-text-secondary disabled:opacity-40"
            >
              <Search aria-hidden="true" className="size-3.5" />
              {running ? "Sucht…" : "Neu suchen"}
            </button>
            {running && <Loader2 aria-hidden="true" className="size-3.5 animate-spin text-text-muted" />}
            <button
              type="button"
              onClick={() => {
                setMultiSelection(results.map((entry) => entry.photo.id));
                onClose();
              }}
              disabled={results.length === 0}
              className="apx-btn-liquid ml-auto rounded border border-border px-2 py-1 text-xs text-text-secondary disabled:opacity-40"
            >
              Alle Treffer auswählen
            </button>
          </div>
        </div>

        <ul
          className="grid min-h-0 flex-1 grid-cols-[repeat(auto-fill,minmax(8rem,1fr))] gap-2 overflow-y-auto"
          data-testid="similar-list"
        >
          {results.map((entry) => (
            <li key={entry.photo.id}>
              <button
                type="button"
                onClick={() => {
                  selectPhoto(entry.photo.id);
                  onClose();
                }}
                aria-label={entry.photo.filename}
                className="flex w-full flex-col gap-1 rounded border border-border bg-bg-raised p-1.5 text-left transition-colors hover:border-accent"
              >
                <img src={previewUrl(entry.photo.id)} alt="" className="h-20 w-full rounded object-cover" />
                <span className="truncate text-[11px] text-text-primary">{entry.photo.filename}</span>
                <span className="text-[10px] tabular-nums text-text-muted">
                  {Math.round(entry.similarity * 100)} % ähnlich
                </span>
              </button>
            </li>
          ))}
        </ul>

        <p className="text-[11px] text-text-muted">
          Verglichen werden die Miniaturansichten — Fotos ohne erzeugte Vorschau bleiben außen vor. Das Referenzfoto selbst
          taucht nie in den Treffern auf.
        </p>

        <div className="flex justify-end">
          <button
            type="button"
            onClick={onClose}
            className="apx-btn-liquid rounded border border-border px-3 py-1 text-xs text-text-secondary"
          >
            Schließen
          </button>
        </div>
      </div>
    </Dialog>
  );
}
