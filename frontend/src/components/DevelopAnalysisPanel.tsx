import { useEffect, useMemo, useRef, useState } from "react";

import { extractPalette } from "../lib/colorPalette";
import { PEAKING_COLORS, type PeakingColor } from "../lib/focusPeaking";

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

/** Bedienung des Fokus-Peakings (Phase 31 Schritt 4). Der Zustand liegt
 * im Viewer — dort entsteht auch die Überlagerung; dieses Panel ist nur
 * die Bedienfläche dafür, wie schon beim Clipping-Overlay. */
/** Die aus dem Foto gezogene Farbpalette (Phase 31 Schritt 5). */
export interface PaletteControls {
  /** Ein Klick auf ein Feld setzt den Weissabgleich auf diese Farbe —
   * dieselbe Wirkung wie die Pipette, nur ohne im Bild zielen zu
   * müssen. */
  onPick: (r: number, g: number, b: number) => void;
}

export interface PeakingControls {
  enabled: boolean;
  threshold: number;
  color: PeakingColor;
  /** Anteil markierter Pixel (0…1) — ohne diese Rückmeldung wäre der
   * Schwellwert Blindflug. */
  coverage: number;
  onToggle: () => void;
  onThresholdChange: (value: number) => void;
  onColorChange: (value: PeakingColor) => void;
}

interface DevelopAnalysisPanelProps {
  peaking?: PeakingControls;
  palette?: PaletteControls;
  /** Angedockt (eigene Spalte neben dem Foto) statt schwebend darüber.
   *
   * Phase 31 Schritt 1: schwebend war die Vorgabe und damit der
   * Normalfall — das Panel lag beim Öffnen des Entwickeln-Moduls IMMER
   * über dem Foto. Verschieben und Einklappen gab es zwar seit Phase 18,
   * aber beides musste man erst tun. Angedockt nimmt das Panel eine
   * eigene Spalte ein, der Viewer misst sich an der Restbreite, und das
   * Foto ist nie verdeckt. Schwebend bleibt für den Fall erhalten, dass
   * jemand die volle Breite fürs Foto will und die Analyse kurz
   * darüberlegt. */
  docked?: boolean;
  onToggleDocked?: () => void;
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

/** Backing-Store-Breite an die tatsächliche CSS-Breite × `devicePixelRatio`
 * koppeln (Phase 18 Schritt 5, siehe `DECISIONS.md` ADR-0046) — dasselbe
 * Muster wie `lib/leafletHeatmap.ts`s DPR-Skalierung. Ohne das bleibt die
 * Canvas-Bitmap auf ihrer anfänglichen Breite eingefroren, während
 * `className="w-full"` sie per CSS beliebig hochskaliert — auf HiDPI-
 * Displays (oder einer per `PaletteFrame` breitgezogenen Palette)
 * sichtbar unscharf. Nur die Breite ist hier dynamisch — die Höhe jedes
 * Analyse-Canvas ist bewusst ein fester CSS-Pixelwert (unverändert
 * gegenüber vorher), ein `ResizeObserver` hält die Breite auch bei
 * Paletten-Größenänderungen aktuell.
 */
function useCanvasDprWidth(canvasRef: React.RefObject<HTMLCanvasElement | null>): { cssWidth: number; dpr: number } {
  const [state, setState] = useState<{ cssWidth: number; dpr: number }>(() => ({ cssWidth: 0, dpr: Math.max(1, window.devicePixelRatio || 1) }));

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const update = () => setState({ cssWidth: canvas.getBoundingClientRect().width, dpr: Math.max(1, window.devicePixelRatio || 1) });
    update();
    const observer = new ResizeObserver(update);
    observer.observe(canvas);
    return () => observer.disconnect();
  }, [canvasRef]);

  return state;
}

