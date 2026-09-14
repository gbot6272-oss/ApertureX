import { useCallback, useRef, useState } from "react";

import {
  normalizedRadiusToScreen,
  normalizedToScreen,
  screenToNormalized,
  type ViewTransform,
} from "../lib/imageToolMath";

/**
 * Das gemeinsame Bild-Overlay der am Bild bedienten Werkzeuge aus
 * Phase 30 (siehe `DECISIONS.md` ADR-0060).
 *
 * **Eine Komponente für alle sieben**, nicht sieben eigene: die
 * Umrechnung Bild ↔ Bildschirm (Zoom, Pan, Ausrichtung) steht hier
 * genau einmal. Sieben Fassungen wären sieben Gelegenheiten, dieselbe
 * Rechnung leicht unterschiedlich falsch zu machen — und beim Zoomen
 * fällt so etwas sofort auf.
 *
 * Die Komponente kennt die Werkzeuge nicht. Sie zeichnet drei Formen —
 * Griff, Linie, Ellipse — und meldet Bewegungen in normierten
 * Bildkoordinaten zurück. Was ein Griff bedeutet, entscheidet der
 * Aufrufer über seine `id` (`"light:2"`, `"spotlight"`, `"horizon1"`).
 */

export interface OverlayHandle {
  id: string;
  /** Normierte Bildkoordinaten. */
  x: number;
  y: number;
  label: string;
  /** Ringradius als Anteil der kürzeren Bildkante (optional). */
  radius?: number;
  /** CSS-Farbe des Griffs — zeigt bei Lichtern deren Lichtfarbe. */
  color?: string;
  selected?: boolean;
}

export interface OverlayLine {
  id: string;
  x1: number;
  y1: number;
  x2: number;
  y2: number;
  /** Gestrichelt für Hilfslinien (Horizont), durchgezogen für Achsen. */
  dashed?: boolean;
}

export interface OverlayEllipse {
  id: string;
  cx: number;
  cy: number;
  rx: number;
  ry: number;
  angleDeg: number;
}

export interface ImageToolOverlayProps {
  view: ViewTransform;
  handles: readonly OverlayHandle[];
  lines?: readonly OverlayLine[];
  ellipses?: readonly OverlayEllipse[];
  /** Wird beim Ziehen fortlaufend gerufen (normierte Koordinaten). */
  onMove: (id: string, x: number, y: number) => void;
  /** Wird einmal am Ende eines Ziehvorgangs gerufen — der Aufrufer
   * schreibt dort in den Verlauf, statt bei jedem Mauspixel. */
  onMoveEnd?: (id: string) => void;
  /** Klick auf eine freie Bildstelle (z. B. „neues Licht hier"). */
  onAddAt?: (x: number, y: number) => void;
  onSelect?: (id: string) => void;
  /** Halbsatz, der oben im Bild steht, solange das Werkzeug aktiv ist. */
  hint?: string;
}

