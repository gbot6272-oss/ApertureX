import { useEffect, useRef, useState } from "react";

import type { DevelopFrame } from "../hooks/useDevelopRender";
import { computeHistogram, countClipping, type Histogram } from "../lib/histogram";
import { computeVectorscope, type Vectorscope } from "../lib/vectorscope";
import { computeWaveform, type Waveform } from "../lib/waveform";

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
  /** Auto-Ton (Phase 9 Schritt 5) — bekommt das bereits berechnete
   * Histogramm übergeben, damit es hier nicht ein zweites Mal berechnet
   * werden muss. */
  onAutoTone: (histogram: Histogram) => void;
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

/** Vektorskop-Canvas (Phase 14 Schritt 6, siehe `lib/vectorscope.ts`s
 * Moduldoku): zeichnet die Cb/Cr-Dichte-Heatmap per `putImageData` (statt
 * vieler einzelner `fillRect`-Aufrufe wie `HistogramCanvas` — bei
 * `size * size` Rasterzellen deutlich schneller) plus ein Fadenkreuz/
 * Kreis-Raster darüber. */
function VectorscopeCanvas({ vectorscope }: { vectorscope: Vectorscope }) {
  const canvasRef = useRef<HTMLCanvasElement>(null);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;
    const { size, grid, maxCount } = vectorscope;
    const imageData = ctx.createImageData(size, size);
    const max = Math.max(1, maxCount);
    const bg = 26;
    for (let i = 0; i < size * size; i++) {
      const count = grid[i] ?? 0;
      const offset = i * 4;
      // Quadratwurzel statt linearer Skalierung — dieselbe Wahrnehmungs-
      // Korrektur wie bei jeder Dichte-Heatmap: wenige, aber vorhandene
      // Pixel sollen noch sichtbar bleiben, nicht von einem einzelnen
      // dominanten Peak visuell verschluckt werden.
      const intensity = count > 0 ? Math.min(1, Math.sqrt(count / max)) : 0;
      imageData.data[offset] = Math.round(bg + intensity * (140 - bg));
      imageData.data[offset + 1] = Math.round(bg + intensity * (235 - bg));
      imageData.data[offset + 2] = Math.round(bg + intensity * (160 - bg));
      imageData.data[offset + 3] = 255;
    }
    ctx.putImageData(imageData, 0, 0);

    const cx = size / 2;
    const cy = size / 2;
    const radius = size / 2 - 1;
    ctx.strokeStyle = "rgba(255,255,255,0.25)";
    ctx.lineWidth = 1;
    ctx.beginPath();
    ctx.arc(cx, cy, radius, 0, Math.PI * 2);
    ctx.stroke();
    ctx.beginPath();
    ctx.moveTo(cx - radius, cy);
    ctx.lineTo(cx + radius, cy);
    ctx.moveTo(cx, cy - radius);
    ctx.lineTo(cx, cy + radius);
    ctx.stroke();
  }, [vectorscope]);

  return <canvas ref={canvasRef} width={vectorscope.size} height={vectorscope.size} className="w-full rounded" aria-label="Vektorskop" />;
}

/** Wellenform-Canvas (Phase 14 Schritt 6, siehe `lib/waveform.ts`s
 * Moduldoku): dieselbe `putImageData`-Strategie wie `VectorscopeCanvas`.
 * Jeder Kanal bekommt seine eigene Grundfarbe, überlappende Kanäle aus
 * einer Spalte/einem Wertebereich werden per Komponenten-Maximum
 * kombiniert — eine per-Pixel-Näherung an `HistogramCanvas`s
 * `"lighten"`-Compositing (dort über `globalCompositeOperation`, hier von
 * Hand, weil `putImageData` selbst keinen Blend-Modus kennt). */
