import { useCallback, useRef, useState } from "react";

import type { MaskGeometry, MaskPoint } from "../lib/edl";

interface MaskOverlayProps {
  /** Position/Größe des angezeigten Bildes in Bildschirm-Pixeln, wie bei
   * `CropOverlay`/`RepairOverlay` bereits von `Viewer.tsx` berechnet. */
  imageLeft: number;
  imageTop: number;
  imageWidth: number;
  imageHeight: number;
  geometry: MaskGeometry;
  onChange: (next: MaskGeometry) => void;
  onCommit: () => void;
  /** Nur für `geometry.kind === "Brush"` benötigt: ein fertig gemalter
   * Strich (bereits ausgedünnter Pfad) bzw. das Entfernen eines
   * bestehenden Strichs. */
  onPaintBrushStroke?: (points: MaskPoint[]) => void;
  onRemoveBrushStroke?: (strokeIndex: number) => void;
}

/** Muss `masks.rs`s Ausdünnungs-Erwartung entsprechen — derselbe Ansatz
 * wie `RepairOverlay.tsx`s `MAX_TARGET_PATH_POINTS` (dort an
 * `repair.rs`s `MAX_PATH_POINTS` gebunden; Masken-Pinselstriche haben
 * keine entsprechende Rust-seitige Obergrenze, aber derselbe Wert hält
 * das EDL-JSON unabhängig von der Zeigerabtastrate kompakt). */
const MAX_BRUSH_STROKE_POINTS = 32;

function thinBrushPath(path: MaskPoint[]): MaskPoint[] {
  if (path.length <= MAX_BRUSH_STROKE_POINTS) return path;
  const step = (path.length - 1) / (MAX_BRUSH_STROKE_POINTS - 1);
  const thinned: MaskPoint[] = [];
  for (let i = 0; i < MAX_BRUSH_STROKE_POINTS; i += 1) {
    const point = path[Math.round(i * step)];
    if (point) thinned.push(point);
  }
  return thinned;
}

function brushPointFromEvent(event: { clientX: number; clientY: number }, rect: DOMRect): MaskPoint {
  return {
    x: Math.min(1, Math.max(0, (event.clientX - rect.left) / rect.width)),
    y: Math.min(1, Math.max(0, (event.clientY - rect.top) / rect.height)),
  };
}

type DragHandle = "start" | "end" | "center" | "radius";

function clamp01(value: number): number {
  return Math.min(1, Math.max(0, value));
}

const HANDLE_CLASS =
  "absolute h-4 w-4 -translate-x-1/2 -translate-y-1/2 rounded-full border-2 border-white bg-accent shadow focus:outline focus:outline-2 focus:outline-white";

/**
 * Ziehgriffe für Linearen/Radialen Verlauf (Phase 6 Schritt 3) + Malen für
 * Pinselmasken (Schritt 4) — analog zu `CropOverlay`s Ecken-Ziehgriffen
 * bzw. `RepairOverlay`s Pfad-Malen: `onChange`/`onPaintBrushStroke` während
 * bzw. am Ende der Interaktion, `onCommit` beim Loslassen.
 *
 * **Bewusste Vereinfachung:** der Radialverlauf-Ziehgriff steuert nur
 * einen einzelnen, gemeinsamen Radius (`radius_x == radius_y`, kreisförmig)
 * — unabhängige Achsen und Rotation sind im Datenmodell bereits vorhanden
 * (`MaskGeometry::RadialGradient`), bekommen aber erst in einem späteren
 * Schritt eigene Ziehgriffe (z. B. Ellipsen-Achsen-Handles + Rotations-
 * Handle), um diesen Schritt nicht unnötig aufzublähen. Die Pinsel-
 * Live-Vorschau ist wie bei `RepairOverlay` rein clientseitig (dieses
 * SVG), der tatsächliche Pipeline-Effekt erscheint erst nach Loslassen.
 */
