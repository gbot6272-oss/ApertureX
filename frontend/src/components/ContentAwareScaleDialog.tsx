import { useState } from "react";

import { useAppStore } from "../store";

const NEUTRAL_FRACTION = 1;
const MIN_FRACTION = 0.3;
const MAX_FRACTION = 2;

/**
 * Inhaltssensitives Skalieren (Content-Aware Scale / Seam Carving, Phase
 * 15 Schritt 4, siehe `DECISIONS.md` ADR-0042 — Photoshop-exklusiv seit
 * CS4, Lightroom kann nur gleichmäßig skalieren/zuschneiden). Wie
 * `CanvasExtendDialog` bewusst ein in sich geschlossener „Zielgröße
 * wählen, Berechnen klicken"-Dialog statt Echtzeit-Reglern — Seam
 * Carving ist für Regler-Ticks zu rechenintensiv (siehe
 * `apx_ai::seam_carving`s Moduldoku).
 */
export function ContentAwareScaleDialog() {
  const open = useAppStore((s) => s.contentAwareScaleDialogOpen);
  const setOpen = useAppStore((s) => s.setContentAwareScaleDialogOpen);
  const selectedPhotoId = useAppStore((s) => s.selectedPhotoId);
  const contentAwareScale = useAppStore((s) => s.developEdl.geometry.content_aware_scale);
  const runContentAwareScale = useAppStore((s) => s.runContentAwareScale);
  const clearContentAwareScale = useAppStore((s) => s.clearContentAwareScale);
  const contentAwareScaleLoading = useAppStore((s) => s.contentAwareScaleLoading);

  const [widthFraction, setWidthFraction] = useState(NEUTRAL_FRACTION);
  const [heightFraction, setHeightFraction] = useState(NEUTRAL_FRACTION);
  const [error, setError] = useState<string | null>(null);

  if (!open) return null;

  function close() {
    setWidthFraction(NEUTRAL_FRACTION);
    setHeightFraction(NEUTRAL_FRACTION);
    setError(null);
    setOpen(false);
  }

  async function handleApply() {
    setError(null);
    try {
      await runContentAwareScale(widthFraction, heightFraction);
      close();
    } catch (err) {
      setError(String(err));
    }
  }

  function handleRemove() {
    clearContentAwareScale();
    close();
  }

  const hasChange = widthFraction !== NEUTRAL_FRACTION || heightFraction !== NEUTRAL_FRACTION;

  return (
    <div className="fixed inset-0 z-50 flex items-start justify-center bg-black/50 pt-8" onClick={close}>
      <div onClick={(e) => e.stopPropagation()} className="flex w-full max-w-md flex-col gap-3 rounded-lg border border-border bg-bg-raised p-4 shadow-xl">
        <h2 className="text-sm font-semibold text-text-primary">Inhaltssensitiv skalieren (Content-Aware Scale)</h2>
        <p className="text-xs text-text-secondary">
          Ändert Breite/Höhe unabhängig voneinander, ohne wichtige Bildinhalte sichtbar zu verzerren (Seam-Carving-Algorithmus) — erkannte Personen/Gesichter werden dabei
          automatisch geschützt. Kein Modell-Download nötig.
        </p>

        {!selectedPhotoId && <p className="text-xs text-danger">Kein Foto ausgewählt.</p>}

        {contentAwareScale?.patch && (
          <p className="text-xs text-text-muted">
            Bereits angewendet: Breite {(contentAwareScale.width_fraction * 100).toFixed(0)}%, Höhe {(contentAwareScale.height_fraction * 100).toFixed(0)}%.
          </p>
        )}

        <div className="grid grid-cols-2 gap-3">
          <label className="flex flex-col gap-1 text-xs text-text-secondary">
            Breite ({(widthFraction * 100).toFixed(0)}%)
            <input
              type="range"
              min={MIN_FRACTION}
              max={MAX_FRACTION}
              step={0.01}
              aria-label="Zielbreite"
              value={widthFraction}
              onChange={(event) => setWidthFraction(Number(event.target.value))}
            />
          </label>
          <label className="flex flex-col gap-1 text-xs text-text-secondary">
            Höhe ({(heightFraction * 100).toFixed(0)}%)
            <input
              type="range"
              min={MIN_FRACTION}
              max={MAX_FRACTION}
              step={0.01}
              aria-label="Zielhöhe"
              value={heightFraction}
              onChange={(event) => setHeightFraction(Number(event.target.value))}
            />
          </label>
        </div>

        {error && <p className="text-xs text-danger">{error}</p>}

        <div className="flex justify-end gap-2">
          <button type="button" onClick={close} className="rounded border border-border px-3 py-1.5 text-xs text-text-secondary hover:border-accent">
            Abbrechen
          </button>
          {contentAwareScale?.patch && (
            <button type="button" onClick={handleRemove} className="rounded border border-border px-3 py-1.5 text-xs text-danger hover:border-danger">
              Entfernen
            </button>
          )}
          <button
            type="button"
            onClick={() => void handleApply()}
            disabled={!selectedPhotoId || !hasChange || contentAwareScaleLoading}
            className="rounded border border-accent bg-accent/10 px-3 py-1.5 text-xs text-accent disabled:cursor-not-allowed disabled:opacity-40"
          >
            {contentAwareScaleLoading ? "Berechnet…" : "Berechnen"}
          </button>
        </div>
      </div>
    </div>
  );
}