function WaveformCanvas({ waveform }: { waveform: Waveform }) {
  const canvasRef = useRef<HTMLCanvasElement>(null);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;
    const { columns, rows, r, g, b, maxCount } = waveform;
    const imageData = ctx.createImageData(columns, rows);
    const max = Math.max(1, maxCount);
    const bg = 26;

    for (let col = 0; col < columns; col++) {
      for (let value = 0; value < rows; value++) {
        const idx = col * rows + value;
        const ri = Math.min(1, Math.sqrt((r[idx] ?? 0) / max));
        const gi = Math.min(1, Math.sqrt((g[idx] ?? 0) / max));
        const bi = Math.min(1, Math.sqrt((b[idx] ?? 0) / max));
        // Zeile 0 im Bild = Wert 255 (oben) — übliche Wellenform-
        // Konvention (Lichter oben, Tiefen unten), Bild-Y wächst aber
        // nach unten, deshalb gespiegelt.
        const canvasRow = rows - 1 - value;
        const offset = (canvasRow * columns + col) * 4;
        const rr = bg + ri * (255 - bg);
        const rg = bg + ri * (90 - bg);
        const rb = bg + ri * (90 - bg);
        const gr = bg + gi * (90 - bg);
        const gg = bg + gi * (255 - bg);
        const gb = bg + gi * (90 - bg);
        const br = bg + bi * (90 - bg);
        const bgGreen = bg + bi * (150 - bg);
        const bb = bg + bi * (255 - bg);
        imageData.data[offset] = Math.round(Math.max(rr, gr, br, bg));
        imageData.data[offset + 1] = Math.round(Math.max(rg, gg, bgGreen, bg));
        imageData.data[offset + 2] = Math.round(Math.max(rb, gb, bb, bg));
        imageData.data[offset + 3] = 255;
      }
    }
    ctx.putImageData(imageData, 0, 0);
  }, [waveform]);

  return <canvas ref={canvasRef} width={waveform.columns} height={waveform.rows} className="h-20 w-full rounded" aria-label="Wellenform" />;
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
type AnalysisTab = "histogram" | "vectorscope" | "waveform";

const ANALYSIS_TAB_LABELS: Record<AnalysisTab, string> = {
  histogram: "Histogramm",
  vectorscope: "Vektorskop",
  waveform: "Wellenform",
};

export function DevelopAnalysisPanel({ frame, pointerSample, clippingOverlayEnabled, onToggleClippingOverlay, viewport, thumbnailUrl, onAutoTone }: DevelopAnalysisPanelProps) {
  // Vor dem `if (!frame) return null;` unten, sonst verletzt der Hook die
  // Rules of Hooks (unterschiedliche Hook-Zahl je nach `frame`).
  const [analysisTab, setAnalysisTab] = useState<AnalysisTab>("histogram");

  if (!frame) return null;

  const histogram = computeHistogram(frame.pixels, frame.width, frame.height);
  const clipping = countClipping(frame.pixels, frame.width, frame.height);
  const shadowPercent = (clipping.shadowClipped / clipping.totalPixels) * 100;
  const highlightPercent = (clipping.highlightClipped / clipping.totalPixels) * 100;
  // Vektorskop/Wellenform sind deutlich teurer als das Histogramm (volle
  // Bildschleife je Kanal statt nur ein 256er-Array-Update) — nur die
  // gerade sichtbare Analyse berechnen, nicht alle drei bei jedem Render.
  const vectorscope: Vectorscope | null = analysisTab === "vectorscope" ? computeVectorscope(frame.pixels, frame.width, frame.height) : null;
  const waveform: Waveform | null = analysisTab === "waveform" ? computeWaveform(frame.pixels, frame.width, frame.height) : null;

  // `pointer-events-none` auf dem Container, `pointer-events-auto` nur auf
  // den beiden Schaltflächen — dieses Panel schwebt über dem Viewer und
  // würde sonst (besonders in einem schmalen Viewer-Ausschnitt neben
  // vielen offenen Seitenleisten) Bildklicks für Werkzeuge wie den
  // Reparatur-Pinsel oder die Weißabgleich-Pipette darunter abfangen.
  return (
    <div className="pointer-events-none absolute right-2 top-2 flex w-56 flex-col gap-2 rounded border border-border bg-bg-raised/95 p-2 text-xs shadow-lg">
      <div>
        <div className="mb-1 flex items-center justify-between">
          <div className="flex gap-1">
            {(Object.keys(ANALYSIS_TAB_LABELS) as AnalysisTab[]).map((tab) => (
              <button
                key={tab}
                type="button"
                onClick={() => setAnalysisTab(tab)}
                aria-pressed={analysisTab === tab}
                className={`pointer-events-auto rounded border px-1 text-[10px] ${analysisTab === tab ? "border-accent text-accent" : "border-border text-text-secondary hover:border-accent"}`}
              >
                {ANALYSIS_TAB_LABELS[tab]}
              </button>
            ))}
          </div>
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
        {analysisTab === "histogram" && <HistogramCanvas histogram={histogram} />}
        {analysisTab === "vectorscope" && vectorscope && <VectorscopeCanvas vectorscope={vectorscope} />}
        {analysisTab === "waveform" && waveform && <WaveformCanvas waveform={waveform} />}
        {analysisTab === "histogram" && (
          <button
            type="button"
            onClick={() => onAutoTone(histogram)}
            className="pointer-events-auto mt-1 w-full rounded border border-border px-1 py-0.5 hover:border-accent"
            title="Belichtung/Kontrast aus dem Histogramm ableiten (Perzentil-Heuristik, keine KI)"
          >
            Auto-Ton
          </button>
        )}
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