function HistogramCanvas({ histogram }: { histogram: Histogram }) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const { cssWidth, dpr } = useCanvasDprWidth(canvasRef);
  const cssHeight = 80;

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas || cssWidth === 0) return;
    canvas.width = Math.round(cssWidth * dpr);
    canvas.height = Math.round(cssHeight * dpr);
    const ctx = canvas.getContext("2d");
    if (!ctx) return;
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    const width = cssWidth;
    const height = cssHeight;
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
  }, [histogram, cssWidth, dpr]);

  return <canvas ref={canvasRef} className="w-full rounded" style={{ height: cssHeight }} aria-label="Histogramm" />;
}

/** Vektorskop-Canvas (Phase 14 Schritt 6, siehe `lib/vectorscope.ts`s
 * Moduldoku): zeichnet die Cb/Cr-Dichte-Heatmap per `putImageData` (statt
 * vieler einzelner `fillRect`-Aufrufe wie `HistogramCanvas` — bei
 * `size * size` Rasterzellen deutlich schneller) plus ein Fadenkreuz/
 * Kreis-Raster darüber. */
function VectorscopeCanvas({ vectorscope }: { vectorscope: Vectorscope }) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const { cssWidth, dpr } = useCanvasDprWidth(canvasRef);
  const cssHeight = vectorscope.size;

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas || cssWidth === 0) return;
    const { size, grid, maxCount } = vectorscope;

    // Die Dichte-Heatmap bleibt in ihrer nativen Rasterauflösung
    // berechnet (unverändert) — `putImageData` respektiert keine
    // Transform-Matrix, deshalb erst auf eine Offscreen-Canvas in
    // Rasterauflösung geschrieben und von dort per `drawImage` auf die
    // DPR-große Ziel-Canvas skaliert (Phase 18 Schritt 5).
    const offscreen = document.createElement("canvas");
    offscreen.width = size;
    offscreen.height = size;
    const offCtx = offscreen.getContext("2d");
    if (!offCtx) return;
    const imageData = offCtx.createImageData(size, size);
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
    offCtx.putImageData(imageData, 0, 0);

    canvas.width = Math.round(cssWidth * dpr);
    canvas.height = Math.round(cssHeight * dpr);
    const ctx = canvas.getContext("2d");
    if (!ctx) return;
    // Das Raster bleibt bewusst blockig (Dichte-Heatmap), nicht
    // zusätzlich weichgezeichnet — nur die Skalierung selbst soll auf
    // HiDPI nicht zusätzlich unscharf wirken.
    ctx.imageSmoothingEnabled = false;
    ctx.clearRect(0, 0, canvas.width, canvas.height);
    ctx.drawImage(offscreen, 0, 0, canvas.width, canvas.height);

    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    const cx = cssWidth / 2;
    const cy = cssHeight / 2;
    const radius = Math.min(cssWidth, cssHeight) / 2 - 1;
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
  }, [vectorscope, cssWidth, cssHeight, dpr]);

  return <canvas ref={canvasRef} className="w-full rounded" style={{ height: cssHeight }} aria-label="Vektorskop" />;
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
  const { cssWidth, dpr } = useCanvasDprWidth(canvasRef);
  const cssHeight = 80;

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas || cssWidth === 0) return;
    const { columns, rows, r, g, b, maxCount } = waveform;

    // Dieselbe Offscreen-Canvas-Strategie wie `VectorscopeCanvas` (Phase
    // 18 Schritt 5): die Dichteberechnung bleibt in ihrer nativen
    // Rasterauflösung unverändert, nur die Skalierung auf die tatsächliche
    // CSS-Anzeigegröße × `devicePixelRatio` ist neu.
    const offscreen = document.createElement("canvas");
    offscreen.width = columns;
    offscreen.height = rows;
    const offCtx = offscreen.getContext("2d");
    if (!offCtx) return;
    const imageData = offCtx.createImageData(columns, rows);
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
    offCtx.putImageData(imageData, 0, 0);

    canvas.width = Math.round(cssWidth * dpr);
    canvas.height = Math.round(cssHeight * dpr);
    const ctx = canvas.getContext("2d");
    if (!ctx) return;
    ctx.imageSmoothingEnabled = false;
    ctx.clearRect(0, 0, canvas.width, canvas.height);
    ctx.drawImage(offscreen, 0, 0, canvas.width, canvas.height);
  }, [waveform, cssWidth, cssHeight, dpr]);

  return <canvas ref={canvasRef} className="w-full rounded" style={{ height: cssHeight }} aria-label="Wellenform" />;
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

