import { useCallback, useRef } from "react";
import type { ReactNode } from "react";

import { useWorkspacePanel } from "../lib/workspaceLayout";

interface PaletteFrameProps {
  id: string;
  side: "left" | "right";
  defaultWidth: number;
  label: string;
  className?: string;
  children: ReactNode;
}

/**
 * Gemeinsame Außenhülle für ein-/ausklappbare, breitenziehbare Paletten
 * (Phase 10 Schritt 3). Ersetzt das bisherige feste `w-64`/`w-72` an den
 * `<aside>`-Elementen von `Sidebar.tsx`/`PresetsPanel.tsx`/
 * `MetadataPanel.tsx` — Standardbreite und Standardzustand
 * (eingeklappt=false) bleiben unverändert, damit jeder bestehende
 * e2e-Test, der Inhalte dieser Paletten anspricht, ohne Anpassung
 * weiterläuft; Ziehen/Einklappen ist rein additiv.
 */
export function PaletteFrame({ id, side, defaultWidth, label, className = "", children }: PaletteFrameProps) {
  const { width, collapsed, toggleCollapsed, setWidth } = useWorkspacePanel(id, defaultWidth);
  const dragState = useRef<{ startX: number; startWidth: number } | null>(null);

  const onPointerMove = useCallback(
    (event: PointerEvent) => {
      if (!dragState.current) return;
      const delta = side === "left" ? event.clientX - dragState.current.startX : dragState.current.startX - event.clientX;
      setWidth(dragState.current.startWidth + delta);
    },
    [side, setWidth],
  );

  const onPointerUp = useCallback(() => {
    dragState.current = null;
    window.removeEventListener("pointermove", onPointerMove);
    window.removeEventListener("pointerup", onPointerUp);
  }, [onPointerMove]);

  const onPointerDown = useCallback(
    (event: React.PointerEvent) => {
      dragState.current = { startX: event.clientX, startWidth: width };
      window.addEventListener("pointermove", onPointerMove);
      window.addEventListener("pointerup", onPointerUp);
    },
    [width, onPointerMove, onPointerUp],
  );

  if (collapsed) {
    return (
      <div className={`flex shrink-0 flex-col items-center gap-2 bg-bg-raised p-1 ${side === "left" ? "border-r" : "border-l"} border-border`}>
        <button
          type="button"
          onClick={toggleCollapsed}
          title={`${label} einblenden`}
          aria-label={`${label} einblenden`}
          className="rounded border border-border px-1 py-2 text-xs text-text-secondary hover:border-accent hover:text-text-primary"
          style={{ writingMode: "vertical-rl" }}
        >
          {label}
        </button>
      </div>
    );
  }

  return (
    <div className={`relative flex shrink-0 ${side === "right" ? "flex-row-reverse" : ""}`} style={{ width }}>
      <aside className={`flex w-full flex-col overflow-y-auto ${className}`} aria-label={label}>
        <div className="flex items-center justify-end px-1 pt-1">
          <button
            type="button"
            onClick={toggleCollapsed}
            title={`${label} einklappen`}
            aria-label={`${label} einklappen`}
            className="rounded px-1 text-xs text-text-muted hover:text-text-primary"
          >
            {side === "left" ? "‹" : "›"}
          </button>
        </div>
        {children}
      </aside>
      {/* eslint-disable-next-line jsx-a11y/no-static-element-interactions -- Ziehgriff, keine semantische Bedeutung außerhalb der Maus-/Touch-Interaktion */}
      <div
        onPointerDown={onPointerDown}
        className="w-1 shrink-0 cursor-col-resize bg-transparent hover:bg-accent/40"
        title={`${label}-Breite ziehen`}
      />
    </div>
  );
}
