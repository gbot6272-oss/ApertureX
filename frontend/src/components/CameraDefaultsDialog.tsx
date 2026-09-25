import { AlertTriangle, Camera, Check, Trash2 } from "lucide-react";
import { useCallback, useEffect, useState } from "react";

import { buildEdlEnvelopeJson } from "../lib/edl";
import {
  deleteCameraDefault,
  listCameraDefaults,
  setCameraDefault,
  type CameraDefaultDto,
} from "../lib/tauri";
import { useAppStore } from "../store";
import { Dialog } from "./ui/Dialog";

/**
 * Standardentwicklung je Kamera (Phase 34 F7, siehe `DECISIONS.md`
 * ADR-0070 und `camera_default.rs`).
 *
 * **Wofür das da ist.** Jede Kamera hat ihren Charakter: die eine
 * unterbelichtet systematisch, die andere braucht immer etwas
 * Entrauschung. Wer mit zwei Gehäusen arbeitet, stellt nach jedem
 * Import dieselben Regler neu ein.
 *
 * **Die Vorgabe ist der aktuelle Entwickeln-Stand**, nicht ein
 * eigens gebauter Satz Regler. Ein zweiter Regler-Editor neben dem
 * Entwickeln-Panel wäre dieselbe Oberfläche ein zweites Mal — hier
 * stellt man ein Foto so ein, wie man es haben will, und erklärt
 * diesen Stand zur Vorgabe.
 *
 * **Wirkt nur auf künftige Importe.** Das steht so in der Oberfläche,
 * weil die Erwartung sonst naheliegend falsch wäre: eine Vorgabe
 * nachträglich auf vorhandene Fotos anzuwenden würde deren Bearbeitung
 * überschreiben.
 */

interface CameraDefaultsDialogProps {
  open: boolean;
  onClose: () => void;
}

export function CameraDefaultsDialog({ open, onClose }: CameraDefaultsDialogProps) {
  const developEdl = useAppStore((s) => s.developEdl);
  const developPhotoId = useAppStore((s) => s.developPhotoId);
  const selectedFolderId = useAppStore((s) => s.selectedFolderId);
  const photosInFolder = useAppStore((s) => (selectedFolderId ? s.photosByFolder[selectedFolderId] : undefined));

  const [defaults, setDefaults] = useState<CameraDefaultDto[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [saved, setSaved] = useState<string | null>(null);

  const currentCamera =
    photosInFolder?.find((photo) => photo.id === developPhotoId)?.camera_model?.trim() || null;

  const refresh = useCallback(async () => {
    try {
      setDefaults(await listCameraDefaults());
    } catch (err) {
      setError(String(err));
    }
  }, []);

  useEffect(() => {
    if (!open) return;
    setError(null);
    setSaved(null);
    void refresh();
  }, [open, refresh]);

  async function save(): Promise<void> {
    if (!currentCamera) return;
    setError(null);
    try {
      await setCameraDefault(currentCamera, buildEdlEnvelopeJson(developEdl));
      setSaved(currentCamera);
      await refresh();
    } catch (err) {
      setError(String(err));
    }
  }

  async function remove(cameraModel: string): Promise<void> {
    setError(null);
    try {
      await deleteCameraDefault(cameraModel);
      if (saved === cameraModel) setSaved(null);
      await refresh();
    } catch (err) {
      setError(String(err));
    }
  }

  return (
    <Dialog
      open={open}
      onClose={onClose}
      label="Standardentwicklung je Kamera"
      className="flex max-h-[85vh] w-[38rem] max-w-[92vw] flex-col"
    >
      <div className="flex min-h-0 flex-col gap-3 p-4">
        <div>
          <h2 className="flex items-center gap-2 text-sm font-semibold text-text-primary">
            <Camera aria-hidden="true" className="size-4" />
            Standardentwicklung je Kamera
          </h2>
          <p className="mt-0.5 text-xs text-text-muted" data-testid="camera-defaults-summary">
            Wirkt nur auf künftig importierte Fotos — vorhandene bleiben, wie sie sind.
          </p>
        </div>

        {error && (
          <p className="flex items-start gap-2 rounded border border-danger/40 bg-danger/10 px-2 py-1.5 text-xs text-danger" role="alert">
            <AlertTriangle aria-hidden="true" className="mt-0.5 size-3.5 shrink-0" />
            {error}
          </p>
        )}

        {saved && (
          <p className="flex items-center gap-2 rounded border border-success/40 bg-success/10 px-2 py-1.5 text-xs text-success" role="status">
            <Check aria-hidden="true" className="size-3.5 shrink-0" />
            Vorgabe für {saved} gespeichert.
          </p>
        )}

        <div className="rounded border border-border p-2">
          <p className="text-xs text-text-secondary">
            {currentCamera
              ? `Der aktuelle Entwickeln-Stand wird zur Vorgabe für „${currentCamera}".`
              : "Kein Foto im Entwickeln-Panel geöffnet, oder das Foto hat keine Kameraangabe."}
          </p>
          <button
            type="button"
            data-testid="camera-defaults-save"
            disabled={!currentCamera}
            onClick={() => void save()}
            className="apx-btn-liquid mt-2 rounded border border-accent bg-accent/10 px-3 py-1 text-xs text-accent transition-colors duration-[var(--duration-fast)] hover:bg-accent/20 disabled:opacity-50"
          >
            Aktuellen Stand als Vorgabe setzen
          </button>
        </div>

        <ul className="min-h-0 flex-1 overflow-y-auto" data-testid="camera-defaults-list">
          {defaults.length === 0 && (
            <li className="py-2 text-xs text-text-muted">Noch keine Vorgabe hinterlegt.</li>
          )}
          {defaults.map((entry) => (
            <li
              key={entry.camera_model}
              className="flex items-center gap-3 border-b border-border py-1.5 text-xs last:border-b-0"
            >
              <span className="min-w-0 flex-1 truncate text-text-primary">{entry.camera_model}</span>
              <button
                type="button"
                aria-label={`Vorgabe für ${entry.camera_model} entfernen`}
                onClick={() => void remove(entry.camera_model)}
                className="apx-btn-liquid rounded border border-border p-1 text-text-secondary transition-colors duration-[var(--duration-fast)] hover:border-danger hover:text-danger"
              >
                <Trash2 aria-hidden="true" className="size-3.5" />
              </button>
            </li>
          ))}
        </ul>
      </div>
    </Dialog>
  );
}
