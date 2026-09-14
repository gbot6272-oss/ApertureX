import {
  diagonalMethodLines,
  goldenRatioLines,
  goldenSpiralPath,
  orientLine,
  thirdsLines,
  triangleLines,
  type GridOrientation,
  type Line,
} from "../lib/compositionGrid";
import { useCallback, useRef, useState } from "react";

import type { CropRect, GridOverlay } from "../lib/edl";

interface CropOverlayProps {
  /** Position/Größe des angezeigten Bildes in Bildschirm-Pixeln,
   * relativ zum selben positionierten Vorfahren wie dieses Overlay
   * (`Viewer.tsx` berechnet das bereits für Zoom/Pan, siehe dort). */
  imageLeft: number;
  imageTop: number;
  imageWidth: number;
  imageHeight: number;
  crop: CropRect;
  overlay: GridOverlay;
  /** Drehung/Spiegelung des Rasters (Phase 32). */
  orientation: GridOrientation;
  /** `null` = freie Seitenverhältniswahl. */
  aspectRatio: number | null;
  onChange: (next: CropRect) => void;
  onCommit: () => void;
}

type DragMode = "move" | "nw" | "ne" | "sw" | "se";

const MIN_SIZE = 0.05;

function clamp01(value: number): number {
  return Math.min(1, Math.max(0, value));
}

/** Berechnet das neue Freistellungsrechteck, wenn eine Ecke (`corner`)
 * um `(dx, dy)` verschoben wird — die gegenüberliegende Ecke bleibt
 * fest. Geteilt zwischen Zieh- und Tastatur-Interaktion. */
function resizeFromCorner(
  base: CropRect,
  corner: Exclude<DragMode, "move">,
  dx: number,
  dy: number,
  aspectRatio: number | null,
): CropRect {
  const left = corner === "nw" || corner === "sw" ? base.x + dx : base.x;
  const top = corner === "nw" || corner === "ne" ? base.y + dy : base.y;
  const right = corner === "ne" || corner === "se" ? base.x + base.width + dx : base.x + base.width;
  const bottom = corner === "sw" || corner === "se" ? base.y + base.height + dy : base.y + base.height;

  let newX = clamp01(Math.min(left, right - MIN_SIZE));
  let newY = clamp01(Math.min(top, bottom - MIN_SIZE));
  let newWidth = clamp01(right) - newX;
  let newHeight = clamp01(bottom) - newY;
  if (aspectRatio) {
    // Höhe der Breite nachführen, ausgehend von der festen Ecke.
    newHeight = newWidth / aspectRatio;
    if (corner === "nw" || corner === "ne") {
      newY = base.y + base.height - newHeight;
    }
  }
  newWidth = Math.max(MIN_SIZE, Math.min(newWidth, 1 - newX));
  newHeight = Math.max(MIN_SIZE, Math.min(newHeight, 1 - newY));
  return { x: newX, y: newY, width: newWidth, height: newHeight };
}

/**
 * Freistellen-Werkzeug (Phase 4 Schritt 11) — Ziehgriffe an den vier
 * Ecken plus Verschieben durch Ziehen im Inneren, dazu eine
 * Rasterüberlagerung zur Bildkomposition (rein visuelle Hilfslinien,
 * berühren nie Pixel — siehe `geometry.rs`s Moduldoku).
 *
 * **Bewusste Vereinfachung** (siehe `DECISIONS.md` ADR-0028/ADR-0030):
 * „Spirale" wird als verschachtelte, nach dem Goldenen Schnitt
 * abnehmende Rechteck-Reihe angenähert statt einer echten logarithmischen
 * Spiralkurve; „Diagonalen" zeigt zwei Ecke-zu-Ecke-Linien statt der
 * vollständigen Vier-Linien-Diagonalmethode.
 */
