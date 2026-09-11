import { useEffect, useRef } from "react";
import gsap from "gsap";

import { usePrefersReducedMotion } from "../lib/motion";

interface ShutdownOverlayProps {
  visible: boolean;
}

const RING_RADIUS = 26;
const RING_CIRCUMFERENCE = 2 * Math.PI * RING_RADIUS;

/**
 * Beenden-Übergang (Phase 19, siehe `DECISIONS.md` ADR-0047 + `App.tsx`s
 * `useShutdownTransition`) — rein visuelles Gegenstück zu
 * `StartupSplash.tsx`: sobald das Fenster geschlossen werden soll, legt
 * sich diese Fläche kurz sichtbar über die App (statt eines ersatzlosen
 * Verschwindens), bevor das Fenster tatsächlich schließt. Anders als
 * `StartupSplash` hier bewusst NICHT `pointer-events-none` — in den
 * paar hundert Millisekunden vor dem echten Schließen soll nichts in der
 * App mehr bedienbar sein.
 */
export function ShutdownOverlay({ visible }: ShutdownOverlayProps) {
  const containerRef = useRef<HTMLDivElement | null>(null);
  const ringRef = useRef<SVGCircleElement | null>(null);
  const reducedMotion = usePrefersReducedMotion();

  useEffect(() => {
    if (!visible || reducedMotion) return;
    const ctx = gsap.context(() => {
      gsap.fromTo(containerRef.current, { opacity: 0 }, { opacity: 1, duration: 0.2, ease: "power1.out" });
      // Umgekehrte Iris-Bewegung zu `StartupSplash`: der Ring schließt
      // sich (`strokeDashoffset` läuft zurück auf den vollen Umfang).
      gsap.fromTo(
        ringRef.current,
        { strokeDashoffset: 0 },
        { strokeDashoffset: RING_CIRCUMFERENCE, duration: 0.4, ease: "power1.in", delay: 0.05 },
      );
    }, containerRef);
    return () => ctx.revert();
  }, [visible, reducedMotion]);

  if (!visible) return null;

  return (
    <div
      ref={containerRef}
      role="alert"
      aria-live="assertive"
      data-testid="shutdown-overlay"
      className="fixed inset-0 z-[300] flex flex-col items-center justify-center gap-3 bg-bg-base"
    >
      <svg width="64" height="64" viewBox="0 0 64 64" className="text-accent">
        <circle
          ref={ringRef}
          cx="32"
          cy="32"
          r={RING_RADIUS}
          fill="none"
          stroke="currentColor"
          strokeWidth={4}
          strokeLinecap="round"
          strokeDasharray={RING_CIRCUMFERENCE}
          strokeDashoffset={0}
          transform="rotate(-90 32 32)"
        />
      </svg>
      <span className="text-sm font-semibold tracking-wide text-text-primary">Aperture X wird beendet …</span>
    </div>
  );
}
