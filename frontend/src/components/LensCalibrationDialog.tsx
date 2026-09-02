import { useMemo, useState } from "react";

import * as api from "../lib/tauri";
import { previewUrl } from "../lib/media";
import { useAppStore } from "../store";

interface Point {
  x: number;
  y: number;
}

const MIN_POINTS_PER_LINE = 3;

/**
 * Objektiv-Kalibrierung aus eigenen Fotos (Phase 12 Schritt 3 Teil B,
 * siehe `DECISIONS.md` ADR-0039, `apx_ai::lens_calibration`s Moduldoku) —
 * für Objektive außerhalb der echten LensFun-Datenbank
 * (`lib/edl.ts`/`apx_pipeline::lens_profiles`).
 *
 * **Ehrlich beschriftet:** „aus eigenen Kalibrierfotos berechnet", nicht
 * „KI-generiert" — es ist klassische Optimierung (Rasterverfeinerung
 * über einen einzigen Verzeichnungskoeffizienten), kein gelerntes
 * Modell. Der Nutzer markiert selbst mehrere Punkte entlang einer in
 * der Realität geraden Linie (Schachbrett-Gitterlinie, Wandkante,
 * Horizont), direkt auf einer Vorschau des aktuellen Fotos — bewusst
 * kein eigener Viewer-Modus wie `RepairOverlay`/`MaskOverlay` (dieselbe
 * Interaktion ließe sich dort einbauen, aber eine in sich geschlossene
 * Dialog-Vorschau reicht für eine einmalige Kalibrierung und hält
 * `Viewer.tsx`s bereits komplexe Zustandsmaschine unangetastet).
 */
