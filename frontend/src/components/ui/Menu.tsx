import { useEffect, useRef, useState, type ReactNode } from "react";

import { DURATION_FAST_MS, usePrefersReducedMotion } from "../../lib/motion";

export interface MenuItem {
  id: string;
  label: string;
  onSelect: () => void;
  disabled?: boolean;
  hint?: string;
}

export interface MenuSection {
  id: string;
  label: string;
  items: MenuItem[];
}

/**
 * Leichtgewichtiges, an einem Auslöser verankertes Menü (Phase 18
 * Schritt 3, siehe `DECISIONS.md` ADR-0046) — bewusst **kein**
 * `<Dialog>`: ein Overflow-Menü soll sich wie eine direkte Erweiterung
 * des auslösenden Knopfes anfühlen, nicht wie eine Unterbrechung mit
 * abgedunkeltem Hintergrund. Schließt bei Klick außerhalb, Escape und
 * nach Auswahl eines Eintrags.
 */
export function Menu({ label, trigger, sections, align = "end" }: { label: string; trigger: ReactNode; sections: MenuSection[]; align?: "start" | "end" }) {
  const [open, setOpen] = useState(false);
  const rootRef = useRef<HTMLDivElement>(null);
  const reducedMotion = usePrefersReducedMotion();

  useEffect(() => {
    if (!open) return;
    function onPointerDown(event: MouseEvent) {
      if (rootRef.current && !rootRef.current.contains(event.target as Node)) setOpen(false);
    }
    function onKeyDown(event: KeyboardEvent) {
      if (event.key === "Escape") setOpen(false);
    }
    window.addEventListener("mousedown", onPointerDown);
    window.addEventListener("keydown", onKeyDown);
    return () => {
      window.removeEventListener("mousedown", onPointerDown);
      window.removeEventListener("keydown", onKeyDown);
    };
  }, [open]);

  return (
    <div ref={rootRef} className="relative">
      <button
        type="button"
        aria-haspopup="menu"
        aria-expanded={open}
        aria-label={label}
        title={label}
        onClick={() => setOpen((value) => !value)}
        className={`rounded border px-3 py-1 text-sm ${open ? "border-accent bg-accent/10 text-accent" : "border-border bg-bg-panel hover:border-accent"}`}
      >
        {trigger}
      </button>
      {open && (
        <div
          role="menu"
          aria-label={label}
          className={`absolute top-full z-40 mt-2 max-h-[70vh] w-72 overflow-y-auto rounded-lg border border-border bg-bg-raised p-1 shadow-lg transition-[opacity,transform] ${
            align === "end" ? "right-0" : "left-0"
          }`}
          style={{ transitionDuration: reducedMotion ? "0ms" : `${DURATION_FAST_MS}ms` }}
        >
          {sections.map((section) => (
            <div key={section.id} className="py-1">
              <div className="px-2 pb-1 text-[10px] font-semibold uppercase tracking-wide text-text-muted">{section.label}</div>
              {section.items.map((item) => (
                <button
                  key={item.id}
                  type="button"
                  role="menuitem"
                  disabled={item.disabled}
                  onClick={() => {
                    item.onSelect();
                    setOpen(false);
                  }}
                  className="flex w-full items-center justify-between gap-2 rounded px-2 py-1.5 text-left text-sm hover:bg-bg-panel disabled:cursor-not-allowed disabled:opacity-40"
                >
                  <span className="truncate">{item.label}</span>
                  {item.hint && <span className="shrink-0 text-xs text-text-muted">{item.hint}</span>}
                </button>
              ))}
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
