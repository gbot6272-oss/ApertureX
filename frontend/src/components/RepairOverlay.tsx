import { useCallback, useRef, useState } from "react";

import type { RepairPoint, RepairStroke } from "../lib/edl";

interface RepairOverlayProps {
  /** Position/Größe des angezeigten Bildes in Bildschirm-Pixeln, wie
   * `CropOverlay`s gleichnamige Props (siehe dort). */
  imageLeft: number;
  imageTop: number;
  imageWidth: number;
  imageHeight: number;
  strokes: RepairStroke[];
  /** `null` = als Nächstes wird per Klick der Quellpunkt gesetzt. */
  pendingSource: RepairPoint | null;
  onSetSource: (point: RepairPoint) => void;
  onPaint: (path: RepairPoint[]) => void;
  onRemoveStroke: (index: number) => void;
}

/** Muss `repair.rs`s `MAX_PATH_POINTS` entsprechen. */
const MAX_TARGET_PATH_POINTS = 32;

/** Dünnt einen dicht abgetasteten Zeigerpfad gleichmäßig auf höchstens
 * [`MAX_TARGET_PATH_POINTS`] Stützpunkte aus — das Frontend übernimmt
 * damit die in `repair.rs`s Moduldoku vorausgesetzte Ausdünnung. */
function thinPath(path: RepairPoint[]): RepairPoint[] {
  if (path.length <= MAX_TARGET_PATH_POINTS) return path;
  const step = (path.length - 1) / (MAX_TARGET_PATH_POINTS - 1);
  const thinned: RepairPoint[] = [];
  for (let i = 0; i < MAX_TARGET_PATH_POINTS; i += 1) {
    const point = path[Math.round(i * step)];
    if (point) thinned.push(point);
  }
  return thinned;
}

function pointFromEvent(event: { clientX: number; clientY: number }, rect: DOMRect): RepairPoint {
  return {
    x: Math.min(1, Math.max(0, (event.clientX - rect.left) / rect.width)),
    y: Math.min(1, Math.max(0, (event.clientY - rect.top) / rect.height)),
  };
}

/**
 * Reparatur-Pinsel-Overlay (Phase 4 Schritt 12) — ein erster Klick setzt
 * den Quellpunkt (Klonen: Kopierquelle; Reparieren: Referenzbereich für
 * das Tiefpass/Hochpass-Überblenden, siehe `repair.rs`s Moduldoku), ein
 * anschließender Ziehvorgang malt den Zielpfad.
 *
 * **Bewusste Vereinfachung:** die Live-Vorschau des gerade gemalten
 * Strichs ist rein clientseitig (dieses SVG-Overlay, unabhängig vom
 * Entwickeln-Rendering) — der tatsächliche Pipeline-Effekt erscheint erst
 * nach dem Loslassen (`onPaint`, bereits ausgedünnt). Ein Durchlauf pro
 * Zeigerbewegung über einen wachsenden, noch nicht ausgedünnten Pfad wäre
 * unnötig teuer und für die Vorschau nicht nötig.
 */
export function RepairOverlay({
  imageLeft,
  imageTop,
  imageWidth,
  imageHeight,
  strokes,
  pendingSource,
  onSetSource,
  onPaint,
  onRemoveStroke,
}: RepairOverlayProps) {
  const [drawingPath, setDrawingPath] = useState<RepairPoint[] | null>(null);
  const pathRef = useRef<RepairPoint[]>([]);
  const paintingRef = useRef(false);

  const handlePointerDown = useCallback(
    (event: React.PointerEvent<HTMLDivElement>) => {
      const rect = event.currentTarget.getBoundingClientRect();
      const point = pointFromEvent(event, rect);
      if (!pendingSource) {
        onSetSource(point);
        return;
      }
      event.currentTarget.setPointerCapture(event.pointerId);
      paintingRef.current = true;
      pathRef.current = [point];
      setDrawingPath(pathRef.current);
    },
    [pendingSource, onSetSource],
  );

  const handlePointerMove = useCallback((event: React.PointerEvent<HTMLDivElement>) => {
    if (!paintingRef.current) return;
    const rect = event.currentTarget.getBoundingClientRect();
    pathRef.current = [...pathRef.current, pointFromEvent(event, rect)];
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
      onPointerUp={handlePointerUp}
    >
      <svg className="pointer-events-none absolute inset-0 h-full w-full" preserveAspectRatio="none" viewBox="0 0 100 100">
        {strokes.map((stroke, index) => {
          const firstTarget = stroke.target_path[0] ?? stroke.source;
          return (
            <g key={index}>
              <line
                x1={stroke.source.x * 100}
                y1={stroke.source.y * 100}
                x2={firstTarget.x * 100}
                y2={firstTarget.y * 100}
                stroke="#38bdf8"
                strokeDasharray="1.5,1.5"
                strokeWidth={0.3}
                vectorEffect="non-scaling-stroke"
              />
              <circle cx={stroke.source.x * 100} cy={stroke.source.y * 100} r={1} fill="none" stroke="#38bdf8" strokeWidth={0.3} vectorEffect="non-scaling-stroke" />
              <polyline
                points={stroke.target_path.map((p) => `${p.x * 100},${p.y * 100}`).join(" ")}
                fill="none"
                stroke={stroke.mode === "Heal" ? "#facc15" : "#4ade80"}
                strokeWidth={0.6}
                vectorEffect="non-scaling-stroke"
              />
            </g>
          );
        })}
        {drawingPath && drawingPath.length > 0 && (
          <polyline points={drawingPath.map((p) => `${p.x * 100},${p.y * 100}`).join(" ")} fill="none" stroke="white" strokeWidth={0.6} vectorEffect="non-scaling-stroke" />
        )}
        {pendingSource && <circle cx={pendingSource.x * 100} cy={pendingSource.y * 100} r={1.2} fill="none" stroke="white" strokeWidth={0.4} vectorEffect="non-scaling-stroke" />}
      </svg>
      {strokes.map((stroke, index) => {
        const last = stroke.target_path[stroke.target_path.length - 1] ?? stroke.source;
        return (
          <button
            key={index}
            type="button"
            aria-label={`Reparatur-Strich ${index + 1} entfernen`}
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
