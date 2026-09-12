import gsap from "gsap";
import { useEffect, useRef, useState } from "react";

import { useT } from "../lib/i18n";
import { usePrefersReducedMotion } from "../lib/motion";
import { useAppStore, selectAnyBackgroundTaskRunning } from "../store";
import { DotLoader } from "./ui/DotLoader";

/**
 * Ein kleines "wanderndes Punktpaar"-Muster, das im 3×3-Ausschnitt
 * (Zeilen/Spalten 2–4) des von `DotLoader` erwarteten 7×7-Rasters
 * einmal rundherum läuft — kompakter, ruhiger Kreislauf statt des
 * größeren Spielfeld-Musters aus dem vom Nutzer beigelegten Vorbild
 * (das war für einen dekorativen, größeren Kontext gedacht; hier soll
 * es als kleiner, dezenter Statuspunkt wirken).
 */
const RING = [2, 3, 4, 11, 18, 17, 16, 9];
const SPINNER_FRAMES: number[][] = RING.map((_, i) => [RING[i]!, RING[(i + 1) % RING.length]!]);

/**
 * Zentraler, dezenter Verarbeitungs-Indikator (Phase 23, siehe
 * DECISIONS.md ADR-0051) — erscheint automatisch unten rechts, sobald
 * `selectAnyBackgroundTaskRunning` `true` liefert (irgendeine der 35+
 * Lade-/Verarbeitungs-Operationen im Store läuft), statt dass jede
 * Aufrufstelle einzeln eine eigene Ladeanimation bräuchte. Rein
 * additiv — verändert keinen bestehenden Text/Titel/aria-label,
 * daher risikofrei gegenüber den bestehenden Playwright-Spezifikationen.
 *
 * Dieselbe Mount-/Eintritt-/Austritt-Dreiteilung wie `ui/Dialog.tsx`
 * (siehe dortige Moduldoku) — `busy` übernimmt hier die Rolle von
 * `open`.
 */
export function GlobalBusyIndicator() {
  const t = useT();
  const busy = useAppStore(selectAnyBackgroundTaskRunning);
  const reducedMotion = usePrefersReducedMotion();
  const [mounted, setMounted] = useState(busy);
  const [entered, setEntered] = useState(false);
  const pillRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (busy) {
      setMounted(true);
      if (reducedMotion) {
        setEntered(true);
        return;
      }
      const raf = requestAnimationFrame(() => setEntered(true));
      return () => cancelAnimationFrame(raf);
    }
    setEntered(false);
    if (reducedMotion) setMounted(false);
  }, [busy, reducedMotion]);

  useEffect(() => {
    if (!entered || reducedMotion) return;
    const ctx = gsap.context(() => {
      gsap.fromTo(pillRef.current, { opacity: 0, y: 12, scale: 0.92 }, { opacity: 1, y: 0, scale: 1, duration: 0.3, ease: "back.out(1.6)" });
    });
    return () => ctx.revert();
  }, [entered, reducedMotion]);

  useEffect(() => {
    if (busy || reducedMotion || !mounted) return;
    const tween = gsap.to(pillRef.current, { opacity: 0, y: 8, scale: 0.94, duration: 0.2, ease: "power1.in", onComplete: () => setMounted(false) });
    return () => {
      tween.kill();
    };
  }, [busy, mounted, reducedMotion]);

  if (!mounted) return null;

  return (
    <div
      ref={pillRef}
      role="status"
      aria-live="polite"
      className="apx-glass-strong pointer-events-none fixed bottom-4 right-4 z-30 flex items-center gap-2 rounded-full border border-[var(--glass-border)] px-3 py-2 text-xs text-text-secondary shadow-lg"
    >
      <DotLoader frames={SPINNER_FRAMES} duration={90} dotClassName="bg-text-muted/25 [&.active]:bg-accent size-1" />
      <span>{t("busyIndicator.label")}</span>
    </div>
  );
}
