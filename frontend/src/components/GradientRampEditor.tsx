import { useCallback, useRef } from "react";

import type { GradientStop } from "../lib/edl";

/**
 * Das Verlaufsband mit frei verschiebbaren Stützstellen (Phase 30
 * Punkt 6, siehe `DECISIONS.md` ADR-0060).
 *
 * Ein Bedienelement, das es im Projekt bisher nicht gab: die
 * Verlaufsabbildung aus Phase 27 hat genau drei feste Farben (Tiefen,
 * Mitten, Lichter). Hier legt der Nutzer die Stützstellen selbst an,
 * schiebt sie entlang der Luminanzachse und färbt sie einzeln.
 *
 * Das Band zeigt den Verlauf als echten CSS-Farbverlauf — also genau
 * das, was die Stufe gleich auf das Foto abbildet, nicht eine
 * schematische Andeutung davon.
 */

export interface GradientRampEditorProps {
  stops: readonly GradientStop[];
  selectedIndex: number;
  onSelect: (index: number) => void;
  onMove: (index: number, position: number) => void;
  onMoveEnd: () => void;
  onColorChange: (index: number, rgb: number[]) => void;
  onAdd: (position: number) => void;
  onRemove: (index: number) => void;
}

function toHex(rgb: readonly number[]): string {
  const part = (v: number) =>
    Math.round(Math.min(1, Math.max(0, v)) * 255)
      .toString(16)
      .padStart(2, "0");
  return `#${part(rgb[0] ?? 0)}${part(rgb[1] ?? 0)}${part(rgb[2] ?? 0)}`;
}

function fromHex(hex: string): number[] {
  const clean = hex.replace("#", "");
  return [0, 2, 4].map((offset) => parseInt(clean.slice(offset, offset + 2), 16) / 255);
}

export function GradientRampEditor({
  stops,
  selectedIndex,
  onSelect,
  onMove,
  onMoveEnd,
  onColorChange,
  onAdd,
  onRemove,
}: GradientRampEditorProps) {
  const bandRef = useRef<HTMLDivElement | null>(null);
  const draggingRef = useRef<number | null>(null);

  // Nach Position sortiert ANZEIGEN, aber über den ursprünglichen Index
  // bearbeiten: sonst würde eine Stützstelle beim Vorbeischieben an
  // einer anderen plötzlich eine andere sein.
  const ordered = stops
    .map((stop, index) => ({ stop, index }))
    .sort((a, b) => a.stop.position - b.stop.position);

  const cssGradient =
    ordered.length >= 2
      ? `linear-gradient(to right, ${ordered
          .map(({ stop }) => `${toHex(stop.color_rgb)} ${(stop.position * 100).toFixed(1)}%`)
          .join(", ")})`
      : "linear-gradient(to right, #000, #fff)";

  const positionFromEvent = useCallback((clientX: number): number => {
    const rect = bandRef.current?.getBoundingClientRect();
    if (!rect || rect.width === 0) return 0.5;
    return Math.min(1, Math.max(0, (clientX - rect.left) / rect.width));
  }, []);

  const handlePointerMove = useCallback(
    (event: React.PointerEvent) => {
      const index = draggingRef.current;
      if (index === null) return;
      onMove(index, positionFromEvent(event.clientX));
    },
    [onMove, positionFromEvent],
  );

  const handlePointerUp = useCallback(() => {
    if (draggingRef.current === null) return;
    draggingRef.current = null;
    onMoveEnd();
  }, [onMoveEnd]);

  const selected = stops[selectedIndex];

  return (
    <div className="flex flex-col gap-2" data-testid="gradient-ramp-editor">
      <div
        ref={bandRef}
        className="relative h-8 rounded border border-[var(--glass-border)]"
        style={{ background: cssGradient }}
        onPointerMove={handlePointerMove}
        onPointerUp={handlePointerUp}
        onPointerLeave={handlePointerUp}
      >
        {/* Doppelklick auf eine freie Stelle legt dort eine neue
            Stützstelle an — der kürzeste Weg, und er stört das Ziehen
            nicht. */}
        <div
          className="absolute inset-0 cursor-copy"
          onDoubleClick={(event) => onAdd(positionFromEvent(event.clientX))}
          aria-hidden
        />
        {ordered.map(({ stop, index }) => (
          <button
            key={index}
            type="button"
            aria-label={`Stützstelle ${index + 1} bei ${Math.round(stop.position * 100)} %`}
            data-stop={index}
            onPointerDown={(event) => {
              (event.target as Element).setPointerCapture?.(event.pointerId);
              draggingRef.current = index;
              onSelect(index);
            }}
            className={`absolute top-1/2 h-5 w-3 -translate-x-1/2 -translate-y-1/2 cursor-grab rounded-sm border-2 transition-transform duration-[var(--duration-fast)] hover:scale-110 ${
              index === selectedIndex ? "scale-110 border-white" : "border-white/60"
            }`}
            style={{ left: `${stop.position * 100}%`, backgroundColor: toHex(stop.color_rgb) }}
          />
        ))}
      </div>

      <div className="flex items-center gap-2">
        <input
          type="color"
          aria-label="Farbe der gewählten Stützstelle"
          data-testid="gradient-stop-color"
          disabled={!selected}
          value={selected ? toHex(selected.color_rgb) : "#808080"}
          onChange={(event) => onColorChange(selectedIndex, fromHex(event.target.value))}
          className="h-7 w-10 cursor-pointer rounded border border-[var(--glass-border)] bg-transparent disabled:opacity-40"
        />
        <button
          type="button"
          onClick={() => onAdd(0.5)}
          className="apx-btn-liquid rounded border border-border px-2 py-1 text-xs text-text-secondary transition-colors duration-[var(--duration-fast)] hover:border-accent hover:text-text-primary"
        >
          + Stützstelle
        </button>
        <button
          type="button"
          disabled={!selected || stops.length <= 2}
          onClick={() => onRemove(selectedIndex)}
          className="apx-btn-liquid rounded border border-border px-2 py-1 text-xs text-text-secondary transition-colors duration-[var(--duration-fast)] hover:border-accent hover:text-text-primary disabled:cursor-not-allowed disabled:opacity-40"
        >
          Entfernen
        </button>
        <span className="ml-auto text-[11px] text-text-muted">
          {stops.length} Stützstellen
        </span>
      </div>
    </div>
  );
}
