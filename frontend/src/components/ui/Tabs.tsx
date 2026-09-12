import gsap from "gsap";
import { useEffect, useRef } from "react";

import { usePrefersReducedMotion } from "../../lib/motion";
import { playCue } from "../../lib/sound";

/**
 * Gemeinsame Registerkarten-Leiste (Phase 18 Schritt 4, siehe
 * `DECISIONS.md` ADR-0046) — von `DevelopPanel.tsx` und `MasksPanel.tsx`
 * genutzt, um die vormals durchgehende Fieldset-Scroll-Spalte in
 * benannte Gruppen zu teilen. Bewusst generisch/minimal (kein Routing,
 * kein Lazy-Mount) — der aktive Tab ist reiner `useState` der
 * aufrufenden Komponente, dieselbe Größenordnung wie die bereits
 * bestehenden lokalen Tab-Leisten (`CURVE_CHANNEL_TABS` u. Ä.).
 *
 * **Gleitender Auswahl-Hintergrund (Phase 23 Nachtrag, siehe
 * DECISIONS.md ADR-0051-Nachtrag):** statt der vorherigen reinen
 * Textfarben-Umschaltung wandert ein Hintergrund-Rechteck per GSAP
 * zwischen den Tabs — Position/Größe werden aus der tatsächlichen
 * `getBoundingClientRect()` des aktiven Tab-Knopfs abgeleitet (nicht
 * fest verdrahtet), damit das bei `flex-wrap` (mehrzeilige Leisten in
 * schmalen Paletten) korrekt bleibt.
 */
export interface TabItem<T extends string> {
  id: T;
  label: string;
}

export function TabBar<T extends string>({
  tabs,
  active,
  onChange,
  label,
}: {
  tabs: ReadonlyArray<TabItem<T>>;
  active: T;
  onChange: (id: T) => void;
  label: string;
}) {
  const containerRef = useRef<HTMLDivElement>(null);
  const buttonRefs = useRef<Map<T, HTMLButtonElement>>(new Map());
  const highlightRef = useRef<HTMLDivElement>(null);
  const hasPositionedRef = useRef(false);
  const reducedMotion = usePrefersReducedMotion();

  useEffect(() => {
    const container = containerRef.current;
    const activeButton = buttonRefs.current.get(active);
    const highlight = highlightRef.current;
    if (!container || !activeButton || !highlight) return;

    const containerRect = container.getBoundingClientRect();
    const buttonRect = activeButton.getBoundingClientRect();
    const target = {
      x: buttonRect.left - containerRect.left,
      y: buttonRect.top - containerRect.top,
      width: buttonRect.width,
      height: buttonRect.height,
    };

    if (reducedMotion || !hasPositionedRef.current) {
      gsap.set(highlight, target);
    } else {
      gsap.to(highlight, { ...target, duration: 0.25, ease: "power2.out" });
    }
    hasPositionedRef.current = true;
    // `tabs` als Abhängigkeit: eine sich ändernde Tab-Breite (z. B.
    // durch Lokalisierungswechsel) soll den Hintergrund neu vermessen,
    // nicht auf der alten Rect-Größe stehen bleiben.
  }, [active, tabs, reducedMotion]);

  return (
    <div ref={containerRef} role="tablist" aria-label={label} className="relative flex flex-wrap gap-0.5 rounded border border-border bg-bg-panel p-0.5">
      <div ref={highlightRef} aria-hidden="true" className="pointer-events-none absolute rounded bg-accent/10" style={{ willChange: "transform" }} />
      {tabs.map((tab) => (
        <button
          key={tab.id}
          ref={(el) => {
            if (el) buttonRefs.current.set(tab.id, el);
            else buttonRefs.current.delete(tab.id);
          }}
          type="button"
          role="tab"
          aria-selected={active === tab.id}
          onClick={() => {
            if (tab.id !== active) playCue("select");
            onChange(tab.id);
          }}
          className={`relative z-[1] flex-1 rounded px-2 py-1 text-xs transition-colors duration-[var(--duration-fast)] ${
            active === tab.id ? "text-accent" : "text-text-secondary hover:text-text-primary"
          }`}
        >
          {tab.label}
        </button>
      ))}
    </div>
  );
}
