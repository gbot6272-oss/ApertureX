import { useEffect, useRef, useState } from "react";
import type { ReactNode } from "react";

import { useFocusTrap } from "../../lib/a11y";
import { DURATION_BASE_MS, usePrefersReducedMotion } from "../../lib/motion";

export interface DialogProps {
  open: boolean;
  onClose: () => void;
  /** `aria-label` des Dialogs — sichtbarer Titel (`<h2>` o. Ä.) bleibt
   * Sache der `children`, dieses Label ist nur für Screenreader, falls
   * kein sichtbarer Titel vorhanden ist bzw. zusätzlich zu ihm. */
  label: string;
  children: ReactNode;
  /** Größen-/Layout-Klassen des Panels (z. B. `max-w-md`, oder
   * `flex flex-col` für einen eigenen festen Kopfbereich wie
   * `SettingsDialog`) — `Dialog` liefert nur den optischen Rahmen
   * (Rundung/Rand/Hintergrund/Schatten/Bewegung), **keine**
   * Innenabstände: jeder Aufrufer behält seine bisherige Innenraum-
   * Gestaltung (meist eigenes `p-4`) unverändert bei der Migration. */
  className?: string;
}

/**
 * Gemeinsame zentrierte Dialog-Hülle (Phase 18 Schritt 2, ADR-0046) —
 * ersetzt die zuvor 25 einzeln handgerollten `fixed inset-0 ...
 * bg-black/50`-Fassungen. Vereinheitlicht gegenüber dem Vorzustand:
 * derselbe `--color-bg-overlay`-Backdrop mit Weichzeichner statt
 * hartem `bg-black/50`, **immer** zentriert (ersetzt das zuvor
 * zufällig wirkende `pt-8`/`pt-16`/`pt-24`/`items-center`-Sammelsurium
 * durch eine einzige Regel), **immer** `useFocusTrap` (schloss zuvor
 * eine 22-von-25-Lücke) und Escape-zum-Schließen, sowie eine sanfte
 * Ein-/Ausblendbewegung, die `usePrefersReducedMotion()` respektiert.
 *
 * Übernimmt bewusst den kompletten Sichtbarkeits-Lebenszyklus
 * (inklusive Ausblend-Verzögerung vorm Entfernen aus dem DOM) — jeder
 * Aufrufer rendert `&lt;Dialog open={open} .../&gt;` deshalb
 * unbedingt (kein eigenes `if (!open) return null` mehr davor).
 */
export function Dialog({
  open,
  onClose,
  label,
  children,
  className = "max-w-md",
}: DialogProps) {
  const [mounted, setMounted] = useState(open);
  const [entered, setEntered] = useState(false);
  const panelRef = useRef<HTMLDivElement>(null);
  const reducedMotion = usePrefersReducedMotion();

  useFocusTrap(panelRef, open);

  useEffect(() => {
    if (open) {
      setMounted(true);
      // Erst im nächsten Frame auf "eingeblendet" schalten, sonst
      // startet der Übergang bereits im Zielzustand (kein sichtbarer
      // Sprung von unsichtbar zu sichtbar möglich, wenn beides im
      // selben Layout-Zyklus passiert).
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
      className={`fixed inset-0 z-50 flex items-center justify-center bg-bg-overlay backdrop-blur-sm transition-opacity duration-[var(--duration-base)] ${
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
        className={`max-h-[85vh] w-full overflow-y-auto rounded-xl border border-border bg-bg-raised shadow-xl transition-[opacity,transform] duration-[var(--duration-base)] ${
          entered ? "translate-y-0 opacity-100" : "translate-y-2 opacity-0"
        } ${className}`}
      >
        {children}
      </div>
    </div>
  );
}
