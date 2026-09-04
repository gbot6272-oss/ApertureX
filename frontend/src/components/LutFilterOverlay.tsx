import { useCallback, useRef, useState } from "react";

import type { LutFilterPoint, LutFilterStroke } from "../lib/edl";

interface LutFilterOverlayProps {
  /** Position/Größe des angezeigten Bildes in Bildschirm-Pixeln, wie
   * `LiquifyOverlay`s gleichnamige Props (siehe dort). */
  imageLeft: number;
  imageTop: number;
  imageWidth: number;
  imageHeight: number;
  strokes: LutFilterStroke[];
  /** Radius des *nächsten* Strichs (normiert, Bruchteil der Bildbreite) —
   * nur für den Wirkradius-Vorschaukreis am Zeiger. */
  radius: number;
  onPaint: (path: LutFilterPoint[]) => void;
  onRemoveStroke: (index: number) => void;
}

/** Dieselbe Ausdünnung wie `LiquifyOverlay`s `MAX_CENTER_PATH_POINTS`. */
const MAX_CENTER_PATH_POINTS = 32;

function thinPath(path: LutFilterPoint[]): LutFilterPoint[] {
  if (path.length <= MAX_CENTER_PATH_POINTS) return path;
  const step = (path.length - 1) / (MAX_CENTER_PATH_POINTS - 1);
  const thinned: LutFilterPoint[] = [];
  for (let i = 0; i < MAX_CENTER_PATH_POINTS; i += 1) {
    const point = path[Math.round(i * step)];
    if (point) thinned.push(point);
  }
  return thinned;
}

function pointFromEvent(event: { clientX: number; clientY: number }, rect: DOMRect): LutFilterPoint {
  return {
    x: Math.min(1, Math.max(0, (event.clientX - rect.left) / rect.width)),
    y: Math.min(1, Math.max(0, (event.clientY - rect.top) / rect.height)),
  };
}

/**
 * Filter-Pinsel-Overlay (Phase 16 Schritt 3, siehe `DECISIONS.md`
 * ADR-0043) — punktuelle statt globaler Filter-Anwendung. Identisches
 * Interaktionsmuster wie `LiquifyOverlay`: ein Ziehvorgang malt direkt
 * den Wirkbereich, beim Loslassen sofort als fertiger Strich committet.
 */
export function LutFilterOverlay({ imageLeft, imageTop, imageWidth, imageHeight, strokes, radius, onPaint, onRemoveStroke }: LutFilterOverlayProps) {
  const [drawingPath, setDrawingPath] = useState<LutFilterPoint[] | null>(null);
  const [hoverPoint, setHoverPoint] = useState<LutFilterPoint | null>(null);
  const pathRef = useRef<LutFilterPoint[]>([]);
  const paintingRef = useRef(false);

  const handlePointerDown = useCallback((event: React.PointerEvent<HTMLDivElement>) => {
    const rect = event.currentTarget.getBoundingClientRect();
    const point = pointFromEvent(event, rect);
    event.currentTarget.setPointerCapture(event.pointerId);
    paintingRef.current = true;
    pathRef.current = [point];
    setDrawingPath(pathRef.current);
  }, []);

  const handlePointerMove = useCallback((event: React.PointerEvent<HTMLDivElement>) => {
    const rect = event.currentTarget.getBoundingClientRect();
    const point = pointFromEvent(event, rect);
    setHoverPoint(point);
    if (!paintingRef.current) return;
    pathRef.current = [...pathRef.current, point];
    setDrawingPath(pathRef.current);
  }, []);

  const handlePointerUp = useCallback(() => {
    if (!paintingRef.current) return;
    paintingRef.current = false;
    const path = pathRef.current;
    pathRef.current = [];
    setDrawingPath(null);
    if (path.length > 0) onPaint(thinPath(path));
  }, [onPaint]);

  return (
    <div
      className="pointer-events-auto absolute cursor-crosshair"
      style={{ left: imageLeft, top: imageTop, width: imageWidth, height: imageHeight }}
      onPointerDown={handlePointerDown}
      onPointerMove={handlePointerMove}
      onPointerLeave={() => setHoverPoint(null)}
      onPointerUp={handlePointerUp}
    >
      <svg className="pointer-events-none absolute inset-0 h-full w-full" preserveAspectRatio="none" viewBox="0 0 100 100">
        {strokes.map((stroke, index) => (
          <polyline
            key={index}
            points={stroke.center_path.map((p) => `${p.x * 100},${p.y * 100}`).join(" ")}
            fill="none"
            stroke="#38bdf8"
            strokeWidth={0.6}
            vectorEffect="non-scaling-stroke"
          />
        ))}
        {drawingPath && drawingPath.length > 0 && (
          <polyline points={drawingPath.map((p) => `${p.x * 100},${p.y * 100}`).join(" ")} fill="none" stroke="white" strokeWidth={0.6} vectorEffect="non-scaling-stroke" />
        )}
        {hoverPoint && !drawingPath && (
          <circle cx={hoverPoint.x * 100} cy={hoverPoint.y * 100} r={radius * 100} fill="none" stroke="white" strokeDasharray="1,1" strokeWidth={0.3} vectorEffect="non-scaling-stroke" />
        )}
      </svg>
      {strokes.map((stroke, index) => {
        const last = stroke.center_path[stroke.center_path.length - 1];
        if (!last) return null;
        return (
          <button
            key={index}
            type="button"
            aria-label={`Filter-Strich ${index + 1} entfernen`}
            onClick={(event) => {
              event.stopPropagation();
              onRemoveStroke(index);
            }}
            className="pointer-events-auto absolute flex h-4 w-4 -translate-x-1/2 -translate-y-1/2 items-center justify-center rounded-full bg-bg-raised/90 text-[10px] leading-none text-text-primary outline-none focus:ring-1 focus:ring-accent"
            style={{ left: `${last.x * 100}%`, top: `${last.y * 100}%` }}
          >
            ×
          </button>
        );
      })}
    </div>
  );
}
