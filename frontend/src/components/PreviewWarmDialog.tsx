import { listen } from "@tauri-apps/api/event";
import { AlertTriangle, Gauge, Loader2 } from "lucide-react";
import { useEffect, useState } from "react";

import {
  cancelPreviewWarm,
  previewWarmPlan,
  startPreviewWarm,
  type PreviewWarmFinishedEvent,
  type PreviewWarmPlanDto,
  type PreviewWarmProgressEvent,
} from "../lib/tauri";
import { useAppStore } from "../store";
import { Dialog } from "./ui/Dialog";

/**
 * Vorschauen für einen Ordner vorbereiten (Phase 34 F10, siehe
 * `DECISIONS.md` ADR-0070 und `apx-app`s `preview_warm`).
 *
 * **Was hier eigentlich repariert wird.** Der Import legt für jedes
 * Foto eine 256px-Miniaturansicht an — die reicht fürs Raster. Die
 * 2048px-Stufe, von der das Durchblättern im Einzelbild lebt, wurde bis
 * jetzt *nie* im Voraus erzeugt: beim ersten Ansehen eines Fotos wird
 * die RAW-Datei dekodiert, jedes Mal, für jedes Foto. Bei einem
 * 45-Megapixel-RAW ist das eine bis zwei Sekunden — genau dann, wenn
 * man durch einen frisch importierten Ordner blättert und gerade nicht
 * warten will.
 *
 * Dieser Dialog erledigt diese Arbeit vorher, am Stück, mit
 * Fortschritt und Abbruch. Danach ist der erste Blick auf ein Foto ein
 * Dateiabruf statt einer Dekodierung.
 *
 * **Die Zahl vorher ist die Hauptsache.** Ein Ordner mit tausend RAWs
 * beschäftigt die Maschine minutenlang. Wer das startet, soll vorher
 * wissen, wie viel Arbeit anfällt — und jederzeit abbrechen können,
 * ohne dass das Bisherige verloren geht (fertige Vorschauen bleiben im
 * Cache).
 */

interface PreviewWarmDialogProps {
  open: boolean;
  onClose: () => void;
}

