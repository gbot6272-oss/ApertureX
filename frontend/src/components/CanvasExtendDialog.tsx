import { useState } from "react";

import { useAppStore } from "../store";

interface MarginState {
  left: number;
  top: number;
  right: number;
  bottom: number;
}

const NEUTRAL_MARGINS: MarginState = { left: 0, top: 0, right: 0, bottom: 0 };
const MAX_MARGIN = 0.5;

/**
 * Leinwand-Erweiterung / Outpainting (Phase 14 Schritt 1, siehe
 * `DECISIONS.md` ADR-0041) — dasselbe heruntergeladene LaMa-Modell wie
 * das Reparatur-Pinsel-„KI-Ausfüllen" (Phase 13 Schritt 1), nur mit
 * einer anderen Maskenform: statt eines gemalten Strichs gilt der
 * gesamte gewählte Rand um das Original als auszufüllen. Wie bei
 * `LensCalibrationDialog` bewusst ein in sich geschlossener Dialog statt
 * eines eigenen Viewer-Modus — hier reichen vier einfache Regler statt
 * einer Punkt-für-Punkt-Interaktion im Bild.
 */
export function CanvasExtendDialog() {
  const open = useAppStore((s) => s.canvasExtendDialogOpen);
  const setOpen = useAppStore((s) => s.setCanvasExtendDialogOpen);
  const selectedPhotoId = useAppStore((s) => s.selectedPhotoId);
  const canvasExtension = useAppStore((s) => s.developEdl.geometry.canvas_extension);
  const runAiOutpaint = useAppStore((s) => s.runAiOutpaint);
  const clearCanvasExtension = useAppStore((s) => s.clearCanvasExtension);
  const aiOutpaintLoading = useAppStore((s) => s.aiOutpaintLoading);

  const [margins, setMargins] = useState<MarginState>(NEUTRAL_MARGINS);
  const [error, setError] = useState<string | null>(null);

  if (!open) return null;

  function close() {
    setMargins(NEUTRAL_MARGINS);
    setError(null);
    setOpen(false);
  }

  function setMargin(key: keyof MarginState, value: number) {
    setMargins((prev) => ({ ...prev, [key]: Math.min(MAX_MARGIN, Math.max(0, value)) }));
  }

  async function handleApply() {
    setError(null);
    try {
      await runAiOutpaint(margins.left, margins.top, margins.right, margins.bottom);
      close();
    } catch (err) {
      setError(String(err));
    }
  }

  function handleRemove() {
    clearCanvasExtension();
    close();
  }

  const hasAnyMargin = margins.left > 0 || margins.top > 0 || margins.right > 0 || margins.bottom > 0;

  return (
    <div className="fixed inset-0 z-50 flex items-start justify-center bg-black/50 pt-8" onClick={close}>
      <div onClick={(e) => e.stopPropagation()} className="flex w-full max-w-md flex-col gap-3 rounded-lg border border-border bg-bg-raised p-4 shadow-xl">
        <h2 className="text-sm font-semibold text-text-primary">Leinwand erweitern (KI-Ausfüllen über den Bildrand)</h2>
        <p className="text-xs text-text-secondary">
          Vergrößert die Leinwand um die gewählten Ränder (Bruchteil der aktuellen Bildbreite/-höhe) und lässt dieselbe LaMa-KI wie beim Reparatur-Pinsel den neuen Rand füllen —
          braucht dasselbe zuvor heruntergeladene Modell (Einstellungen → KI).
        </p>

        {!selectedPhotoId && <p className="text-xs text-danger">Kein Foto ausgewählt.</p>}

        {canvasExtension?.patch && (
          <p className="text-xs text-text-muted">
            Bereits erweitert: links {(canvasExtension.margin_left * 100).toFixed(0)}%, oben {(canvasExtension.margin_top * 100).toFixed(0)}%, rechts{" "}
            {(canvasExtension.margin_right * 100).toFixed(0)}%, unten {(canvasExtension.margin_bottom * 100).toFixed(0)}%.
          </p>
        )}

        <div className="grid grid-cols-2 gap-3">
          {(
            [
              ["left", "Links"],
              ["top", "Oben"],
              ["right", "Rechts"],
              ["bottom", "Unten"],
            ] as const
          ).map(([key, label]) => (
            <label key={key} className="flex flex-col gap-1 text-xs text-text-secondary">
              {label} ({(margins[key] * 100).toFixed(0)}%)
              <input
                type="range"
                min={0}
                max={MAX_MARGIN}
                step={0.01}
                aria-label={`Rand ${label}`}
                value={margins[key]}
                onChange={(event) => setMargin(key, Number(event.target.value))}
              />
            </label>
          ))}
        </div>

        {error && <p className="text-xs text-danger">{error}</p>}

        <div className="flex justify-end gap-2">
          <button type="button" onClick={close} className="rounded border border-border px-3 py-1.5 text-xs text-text-secondary hover:border-accent">
            Abbrechen
          </button>
          {canvasExtension?.patch && (
            <button type="button" onClick={handleRemove} className="rounded border border-border px-3 py-1.5 text-xs text-danger hover:border-danger">
              Entfernen
            </button>
          )}
          <button
            type="button"
            onClick={() => void handleApply()}
            disabled={!selectedPhotoId || !hasAnyMargin || aiOutpaintLoading}
            className="rounded border border-accent bg-accent/10 px-3 py-1.5 text-xs text-accent disabled:cursor-not-allowed disabled:opacity-40"
          >
            {aiOutpaintLoading ? "Berechne…" : "Anwenden"}
          </button>
        </div>
      </div>
    </div>
  );
}
