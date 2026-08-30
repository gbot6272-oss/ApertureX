import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import { useDevelopPreviewThumbnail } from "../hooks/useDevelopRender";
import { useElementSize } from "../hooks/useElementSize";
import { buildEdlEnvelopeJson } from "../lib/edl";
import * as api from "../lib/tauri";
import { clampZoom, computeBaseScale, imageOrigin, nextZoomStep, panForZoomAtCursor } from "../lib/viewerMath";
import { QuadRenderer } from "../lib/webgl";
import { edlFromHistoryPosition, useAppStore } from "../store";

interface ReferencePaneProps {
  photoId: string | null;
  edlJson: string | null;
  maxEdge: number | undefined;
  label: string;
}

/**
 * Eine einzelne Bildhälfte der Referenzansicht — eigener `QuadRenderer`
 * (`lib/webgl.ts`, dieselbe Textur-Hochlade-/Zeichenlogik wie
 * `Viewer.tsx`) mit einem rein **lokalen** Zoom/Pan-Zustand statt des
 * globalen `zoom`/`panX`/`panY` aus dem Store: `SPEC.md` §3.4/§7
 * verlangt ausdrücklich "unabhängigen Zoom/Pan" für Referenz- und
 * Arbeitsbild, die beiden Hälften dürfen sich also beim Zoomen/
 * Verschieben nicht gegenseitig beeinflussen — mit dem globalen
 * Store-Zustand wäre das nicht möglich (der gilt für genau *ein* Bild).
 * Wiederverwendet dieselben reinen Geometrie-Helfer wie `Viewer.tsx`
 * (`imageOrigin`/`computeBaseScale`/`panForZoomAtCursor`/`nextZoomStep`
 * aus `lib/viewerMath.ts`), aber bewusst ohne dessen Overlay-/
 * Werkzeug-Maschinerie (Masken-/Crop-/Reparatur-Overlays, Pipetten) —
 * eine Referenzansicht braucht nichts davon, nur Betrachten.
 */
function ReferencePane({ photoId, edlJson, maxEdge, label }: ReferencePaneProps) {
  const frame = useDevelopPreviewThumbnail(photoId, edlJson, maxEdge);
  const containerRef = useRef<HTMLDivElement>(null);
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const rendererRef = useRef<QuadRenderer | null>(null);
  const containerSize = useElementSize(containerRef);
  const dpr = window.devicePixelRatio || 1;

  const [zoom, setZoomState] = useState(1);
  const [fitMode, setFitMode] = useState<"fit" | "manual">("fit");
  const [pan, setPan] = useState({ x: 0, y: 0 });
  const dragState = useRef<{ startX: number; startY: number; startPanX: number; startPanY: number } | null>(null);

  const imgW = frame?.width ?? 0;
  const imgH = frame?.height ?? 0;
  const fitScale = useMemo(
    () => computeBaseScale("fit", containerSize.width, containerSize.height, imgW, imgH),
    [containerSize.width, containerSize.height, imgW, imgH],
  );
  const effectiveScale = fitMode === "manual" ? zoom : fitScale;

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    try {
      rendererRef.current = new QuadRenderer(canvas);
    } catch (err) {
      console.error("WebGL2-Renderer (Referenzansicht) konnte nicht erstellt werden:", err);
      rendererRef.current = null;
    }
    return () => {
      rendererRef.current?.dispose();
      rendererRef.current = null;
    };
  }, []);

  useEffect(() => {
    const renderer = rendererRef.current;
    if (!renderer) return;
    const cssWidth = containerSize.width;
    const cssHeight = containerSize.height;
    if (!frame || imgW <= 0 || imgH <= 0) {
      renderer.draw(cssWidth, cssHeight, dpr, { x: 0, y: 0 }, 0, 0);
      return;
    }
    renderer.uploadRgba8(frame.width, frame.height, frame.pixels);
    const origin = imageOrigin(cssWidth, cssHeight, imgW, imgH, effectiveScale, pan);
    renderer.setSmoothing(effectiveScale <= 1);
    renderer.draw(cssWidth, cssHeight, dpr, origin, imgW * effectiveScale, imgH * effectiveScale);
  }, [frame, containerSize.width, containerSize.height, dpr, effectiveScale, imgW, imgH, pan]);

  const handleWheel = useCallback(
    (event: React.WheelEvent<HTMLDivElement>) => {
      if (imgW <= 0 || imgH <= 0) return;
      event.preventDefault();
      const rect = event.currentTarget.getBoundingClientRect();
      const cursor = { x: event.clientX - rect.left, y: event.clientY - rect.top };
      const direction: 1 | -1 = event.deltaY < 0 ? 1 : -1;
      const newZoom = clampZoom(nextZoomStep(effectiveScale, direction, fitScale));
      const newPan = panForZoomAtCursor(cursor, containerSize.width, containerSize.height, imgW, imgH, effectiveScale, newZoom, pan);
      setZoomState(newZoom);
      setFitMode("manual");
      setPan(newPan);
    },
    [effectiveScale, fitScale, imgW, imgH, containerSize.width, containerSize.height, pan],
  );

  const handleMouseDown = useCallback(
    (event: React.MouseEvent<HTMLDivElement>) => {
      dragState.current = { startX: event.clientX, startY: event.clientY, startPanX: pan.x, startPanY: pan.y };
    },
    [pan],
  );

  const handleMouseMove = useCallback((event: React.MouseEvent<HTMLDivElement>) => {
    if (!dragState.current) return;
    const { startX, startY, startPanX, startPanY } = dragState.current;
    setPan({ x: startPanX + (event.clientX - startX), y: startPanY + (event.clientY - startY) });
  }, []);

  const endDrag = useCallback(() => {
    dragState.current = null;
  }, []);

  return (
    <div
      ref={containerRef}
      className="relative flex flex-1 items-center justify-center overflow-hidden bg-bg-base"
      onWheel={handleWheel}
      onMouseDown={handleMouseDown}
      onMouseMove={handleMouseMove}
      onMouseUp={endDrag}
      onMouseLeave={endDrag}
      style={{ cursor: dragState.current ? "grabbing" : "grab" }}
    >
      <canvas ref={canvasRef} className="pointer-events-none absolute inset-0" />
      <span className="pointer-events-none absolute left-2 top-2 rounded bg-bg-raised/80 px-1.5 py-0.5 text-xs text-text-secondary">{label}</span>
      {!photoId && <p className="pointer-events-none text-sm text-text-muted">Kein Referenzfoto gewählt.</p>}
    </div>
  );
}

