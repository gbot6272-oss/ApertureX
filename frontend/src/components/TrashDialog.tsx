import { AlertTriangle, RotateCcw, Trash2 } from "lucide-react";
import { useEffect, useMemo, useState } from "react";

import { previewUrl } from "../lib/media";
import type { TrashEntryDto, TrashReason } from "../lib/tauri";
import { useAppStore } from "../store";
import { Dialog } from "./ui/Dialog";

/**
 * Papierkorb (Phase 33 F1).
 *
 * **Warum das kein simpler „Löschen"-Knopf ist.** Ein Foto hängt an
 * einem Dutzend Tabellen — Bearbeitungsstände, Schnappschüsse,
 * Stichwörter, Notizen, Sammlungen, Stapel, erkannte Gesichter. Ein
 * echtes Löschen reißt das alles mit, und ein „Rückgängig" könnte davon
 * nichts zurückholen. Deshalb bleibt die Zeile im Katalog stehen und
 * bekommt nur ein Datum; siehe `migrations/0014_trash.sql`.
 *
 * **Zwei getrennte Stufen, bewusst.** „Wiederherstellen" ist gratis und
 * jederzeit möglich. „Endgültig löschen" ist die einzige Stelle in der
 * ganzen App, an der Dateien von der Platte verschwinden können — und
 * genau deshalb muss man dort den Haken für die Dateien selbst setzen
 * und die Frage danach noch einmal bestätigen.
 */

const REASON_LABEL: Record<TrashReason, string> = {
  manual: "Von Hand weggeworfen",
  duplicate: "Als Duplikat aussortiert",
  blurry: "Als unscharf aussortiert",
  missing: "Beim Ordner-Abgleich verschwunden",
};

const REASON_ORDER: TrashReason[] = ["manual", "duplicate", "blurry", "missing"];

function formatDate(value: string): string {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return value;
  return date.toLocaleDateString(undefined, { year: "numeric", month: "short", day: "numeric" });
}

interface TrashDialogProps {
  open: boolean;
  onClose: () => void;
}

