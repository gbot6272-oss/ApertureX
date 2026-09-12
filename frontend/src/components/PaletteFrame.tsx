import { useCallback, useRef, useState } from "react";
import type { ReactNode } from "react";

import { DURATION_BASE_MS, EASE_OUT, usePrefersReducedMotion } from "../lib/motion";
import { playCue } from "../lib/sound";
import { useAppStore } from "../store";
import { useWorkspacePanel } from "../lib/workspaceLayout";

interface PaletteFrameProps {
  id: string;
  side: "left" | "right";
  defaultWidth: number;
  label: string;
  className?: string;
  children: ReactNode;
}

/** Breite der eingeklappten Fassung (nur der senkrechte Beschriftungs-
 * knopf) — als fester Wert statt `auto`, damit die Einklapp-Animation
 * (Phase 18 Schritt 5) eine echte CSS-`width`-Transition zwischen zwei
 * konkreten Zahlen ist statt eines Sprungs zu/von `auto`, das die
 * meisten Browser nicht sauber animieren. */
const COLLAPSED_WIDTH = 36;

/**
 * Gemeinsame Außenhülle für ein-/ausklappbare, breitenziehbare Paletten
 * (Phase 10 Schritt 3). Ersetzt das bisherige feste `w-64`/`w-72` an den
 * `<aside>`-Elementen von `Sidebar.tsx`/`PresetsPanel.tsx`/
 * `MetadataPanel.tsx` — Standardbreite und Standardzustand
 * (eingeklappt=false) bleiben unverändert, damit jeder bestehende
 * e2e-Test, der Inhalte dieser Paletten anspricht, ohne Anpassung
 * weiterläuft; Ziehen/Einklappen ist rein additiv.
 *
 * Trägt seit Phase 21 (Liquid-Glass-Überarbeitung, `DECISIONS.md`
 * ADR-0049) zentral die `apx-glass`-Materialklasse (siehe `index.css`)
 * auf beiden Zuständen (ein-/ausgeklappt) — alle sechs Aufrufer
 * (Sidebar/Presets/Metadaten/Entwickeln/Masken-Panel) bekommen die
 * neue Glas-Optik dadurch automatisch, ohne selbst `bg-bg-raised` zu
 * setzen; sie liefern nur noch Rand-/Abstands-/Lückenklassen über
 * `className`.
 *
 * Trägt seit Phase 25 Nachtrag III zusätzlich zentral die
 * Kopfzeilen-Ausgleichslücke: `Header.tsx` schwebt inzwischen als
 * `fixed`-Überlagerung außerhalb des Dokumentflusses (Nutzerwunsch
 * "Kopfzeile als Overlay über den Fotos"), nimmt also keinen Platz
 * mehr in der Zeile ein, in der diese Paletten stehen — ohne
 * Ausgleich würden Sidebar/Presets/Metadaten/Entwickeln/Masken-Panel
 * an ihrem oberen Rand unter der schwebenden Kopfzeile verschwinden.
 * In den Ansichten mit `FilterBar` (Raster/Übersicht) übernimmt
 * bereits `FilterBar.tsx` selbst diesen Ausgleich für die ganze Zeile
 * darunter — hier `pt-12` deshalb NUR, wenn keine `FilterBar`
 * angezeigt wird, sonst gäbe es eine doppelte, zu große Lücke.
 */
export function PaletteFrame({ id, side, defaultWidth, label, className = "", children }: PaletteFrameProps) {
  const { width, collapsed, toggleCollapsed, setWidth } = useWorkspacePanel(id, defaultWidth);
  const centerView = useAppStore((s) => s.centerView);
  const filterBarCompensatesHeader = centerView === "grid" || centerView === "overview";
  const dragState = useRef<{ startX: number; startWidth: number } | null>(null);
  const reducedMotion = usePrefersReducedMotion();
  // Während eines aktiven Zieh-Vorgangs darf keine CSS-`width`-Transition
  // laufen — sonst folgt die Palette dem Zeiger nur gedämpft/verzögert
  // statt 1:1. Nur der Einklapp-/Ausklapp-Wechsel soll animiert sein.
  const [dragging, setDragging] = useState(false);

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
    setDragging(false);
    playCue("drop");
    window.removeEventListener("pointermove", onPointerMove);
    window.removeEventListener("pointerup", onPointerUp);
  }, [onPointerMove]);

  const onPointerDown = useCallback(
    (event: React.PointerEvent) => {
      dragState.current = { startX: event.clientX, startWidth: width };
      setDragging(true);
      playCue("drag-start");
      window.addEventListener("pointermove", onPointerMove);
      window.addEventListener("pointerup", onPointerUp);
    },
    [width, onPointerMove, onPointerUp],
  );

  const transition = !reducedMotion && !dragging ? `width ${DURATION_BASE_MS}ms ${EASE_OUT}` : "none";

  return (
    <div
      className={`relative flex shrink-0 overflow-hidden ${side === "right" ? "flex-row-reverse" : ""} ${filterBarCompensatesHeader ? "" : "pt-12"}`}
      style={{ width: collapsed ? COLLAPSED_WIDTH : width, transition }}
    >
      {collapsed ? (
        <div className={`apx-glass flex w-full shrink-0 flex-col items-center gap-2 p-1 ${side === "left" ? "border-r" : "border-l"} border-[var(--glass-border)]`}>
          <button
            type="button"
            onClick={() => {
              playCue("expand");
              toggleCollapsed();
            }}
            title={`${label} einblenden`}
            aria-label={`${label} einblenden`}
            className="rounded border border-border px-1 py-2 text-xs text-text-secondary hover:border-accent hover:text-text-primary"
            style={{ writingMode: "vertical-rl" }}
          >
            {label}
          </button>
        </div>
      ) : (
        <>
          <aside className={`apx-glass flex w-full shrink-0 flex-col overflow-y-auto ${className}`} aria-label={label}>
            <div className="flex items-center justify-end px-1 pt-1">
              <button
                type="button"
                onClick={() => {
                  playCue("collapse");
                  toggleCollapsed();
                }}
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
        </>
      )}
    </div>
  );
}
