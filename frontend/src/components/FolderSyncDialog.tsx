import { AlertTriangle, FilePlus2, FileX2, RefreshCw, RotateCcw } from "lucide-react";
import { useEffect } from "react";

import type { SyncChange } from "../lib/tauri";
import { useAppStore } from "../store";
import { Dialog } from "./ui/Dialog";

/**
 * Ordner-Abgleich (Phase 33 F2).
 *
 * **Warum das nicht der Import ist.** Der Import geht in eine Richtung:
 * er liest alles vom Dateisystem und legt an, was fehlt. Was der
 * Katalog *mehr* hat als der Ordner, sieht er nie — und was sich an
 * einer bereits importierten Datei geändert hat, meldet er nicht,
 * sondern zieht es stillschweigend nach. Nach ein paar Wochen Arbeit am
 * Dateisystem vorbei ist genau das die Frage, die man hat: was ist
 * anders?
 *
 * Deshalb ist die Vorschau hier verpflichtend und das Anwenden ein
 * zweiter Schritt. Die Zuordnung läuft über den Dateinamen — eine
 * umbenannte Datei erscheint als ein Verschwundenes plus ein Neues.
 * Das ist so gewollt und steht als Hinweis auch im Dialog: falsch
 * geraten wäre schlimmer als zweimal gemeldet.
 */

const CHANGE_LABEL: Record<SyncChange, string> = {
  new: "Neu im Ordner",
  vanished: "Nicht mehr im Ordner",
  modified: "Datei geändert",
  returned: "Wieder aufgetaucht",
};

const CHANGE_HINT: Record<SyncChange, string> = {
  new: "Wird beim Anwenden importiert.",
  vanished: "Wird als fehlend markiert — oder auf Wunsch in den Papierkorb geworfen.",
  modified: "Größe oder Zeitstempel weichen ab. Metadaten und Vorschau werden neu gelesen.",
  returned: "War als fehlend markiert und liegt wieder da. Die Markierung wird aufgehoben.",
};

const CHANGE_ICON: Record<SyncChange, typeof FilePlus2> = {
  new: FilePlus2,
  vanished: FileX2,
  modified: RefreshCw,
  returned: RotateCcw,
};

const CHANGE_ORDER: SyncChange[] = ["new", "modified", "vanished", "returned"];

interface FolderSyncDialogProps {
  open: boolean;
  onClose: () => void;
}

