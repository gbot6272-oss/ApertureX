import { useCallback, useRef, useState } from "react";

import type { MaskGeometry } from "../lib/edl";

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
}

type DragHandle = "start" | "end" | "center" | "radius";

function clamp01(value: number): number {
  return Math.min(1, Math.max(0, value));
}

const HANDLE_CLASS =
  "absolute h-4 w-4 -translate-x-1/2 -translate-y-1/2 rounded-full border-2 border-white bg-accent shadow focus:outline focus:outline-2 focus:outline-white";

/**
 * Ziehgriffe für Linearen/Radialen Verlauf (Phase 6 Schritt 3) — analog
 * zu `CropOverlay`s Ecken-Ziehgriffen: `onChange` während des Ziehens
 * (Live-Vorschau, kein Commit), `onCommit` beim Loslassen.
 *
 * **Bewusste Vereinfachung:** der Radialverlauf-Ziehgriff steuert nur
 * einen einzelnen, gemeinsamen Radius (`radius_x == radius_y`, kreisförmig)
 * — unabhängige Achsen und Rotation sind im Datenmodell bereits vorhanden
 * (`MaskGeometry::RadialGradient`), bekommen aber erst in einem späteren
 * Schritt eigene Ziehgriffe (z. B. Ellipsen-Achsen-Handles + Rotations-
 * Handle), um diesen Schritt nicht unnötig aufzublähen.
 */
export function MaskOverlay({ imageLeft, imageTop, imageWidth, imageHeight, geometry, onChange, onCommit }: MaskOverlayProps) {
  const [dragHandle, setDragHandle] = useState<DragHandle | null>(null);
  const dragStart = useRef<{ x: number; y: number; geometry: MaskGeometry } | null>(null);

  const startDrag = useCallback(
    (handle: DragHandle, event: React.PointerEvent) => {
      event.stopPropagation();
      event.currentTarget.setPointerCapture(event.pointerId);
      setDragHandle(handle);
      dragStart.current = { x: event.clientX, y: event.clientY, geometry };
    },
    [geometry],
  );

  const handlePointerMove = useCallback(
    (event: React.PointerEvent) => {
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
    if (dragHandle) {
      setDragHandle(null);
      onCommit();
    }
  }, [dragHandle, onCommit]);

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
      className="absolute"
      style={{ left: imageLeft, top: imageTop, width: imageWidth, height: imageHeight }}
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
    </div>
  );
}
