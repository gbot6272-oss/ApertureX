import { AlertTriangle, Check, MapPin } from "lucide-react";
import { useState } from "react";

import { applyGpxGeotag, pickFilePath, previewGpxGeotag, type GpxMatchDto } from "../lib/tauri";
import { useAppStore } from "../store";
import { Dialog } from "./ui/Dialog";

/**
 * Fotos aus einem GPX-Track verorten (Phase 34 F4, siehe
 * `DECISIONS.md` ADR-0070 und `gpx_match.rs`).
 *
 * **Erst zeigen, dann schreiben** — dieselbe Trennung wie beim
 * Ordner-Abgleich (Phase 33 F2). Gerade der Kamerauhr-Versatz sitzt im
 * ersten Versuch fast nie; ohne Vorschau müsste man das Ergebnis
 * hinterher am Katalog prüfen und wieder zurücknehmen.
 *
 * **Vorhandene Positionen bleiben standardmäßig stehen.** Eine von
 * Hand auf der Karte gesetzte Koordinate ist eine Entscheidung und
 * wird nicht stillschweigend von einem Track überschrieben; wer es
 * doch will, schaltet es ausdrücklich ein.
 *
 * Der Zustand liegt bewusst lokal in dieser Komponente und nicht im
 * Store: Trackpfad, Versatz und Toleranz gelten für genau diesen einen
 * Vorgang und sollen ihn nicht überdauern.
 */

interface GpxGeotagDialogProps {
  open: boolean;
  onClose: () => void;
}

const DEFAULT_TOLERANCE = 600;

