import { useCallback, useRef, useState } from "react";

import { radialGradientAxisHandlePositions, radialGradientBoundaryPoints, type MaskGeometry, type MaskPoint } from "../lib/edl";

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

type DragHandle = "start" | "end" | "center" | "radiusX" | "radiusY" | "rotation";

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
 * **Radialverlauf-Ellipse + Rotation (Phase 12 Schritt 2, siehe
 * `DECISIONS.md` ADR-0039):** `radius_x`/`radius_y`/`angle_degrees` waren
 * im Datenmodell und in der Pipeline (`masks.rs`s `radial_gradient_alpha`)
 * schon länger unabhängig voneinander — nur diese Komponente hielt sie
 * bislang künstlich gleich. Jetzt drei eigene Griffe: `radiusX`/`radiusY`
 * entlang der (ggf. rotierten) Ellipsen-Achsen, `rotation` etwas weiter
 * außen auf der X-Achse. Alle drei rechnen im selben Bild-Bruchteilsraum
 * wie die Pipeline selbst (siehe `radialGradientBoundaryPoints`s
 * Moduldoku in `lib/edl.ts` zur Rotationskonvention).
 *
 * Die Pinsel-Live-Vorschau ist wie bei `RepairOverlay` rein clientseitig
 * (dieses SVG), der tatsächliche Pipeline-Effekt erscheint erst nach
 * Loslassen.
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
      const base = dragStart.current.geometry;

      // Achsen-/Rotations-Griffe (Phase 12 Schritt 2): rechnen mit der
      // *absoluten* Bruchteilsposition des Zeigers relativ zum Mittelpunkt
      // statt einer Delta-Akkumulation — für einen Rotations-Griff ist
      // "zeig auf die gewünschte Ausrichtung" die einzig sinnvolle
      // Interaktion, für die beiden Radius-Griffe konsistent mitgenutzt.
      if (base.kind === "RadialGradient" && (dragHandle === "radiusX" || dragHandle === "radiusY" || dragHandle === "rotation")) {
        const rect = event.currentTarget.getBoundingClientRect();
        const fx = clamp01((event.clientX - rect.left) / rect.width);
        const fy = clamp01((event.clientY - rect.top) / rect.height);
        const dxAbs = fx - base.center_x;
        const dyAbs = fy - base.center_y;

        if (dragHandle === "rotation") {
          onChange({ ...base, angle_degrees: (Math.atan2(dyAbs, dxAbs) * 180) / Math.PI });
          return;
        }

        const angleRad = (base.angle_degrees * Math.PI) / 180;
        const cosA = Math.cos(angleRad);
        const sinA = Math.sin(angleRad);
        if (dragHandle === "radiusX") {
          // Projektion des Zeiger-Vektors auf die lokale (rotierte) X-Achse.
          onChange({ ...base, radius_x: Math.max(0.02, dxAbs * cosA + dyAbs * sinA) });
        } else {
          // Projektion auf die lokale Y-Achse (senkrecht zur X-Achse).
          onChange({ ...base, radius_y: Math.max(0.02, -dxAbs * sinA + dyAbs * cosA) });
        }
        return;
      }

      const dx = (event.clientX - dragStart.current.x) / imageWidth;
      const dy = (event.clientY - dragStart.current.y) / imageHeight;

      if (base.kind === "LinearGradient") {
        if (dragHandle === "start") {
          onChange({ ...base, x1: clamp01(base.x1 + dx), y1: clamp01(base.y1 + dy) });
        } else if (dragHandle === "end") {
          onChange({ ...base, x2: clamp01(base.x2 + dx), y2: clamp01(base.y2 + dy) });
        }
        return;
      }

      if (base.kind === "RadialGradient" && dragHandle === "center") {
        onChange({ ...base, center_x: clamp01(base.center_x + dx), center_y: clamp01(base.center_y + dy) });
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
        } else if (handle === "radiusX") {
          onChange({ ...geometry, radius_x: Math.max(0.02, geometry.radius_x + dx) });
        } else if (handle === "radiusY") {
          // ArrowUp (dy < 0) vergrößert, ArrowDown verkleinert.
          onChange({ ...geometry, radius_y: Math.max(0.02, geometry.radius_y - dy) });
        } else if (handle === "rotation") {
          onChange({ ...geometry, angle_degrees: geometry.angle_degrees + dx * 180 });
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

      {geometry.kind === "RadialGradient" &&
        (() => {
          const handles = radialGradientAxisHandlePositions(geometry);
          const boundary = radialGradientBoundaryPoints(geometry);
          return (
            <>
              <svg className="pointer-events-none absolute inset-0 h-full w-full overflow-visible" preserveAspectRatio="none" viewBox="0 0 100 100">
                <polygon
                  points={boundary.map((p) => `${p.x * 100},${p.y * 100}`).join(" ")}
                  fill="none"
                  stroke="white"
                  strokeWidth={0.3}
                  strokeDasharray="1.2 1.2"
                  vectorEffect="non-scaling-stroke"
                />
                <line
                  x1={`${geometry.center_x * 100}`}
                  y1={`${geometry.center_y * 100}`}
                  x2={`${handles.rotation.x * 100}`}
                  y2={`${handles.rotation.y * 100}`}
                  stroke="white"
                  strokeWidth={0.2}
                  strokeDasharray="0.6 0.6"
                  vectorEffect="non-scaling-stroke"
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
                aria-label="Radialer Verlauf: Radius X-Achse"
                aria-valuenow={Math.round(geometry.radius_x * 100)}
                className={HANDLE_CLASS}
                style={{ left: `${handles.radiusX.x * 100}%`, top: `${handles.radiusX.y * 100}%` }}
                onPointerDown={(event) => startDrag("radiusX", event)}
                onKeyDown={handleKeyDown("radiusX")}
              />
              <div
                role="slider"
                tabIndex={0}
                aria-label="Radialer Verlauf: Radius Y-Achse"
                aria-valuenow={Math.round(geometry.radius_y * 100)}
                className={HANDLE_CLASS}
                style={{ left: `${handles.radiusY.x * 100}%`, top: `${handles.radiusY.y * 100}%` }}
                onPointerDown={(event) => startDrag("radiusY", event)}
                onKeyDown={handleKeyDown("radiusY")}
              />
              <div
                role="slider"
                tabIndex={0}
                aria-label="Radialer Verlauf: Rotation"
                aria-valuenow={Math.round(geometry.angle_degrees)}
                className={HANDLE_CLASS}
                style={{ left: `${handles.rotation.x * 100}%`, top: `${handles.rotation.y * 100}%` }}
                onPointerDown={(event) => startDrag("rotation", event)}
                onKeyDown={handleKeyDown("rotation")}
              />
            </>
          );
        })()}

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
