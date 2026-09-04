import { useCallback, useRef, useState } from "react";

interface Rect {
  x: number;
  y: number;
  width: number;
  height: number;
}

interface ContentAwareMoveOverlayProps {
  /** Position/Größe des angezeigten Bildes in Bildschirm-Pixeln,
   * relativ zum selben positionierten Vorfahren — dieselbe Konvention
   * wie `CropOverlay`. */
  imageLeft: number;
  imageTop: number;
  imageWidth: number;
  imageHeight: number;
  rect: Rect | null;
  loading: boolean;
  onRectDrawn: (rect: Rect) => void;
  onMoveCommitted: (destCenterX: number, destCenterY: number) => void;
}

const MIN_SIZE = 0.02;

function clamp01(value: number): number {
  return Math.min(1, Math.max(0, value));
}

/**
 * Photoshop-Funktion: Content-Aware Move (Phase 15 Schritt 1, siehe
 * `DECISIONS.md` ADR-0042) — zwei Zieh-Phasen in einem Overlay: erst
 * ein Rechteck aufziehen (kein bestehendes EDL-Feld, rein lokaler
 * Zwischenzustand `rect === null`), danach dasselbe Rechteck an eine
 * neue Stelle ziehen. Beim Loslassen der zweiten Ziehbewegung feuert
 * `onMoveCommitted` mit der normierten Zielmitte — der Store baut
 * daraus den `content_aware_move`-Aufruf.
 */
export function ContentAwareMoveOverlay({
  imageLeft,
  imageTop,
  imageWidth,
  imageHeight,
  rect,
  loading,
  onRectDrawn,
  onMoveCommitted,
}: ContentAwareMoveOverlayProps) {
  const [drawStart, setDrawStart] = useState<{ x: number; y: number } | null>(null);
  const [drawCurrent, setDrawCurrent] = useState<{ x: number; y: number } | null>(null);
  const [dragDelta, setDragDelta] = useState<{ dx: number; dy: number } | null>(null);
  const dragOrigin = useRef<{ x: number; y: number } | null>(null);

  const toNormalized = useCallback(
    (event: React.PointerEvent) => {
      const bounds = event.currentTarget.getBoundingClientRect();
      return {
        x: clamp01((event.clientX - bounds.left) / imageWidth),
        y: clamp01((event.clientY - bounds.top) / imageHeight),
      };
    },
    [imageWidth, imageHeight],
  );

  const handleDrawPointerDown = useCallback(
    (event: React.PointerEvent) => {
      if (loading) return;
      event.currentTarget.setPointerCapture(event.pointerId);
      const point = toNormalized(event);
      setDrawStart(point);
      setDrawCurrent(point);
    },
    [loading, toNormalized],
  );

  const handleDrawPointerMove = useCallback(
    (event: React.PointerEvent) => {
      if (!drawStart) return;
      setDrawCurrent(toNormalized(event));
    },
    [drawStart, toNormalized],
  );

  const handleDrawPointerUp = useCallback(() => {
    if (!drawStart || !drawCurrent) return;
    const x0 = Math.min(drawStart.x, drawCurrent.x);
    const y0 = Math.min(drawStart.y, drawCurrent.y);
    const width = Math.abs(drawCurrent.x - drawStart.x);
    const height = Math.abs(drawCurrent.y - drawStart.y);
    setDrawStart(null);
    setDrawCurrent(null);
    if (width >= MIN_SIZE && height >= MIN_SIZE) {
      onRectDrawn({ x: x0, y: y0, width, height });
    }
  }, [drawStart, drawCurrent, onRectDrawn]);

  const handleMovePointerDown = useCallback(
    (event: React.PointerEvent) => {
      if (loading) return;
      event.stopPropagation();
      event.currentTarget.setPointerCapture(event.pointerId);
      dragOrigin.current = { x: event.clientX, y: event.clientY };
      setDragDelta({ dx: 0, dy: 0 });
    },
    [loading],
  );

  const handleMovePointerMove = useCallback(
    (event: React.PointerEvent) => {
      if (!dragOrigin.current) return;
      setDragDelta({
        dx: (event.clientX - dragOrigin.current.x) / imageWidth,
        dy: (event.clientY - dragOrigin.current.y) / imageHeight,
      });
    },
    [imageWidth, imageHeight],
  );

  const handleMovePointerUp = useCallback(() => {
    if (!rect || !dragOrigin.current || !dragDelta) return;
    dragOrigin.current = null;
    const centerX = clamp01(rect.x + rect.width / 2 + dragDelta.dx);
    const centerY = clamp01(rect.y + rect.height / 2 + dragDelta.dy);
    setDragDelta(null);
    onMoveCommitted(centerX, centerY);
  }, [rect, dragDelta, onMoveCommitted]);

  const drawRect =
    drawStart && drawCurrent
      ? {
          x: Math.min(drawStart.x, drawCurrent.x),
          y: Math.min(drawStart.y, drawCurrent.y),
          width: Math.abs(drawCurrent.x - drawStart.x),
          height: Math.abs(drawCurrent.y - drawStart.y),
        }
      : null;

  return (
    <div className="absolute" style={{ left: imageLeft, top: imageTop, width: imageWidth, height: imageHeight }}>
      {!rect && (
        <div
          className="absolute inset-0 cursor-crosshair"
          onPointerDown={handleDrawPointerDown}
          onPointerMove={handleDrawPointerMove}
          onPointerUp={handleDrawPointerUp}
        >
          {drawRect && (
            <div
              className="pointer-events-none absolute border-2 border-dashed border-white/90"
              style={{
                left: `${drawRect.x * 100}%`,
                top: `${drawRect.y * 100}%`,
                width: `${drawRect.width * 100}%`,
                height: `${drawRect.height * 100}%`,
              }}
            />
          )}
        </div>
      )}

      {rect && (
        <>
          <div
            className="pointer-events-none absolute border-2 border-white/60"
            style={{
              left: `${rect.x * 100}%`,
              top: `${rect.y * 100}%`,
              width: `${rect.width * 100}%`,
              height: `${rect.height * 100}%`,
            }}
          />
          <div
            role="button"
            tabIndex={0}
            aria-label="Verschobenes Objekt"
            onPointerDown={handleMovePointerDown}
            onPointerMove={handleMovePointerMove}
            onPointerUp={handleMovePointerUp}
            className={`absolute cursor-move border-2 border-accent outline-none ${loading ? "opacity-50" : ""}`}
            style={{
              left: `${(rect.x + (dragDelta?.dx ?? 0)) * 100}%`,
              top: `${(rect.y + (dragDelta?.dy ?? 0)) * 100}%`,
              width: `${rect.width * 100}%`,
              height: `${rect.height * 100}%`,
            }}
          />
        </>
      )}
    </div>
  );
}