export function FolderSyncDialog({ open, onClose }: FolderSyncDialogProps) {
  const plan = useAppStore((s) => s.folderSyncPlan);
  const loading = useAppStore((s) => s.folderSyncLoading);
  const running = useAppStore((s) => s.folderSyncRunning);
  const result = useAppStore((s) => s.folderSyncResult);
  const error = useAppStore((s) => s.folderSyncError);
  const trashVanished = useAppStore((s) => s.folderSyncTrashVanished);
  const setTrashVanished = useAppStore((s) => s.setFolderSyncTrashVanished);
  const preview = useAppStore((s) => s.previewFolderSync);
  const apply = useAppStore((s) => s.applyFolderSync);
  const selectedFolderId = useAppStore((s) => s.selectedFolderId);

  useEffect(() => {
    if (open) void preview();
  }, [open, preview]);

  const counts: Record<SyncChange, number> = {
    new: plan?.new_count ?? 0,
    vanished: plan?.vanished_count ?? 0,
    modified: plan?.modified_count ?? 0,
    returned: plan?.returned_count ?? 0,
  };
  const total = plan?.entries.length ?? 0;

  return (
    <Dialog
      open={open}
      onClose={onClose}
      label="Ordner abgleichen"
      className="flex max-h-[85vh] w-[46rem] max-w-[92vw] flex-col"
    >
      <div className="flex min-h-0 flex-col gap-3 p-4">
        <div>
          <h2 className="text-sm font-semibold text-text-primary">Ordner abgleichen</h2>
          <p className="mt-0.5 text-xs text-text-muted" data-testid="folder-sync-summary">
            {!selectedFolderId
              ? "Kein Ordner gewählt."
              : loading
                ? "Vergleicht Ordner und Katalog…"
                : total === 0
                  ? "Ordner und Katalog stimmen überein."
                  : `${total} ${total === 1 ? "Abweichung" : "Abweichungen"} · ${counts.new} neu · ${counts.modified} geändert · ${counts.vanished} verschwunden · ${counts.returned} wieder da`}
          </p>
        </div>

        {error && (
          <p className="flex items-start gap-2 rounded border border-danger/40 bg-danger/10 px-2 py-1.5 text-xs text-danger" role="alert">
            <AlertTriangle aria-hidden="true" className="mt-0.5 size-3.5 shrink-0" />
            {error}
          </p>
        )}

        {result && (
          <p className="rounded border border-success/40 bg-success/10 px-2 py-1.5 text-xs text-success" data-testid="folder-sync-result">
            {result.handled_vanished} verschwundene {result.handled_vanished === 1 ? "Datei" : "Dateien"} behandelt ·{" "}
            {result.returned} wieder aufgenommen
            {result.import_started ? " · Import für die neuen und geänderten Dateien läuft" : ""}
          </p>
        )}

        <div className="flex flex-wrap items-center gap-2">
          <button
            type="button"
            onClick={() => void preview()}
            disabled={loading || !selectedFolderId}
            className="apx-btn-liquid rounded border border-border px-2 py-1 text-xs text-text-secondary disabled:opacity-40"
          >
            {loading ? "Vergleicht…" : "Neu vergleichen"}
          </button>
          <label className="flex items-center gap-1.5 text-xs text-text-secondary">
            <input
              type="checkbox"
              checked={trashVanished}
              onChange={(event) => setTrashVanished(event.target.checked)}
              aria-label="Verschwundene Fotos in den Papierkorb werfen"
            />
            Verschwundene in den Papierkorb
          </label>
          <button
            type="button"
            onClick={() => void apply()}
            disabled={running || total === 0}
            data-testid="folder-sync-apply"
            className="apx-btn-liquid ml-auto rounded border border-accent/60 px-3 py-1 text-xs text-accent disabled:opacity-40"
          >
            {running ? "Gleicht ab…" : "Abgleich anwenden"}
          </button>
        </div>

        <div className="min-h-0 flex-1 overflow-y-auto" data-testid="folder-sync-list">
          {total === 0 && !loading && (
            <p className="text-xs text-text-muted">
              Jede Datei im Ordner steht im Katalog, und jedes Foto im Katalog liegt im Ordner — in gleicher Größe und mit
              demselben Zeitstempel.
            </p>
          )}

          <div className="flex flex-col gap-3">
            {CHANGE_ORDER.filter((change) => counts[change] > 0).map((change) => {
              const Icon = CHANGE_ICON[change];
              return (
                <section key={change}>
                  <h3 className="mb-1 flex items-center gap-1.5 text-xs font-semibold text-text-secondary">
                    <Icon aria-hidden="true" className="size-3.5" />
                    {CHANGE_LABEL[change]} <span className="font-normal text-text-muted">({counts[change]})</span>
                  </h3>
                  <p className="mb-1 text-[11px] text-text-muted">{CHANGE_HINT[change]}</p>
                  <ul className="flex flex-col gap-0.5 rounded border border-border bg-bg-raised p-1.5">
                    {(plan?.entries ?? [])
                      .filter((entry) => entry.change === change)
                      .map((entry) => (
                        <li key={`${change}-${entry.filename}`} className="truncate font-mono text-[11px] text-text-primary">
                          {entry.filename}
                        </li>
                      ))}
                  </ul>
                </section>
              );
            })}
          </div>
        </div>

        <p className="text-[11px] text-text-muted">
          Eine umbenannte Datei erscheint als ein Verschwundenes plus ein Neues. Den Unterschied könnte nur ein Vergleich der
          Bildinhalte sicher machen — falsch geraten würde die Bearbeitungen des einen Fotos einem anderen zuschlagen.
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