export function TrashDialog({ open, onClose }: TrashDialogProps) {
  const entries = useAppStore((s) => s.trashEntries);
  const loading = useAppStore((s) => s.trashLoading);
  const error = useAppStore((s) => s.trashError);
  const selectedIds = useAppStore((s) => s.trashSelectedIds);
  const refreshTrash = useAppStore((s) => s.refreshTrash);
  const toggleSelection = useAppStore((s) => s.toggleTrashSelection);
  const setSelection = useAppStore((s) => s.setTrashSelection);
  const restore = useAppStore((s) => s.restoreFromTrash);
  const empty = useAppStore((s) => s.emptyTrash);

  const [deleteFiles, setDeleteFiles] = useState(false);
  const [confirming, setConfirming] = useState(false);

  useEffect(() => {
    if (open) void refreshTrash();
  }, [open, refreshTrash]);

  // Die Bestätigung gilt für genau einen Knopfdruck. Bleibt sie über das
  // Schließen hinaus stehen, kann beim nächsten Öffnen ein einziger
  // Klick den ganzen Papierkorb leeren.
  useEffect(() => {
    if (!open) {
      setConfirming(false);
      setDeleteFiles(false);
    }
  }, [open]);

  const grouped = useMemo(() => {
    const byReason = new Map<TrashReason, TrashEntryDto[]>();
    for (const entry of entries) {
      const list = byReason.get(entry.reason) ?? [];
      list.push(entry);
      byReason.set(entry.reason, list);
    }
    return REASON_ORDER.filter((reason) => byReason.has(reason)).map((reason) => ({
      reason,
      entries: byReason.get(reason) ?? [],
    }));
  }, [entries]);

  const targets = selectedIds.length > 0 ? selectedIds : [];
  const allSelected = entries.length > 0 && selectedIds.length === entries.length;

  return (
    <Dialog
      open={open}
      onClose={onClose}
      label="Papierkorb"
      className="flex max-h-[85vh] w-[48rem] max-w-[92vw] flex-col"
    >
      <div className="flex min-h-0 flex-col gap-3 p-4">
        <div>
          <h2 className="text-sm font-semibold text-text-primary">Papierkorb</h2>
          <p className="mt-0.5 text-xs text-text-muted" data-testid="trash-summary">
            {entries.length === 0
              ? "Der Papierkorb ist leer."
              : `${entries.length} ${entries.length === 1 ? "Foto" : "Fotos"} weggeworfen · ${selectedIds.length} ausgewählt`}
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
            onClick={() => setSelection(allSelected ? [] : entries.map((entry) => entry.photo.id))}
            disabled={entries.length === 0}
            className="apx-btn-liquid rounded border border-border px-2 py-1 text-xs text-text-secondary disabled:opacity-40"
          >
            {allSelected ? "Auswahl aufheben" : "Alle auswählen"}
          </button>
          <button
            type="button"
            onClick={() => void restore(targets)}
            disabled={targets.length === 0}
            className="apx-btn-liquid flex items-center gap-1.5 rounded border border-border px-2 py-1 text-xs text-text-secondary disabled:opacity-40"
          >
            <RotateCcw aria-hidden="true" className="size-3.5" />
            Wiederherstellen
          </button>

          <span className="ml-auto flex items-center gap-2">
            <label className="flex items-center gap-1.5 text-xs text-text-secondary">
              <input
                type="checkbox"
                checked={deleteFiles}
                onChange={(event) => {
                  setDeleteFiles(event.target.checked);
                  setConfirming(false);
                }}
                aria-label="Dateien von der Platte mitlöschen"
              />
              Dateien mitlöschen
            </label>
            <button
              type="button"
              onClick={() => {
                if (!confirming) {
                  setConfirming(true);
                  return;
                }
                setConfirming(false);
                void empty(targets, deleteFiles);
              }}
              disabled={entries.length === 0}
              data-testid="trash-purge"
              className="apx-btn-liquid flex items-center gap-1.5 rounded border border-danger/50 px-2 py-1 text-xs text-danger disabled:opacity-40"
            >
              <Trash2 aria-hidden="true" className="size-3.5" />
              {confirming
                ? "Wirklich? Noch einmal klicken"
                : targets.length > 0
                  ? `${targets.length} endgültig löschen`
                  : "Papierkorb leeren"}
            </button>
          </span>
        </div>

        {confirming && (
          <p className="rounded border border-danger/40 bg-danger/10 px-2 py-1.5 text-[11px] text-danger">
            {deleteFiles
              ? "Die Dateien werden von der Platte gelöscht. Das lässt sich hier nicht rückgängig machen."
              : "Die Katalogeinträge samt Bearbeitungsständen, Notizen und Stichwörtern sind danach weg. Die Bilddateien selbst bleiben liegen."}
          </p>
        )}

        <div className="min-h-0 flex-1 overflow-y-auto" data-testid="trash-list">
          {loading && entries.length === 0 && <p className="text-xs text-text-muted">Lädt…</p>}
          {!loading && entries.length === 0 && (
            <p className="text-xs text-text-muted">
              Hier landen weggeworfene Fotos. Sie verschwinden aus Raster, Suche und Statistik, bleiben aber samt allem, was an
              ihnen hängt, erhalten — bis sie hier endgültig gelöscht werden.
            </p>
          )}

          <div className="flex flex-col gap-4">
            {grouped.map((group) => (
              <section key={group.reason}>
                <h3 className="mb-1.5 text-xs font-semibold text-text-secondary">
                  {REASON_LABEL[group.reason]}{" "}
                  <span className="font-normal text-text-muted">({group.entries.length})</span>
                </h3>
                <ul className="grid grid-cols-[repeat(auto-fill,minmax(9rem,1fr))] gap-2">
                  {group.entries.map((entry) => {
                    const selected = selectedIds.includes(entry.photo.id);
                    return (
                      <li key={entry.photo.id}>
                        <button
                          type="button"
                          onClick={() => toggleSelection(entry.photo.id)}
                          aria-pressed={selected}
                          aria-label={entry.photo.filename}
                          className={`flex w-full flex-col gap-1 rounded border p-1.5 text-left transition-colors ${
                            selected ? "border-accent bg-accent/10" : "border-border bg-bg-raised hover:border-text-muted"
                          }`}
                        >
                          <img
                            src={previewUrl(entry.photo.id)}
                            alt=""
                            className="h-20 w-full rounded object-cover opacity-70"
                          />
                          <span className="truncate text-[11px] text-text-primary">{entry.photo.filename}</span>
                          <span className="text-[10px] text-text-muted">{formatDate(entry.deleted_at)}</span>
                        </button>
                      </li>
                    );
                  })}
                </ul>
              </section>
            ))}
          </div>
        </div>

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
