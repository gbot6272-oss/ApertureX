import { useEffect, useRef, useState } from "react";
import type { ReactNode } from "react";
import gsap from "gsap";

import { useFocusTrap } from "../../lib/a11y";
import { usePrefersReducedMotion } from "../../lib/motion";
import { playCue } from "../../lib/sound";

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
 *
 * **Echte GSAP-Bewegung statt reiner CSS-Transition (Phase 20, siehe
 * DECISIONS.md ADR-0048)** — dieselbe Begründung/Drei-Effekt-Struktur
 * wie `Dialog.tsx`: spürbares Einschieben mit leichtem Überschwingen
 * statt der vorherigen 32-px-`translate-x`-CSS-Transition.
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
  const backdropRef = useRef<HTMLDivElement>(null);
  const panelRef = useRef<HTMLDivElement>(null);
  const reducedMotion = usePrefersReducedMotion();
  // Siehe `Dialog.tsx`s identischer Wächter-Kommentar — verhindert einen
  // Sound beim allerersten Rendern jedes dauerhaft gemounteten Sheets.
  const isFirstRenderRef = useRef(true);

  useFocusTrap(panelRef, open);

  useEffect(() => {
    if (open) {
      setMounted(true);
      if (!isFirstRenderRef.current) playCue("open");
      isFirstRenderRef.current = false;
      if (reducedMotion) {
        setEntered(true);
        return;
      }
      const raf = requestAnimationFrame(() => setEntered(true));
      return () => cancelAnimationFrame(raf);
    }
    if (!isFirstRenderRef.current) playCue("close");
    isFirstRenderRef.current = false;
    setEntered(false);
    if (reducedMotion) setMounted(false);
  }, [open, reducedMotion]);

  useEffect(() => {
    if (!entered || reducedMotion) return;
    const ctx = gsap.context(() => {
      gsap.fromTo(backdropRef.current, { opacity: 0 }, { opacity: 1, duration: 0.2, ease: "power1.out" });
      gsap.fromTo(panelRef.current, { x: 48 }, { x: 0, duration: 0.36, ease: "back.out(1.4)" });
    });
    return () => ctx.revert();
  }, [entered, reducedMotion]);

  useEffect(() => {
    if (open || reducedMotion || !mounted) return;
    const tl = gsap.timeline({ onComplete: () => setMounted(false) });
    tl.to(panelRef.current, { x: 32, duration: 0.2, ease: "power1.in" }, 0);
    tl.to(backdropRef.current, { opacity: 0, duration: 0.2, ease: "power1.in" }, 0);
    return () => {
      tl.kill();
    };
  }, [open, mounted, reducedMotion]);

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
      ref={backdropRef}
      className="fixed inset-0 z-50 flex justify-end bg-bg-overlay backdrop-blur-sm"
      style={{ opacity: reducedMotion ? 1 : entered ? undefined : 0 }}
      onClick={onClose}
    >
      <div
        ref={panelRef}
        role="dialog"
        aria-modal="true"
        aria-label={label}
        onClick={(event) => event.stopPropagation()}
        className={`apx-glass-strong h-full w-full overflow-y-auto border-l border-[var(--glass-border)] shadow-2xl ${className}`}
        style={reducedMotion ? undefined : { transform: entered ? undefined : "translateX(2rem)" }}
      >
        {children}
      </div>
    </div>
  );
}