export function MaskOverlay({
  imageLeft,
  imageTop,
  imageWidth,
  imageHeight,
  geometry,
  onChange,
  onCommit,
  onPaintBrushStroke,
  onRemoveBrushStroke,
}: MaskOverlayProps) {
  const [dragHandle, setDragHandle] = useState<DragHandle | null>(null);
  const dragStart = useRef<{ x: number; y: number; geometry: MaskGeometry } | null>(null);
  const [drawingBrushPath, setDrawingBrushPath] = useState<MaskPoint[] | null>(null);
  const brushPathRef = useRef<MaskPoint[]>([]);
  const brushPaintingRef = useRef(false);

  const startDrag = useCallback(
    (handle: DragHandle, event: React.PointerEvent) => {
      event.stopPropagation();
      event.currentTarget.setPointerCapture(event.pointerId);
      setDragHandle(handle);
      dragStart.current = { x: event.clientX, y: event.clientY, geometry };
    },
    [geometry],
  );

  const handleBrushPointerDown = useCallback(
    (event: React.PointerEvent<HTMLDivElement>) => {
      if (geometry.kind !== "Brush") return;
      event.currentTarget.setPointerCapture(event.pointerId);
      const rect = event.currentTarget.getBoundingClientRect();
      const point = brushPointFromEvent(event, rect);
      brushPaintingRef.current = true;
      brushPathRef.current = [point];
      setDrawingBrushPath(brushPathRef.current);
    },
    [geometry.kind],
  );

  const handlePointerMove = useCallback(
    (event: React.PointerEvent<HTMLDivElement>) => {
      if (brushPaintingRef.current) {
        const rect = event.currentTarget.getBoundingClientRect();
        brushPathRef.current = [...brushPathRef.current, brushPointFromEvent(event, rect)];
        setDrawingBrushPath(brushPathRef.current);
        return;
      }

      if (!dragHandle || !dragStart.current || imageWidth <= 0 || imageHeight <= 0) return;
      const dx = (event.clientX - dragStart.current.x) / imageWidth;
      const dy = (event.clientY - dragStart.current.y) / imageHeight;
      const base = dragStart.current.geometry;

      if (base.kind === "LinearGradient") {
        if (dragHandle === "start") {
          onChange({ ...base, x1: clamp01(base.x1 + dx), y1: clamp01(base.y1 + dy) });
        } else if (dragHandle === "end") {
          onChange({ ...base, x2: clamp01(base.x2 + dx), y2: clamp01(base.y2 + dy) });
        }
        return;
      }

      if (base.kind === "RadialGradient") {
        if (dragHandle === "center") {
          onChange({ ...base, center_x: clamp01(base.center_x + dx), center_y: clamp01(base.center_y + dy) });
        } else if (dragHandle === "radius") {
          const newRadius = Math.max(0.02, base.radius_x + dx);
          onChange({ ...base, radius_x: newRadius, radius_y: newRadius });
        }
      }
    },
    [dragHandle, imageWidth, imageHeight, onChange],
  );

  const handlePointerUp = useCallback(() => {
    if (brushPaintingRef.current) {
      brushPaintingRef.current = false;
      const path = brushPathRef.current;
      brushPathRef.current = [];
      setDrawingBrushPath(null);
      if (path.length > 0) onPaintBrushStroke?.(thinBrushPath(path));
      return;
    }

    if (dragHandle) {
      setDragHandle(null);
      onCommit();
    }
  }, [dragHandle, onCommit, onPaintBrushStroke]);

  const handleKeyDown = useCallback(
    (handle: DragHandle) => (event: React.KeyboardEvent) => {
      const step = event.shiftKey ? 0.05 : 0.01;
      let dx = 0;
      let dy = 0;
      if (event.key === "ArrowLeft") dx = -step;
      else if (event.key === "ArrowRight") dx = step;
      else if (event.key === "ArrowUp") dy = -step;
      else if (event.key === "ArrowDown") dy = step;
      else return;
      event.preventDefault();
      event.stopPropagation();

      if (geometry.kind === "LinearGradient") {
        if (handle === "start") {
          onChange({ ...geometry, x1: clamp01(geometry.x1 + dx), y1: clamp01(geometry.y1 + dy) });
        } else if (handle === "end") {
          onChange({ ...geometry, x2: clamp01(geometry.x2 + dx), y2: clamp01(geometry.y2 + dy) });
        }
      } else if (geometry.kind === "RadialGradient") {
        if (handle === "center") {
          onChange({ ...geometry, center_x: clamp01(geometry.center_x + dx), center_y: clamp01(geometry.center_y + dy) });
        } else if (handle === "radius") {
          onChange({ ...geometry, radius_x: Math.max(0.02, geometry.radius_x + dx), radius_y: Math.max(0.02, geometry.radius_y + dx) });
        }
      }
      onCommit();
    },
    [geometry, onChange, onCommit],
  );

  return (
    <div
      className={`absolute${geometry.kind === "Brush" ? " cursor-crosshair" : ""}`}
      style={{ left: imageLeft, top: imageTop, width: imageWidth, height: imageHeight }}
      onPointerDown={handleBrushPointerDown}
      onPointerMove={handlePointerMove}
      onPointerUp={handlePointerUp}
    >
      {geometry.kind === "LinearGradient" && (
        <>
          <svg className="pointer-events-none absolute inset-0 h-full w-full overflow-visible">
            <line
              x1={`${geometry.x1 * 100}%`}
              y1={`${geometry.y1 * 100}%`}
              x2={`${geometry.x2 * 100}%`}
              y2={`${geometry.y2 * 100}%`}
              stroke="white"
              strokeWidth={2}
              strokeDasharray="4 4"
            />
          </svg>
          <div
            role="slider"
            tabIndex={0}
            aria-label="Linearer Verlauf: Startpunkt"
            aria-valuenow={Math.round(geometry.x1 * 100)}
            className={HANDLE_CLASS}
            style={{ left: `${geometry.x1 * 100}%`, top: `${geometry.y1 * 100}%` }}
            onPointerDown={(event) => startDrag("start", event)}
            onKeyDown={handleKeyDown("start")}
          />
          <div
            role="slider"
            tabIndex={0}
            aria-label="Linearer Verlauf: Endpunkt"
            aria-valuenow={Math.round(geometry.x2 * 100)}
            className={HANDLE_CLASS}
            style={{ left: `${geometry.x2 * 100}%`, top: `${geometry.y2 * 100}%` }}
            onPointerDown={(event) => startDrag("end", event)}
            onKeyDown={handleKeyDown("end")}
          />
        </>
      )}

      {geometry.kind === "RadialGradient" && (
        <>
          <svg className="pointer-events-none absolute inset-0 h-full w-full overflow-visible">
            <ellipse
              cx={`${geometry.center_x * 100}%`}
              cy={`${geometry.center_y * 100}%`}
              rx={`${geometry.radius_x * 100}%`}
              ry={`${geometry.radius_y * 100}%`}
              fill="none"
              stroke="white"
              strokeWidth={2}
              strokeDasharray="4 4"
            />
          </svg>
          <div
            role="slider"
            tabIndex={0}
            aria-label="Radialer Verlauf: Mittelpunkt"
            aria-valuenow={Math.round(geometry.center_x * 100)}
            className={HANDLE_CLASS}
            style={{ left: `${geometry.center_x * 100}%`, top: `${geometry.center_y * 100}%` }}
            onPointerDown={(event) => startDrag("center", event)}
            onKeyDown={handleKeyDown("center")}
          />
          <div
            role="slider"
            tabIndex={0}
            aria-label="Radialer Verlauf: Radius"
            aria-valuenow={Math.round(geometry.radius_x * 100)}
            className={HANDLE_CLASS}
            style={{ left: `${(geometry.center_x + geometry.radius_x) * 100}%`, top: `${geometry.center_y * 100}%` }}
            onPointerDown={(event) => startDrag("radius", event)}
            onKeyDown={handleKeyDown("radius")}
          />
        </>
      )}

      {geometry.kind === "Brush" && (
        <>
          <svg className="pointer-events-none absolute inset-0 h-full w-full" preserveAspectRatio="none" viewBox="0 0 100 100">
            {geometry.strokes.map((stroke, index) => (
              <polyline
                key={index}
                points={stroke.points.map((p) => `${p.x * 100},${p.y * 100}`).join(" ")}
                fill="none"
                stroke="#4ade80"
                strokeWidth={Math.max(0.4, stroke.radius * 100)}
                strokeLinecap="round"
                strokeLinejoin="round"
                opacity={0.5}
                vectorEffect="non-scaling-stroke"
              />
            ))}
            {drawingBrushPath && drawingBrushPath.length > 0 && (
              // Feste Vorschaubreite statt des Entwurfs-Radius aus dem
              // Store — dieses Overlay kennt nur die Geometrie, nicht die
              // draftRadius/-Feather-Werte des Aufrufers; die Live-Linie
              // dient ohnehin nur der Pfad-Vorschau, nicht der Radiusgröße.
              <polyline
                points={drawingBrushPath.map((p) => `${p.x * 100},${p.y * 100}`).join(" ")}
                fill="none"
                stroke="white"
                strokeWidth={1.5}
                strokeLinecap="round"
                strokeLinejoin="round"
                vectorEffect="non-scaling-stroke"
              />
            )}
          </svg>
          {geometry.strokes.map((stroke, index) => {
            const last = stroke.points[stroke.points.length - 1];
            if (!last) return null;
            return (
              <button
                key={index}
                type="button"
                aria-label={`Pinselstrich ${index + 1} entfernen`}
                onClick={(event) => {
                  event.stopPropagation();
                  onRemoveBrushStroke?.(index);
                }}
                className="pointer-events-auto absolute flex h-4 w-4 -translate-x-1/2 -translate-y-1/2 items-center justify-center rounded-full bg-bg-raised/90 text-[10px] leading-none text-text-primary outline-none focus:ring-1 focus:ring-accent"
                style={{ left: `${last.x * 100}%`, top: `${last.y * 100}%` }}
              >
                ×
              </button>
            );
          })}
        </>
      )}
    </div>
  );
}
