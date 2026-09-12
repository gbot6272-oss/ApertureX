import gsap from "gsap";
import { useEffect, useRef } from "react";

import { usePrefersReducedMotion } from "../../lib/motion";

const SPARK_COUNT = 6;

/**
 * Kleiner, selbst gebauter Funken-Ausbruch für einen echten
 * Abschluss-Moment (Phase 23, siehe DECISIONS.md ADR-0051) — die
 * leichte, GSAP-basierte Alternative zum vom Nutzer beigelegten
 * `SparklesCore`-Vorbild (`@tsparticles/*`, ein vollständiger
 * Partikel-Physik-Motor): sechs feste `<span>`-Punkte statt
 * hunderter simulierter Partikel, aber derselbe "beschwingter
 * Abschluss"-Effekt für einen einzelnen Erfolgs-Moment.
 *
 * `active` löst den Ausbruch nur beim Wechsel `false`→`true` aus
 * (nicht bei jedem Re-Render, solange `active` `true` bleibt) —
 * `usePrefersReducedMotion()` unterdrückt die Komponente komplett.
 */
export function SuccessSpark({ active }: { active: boolean }) {
  const containerRef = useRef<HTMLDivElement>(null);
  const wasActive = useRef(false);
  const prefersReducedMotion = usePrefersReducedMotion();

  useEffect(() => {
    if (active && !wasActive.current && containerRef.current) {
      const dots = Array.from(containerRef.current.children) as HTMLElement[];
      gsap.set(dots, { opacity: 1, x: 0, y: 0, scale: 1 });
      dots.forEach((dot, i) => {
        const angle = (i / dots.length) * Math.PI * 2;
        const distance = 16 + (i % 3) * 6;
        gsap.to(dot, {
          x: Math.cos(angle) * distance,
          y: Math.sin(angle) * distance,
          opacity: 0,
          scale: 0.4,
          duration: 0.55,
          ease: "power2.out",
        });
      });
    }
    wasActive.current = active;
  }, [active]);

  if (prefersReducedMotion) return null;

  return (
    <div ref={containerRef} className="pointer-events-none absolute left-1 top-1/2 -translate-y-1/2" aria-hidden="true">
      {Array.from({ length: SPARK_COUNT }).map((_, i) => (
        <span key={i} className="absolute left-0 top-0 h-1 w-1 rounded-full bg-accent opacity-0" />
      ))}
    </div>
  );
}
