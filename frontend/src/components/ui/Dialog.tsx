import { useEffect, useRef, useState } from "react";
import type { ReactNode } from "react";
import gsap from "gsap";

import { useFocusTrap } from "../../lib/a11y";
import { usePrefersReducedMotion } from "../../lib/motion";
import { playCue } from "../../lib/sound";

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
 * eine 22-von-25-Lücke) und Escape-zum-Schließen.
 *
 * **Echte GSAP-Bewegung statt reiner CSS-Transition (Phase 20, siehe
 * DECISIONS.md ADR-0048):** die ursprüngliche Phase-18-Fassung nutzte
 * nur `transition-opacity`/`transition-transform` mit 8 px Versatz —
 * technisch eine Animation, aber so unauffällig, dass sie zur Nutzer-
 * Rückmeldung "kein Hover, kein gar nix" beitrug, obwohl `gsap` seit
 * Phase 19 als Abhängigkeit bereitsteht, bis dahin aber nur in
 * `StartupSplash.tsx`/`ShutdownOverlay.tsx` tatsächlich verwendet
 * wurde. Jetzt: spürbares Herausskalieren (0.94→1) mit leichtem
 * Überschwingen (`back.out`) beim Öffnen — bei über 25 Dialogen/
 * `CommandPalette`/`KeybindingsCheatsheet`, die sich diese eine Hülle
 * teilen, wirkt sich das strukturell auf die gesamte App aus, ohne
 * einen einzigen Aufrufer anzufassen. Dieselbe Drei-Effekt-Struktur
 * wie `StartupSplash.tsx` (Mount+Sound / Eintritt-Tween ausgelöst von
 * `entered` / Austritt-Tween ausgelöst von `open===false`), damit die
 * `mounted`/`entered`-Umschaltung nicht mit den GSAP-`useEffect`-
 * Abhängigkeiten kollidiert.
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
  const backdropRef = useRef<HTMLDivElement>(null);
  const panelRef = useRef<HTMLDivElement>(null);
  const reducedMotion = usePrefersReducedMotion();
  // Verhindert einen Sound beim allerersten Rendern (jeder Dialog wird
  // laut Modul-Doku oben dauerhaft mit `open={false}` gerendert, nicht
  // erst bei Bedarf gemountet — ohne diese Wächter würde jeder der 25
  // Dialoge beim App-Start einmal lautlos-gemeinten "close"-Cue
  // auslösen).
  const isFirstRenderRef = useRef(true);

  useFocusTrap(panelRef, open);

  // Mount + Sound, dann (verzögert auf den nächsten Frame, sonst startet
  // der Übergang bereits im Zielzustand) "eingetreten" schalten.
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

  // Eintritt-Tween — läuft genau einmal je Öffnen-Vorgang (`entered`
  // wechselt genau dann auf `true`).
  useEffect(() => {
    if (!entered || reducedMotion) return;
    const ctx = gsap.context(() => {
      gsap.fromTo(backdropRef.current, { opacity: 0 }, { opacity: 1, duration: 0.2, ease: "power1.out" });
      gsap.fromTo(
        panelRef.current,
        { opacity: 0, scale: 0.94, y: 10 },
        { opacity: 1, scale: 1, y: 0, duration: 0.32, ease: "back.out(1.6)" },
      );
    });
    return () => ctx.revert();
  }, [entered, reducedMotion]);

  // Austritt-Tween — `open === false` bei noch gemountetem Panel heißt
  // "gerade am Schließen"; `onComplete` entfernt das Panel danach aus
  // dem DOM (ersetzt das vorherige feste `setTimeout(DURATION_BASE_MS)`
  // durch die tatsächliche Tween-Dauer).
  useEffect(() => {
    if (open || reducedMotion || !mounted) return;
    const tl = gsap.timeline({ onComplete: () => setMounted(false) });
    tl.to(panelRef.current, { opacity: 0, scale: 0.96, y: 6, duration: 0.18, ease: "power1.in" }, 0);
    tl.to(backdropRef.current, { opacity: 0, duration: 0.18, ease: "power1.in" }, 0);
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
      className="fixed inset-0 z-50 flex items-center justify-center bg-bg-overlay backdrop-blur-sm"
      style={{ opacity: reducedMotion ? 1 : entered ? undefined : 0 }}
      onClick={onClose}
    >
      <div
        ref={panelRef}
        role="dialog"
        aria-modal="true"
        aria-label={label}
        onClick={(event) => event.stopPropagation()}
        className={`max-h-[85vh] w-full overflow-y-auto rounded-xl border border-border bg-bg-raised shadow-xl ${className}`}
        style={{ opacity: reducedMotion ? 1 : entered ? undefined : 0 }}
      >
        {children}
      </div>
    </div>
  );
}