export function CropOverlay({
  imageLeft,
  imageTop,
  imageWidth,
  imageHeight,
  crop,
  overlay,
  orientation,
  aspectRatio,
  onChange,
  onCommit,
}: CropOverlayProps) {
  const [dragMode, setDragMode] = useState<DragMode | null>(null);
  const dragStart = useRef<{ x: number; y: number; crop: CropRect } | null>(null);

  const startDrag = useCallback((mode: DragMode, event: React.PointerEvent) => {
    event.stopPropagation();
    event.currentTarget.setPointerCapture(event.pointerId);
    setDragMode(mode);
    dragStart.current = { x: event.clientX, y: event.clientY, crop };
  }, [crop]);

  const handlePointerMove = useCallback(
    (event: React.PointerEvent) => {
      if (!dragMode || !dragStart.current || imageWidth <= 0 || imageHeight <= 0) return;
      const dx = (event.clientX - dragStart.current.x) / imageWidth;
      const dy = (event.clientY - dragStart.current.y) / imageHeight;
      const base = dragStart.current.crop;

      if (dragMode === "move") {
        onChange({
          ...base,
          x: clamp01(Math.min(base.x + dx, 1 - base.width)),
          y: clamp01(Math.min(base.y + dy, 1 - base.height)),
        });
        return;
      }

      onChange(resizeFromCorner(base, dragMode, dx, dy, aspectRatio));
    },
    [dragMode, imageWidth, imageHeight, aspectRatio, onChange],
  );

  const handlePointerUp = useCallback(() => {
    if (dragMode) {
      setDragMode(null);
      onCommit();
    }
  }, [dragMode, onCommit]);

  const handleKeyDown = useCallback(
    (mode: DragMode) => (event: React.KeyboardEvent) => {
      const step = event.shiftKey ? 0.05 : 0.01;
      let dx = 0;
      let dy = 0;
      if (event.key === "ArrowLeft") dx = -step;
      else if (event.key === "ArrowRight") dx = step;
      else if (event.key === "ArrowUp") dy = -step;
      else if (event.key === "ArrowDown") dy = step;
      else return;
      event.preventDefault();
      // Ohne dies würde ein Tastendruck auf einem Ecken-Ziehgriff zum
      // umschließenden Rechteck-Div hochblubbern (beide sind
      // `role="slider"`-Elemente mit eigenem `onKeyDown`) — dessen
      // "move"-Handler liefe dann mit einem veralteten `crop`-Stand aus
      // demselben Render und würde die gerade vorgenommene
      // Größenänderung sofort wieder überschreiben.
      event.stopPropagation();

      if (mode === "move") {
        onChange({
          ...crop,
          x: clamp01(Math.min(crop.x + dx, 1 - crop.width)),
          y: clamp01(Math.min(crop.y + dy, 1 - crop.height)),
        });
      } else {
        onChange(resizeFromCorner(crop, mode, dx, dy, aspectRatio));
      }
      onCommit();
    },
    [crop, aspectRatio, onChange, onCommit],
  );

  const rectStyle: React.CSSProperties = {
    position: "absolute",
    left: `${crop.x * 100}%`,
    top: `${crop.y * 100}%`,
    width: `${crop.width * 100}%`,
    height: `${crop.height * 100}%`,
  };

  return (
    <div
      className="pointer-events-none absolute"
      style={{ left: imageLeft, top: imageTop, width: imageWidth, height: imageHeight }}
    >
      <div
        role="slider"
        tabIndex={0}
        aria-label="Freistellen-Rechteck"
        aria-valuenow={Math.round(crop.width * 100)}
        onPointerDown={(event) => startDrag("move", event)}
        onPointerMove={handlePointerMove}
        onPointerUp={handlePointerUp}
        onKeyDown={handleKeyDown("move")}
        className="pointer-events-auto cursor-move border-2 border-white/90 outline-none focus:border-accent"
        style={rectStyle}
      >
        <GridOverlayLines overlay={overlay} orientation={orientation} />
        {(["nw", "ne", "sw", "se"] as const).map((corner) => (
          <div
            key={corner}
            role="slider"
            tabIndex={0}
            aria-label={`Freistellen-Ziehgriff ${corner}`}
            aria-valuenow={Math.round(crop.width * 100)}
            onPointerDown={(event) => startDrag(corner, event)}
            onPointerMove={handlePointerMove}
            onPointerUp={handlePointerUp}
            onKeyDown={handleKeyDown(corner)}
            className="pointer-events-auto absolute h-3 w-3 -translate-x-1/2 -translate-y-1/2 border border-black bg-white outline-none focus:bg-accent"
            style={{
              left: corner.includes("w") ? 0 : "100%",
              top: corner.includes("n") ? 0 : "100%",
              cursor: `${corner}-resize`,
            }}
          />
        ))}
      </div>
    </div>
  );
}

function GridOverlayLines({
  overlay,
  orientation,
}: {
  overlay: GridOverlay;
  orientation: GridOrientation;
}) {
  if (overlay === "None") return null;

  // Phase 32: Die Geometrie liegt jetzt in `lib/compositionGrid.ts` und
  // ist dort getestet. Drei Dinge sind dabei echt besser geworden und
  // nicht nur verschoben:
  //
  // - Die Spirale ist eine Kurve statt eines Kästchengerüsts. Man legt
  //   sie an, um zu sehen, ob das Auge dem Schwung zum Motiv folgt —
  //   dafür braucht es die Kurve.
  // - Die Diagonalmethode zieht sechs Linien statt zwei; erst deren
  //   Schnittpunkte sind die Platzierungspunkte, um die es geht.
  // - Alle Raster lassen sich drehen und spiegeln. Ohne das sitzt die
  //   Spirale fest in einer Ecke und ist in der Hälfte der Fälle
  //   unbrauchbar.
  const lines: Line[] =
    overlay === "Thirds"
      ? thirdsLines()
      : overlay === "GoldenRatio"
        ? goldenRatioLines()
        : overlay === "Diagonals"
          ? diagonalMethodLines()
          : overlay === "Triangles"
            ? triangleLines()
            : [];

  // Drittel und Goldener Schnitt sind punktsymmetrisch — sie zu drehen
  // ändert nichts, also bleibt die Lage dort ungenutzt.
  const oriented =
    overlay === "Thirds" || overlay === "GoldenRatio"
      ? lines
      : lines.map((line) => orientLine(line, orientation));

  return (
    <svg
      className="pointer-events-none absolute inset-0 h-full w-full"
      preserveAspectRatio="none"
      viewBox="0 0 100 100"
      data-testid="composition-grid"
    >
      {oriented.map((line, index) => (
        <line
          key={index}
          x1={line.x1}
          y1={line.y1}
          x2={line.x2}
          y2={line.y2}
          stroke="white"
          strokeOpacity={0.7}
          strokeWidth={0.3}
          vectorEffect="non-scaling-stroke"
        />
      ))}
      {overlay === "Spiral" ? (
        <path
          d={goldenSpiralPath(orientation)}
          fill="none"
          stroke="white"
          strokeOpacity={0.85}
          strokeWidth={0.4}
          vectorEffect="non-scaling-stroke"
        />
      ) : null}
    </svg>
  );
}