export function GpxGeotagDialog({ open, onClose }: GpxGeotagDialogProps) {
  const multiSelectedIds = useAppStore((s) => s.multiSelectedIds);
  const selectedPhotoId = useAppStore((s) => s.selectedPhotoId);
  const selectedFolderId = useAppStore((s) => s.selectedFolderId);
  const loadPhotosForFolder = useAppStore((s) => s.loadPhotosForFolder);

  const [gpxPath, setGpxPath] = useState<string | null>(null);
  const [offsetHours, setOffsetHours] = useState(0);
  const [tolerance, setTolerance] = useState(DEFAULT_TOLERANCE);
  const [overwrite, setOverwrite] = useState(false);
  const [matches, setMatches] = useState<GpxMatchDto[]>([]);
  const [written, setWritten] = useState<number | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const targets = multiSelectedIds.length > 0 ? multiSelectedIds : selectedPhotoId ? [selectedPhotoId] : [];
  const writable = matches.filter(
    (entry) => entry.lat !== null && entry.lon !== null && (overwrite || !entry.had_position),
  );

  async function choose(): Promise<void> {
    const path = await pickFilePath("GPX-Track", ["gpx"]);
    if (path) {
      setGpxPath(path);
      setMatches([]);
      setWritten(null);
    }
  }

  async function preview(path: string): Promise<void> {
    setBusy(true);
    setError(null);
    setWritten(null);
    try {
      setMatches(await previewGpxGeotag(path, targets, Math.round(offsetHours * 3600), tolerance));
    } catch (err) {
      setError(String(err));
      setMatches([]);
    } finally {
      setBusy(false);
    }
  }

  async function apply(): Promise<void> {
    setBusy(true);
    setError(null);
    try {
      const count = await applyGpxGeotag(
        writable.map((entry) => ({ photo_id: entry.photo_id, lat: entry.lat!, lon: entry.lon! })),
      );
      setWritten(count);
      if (selectedFolderId) await loadPhotosForFolder(selectedFolderId);
    } catch (err) {
      setError(String(err));
    } finally {
      setBusy(false);
    }
  }

  return (
    <Dialog
      open={open}
      onClose={onClose}
      label="Aus GPX-Track verorten"
      className="flex max-h-[85vh] w-[44rem] max-w-[92vw] flex-col"
    >
      <div className="flex min-h-0 flex-col gap-3 p-4">
        <div>
          <h2 className="flex items-center gap-2 text-sm font-semibold text-text-primary">
            <MapPin aria-hidden="true" className="size-4" />
            Aus GPX-Track verorten
          </h2>
          <p className="mt-0.5 text-xs text-text-muted" data-testid="gpx-geotag-summary">
            {targets.length === 0
              ? "Kein Foto ausgewählt."
              : matches.length === 0
                ? `${targets.length} ${targets.length === 1 ? "Foto" : "Fotos"} ausgewählt · Track wählen`
                : `${writable.length} von ${matches.length} ${matches.length === 1 ? "Foto" : "Fotos"} werden verortet`}
          </p>
        </div>

        {error && (
          <p className="flex items-start gap-2 rounded border border-danger/40 bg-danger/10 px-2 py-1.5 text-xs text-danger" role="alert">
            <AlertTriangle aria-hidden="true" className="mt-0.5 size-3.5 shrink-0" />
            {error}
          </p>
        )}

        {written !== null && !error && (
          <p className="flex items-center gap-2 rounded border border-success/40 bg-success/10 px-2 py-1.5 text-xs text-success" role="status">
            <Check aria-hidden="true" className="size-3.5 shrink-0" />
            {written} {written === 1 ? "Foto verortet" : "Fotos verortet"}.
          </p>
        )}

        <div className="flex flex-wrap items-center gap-3">
          <button
            type="button"
            data-testid="gpx-geotag-choose"
            onClick={() => void choose()}
            disabled={busy || targets.length === 0}
            className="apx-btn-liquid rounded border border-border px-3 py-1 text-xs text-text-secondary transition-colors duration-[var(--duration-fast)] hover:border-accent hover:text-text-primary disabled:opacity-50"
          >
            {gpxPath ? "Anderen Track wählen…" : "GPX-Track wählen…"}
          </button>
          {gpxPath && <span className="truncate text-xs text-text-muted">{gpxPath}</span>}
        </div>

        <div className="flex flex-wrap items-center gap-4">
          <label className="flex items-center gap-2 text-xs text-text-secondary">
            Kamerauhr-Versatz
            <input
              type="number"
              step={0.5}
              value={offsetHours}
              onChange={(event) => setOffsetHours(Number(event.target.value))}
              aria-label="Kamerauhr-Versatz in Stunden"
              className="w-20 rounded border border-border bg-bg-panel px-1.5 py-0.5 text-xs tabular-nums"
            />
            Stunden
          </label>
          <label className="flex items-center gap-2 text-xs text-text-secondary">
            Toleranz
            <input
              type="number"
              step={60}
              min={0}
              value={tolerance}
              onChange={(event) => setTolerance(Number(event.target.value))}
              aria-label="Toleranz in Sekunden"
              className="w-24 rounded border border-border bg-bg-panel px-1.5 py-0.5 text-xs tabular-nums"
            />
            Sekunden
          </label>
          <label className="flex items-center gap-2 text-xs text-text-secondary">
            <input type="checkbox" checked={overwrite} onChange={(event) => setOverwrite(event.target.checked)} />
            Vorhandene Positionen überschreiben
          </label>
          <button
            type="button"
            data-testid="gpx-geotag-preview"
            onClick={() => gpxPath && void preview(gpxPath)}
            disabled={busy || !gpxPath}
            className="apx-btn-liquid rounded border border-border px-3 py-1 text-xs text-text-secondary transition-colors duration-[var(--duration-fast)] hover:border-accent hover:text-text-primary disabled:opacity-50"
          >
            Zuordnung zeigen
          </button>
        </div>

        <ul className="min-h-0 flex-1 overflow-y-auto" data-testid="gpx-geotag-list">
          {matches.map((entry) => (
            <li key={entry.photo_id} className="flex items-center gap-3 border-b border-border py-1.5 text-xs last:border-b-0">
              <span className="min-w-0 flex-1 truncate text-text-secondary">{entry.filename}</span>
              {entry.had_position && !overwrite && entry.lat !== null && (
                <span className="text-text-muted">hat schon eine Position</span>
              )}
              {entry.lat !== null && entry.lon !== null ? (
                <span className="tabular-nums text-text-primary">
                  {entry.lat.toFixed(5)}, {entry.lon.toFixed(5)}
                </span>
              ) : (
                <span className="text-text-muted">{entry.reason ?? "kein Treffer"}</span>
              )}
            </li>
          ))}
        </ul>

        <button
          type="button"
          data-testid="gpx-geotag-apply"
          onClick={() => void apply()}
          disabled={busy || writable.length === 0}
          className="apx-btn-liquid self-start rounded border border-accent bg-accent/10 px-3 py-1 text-xs text-accent transition-colors duration-[var(--duration-fast)] hover:bg-accent/20 disabled:opacity-50"
        >
          {writable.length} {writable.length === 1 ? "Position" : "Positionen"} schreiben
        </button>
      </div>
    </Dialog>
  );
}
