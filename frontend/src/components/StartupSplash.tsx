import { useEffect, useRef, useState } from "react";
import gsap from "gsap";

import { usePrefersReducedMotion } from "../lib/motion";

interface StartupSplashProps {
  /** `true`, sobald die für den ersten sinnvollen Bildschirm nötigen
   * Daten geladen sind (siehe `App.tsx`: `uiSettings` + `catalogStatus`). */
  ready: boolean;
}

const RING_RADIUS = 26;
const RING_CIRCUMFERENCE = 2 * Math.PI * RING_RADIUS;

/**
 * Start-Ladeschirm (Phase 19, siehe `DECISIONS.md` ADR-0047) — ersetzt die
 * zuvor kommentarlos graue Fläche, die beim App-Start sichtbar war, bis
 * `refreshFolders`/`refreshCatalogStatus`/`loadUiSettings` (siehe
 * `App.tsx`) zurückkehrten. Bewusst als zusätzliches, rein dekoratives
 * Overlay (`pointer-events-none`, `aria-hidden`) statt als Gate, das den
 * Rest von `App.tsx` erst nach dem Laden mountet: der übrige Baum (Header,
 * Paletten, …) mountet unverändert sofort weiter — nur diese Fläche legt
 * sich kurz sichtbar darüber und blendet sich aus, sobald `ready` wahr
 * wird. Das hält das Risiko für die bestehende Test-Suite (deren
 * `getByRole`-Locators ohnehin automatisch abwarten) klein, während ein
 * echter, langsamer erster Start (großer Katalog) trotzdem einen
 * hochwertigen Übergang statt eines grauen Blitzes zeigt.
 *
 * Iris-Ring (SVG-Kreis, per `stroke-dashoffset` "gezeichnet") statt eines
 * Logo-Bilds — die App hat kein eigenes Vektor-Logo (nur Tauri-
 * Anwendungssymbole als PNG/ICO/ICNS, siehe `crates/apx-app/icons/`),
 * und ein sich öffnender Blendenring passt inhaltlich zum Namen
 * "Aperture X", ohne ein externes Asset zu benötigen (siehe
 * `DECISIONS.md` ADR-0047: Netzwerk-Policy dieser Umgebung blockiert
 * ohnehin jede externe Asset-Quelle außer GitHub/npm).
 */
export function StartupSplash({ ready }: StartupSplashProps) {
  const [mounted, setMounted] = useState(true);
  const containerRef = useRef<HTMLDivElement | null>(null);
  const ringRef = useRef<SVGCircleElement | null>(null);
  const wordmarkRef = useRef<HTMLSpanElement | null>(null);
  const reducedMotion = usePrefersReducedMotion();

  // Öffnen-Animation + sanfte Warte-Pulsation, einmalig beim Mount.
  useEffect(() => {
    if (!mounted) return;
    if (reducedMotion) return; // Statischer, vollständig gezeichneter Ring reicht.

    const ctx = gsap.context(() => {
      const tl = gsap.timeline();
      tl.fromTo(
        ringRef.current,
        { strokeDashoffset: RING_CIRCUMFERENCE, rotate: -90, transformOrigin: "50% 50%" },
        { strokeDashoffset: 0, duration: 0.8, ease: "power2.out" },
      );
      tl.fromTo(wordmarkRef.current, { opacity: 0, y: 8 }, { opacity: 1, y: 0, duration: 0.35, ease: "power1.out" }, "-=0.25");
      // Sanfte Warte-Pulsation (dieselbe Werteklasse wie die "Loading /
      // Skeleton"-Presets des `ui-ux-pro-max`-Skills: sine.inOut,
      // 1.2–1.6s-Loop) — läuft weiter, bis der Ausblend-Effekt unten sie
      // per `ctx.revert()` beendet.
      tl.to(ringRef.current, { scale: 1.04, transformOrigin: "50% 50%", duration: 1.4, ease: "sine.inOut", yoyo: true, repeat: -1 });
    }, containerRef);

    return () => ctx.revert();
  }, [mounted, reducedMotion]);

  // Ausblenden, sobald die App wirklich bereit ist.
  useEffect(() => {
    if (!ready || !mounted) return;

    if (reducedMotion) {
      setMounted(false);
      return;
    }

    const tween = gsap.to(containerRef.current, {
      opacity: 0,
      scale: 1.02,
      duration: 0.32,
      ease: "power1.inOut",
      onComplete: () => setMounted(false),
    });
    return () => {
      tween.kill();
    };
  }, [ready, mounted, reducedMotion]);

  if (!mounted) return null;

  return (
    <div
      ref={containerRef}
      aria-hidden="true"
      data-testid="startup-splash"
      className="apx-view-fade-in pointer-events-none fixed inset-0 z-[200] flex flex-col items-center justify-center gap-3 bg-bg-base"
    >
      <svg width="64" height="64" viewBox="0 0 64 64" className="text-accent">
        <circle cx="32" cy="32" r={RING_RADIUS} fill="none" stroke="currentColor" strokeOpacity={0.15} strokeWidth={4} />
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
          strokeDashoffset={reducedMotion ? 0 : RING_CIRCUMFERENCE}
          transform="rotate(-90 32 32)"
        />
      </svg>
      <span ref={wordmarkRef} className="text-sm font-semibold tracking-wide text-text-primary">
        Aperture X
      </span>
    </div>
  );
}
