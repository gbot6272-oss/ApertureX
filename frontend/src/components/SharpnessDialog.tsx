import { AlertTriangle, Crown, Loader2, Trash2 } from "lucide-react";
import { useEffect, useState } from "react";

import { previewUrl } from "../lib/media";
import { useAppStore } from "../store";
import { Dialog } from "./ui/Dialog";

/**
 * Schärfe-Bewertung einer Auswahl (Phase 33 F3).
 *
 * **Wofür das da ist.** Die Serien-Erkennung aus Phase 32 F7 sagt „das
 * hier sind zwanzig Aufnahmen einer Reihenaufnahme" — und davon will man
 * genau eine behalten. Welche, sah man bisher nur, indem man jede
 * einzeln auf 100 % zoomte. Das ist die Antwort darauf.
 *
 * **Warum zwei Zahlen pro Foto.** `score` ist das Maximum: wie scharf
 * ist das Bild dort, wo es scharf sein soll. `mean` ist der Schnitt über
 * das ganze Bild. Ein Porträt mit offener Blende hat einen hohen
 * `score` bei niedrigem `mean` — und das ist genau richtig so, nicht
 * schlechter als ein durchgehend mittelmäßiges Bild. Der Balken zeigt
 * den relativen `score`, weil die Rohwerte zwischen verschiedenen
 * Motiven nichts bedeuten.
 *
 * **Das Wegwerfen ist bewusst hier.** „Die schärfste behalten, den Rest
 * weg" ist der eigentliche Arbeitsschritt; ihn in einen zweiten Dialog
 * zu verlegen hieße, die Auswahl noch einmal von Hand nachzubauen. Weg
 * heißt Papierkorb (Phase 33 F1), mit dem Grund „unscharf" — im
 * Papierkorb bleibt damit sichtbar, warum.
 */

interface SharpnessDialogProps {
  open: boolean;
  onClose: () => void;
}

export function SharpnessDialog({ open, onClose }: SharpnessDialogProps) {
  const results = useAppStore((s) => s.sharpnessResults);
  const running = useAppStore((s) => s.sharpnessRunning);
  const error = useAppStore((s) => s.sharpnessError);
  const score = useAppStore((s) => s.scoreSelectionSharpness);
  const trashAllBut = useAppStore((s) => s.trashAllButSharpest);
  const multiSelectedIds = useAppStore((s) => s.multiSelectedIds);
  const selectedPhotoId = useAppStore((s) => s.selectedPhotoId);

  const [confirmingTrash, setConfirmingTrash] = useState(false);

  const selectionSize = multiSelectedIds.length > 0 ? multiSelectedIds.length : selectedPhotoId ? 1 : 0;
  const best = results[0];

  useEffect(() => {
    if (open) void score();
  }, [open, score]);

  useEffect(() => {
    if (!open) setConfirmingTrash(false);
  }, [open]);

  return (
    <Dialog
      open={open}
      onClose={onClose}
      label="Schärfe bewerten"
      className="flex max-h-[85vh] w-[44rem] max-w-[92vw] flex-col"
    >
      <div className="flex min-h-0 flex-col gap-3 p-4">
        <div>
          <h2 className="text-sm font-semibold text-text-primary">Schärfe bewerten</h2>
          <p className="mt-0.5 text-xs text-text-muted" data-testid="sharpness-summary">
            {selectionSize === 0
              ? "Kein Foto ausgewählt."
              : running
                ? "Bewertet…"
                : results.length === 0
                  ? "Keine Vorschau vorhanden — die Bewertung braucht eine erzeugte Standardvorschau."
                  : `${results.length} ${results.length === 1 ? "Aufnahme" : "Aufnahmen"} bewertet · schärfste: ${best?.filename}`}
          </p>
        </div>

        {error && (
          <p className="flex items-start gap-2 rounded border border-danger/40 bg-danger/10 px-2 py-1.5 text-xs text-danger" role="alert">
            <AlertTriangle aria-hidden="true" className="mt-0.5 size-3.5 shrink-0" />
            {error}
          </p>
        )}

        <div className="flex flex-wrap items-center gap-2">
          <button
            type="button"
            onClick={() => void score()}
            disabled={running || selectionSize === 0}
            className="apx-btn-liquid rounded border border-border px-2 py-1 text-xs text-text-secondary disabled:opacity-40"
          >
            {running ? "Bewertet…" : "Neu bewerten"}
          </button>
          {running && <Loader2 aria-hidden="true" className="size-3.5 animate-spin text-text-muted" />}
          <button
            type="button"
            onClick={() => {
              if (!best) return;
              if (!confirmingTrash) {
                setConfirmingTrash(true);
                return;
              }
              setConfirmingTrash(false);
              void trashAllBut(best.photo_id);
            }}
            disabled={results.length < 2}
            data-testid="sharpness-keep-best"
            className="apx-btn-liquid ml-auto flex items-center gap-1.5 rounded border border-danger/50 px-2 py-1 text-xs text-danger disabled:opacity-40"
          >
            <Trash2 aria-hidden="true" className="size-3.5" />
            {confirmingTrash ? "Wirklich? Noch einmal klicken" : `Nur die schärfste behalten (${results.length - 1} weg)`}
          </button>
        </div>

        {confirmingTrash && (
          <p className="rounded border border-danger/40 bg-danger/10 px-2 py-1.5 text-[11px] text-danger">
            Die anderen {results.length - 1} Aufnahmen wandern in den Papierkorb — mit dem Grund „unscharf". Von dort lassen sie
            sich jederzeit zurückholen.
          </p>
        )}

        <ul className="flex min-h-0 flex-1 flex-col gap-1.5 overflow-y-auto" data-testid="sharpness-list">
          {results.map((entry) => (
            <li
              key={entry.photo_id}
              className={`flex items-center gap-2 rounded border p-1.5 ${
                entry.rank === 1 ? "border-accent bg-accent/10" : "border-border bg-bg-raised"
              }`}
            >
              <img src={previewUrl(entry.photo_id)} alt="" className="size-12 rounded object-cover" />
              <div className="min-w-0 flex-1">
                <div className="flex items-center gap-1.5">
                  {entry.rank === 1 && <Crown aria-hidden="true" className="size-3.5 shrink-0 text-accent" />}
                  <span className="truncate text-xs text-text-primary">{entry.filename}</span>
                  <span className="ml-auto shrink-0 text-[11px] tabular-nums text-text-muted">
                    {Math.round(entry.relative * 100)} %
                  </span>
                </div>
                <div className="mt-1 h-1.5 w-full overflow-hidden rounded bg-bg-base">
                  <div
                    className="h-full rounded bg-accent"
                    style={{ width: `${Math.max(2, Math.round(entry.relative * 100))}%` }}
                  />
                </div>
                <p className="mt-0.5 text-[10px] text-text-muted">
                  {entry.mean < entry.score * 0.5 ? "Selektiv scharf (offene Blende)" : "Großflächig scharf"}
                </p>
              </div>
            </li>
          ))}
        </ul>

        <p className="text-[11px] text-text-muted">
          Gemessen wird auf der Standardvorschau, nicht auf dem Original — die Werte vergleichen Aufnahmen desselben Motivs
          miteinander, sie sind kein absolutes Schärfemaß.
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