interface ReferenceViewProps {
  /** Das aktuell im Entwickeln-Modul bearbeitete Foto (rechte Hälfte). */
  workingPhotoId: string | null;
  /** Dessen aktuelles EDL als bereits gebautes Envelope-JSON — derselbe
   * Wert, den auch der normale Live-Viewer rendert. */
  workingEdlJson: string | null;
  maxEdge: number | undefined;
}

/**
 * Referenzansicht (Phase 6 Schritt 10, `SPEC.md` §3.4/§7: "Referenzbild
 * links, Arbeitsbild rechts") — ersetzt den normalen Viewer-Inhalt
 * vollständig, solange `referenceViewActive` gesetzt ist (siehe
 * `Viewer.tsx`), analog zu `BeforeAfterView`s Einhängung.
 *
 * Anders als Vorher/Nachher vergleicht diese Ansicht **zwei
 * verschiedene Fotos**: `referencePhotoId` (frei wählbar über
 * `DevelopPanel.tsx`) wird links **statisch** mit seinem zuletzt
 * committeten Stand gezeigt (über `current_develop_edit` geladen, wie
 * `applyPreviousSettings` es für "Vorherige übernehmen" bereits tut),
 * das Arbeitsbild rechts live mit dem gerade bearbeiteten `developEdl`.
 */
export function ReferenceView({ workingPhotoId, workingEdlJson, maxEdge }: ReferenceViewProps) {
  const referencePhotoId = useAppStore((s) => s.referencePhotoId);
  const [referenceEdlJson, setReferenceEdlJson] = useState<string | null>(null);

  useEffect(() => {
    if (!referencePhotoId) {
      setReferenceEdlJson(null);
      return;
    }
    let cancelled = false;
    void (async () => {
      try {
        const position = await api.currentDevelopEdit(referencePhotoId);
        if (cancelled) return;
        setReferenceEdlJson(buildEdlEnvelopeJson(edlFromHistoryPosition(position)));
      } catch (err) {
        console.error("Referenzfoto-Stand konnte nicht geladen werden:", err);
        if (!cancelled) setReferenceEdlJson(null);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [referencePhotoId]);

  return (
    <div className="absolute inset-0 flex flex-row gap-px bg-border" aria-label="Referenzansicht">
      <ReferencePane photoId={referencePhotoId} edlJson={referencePhotoId ? referenceEdlJson : null} maxEdge={maxEdge} label="Referenz" />
      <ReferencePane photoId={workingPhotoId} edlJson={workingEdlJson} maxEdge={maxEdge} label="Arbeitsbild" />
    </div>
  );
}