export function DevelopAnalysisPanel({ frame, pointerSample, clippingOverlayEnabled, onToggleClippingOverlay, viewport, thumbnailUrl, onAutoTone, docked = false, onToggleDocked, peaking, palette }: DevelopAnalysisPanelProps) {
  // Vor dem `if (!frame) return null;` unten, sonst verletzt der Hook die
  // Rules of Hooks (unterschiedliche Hook-Zahl je nach `frame`).
  const [analysisTab, setAnalysisTab] = useState<AnalysisTab>("histogram");

  // Verschiebbar + einklappbar (Phase 18-Nachtrag, siehe `DECISIONS.md`):
  // bislang fest `absolute right-2 top-2`, ohne jede Möglichkeit, das
  // Panel aus dem Weg zu räumen, wenn es gerade den darunterliegenden
  // Bildausschnitt verdeckt — genau der gemeldete "lässt sich weder
  // verschieben noch minimieren"-Befund. `offset` verschiebt das Panel
  // per `transform` relativ zu seiner Standardposition (obere rechte
  // Ecke) statt `left`/`top` neu zu berechnen — bewusst nicht
  // `localStorage`-persistiert wie `PaletteFrame`s Breite: dieses Panel
  // ist eine leichte, foto-lokale Analyse-Überlagerung, kein dauerhaftes
  // Layout-Element.
  const [offset, setOffset] = useState({ x: 0, y: 0 });
  const [collapsed, setCollapsed] = useState(false);
  const dragState = useRef<{ startClientX: number; startClientY: number; startOffsetX: number; startOffsetY: number } | null>(null);

  useEffect(() => {
    function handleMouseMove(event: MouseEvent) {
      const drag = dragState.current;
      if (!drag) return;
      setOffset({
        x: drag.startOffsetX + (event.clientX - drag.startClientX),
        y: drag.startOffsetY + (event.clientY - drag.startClientY),
      });
    }
    function handleMouseUp() {
      dragState.current = null;
    }
    window.addEventListener("mousemove", handleMouseMove);
    window.addEventListener("mouseup", handleMouseUp);
    return () => {
      window.removeEventListener("mousemove", handleMouseMove);
      window.removeEventListener("mouseup", handleMouseUp);
    };
  }, []);

  const handleDragHandleMouseDown = (event: React.MouseEvent) => {
    // Nur die linke Maustaste startet das Ziehen, und nicht, wenn der
    // Klick eigentlich einem der Knöpfe in der Kopfzeile galt (die
    // stoppen selbst per `stopPropagation`, siehe unten) — hier zusätzlich
    // defensiv gegen künftige Erweiterungen der Kopfzeile.
    if (event.button !== 0) return;
    dragState.current = { startClientX: event.clientX, startClientY: event.clientY, startOffsetX: offset.x, startOffsetY: offset.y };
  };

  if (!frame) return null;

  const histogram = computeHistogram(frame.pixels, frame.width, frame.height);
  const clipping = countClipping(frame.pixels, frame.width, frame.height);
  const shadowPercent = (clipping.shadowClipped / clipping.totalPixels) * 100;
  const highlightPercent = (clipping.highlightClipped / clipping.totalPixels) * 100;
  // Vektorskop/Wellenform sind deutlich teurer als das Histogramm (volle
  // Bildschleife je Kanal statt nur ein 256er-Array-Update) — nur die
  // gerade sichtbare Analyse berechnen, nicht alle drei bei jedem Render.
  const vectorscope: Vectorscope | null = analysisTab === "vectorscope" ? computeVectorscope(frame.pixels, frame.width, frame.height) : null;
  // Die Palette hängt nur am Bild, nicht an der Reiterwahl — `useMemo`
  // verhindert, dass k-Means bei jedem Zeigerzucken neu läuft (der
  // Punktfarbmesser löst sehr häufige Neurenderings aus).
  const swatches = useMemo(
    () => (palette && frame ? extractPalette(frame.pixels, frame.width, frame.height) : []),
    [palette, frame],
  );

  const waveform: Waveform | null = analysisTab === "waveform" ? computeWaveform(frame.pixels, frame.width, frame.height) : null;

  // `pointer-events-none` auf dem Container, `pointer-events-auto` nur auf
  // einzelnen Schaltflächen/der Kopfzeile — dieses Panel schwebt über dem
  // Viewer und würde sonst (besonders in einem schmalen Viewer-Ausschnitt
  // neben vielen offenen Seitenleisten) Bildklicks für Werkzeuge wie den
  // Reparatur-Pinsel oder die Weißabgleich-Pipette darunter abfangen.
  //
  // `top-12` statt `top-2`: die neue, über die volle Breite ziehbare
  // Kopfzeile (siehe unten) liegt sonst genau über dem ebenfalls
  // `absolute right-3 top-3` positionierten TAT-Werkzeugleisten-Block
  // weiter oben in dieser Datei — vorher fing dort nur ein kleiner
  // Knopf Klicks ab, jetzt eine volle Zeile, was TAT-Klicks verschluckt
  // hätte. Etwas Abstand nach unten statt Koordination mit der
  // TAT-Leiste, da dieses Panel ohnehin frei verschiebbar ist.
  return (
    <div
      className={
        docked
          ? // Angedockt: normales Layout-Element. Kein `pointer-events-none`
            // nötig — hier liegt nichts mehr über dem Bild, das Klicks für
            // Pipette oder Reparatur-Pinsel abfangen könnte.
            "flex h-full w-full flex-col gap-2 overflow-y-auto border-l border-border bg-bg-raised p-2 text-xs"
          : "pointer-events-none absolute right-2 top-12 flex w-60 flex-col gap-2 rounded border border-border bg-bg-raised/95 p-2 text-xs shadow-lg"
      }
      style={docked ? undefined : { transform: `translate(${offset.x}px, ${offset.y}px)` }}
      data-testid="develop-analysis-panel"
    >
      {/* Kopfzeile ist der Ziehgriff fürs Verschieben (die ganze Zeile,
          nicht nur ein kleines Symbol — großzügigere Trefferfläche) plus
          Einklapp-Knopf. `onMouseDown` auf der Zeile, nicht auf einzelnen
          Knöpfen — die stoppen die Propagation selbst, sonst würde jeder
          Klick auf Registerkarte/Knopf zusätzlich das Ziehen starten. */}
      <div
        className={
          docked
            ? "-m-2 mb-0 flex items-center justify-between border-b border-border bg-bg-panel px-2 py-1"
            : "pointer-events-auto -m-2 mb-0 flex cursor-grab items-center justify-between rounded-t border-b border-border bg-bg-panel px-2 py-1 active:cursor-grabbing"
        }
        onMouseDown={docked ? undefined : handleDragHandleMouseDown}
      >
        <span className="pointer-events-none select-none font-semibold text-text-secondary">Analyse</span>
        {onToggleDocked ? (
          <button
            type="button"
            onClick={(event) => {
              event.stopPropagation();
              onToggleDocked();
            }}
            className="pointer-events-auto ml-auto mr-1 rounded border border-border px-1 text-text-secondary hover:border-accent"
            title={docked ? "Analyse über das Foto legen" : "Analyse neben das Foto andocken"}
          >
            {docked ? "Lösen" : "Andocken"}
          </button>
        ) : null}
        <button
          type="button"
          onClick={(event) => {
            event.stopPropagation();
            setCollapsed((value) => !value);
          }}
          onMouseDown={(event) => event.stopPropagation()}
          aria-expanded={!collapsed}
          aria-label={collapsed ? "Analyse-Panel ausklappen" : "Analyse-Panel einklappen"}
          title={collapsed ? "Ausklappen" : "Einklappen"}
          className="pointer-events-auto rounded border border-border px-1 text-text-secondary hover:border-accent"
        >
          {collapsed ? "▸" : "▾"}
        </button>
      </div>

      {!collapsed && (
        <>
          <div>
            <div className="mb-1 flex flex-wrap items-center justify-between gap-1">
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
        </>
      )}

      {/* Fokus-Peaking (Phase 31 Schritt 4) — markiert farbig, welche
          Kanten wirklich scharf sind. Steht hier bei den übrigen
          Beurteilungswerkzeugen, nicht im Entwickeln-Panel: es verändert
          das Foto nicht. */}
      {peaking ? (
        <div className="flex flex-col gap-1 border-t border-border pt-2">
          <label className="pointer-events-auto flex items-center gap-2">
            <input
              type="checkbox"
              checked={peaking.enabled}
              onChange={peaking.onToggle}
              className="accent-[var(--color-accent)]"
            />
            <span className="font-semibold text-text-secondary">Fokus-Peaking</span>
            {peaking.enabled ? (
              <span className="ml-auto tabular-nums text-text-muted">
                {(peaking.coverage * 100).toFixed(1)} %
              </span>
            ) : null}
          </label>
          {peaking.enabled ? (
            <div className="pointer-events-auto flex flex-col gap-1">
              <label className="flex items-center gap-2 text-text-secondary">
                Schwelle
                <input
                  type="range"
                  aria-label="Peaking-Schwelle"
                  min={0.02}
                  max={0.6}
                  step={0.01}
                  value={peaking.threshold}
                  onChange={(event) => peaking.onThresholdChange(Number(event.target.value))}
                  className="apx-range min-w-0 flex-1"
                />
              </label>
              <div className="flex items-center gap-2 text-text-secondary">
                Farbe
                {(["red", "green", "blue"] as const).map((name) => (
                  <button
                    key={name}
                    type="button"
                    aria-label={`Peaking-Farbe ${name}`}
                    aria-pressed={peaking.color === name}
                    onClick={() => peaking.onColorChange(name)}
                    className={`size-4 rounded-full border ${
                      peaking.color === name ? "border-text-primary" : "border-border"
                    }`}
                    style={{ backgroundColor: `rgb(${PEAKING_COLORS[name].join(" ")})` }}
                  />
                ))}
              </div>
            </div>
          ) : null}
        </div>
      ) : null}

      {/* Farbpalette aus dem Foto (Phase 31 Schritt 5) — ein Klick setzt
          den Weissabgleich auf diese Farbe, ohne dass man im Bild
          zielen muss. Besonders nützlich für eine Fläche, die zu klein
          zum Treffen ist. */}
      {palette && swatches.length > 0 ? (
        <div className="flex flex-col gap-1 border-t border-border pt-2">
          <span className="font-semibold text-text-secondary">Farben im Bild</span>
          <div className="pointer-events-auto flex gap-1" data-testid="photo-palette">
            {swatches.map((swatch) => (
              <button
                key={swatch.hex}
                type="button"
                onClick={() => palette.onPick(swatch.r, swatch.g, swatch.b)}
                title={`${swatch.hex} · ${(swatch.share * 100).toFixed(0)} % des Bildes — als Weissabgleich übernehmen`}
                aria-label={`Farbe ${swatch.hex} als Weissabgleich übernehmen`}
                className="h-6 min-w-0 flex-1 rounded border border-border transition-transform duration-[var(--duration-fast)] hover:scale-110 hover:border-accent"
                style={{ backgroundColor: swatch.hex }}
              />
            ))}
          </div>
        </div>
      ) : null}
    </div>
  );
}
