import { useEffect, useRef } from "react";

import type { DevelopFrame } from "../hooks/useDevelopRender";
import { computeHistogram, countClipping, type Histogram } from "../lib/histogram";

interface Viewport {
  /** Bildkoordinaten des sichtbaren Ausschnitts, 0..1 normiert. */
  x: number;
  y: number;
  width: number;
  height: number;
}

interface DevelopAnalysisPanelProps {
  frame: DevelopFrame | null;
  pointerSample: { r: number; g: number; b: number } | null;
  clippingOverlayEnabled: boolean;
  onToggleClippingOverlay: () => void;
  viewport: Viewport | null;
  thumbnailUrl: string | null;
}

function HistogramCanvas({ histogram }: { histogram: Histogram }) {
  const canvasRef = useRef<HTMLCanvasElement>(null);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;
    const { width, height } = canvas;
    ctx.clearRect(0, 0, width, height);
    ctx.fillStyle = "#1a1a1a";
    ctx.fillRect(0, 0, width, height);

    const channels: Array<{ data: number[]; color: string }> = [
      { data: histogram.r, color: "rgba(255,80,80,0.65)" },
      { data: histogram.g, color: "rgba(80,255,80,0.65)" },
      { data: histogram.b, color: "rgba(80,140,255,0.65)" },
    ];
    const max = Math.max(1, histogram.maxCount);
    const barWidth = width / 256;

    ctx.globalCompositeOperation = "lighten";
    for (const channel of channels) {
      ctx.fillStyle = channel.color;
      ctx.beginPath();
      for (let i = 0; i < 256; i++) {
        const barHeight = ((channel.data[i] ?? 0) / max) * height;
        ctx.rect(i * barWidth, height - barHeight, barWidth, barHeight);
      }
      ctx.fill();
    }
    ctx.globalCompositeOperation = "source-over";
  }, [histogram]);

  return <canvas ref={canvasRef} width={256} height={80} className="w-full rounded" aria-label="Histogramm" />;
}

/**
 * Entwickeln-Analysewerkzeuge (Phase 9 Schritt 4, siehe `PLAN.md`/
 * `DECISIONS.md` ADR-0035): Live-Histogramm, Clipping-Warnungen,
 * Punktfarbmesser, Navigator-Miniaturansicht — reine Anzeige/Analyse über
 * den bereits vorhandenen `render_rgba8`-Ausgabepuffer (`Viewer.tsx`s
 * `developFrame`), kein neuer Rendering-Pfad. Nur sichtbar, solange das
 * Entwickeln-Panel offen ist (`Viewer.tsx` reicht `frame`/`pointerSample`
 * nur dann durch).
 */
export function DevelopAnalysisPanel({ frame, pointerSample, clippingOverlayEnabled, onToggleClippingOverlay, viewport, thumbnailUrl }: DevelopAnalysisPanelProps) {
  if (!frame) return null;

  const histogram = computeHistogram(frame.pixels, frame.width, frame.height);
  const clipping = countClipping(frame.pixels, frame.width, frame.height);
  const shadowPercent = (clipping.shadowClipped / clipping.totalPixels) * 100;
  const highlightPercent = (clipping.highlightClipped / clipping.totalPixels) * 100;

  // `pointer-events-none` auf dem Container, `pointer-events-auto` nur auf
  // den beiden Schaltflächen — dieses Panel schwebt über dem Viewer und
  // würde sonst (besonders in einem schmalen Viewer-Ausschnitt neben
  // vielen offenen Seitenleisten) Bildklicks für Werkzeuge wie den
  // Reparatur-Pinsel oder die Weißabgleich-Pipette darunter abfangen.
  return (
    <div className="pointer-events-none absolute right-2 top-2 flex w-56 flex-col gap-2 rounded border border-border bg-bg-raised/95 p-2 text-xs shadow-lg">
      <div>
        <div className="mb-1 flex items-center justify-between">
          <span className="font-semibold text-text-secondary">Histogramm</span>
          <div className="flex gap-1">
            <button
              type="button"
              onClick={onToggleClippingOverlay}
              aria-pressed={clippingOverlayEnabled}
              title={`Tiefen geclippt: ${shadowPercent.toFixed(1)}%`}
              className={`pointer-events-auto rounded border px-1 ${shadowPercent > 0 ? "border-blue-400 text-blue-400" : "border-border text-text-muted"} ${clippingOverlayEnabled ? "bg-blue-400/20" : ""}`}
            >
              ▲
            </button>
            <button
              type="button"
              onClick={onToggleClippingOverlay}
              aria-pressed={clippingOverlayEnabled}
              title={`Lichter geclippt: ${highlightPercent.toFixed(1)}%`}
              className={`pointer-events-auto rounded border px-1 ${highlightPercent > 0 ? "border-danger text-danger" : "border-border text-text-muted"} ${clippingOverlayEnabled ? "bg-danger/20" : ""}`}
            >
              ▲
            </button>
          </div>
        </div>
        <HistogramCanvas histogram={histogram} />
      </div>

      {thumbnailUrl && viewport && (
        <div>
          <span className="mb-1 block font-semibold text-text-secondary">Navigator</span>
          <div className="relative overflow-hidden rounded border border-border">
            <img src={thumbnailUrl} alt="Navigator" className="block w-full" />
            <div
              className="absolute border-2 border-accent"
              style={{
                left: `${viewport.x * 100}%`,
                top: `${viewport.y * 100}%`,
                width: `${viewport.width * 100}%`,
                height: `${viewport.height * 100}%`,
              }}
            />
          </div>
        </div>
      )}

      <div>
        <span className="font-semibold text-text-secondary">Punktfarbmesser</span>{" "}
        {pointerSample ? (
          <span>
            R {pointerSample.r} · G {pointerSample.g} · B {pointerSample.b}
          </span>
        ) : (
          <span className="text-text-muted">Bild überfahren…</span>
        )}
      </div>
    </div>
  );
}
