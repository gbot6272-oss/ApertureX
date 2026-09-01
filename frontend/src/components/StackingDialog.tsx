import { useAppStore } from "../store";

interface StackingDialogProps {
  open: boolean;
  onClose: () => void;
}

/**
 * Fokus-/HDR-/Panorama-/Astro-Stacking (Phase 9 Schritt 8, siehe
 * `PLAN.md`, `DECISIONS.md` ADR-0035 Punkt 2) — nimmt die aktuelle
 * Mehrfachauswahl, ruft den jeweiligen Algorithmus in `apx-stacking` auf
 * (reine, deterministische Bildverarbeitung, keine externe Registrierungs-
 * Bibliothek) und importiert das Ergebnis als neues, per Stapel mit den
 * Quellfotos verknüpftes Katalogfoto (`Catalog::create_stack`, Phase 9
 * Schritt 1).
 *
 * **Bewusste Vereinfachung**: alle vier Algorithmen setzen bereits
 * pixelgenau ausgerichtete Quellbilder voraus (Fokus/HDR: Stativaufnahmen;
 * Panorama/Astro: reine Verschiebungs-Registrierung per Phasenkorrelation,
 * kein Homographie-Stitching für Freihandaufnahmen — siehe
 * `apx_stacking::panorama`s Moduldoku).
 */
export function StackingDialog({ open, onClose }: StackingDialogProps) {
  const multiSelectedIds = useAppStore((s) => s.multiSelectedIds);
  const stackingRunning = useAppStore((s) => s.stackingRunning);
  const stackingStatus = useAppStore((s) => s.stackingStatus);
  const runStackFocus = useAppStore((s) => s.runStackFocus);
  const runStackHdr = useAppStore((s) => s.runStackHdr);
  const runStackPanorama = useAppStore((s) => s.runStackPanorama);
  const runStackAstro = useAppStore((s) => s.runStackAstro);

  if (!open) return null;

  const count = multiSelectedIds.length;
  const busy = stackingRunning !== null;

  return (
    <div className="fixed inset-0 z-50 flex items-start justify-center bg-black/50 pt-24" onClick={onClose}>
      <div
        role="dialog"
        aria-label="Stacking"
        className="w-full max-w-md rounded-lg border border-border bg-bg-raised p-4 shadow-xl"
        onClick={(event) => event.stopPropagation()}
      >
        <h2 className="mb-1 text-sm font-semibold text-text-primary">Stacking</h2>
        <p className="mb-3 text-xs text-text-muted">
          {count} {count === 1 ? "Foto" : "Fotos"} ausgewählt — alle vier Verfahren setzen bereits ausgerichtete Aufnahmen voraus.
        </p>

        <div className="flex flex-col gap-2">
          <button
            type="button"
            onClick={() => void runStackFocus()}
            disabled={busy || count < 2}
            title="Laplacian-Schärfemaß, schärfste Quelle je Pixel — mindestens 2 Fotos"
            className="rounded border border-border px-3 py-1.5 text-left text-xs hover:border-accent disabled:cursor-not-allowed disabled:opacity-40"
          >
            Fokus-Stacking
          </button>
          <button
            type="button"
            onClick={() => void runStackHdr()}
            disabled={busy || count < 2}
            title="Belichtungsreihe fusionieren (jedes Foto braucht eine EXIF-Belichtungszeit) — mindestens 2 Fotos"
            className="rounded border border-border px-3 py-1.5 text-left text-xs hover:border-accent disabled:cursor-not-allowed disabled:opacity-40"
          >
            HDR-Zusammenführung
          </button>
          <button
            type="button"
            onClick={() => void runStackPanorama()}
            disabled={busy || count < 2}
            title="Nur Verschiebungs-Registrierung (Stativ-/gleicher-Blickpunkt-Aufnahmen) — mindestens 2 Fotos"
            className="rounded border border-border px-3 py-1.5 text-left text-xs hover:border-accent disabled:cursor-not-allowed disabled:opacity-40"
          >
            Panorama-Zusammenführung
          </button>
          <button
            type="button"
            onClick={() => void runStackAstro()}
            disabled={busy || count < 3}
            title="Sigma-geclipptes Mittel über Kurzbelichtungen — mindestens 3 Fotos"
            className="rounded border border-border px-3 py-1.5 text-left text-xs hover:border-accent disabled:cursor-not-allowed disabled:opacity-40"
          >
            Astro-Stacking
          </button>
        </div>

        {busy && <p className="mt-3 text-xs text-text-muted">Läuft…</p>}
        {stackingStatus && !busy && <p className="mt-3 text-xs text-text-secondary">{stackingStatus}</p>}

        <div className="mt-3 flex justify-end">
          <button type="button" onClick={onClose} className="rounded border border-border px-3 py-1 text-xs text-text-secondary hover:bg-bg-panel">
            Schließen
          </button>
        </div>
      </div>
    </div>
  );
}
