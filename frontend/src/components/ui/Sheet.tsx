import { useEffect, useRef, useState } from "react";
import type { ReactNode } from "react";

import { useFocusTrap } from "../../lib/a11y";
import { DURATION_BASE_MS, usePrefersReducedMotion } from "../../lib/motion";

export interface SheetProps {
  open: boolean;
  onClose: () => void;
  label: string;
  children: ReactNode;
  /** Größen-/Layout-Klassen des Panels (z. B. `max-w-xl`) — dieselbe
   * "nur Rahmen, keine Innenabstände"-Regel wie `Dialog`. */
  className?: string;
}

/**
 * Von der rechten Kante einschiebende Werkzeug-Hülle (Phase 18
 * Schritt 2, ADR-0046) — für die großen, mehrstufigen Werkzeuge
 * (Export, Zeitachse/Video-Editor, Stapelverarbeitung), bei denen eine
 * zentrierte Box sich wie eine Unterbrechung anfühlt statt wie Teil
 * des Arbeitsflusses. Teilt sich mit `Dialog` denselben Lebenszyklus
 * (Fokus-Falle, Escape, verzögertes Entfernen aus dem DOM für die
 * Ausblendbewegung, `usePrefersReducedMotion()`) — nur die Bewegung
 * selbst ist eine horizontale Verschiebung statt Skalieren/Einblenden.
 */
export function Sheet({
  open,
  onClose,
  label,
  children,
  className = "max-w-xl",
}: SheetProps) {
  const [mounted, setMounted] = useState(open);
  const [entered, setEntered] = useState(false);
  const panelRef = useRef<HTMLDivElement>(null);
  const reducedMotion = usePrefersReducedMotion();

  useFocusTrap(panelRef, open);

  useEffect(() => {
    if (open) {
      setMounted(true);
      const raf = requestAnimationFrame(() => setEntered(true));
      return () => cancelAnimationFrame(raf);
    }
    setEntered(false);
    const delay = reducedMotion ? 0 : DURATION_BASE_MS;
    const timeout = setTimeout(() => setMounted(false), delay);
    return () => clearTimeout(timeout);
  }, [open, reducedMotion]);

  useEffect(() => {
    if (!open) return;
    function onKeyDown(event: KeyboardEvent) {
      if (event.key === "Escape") onClose();
    }
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [open, onClose]);

  if (!mounted) return null;

  return (
    <div
      className={`fixed inset-0 z-50 flex justify-end bg-bg-overlay backdrop-blur-sm transition-opacity duration-[var(--duration-base)] ${
        entered ? "opacity-100" : "opacity-0"
      }`}
      onClick={onClose}
    >
      <div
        ref={panelRef}
        role="dialog"
        aria-modal="true"
        aria-label={label}
        onClick={(event) => event.stopPropagation()}
        className={`h-full w-full overflow-y-auto border-l border-border bg-bg-raised shadow-xl transition-transform duration-[var(--duration-base)] ${
          entered ? "translate-x-0" : "translate-x-8"
        } ${className}`}
      >
        {children}
      </div>
    </div>
  );
}