export function ImageToolOverlay({
  view,
  handles,
  lines = [],
  ellipses = [],
  onMove,
  onMoveEnd,
  onAddAt,
  onSelect,
  hint,
}: ImageToolOverlayProps) {
  const [dragging, setDragging] = useState<string | null>(null);
  const surfaceRef = useRef<HTMLDivElement | null>(null);

  const pointerToNormalized = useCallback(
    (event: React.PointerEvent) => {
      const rect = surfaceRef.current?.getBoundingClientRect();
      if (!rect) return { x: 0, y: 0 };
      // `rect` ist bereits das BILD-Rechteck (das Overlay liegt genau
      // darauf), `screenToNormalized` erwartet dagegen
      // CONTAINER-Koordinaten und zieht `origin` selbst ab. Der Ursprung
      // muss deshalb hier wieder dazu — ihn zweimal abzuziehen hat beim
      // ersten Anlauf jeden Klick in der oberen Bildhälfte auf `y = 0`
      // geklemmt (bei seitlich anliegendem Bild fiel es in x gar nicht
      // auf, weil `origin.x` dort 0 ist).
      return screenToNormalized(
        event.clientX - rect.left + view.origin.x,
        event.clientY - rect.top + view.origin.y,
        view,
      );
    },
    [view],
  );

  const handlePointerDown = useCallback(
    (event: React.PointerEvent, id: string) => {
      event.stopPropagation();
      event.preventDefault();
      // Zeiger einfangen: sonst verliert das Ziehen den Griff, sobald
      // der Zeiger den kleinen Kreis verlässt — also praktisch sofort.
      (event.target as Element).setPointerCapture?.(event.pointerId);
      setDragging(id);
      onSelect?.(id);
    },
    [onSelect],
  );

  const handlePointerMove = useCallback(
    (event: React.PointerEvent) => {
      if (!dragging) return;
      const p = pointerToNormalized(event);
      onMove(dragging, p.x, p.y);
    },
    [dragging, onMove, pointerToNormalized],
  );

  const handlePointerUp = useCallback(() => {
    if (!dragging) return;
    onMoveEnd?.(dragging);
    setDragging(null);
  }, [dragging, onMoveEnd]);

  const handleSurfaceClick = useCallback(
    (event: React.PointerEvent) => {
      if (dragging || !onAddAt) return;
      const p = pointerToNormalized(event);
      onAddAt(p.x, p.y);
    },
    [dragging, onAddAt, pointerToNormalized],
  );

  const width = view.imgW * view.scale;
  const height = view.imgH * view.scale;

  return (
    <div
      ref={surfaceRef}
      data-testid="image-tool-overlay"
      // `pointer-events-auto` NUR wenn das Werkzeug etwas anfangen kann:
      // sonst würde das Overlay Zoom und Verschieben des Viewers
      // blockieren, obwohl gerade gar nichts zu bedienen ist.
      className={onAddAt ? "absolute cursor-crosshair" : "absolute"}
      style={{ left: view.origin.x, top: view.origin.y, width, height }}
      onPointerDown={handleSurfaceClick}
      onPointerMove={handlePointerMove}
      onPointerUp={handlePointerUp}
      onPointerCancel={handlePointerUp}
    >
      <svg
        className="pointer-events-none absolute inset-0"
        width={width}
        height={height}
        aria-hidden
      >
        {ellipses.map((ellipse) => {
          const center = normalizedToScreen(ellipse.cx, ellipse.cy, view);
          return (
            <ellipse
              key={ellipse.id}
              cx={center.x - view.origin.x}
              cy={center.y - view.origin.y}
              rx={normalizedRadiusToScreen(ellipse.rx, view)}
              ry={normalizedRadiusToScreen(ellipse.ry, view)}
              transform={`rotate(${ellipse.angleDeg} ${center.x - view.origin.x} ${center.y - view.origin.y})`}
              fill="none"
              stroke="var(--color-accent)"
              strokeWidth={1.5}
              strokeDasharray="6 4"
              opacity={0.9}
            />
          );
        })}
        {lines.map((line) => {
          const a = normalizedToScreen(line.x1, line.y1, view);
          const b = normalizedToScreen(line.x2, line.y2, view);
          return (
            <line
              key={line.id}
              x1={a.x - view.origin.x}
              y1={a.y - view.origin.y}
              x2={b.x - view.origin.x}
              y2={b.y - view.origin.y}
              stroke="var(--color-accent)"
              strokeWidth={1.5}
              strokeDasharray={line.dashed ? "6 4" : undefined}
              opacity={0.9}
            />
          );
        })}
        {handles
          .filter((handle) => handle.radius !== undefined)
          .map((handle) => {
            const center = normalizedToScreen(handle.x, handle.y, view);
            return (
              <circle
                key={`${handle.id}-ring`}
                cx={center.x - view.origin.x}
                cy={center.y - view.origin.y}
                r={normalizedRadiusToScreen(handle.radius ?? 0, view)}
                fill="none"
                stroke={handle.color ?? "var(--color-accent)"}
                strokeWidth={1}
                strokeDasharray="4 4"
                opacity={handle.selected ? 0.9 : 0.45}
              />
            );
          })}
      </svg>

      {handles.map((handle) => {
        const center = normalizedToScreen(handle.x, handle.y, view);
        return (
          <button
            key={handle.id}
            type="button"
            aria-label={handle.label}
            data-handle={handle.id}
            onPointerDown={(event) => handlePointerDown(event, handle.id)}
            className={`absolute h-4 w-4 -translate-x-1/2 -translate-y-1/2 rounded-full border-2 shadow-[0_0_0_1px_rgb(0_0_0/45%)] transition-transform duration-[var(--duration-fast)] hover:scale-125 ${
              handle.selected ? "scale-125 border-white" : "border-white/70"
            }`}
            style={{
              left: center.x - view.origin.x,
              top: center.y - view.origin.y,
              backgroundColor: handle.color ?? "var(--color-accent)",
              cursor: "grab",
            }}
          />
        );
      })}

      {hint && (
        <p className="apx-glass pointer-events-none absolute left-1/2 top-3 -translate-x-1/2 rounded px-2 py-1 text-xs whitespace-nowrap text-text-primary">
          {hint}
        </p>
      )}
    </div>
  );
}