export function PreviewWarmDialog({ open, onClose }: PreviewWarmDialogProps) {
  const selectedFolderId = useAppStore((s) => s.selectedFolderId);
  const folders = useAppStore((s) => s.folders);

  const [wholeCatalog, setWholeCatalog] = useState(false);
  const [force, setForce] = useState(false);
  const [plan, setPlan] = useState<PreviewWarmPlanDto | null>(null);
  const [progress, setProgress] = useState<PreviewWarmProgressEvent | null>(null);
  const [finished, setFinished] = useState<PreviewWarmFinishedEvent | null>(null);
  const [running, setRunning] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const selectedFolder = folders.find((folder) => folder.id === selectedFolderId) ?? null;
  const folderArg = wholeCatalog ? null : selectedFolderId;

  useEffect(() => {
    if (!open) return;
    const unlistenProgress = listen<PreviewWarmProgressEvent>("preview-warm:progress", (event) => {
      setProgress(event.payload);
    });
    const unlistenFinished = listen<PreviewWarmFinishedEvent>("preview-warm:finished", (event) => {
      setFinished(event.payload);
      setRunning(false);
      setProgress(null);
      // Nach dem Lauf steht eine andere Zahl an — neu rechnen, statt die
      // alte stehen zu lassen und zu behaupten, es sei noch alles offen.
      void previewWarmPlan(folderArg, false).then(setPlan).catch(() => undefined);
    });
    return () => {
      void unlistenProgress.then((fn) => fn());
      void unlistenFinished.then((fn) => fn());
    };
  }, [open, folderArg]);

  useEffect(() => {
    if (!open) return;
    setFinished(null);
    setError(null);
    void previewWarmPlan(folderArg, force)
      .then(setPlan)
      .catch((err) => {
        setPlan(null);
        setError(String(err));
      });
  }, [open, folderArg, force]);

  async function start() {
    setError(null);
    setFinished(null);
    try {
      const total = await startPreviewWarm(folderArg, force);
      if (total === 0) {
        setFinished({ prepared: 0, failed: 0, cancelled: false });
        return;
      }
      setRunning(true);
      setProgress({ done: 0, total, current_file: null });
    } catch (err) {
      setError(String(err));
    }
  }

  const percent = progress && progress.total > 0 ? Math.round((progress.done / progress.total) * 100) : 0;

  return (
    <Dialog
      open={open}
      onClose={onClose}
      label="Vorschauen vorbereiten"
      className="flex w-[36rem] max-w-[92vw] flex-col"
    >
      <div className="flex flex-col gap-3 p-4">
        <div>
          <h2 className="flex items-center gap-1.5 text-sm font-semibold text-text-primary">
            <Gauge aria-hidden="true" className="size-4" />
            Vorschauen vorbereiten
          </h2>
          <p className="mt-0.5 text-xs text-text-muted" data-testid="preview-warm-summary">
            {plan
              ? `${plan.pending} ${plan.pending === 1 ? "Foto" : "Fotos"} offen · ${plan.already} schon vorbereitet`
              : "Zählt…"}
          </p>
        </div>

        <p className="text-[11px] text-text-muted">
          Ohne Vorbereitung wird jedes Foto beim ersten Ansehen neu aus der RAW-Datei dekodiert. Das hier erledigt diese
          Arbeit am Stück — danach ist das Durchblättern flüssig.
        </p>

        <div className="flex flex-col gap-1.5 rounded border border-border bg-bg-raised p-2">
          <label className="flex items-center gap-1.5 text-xs text-text-secondary">
            <input
              type="checkbox"
              checked={wholeCatalog}
              disabled={running}
              onChange={(event) => setWholeCatalog(event.target.checked)}
              aria-label="Den ganzen Katalog vorbereiten"
            />
            Ganzen Katalog statt nur {selectedFolder ? `„${selectedFolder.path}“` : "des gewählten Ordners"}
          </label>
          <label className="flex items-center gap-1.5 text-xs text-text-secondary">
            <input
              type="checkbox"
              checked={force}
              disabled={running}
              onChange={(event) => setForce(event.target.checked)}
              aria-label="Vorhandene Vorschauen neu berechnen"
            />
            Vorhandene neu berechnen
          </label>
        </div>

        {error && (
          <p className="flex items-start gap-2 rounded border border-danger/40 bg-danger/10 px-2 py-1.5 text-xs text-danger" role="alert">
            <AlertTriangle aria-hidden="true" className="mt-0.5 size-3.5 shrink-0" />
            {error}
          </p>
        )}

        {running && progress && (
          <div data-testid="preview-warm-progress">
            <div className="h-1.5 overflow-hidden rounded bg-bg-panel">
              <div
                className="h-full bg-accent transition-[width] duration-[var(--duration-base)]"
                style={{ width: `${percent}%` }}
              />
            </div>
            <p className="mt-1 truncate text-[11px] text-text-muted">
              {progress.done} von {progress.total}
              {progress.current_file ? ` · ${progress.current_file}` : ""}
            </p>
          </div>
        )}

        {finished && (
          <p
            className="rounded border border-success/40 bg-success/10 px-2 py-1.5 text-xs text-success"
            data-testid="preview-warm-result"
          >
            {finished.prepared} vorbereitet
            {finished.failed > 0 ? ` · ${finished.failed} nicht lesbar` : ""}
            {finished.cancelled ? " · abgebrochen, das Fertige bleibt erhalten" : ""}
          </p>
        )}

        <div className="flex items-center gap-2">
          {running ? (
            <button
              type="button"
              onClick={() => void cancelPreviewWarm()}
              data-testid="preview-warm-cancel"
              className="apx-btn-liquid ml-auto rounded border border-danger/60 px-3 py-1 text-xs text-danger"
            >
              Abbrechen
            </button>
          ) : (
            <button
              type="button"
              onClick={() => void start()}
              disabled={!plan || plan.pending === 0}
              data-testid="preview-warm-start"
              className="apx-btn-liquid ml-auto rounded border border-accent/60 px-3 py-1 text-xs text-accent disabled:opacity-40"
            >
              {plan && plan.pending > 0 ? `${plan.pending} vorbereiten` : "Nichts zu tun"}
            </button>
          )}
          <button
            type="button"
            onClick={onClose}
            className="apx-btn-liquid rounded border border-border px-3 py-1 text-xs text-text-secondary"
          >
            Schließen
          </button>
        </div>

        {running && (
          <p className="flex items-center gap-1.5 text-[11px] text-text-muted">
            <Loader2 aria-hidden="true" className="size-3 animate-spin" />
            Läuft im Hintergrund — der Dialog darf zu.
          </p>
        )}
      </div>
    </Dialog>
  );
}