export function LensCalibrationDialog() {
  const open = useAppStore((s) => s.lensCalibrationDialogOpen);
  const setOpen = useAppStore((s) => s.setLensCalibrationDialogOpen);
  const selectedPhotoId = useAppStore((s) => s.selectedPhotoId);
  const setCustomDistortionK1 = useAppStore((s) => s.setLensCorrectionCustomDistortionK1);

  const [lines, setLines] = useState<Point[][]>([[]]);
  const [result, setResult] = useState<number | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const validLineCount = useMemo(() => lines.filter((line) => line.length >= MIN_POINTS_PER_LINE).length, [lines]);
  const currentLine = lines[lines.length - 1] ?? [];

  if (!open) return null;

  function handleImageClick(event: React.MouseEvent<HTMLDivElement>) {
    const rect = event.currentTarget.getBoundingClientRect();
    const x = Math.min(1, Math.max(0, (event.clientX - rect.left) / rect.width));
    const y = Math.min(1, Math.max(0, (event.clientY - rect.top) / rect.height));
    setResult(null);
    setLines((prev) => {
      const next = prev.map((line) => [...line]);
      const last = next[next.length - 1];
      if (last) last.push({ x, y });
      return next;
    });
  }

  function addNewLine() {
    setResult(null);
    setLines((prev) => [...prev, []]);
  }

  function undoLastPoint() {
    setResult(null);
    setLines((prev) => {
      const next = prev.map((line) => [...line]);
      const last = next[next.length - 1];
      if (last && last.length > 0) {
        last.pop();
      } else if (next.length > 1) {
        next.pop();
      }
      return next;
    });
  }

  function reset() {
    setLines([[]]);
    setResult(null);
    setError(null);
  }

  function close() {
    reset();
    setOpen(false);
  }

  async function handleCalibrate() {
    setBusy(true);
    setError(null);
    try {
      const usableLines = lines.filter((line) => line.length >= MIN_POINTS_PER_LINE);
      const k1 = await api.calibrateLensDistortion(usableLines);
      setResult(k1);
    } catch (err) {
      setError(String(err));
    } finally {
      setBusy(false);
    }
  }

  function handleApply() {
    if (result === null) return;
    setCustomDistortionK1(result);
    close();
  }

  return (
    <div className="fixed inset-0 z-50 flex items-start justify-center bg-black/50 pt-8" onClick={close}>
      <div onClick={(e) => e.stopPropagation()} className="flex max-h-[90vh] w-full max-w-3xl flex-col gap-3 overflow-y-auto rounded-lg border border-border bg-bg-raised p-4 shadow-xl">
        <h2 className="text-sm font-semibold text-text-primary">Objektiv kalibrieren</h2>
        <p className="text-xs text-text-secondary">
          Markiere mehrere Punkte entlang einer Linie, die in der Realität gerade ist (z. B. eine Schachbrett-Gitterlinie, eine Wandkante oder der Horizont) — mindestens{" "}
          {MIN_POINTS_PER_LINE} Punkte pro Linie, am besten mindestens zwei Linien an unterschiedlichen Stellen des Bildes. Aus der Krümmung dieser Linien wird ein
          Verzeichnungskoeffizient berechnet (klassische Optimierung, kein gelerntes Modell) — kein Ersatz für ein echtes Objektivprofil, aber besser als keine Korrektur für
          Objektive außerhalb der LensFun-Datenbank.
        </p>

        {!selectedPhotoId && <p className="text-xs text-danger">Kein Foto ausgewählt.</p>}

        {selectedPhotoId && (
          <div className="relative w-full cursor-crosshair select-none overflow-hidden rounded border border-border" onClick={handleImageClick}>
            <img src={previewUrl(selectedPhotoId, 1)} alt="Foto zur Objektiv-Kalibrierung" className="pointer-events-none block w-full" draggable={false} />
            <svg className="pointer-events-none absolute inset-0 h-full w-full overflow-visible" preserveAspectRatio="none" viewBox="0 0 100 100">
              {lines.map((line, lineIndex) => (
                <g key={lineIndex}>
                  <polyline
                    points={line.map((p) => `${p.x * 100},${p.y * 100}`).join(" ")}
                    fill="none"
                    stroke={lineIndex === lines.length - 1 ? "#ffd60a" : "#34c759"}
                    strokeWidth={0.3}
                    vectorEffect="non-scaling-stroke"
                  />
                  {line.map((p, pointIndex) => (
                    <circle key={pointIndex} cx={p.x * 100} cy={p.y * 100} r={0.5} fill={lineIndex === lines.length - 1 ? "#ffd60a" : "#34c759"} vectorEffect="non-scaling-stroke" />
                  ))}
                </g>
              ))}
            </svg>
          </div>
        )}

        <div className="flex flex-wrap items-center gap-2">
          <button type="button" onClick={addNewLine} disabled={currentLine.length === 0} className="rounded border border-border px-2 py-1 text-xs text-text-secondary hover:border-accent disabled:cursor-not-allowed disabled:opacity-40">
            + Neue Linie
          </button>
          <button type="button" onClick={undoLastPoint} className="rounded border border-border px-2 py-1 text-xs text-text-secondary hover:border-accent">
            Letzten Punkt entfernen
          </button>
          <button type="button" onClick={reset} className="rounded border border-border px-2 py-1 text-xs text-text-secondary hover:border-accent">
            Zurücksetzen
          </button>
          <span className="text-xs text-text-muted">{validLineCount} nutzbare Linie(n) markiert</span>
        </div>

        {error && <p className="text-xs text-danger">{error}</p>}

        {result !== null && (
          <p className="text-xs text-text-primary">
            Gefundener Verzeichnungskoeffizient: <strong>{result.toFixed(4)}</strong>
          </p>
        )}

        <div className="flex justify-end gap-2">
          <button type="button" onClick={close} className="rounded border border-border px-3 py-1.5 text-xs text-text-secondary hover:border-accent">
            Abbrechen
          </button>
          <button
            type="button"
            onClick={() => void handleCalibrate()}
            disabled={busy || validLineCount === 0}
            className="rounded border border-border px-3 py-1.5 text-xs text-text-secondary hover:border-accent disabled:cursor-not-allowed disabled:opacity-40"
          >
            {busy ? "Berechne…" : "Berechnen"}
          </button>
          <button
            type="button"
            onClick={handleApply}
            disabled={result === null}
            className="rounded border border-accent bg-accent/10 px-3 py-1.5 text-xs text-accent disabled:cursor-not-allowed disabled:opacity-40"
          >
            Anwenden
          </button>
        </div>
      </div>
    </div>
  );
}
