import { useEffect, useRef, useState } from "react";
import gsap from "gsap";

import { usePrefersReducedMotion } from "../lib/motion";

interface StartupSplashProps {
  /** `true`, sobald die für den ersten sinnvollen Bildschirm nötigen
   * Daten geladen sind (siehe `App.tsx`: `uiSettings` + `catalogStatus`). */
  ready: boolean;
}

const RING_RADIUS = 40;
const RING_CIRCUMFERENCE = 2 * Math.PI * RING_RADIUS;
const INNER_RING_RADIUS = 30;
const INNER_RING_CIRCUMFERENCE = 2 * Math.PI * INNER_RING_RADIUS;

// Mindestanzeigedauer (Phase 20, siehe `DECISIONS.md` ADR-0048): ein
// bereits beim Start vorliegender/schnell ladender Katalog machte
// `ready` zuvor teils innerhalb weniger hundert Millisekunden wahr —
// der Splash blitzte dann nur kurz auf ("ein Rad, das sich 200 ms
// dreht", Nutzer-Rückmeldung), statt als bewusster Start-Moment
// wahrgenommen zu werden. Unabhängig davon, wie schnell `ready` wird,
// bleibt der Splash mindestens so lange sichtbar, dass die
// Öffnen-Animation vollständig abspielen kann, bevor er sich ausblendet.
const MIN_VISIBLE_MS = 900;

/**
 * Start-Ladeschirm (Phase 19, siehe `DECISIONS.md` ADR-0047; deutlich
 * verstärkt in Phase 20, ADR-0048) — ersetzt die zuvor kommentarlos graue
 * Fläche, die beim App-Start sichtbar war, bis
 * `refreshFolders`/`refreshCatalogStatus`/`loadUiSettings` (siehe
 * `App.tsx`) zurückkehrten. Bewusst als zusätzliches, rein dekoratives
 * Overlay (`pointer-events-none`, `aria-hidden`) statt als Gate, das den
 * Rest von `App.tsx` erst nach dem Laden mountet: der übrige Baum (Header,
 * Paletten, …) mountet unverändert sofort weiter — nur diese Fläche legt
 * sich kurz sichtbar darüber und blendet sich aus, sobald `ready` wahr
 * wird UND die Mindestanzeigedauer (`MIN_VISIBLE_MS`) verstrichen ist.
 * Das hält das Risiko für die bestehende Test-Suite (deren
 * `getByRole`-Locators ohnehin automatisch abwarten) klein, während ein
 * echter, langsamer erster Start (großer Katalog) trotzdem einen
 * hochwertigen Übergang statt eines grauen Blitzes zeigt.
 *
 * Doppelter Iris-Ring (zwei SVG-Kreise, gegenläufig gezeichnet/rotierend)
 * statt eines Logo-Bilds — die App hat kein eigenes Vektor-Logo (nur
 * Tauri-Anwendungssymbole als PNG/ICO/ICNS, siehe `crates/apx-app/icons/`),
 * und ein sich öffnender, doppelter Blendenring passt inhaltlich zum
 * Namen "Aperture X", ohne ein externes Asset zu benötigen (siehe
 * `DECISIONS.md` ADR-0047: Netzwerk-Policy dieser Umgebung blockiert
 * ohnehin jede externe Asset-Quelle außer GitHub/npm).
 */
export function StartupSplash({ ready }: StartupSplashProps) {
  const [mounted, setMounted] = useState(true);
  const containerRef = useRef<HTMLDivElement | null>(null);
  const ringRef = useRef<SVGCircleElement | null>(null);
  const innerRingRef = useRef<SVGCircleElement | null>(null);
  const glowRef = useRef<HTMLDivElement | null>(null);
  const wordmarkRef = useRef<HTMLSpanElement | null>(null);
  const taglineRef = useRef<HTMLSpanElement | null>(null);
  const reducedMotion = usePrefersReducedMotion();
  const mountedAtRef = useRef(performance.now());

  // Öffnen-Animation + sanfte Warte-Rotation, einmalig beim Mount.
  useEffect(() => {
    if (!mounted) return;
    if (reducedMotion) return; // Statischer, vollständig gezeichneter Ring reicht.

    const ctx = gsap.context(() => {
      const tl = gsap.timeline();
      tl.fromTo(glowRef.current, { opacity: 0, scale: 0.85 }, { opacity: 1, scale: 1, duration: 0.7, ease: "power2.out" }, 0);
      tl.fromTo(
        ringRef.current,
        { strokeDashoffset: RING_CIRCUMFERENCE, rotate: -90, transformOrigin: "50% 50%" },
        { strokeDashoffset: 0, duration: 0.85, ease: "power2.out" },
        0,
      );
      tl.fromTo(
        innerRingRef.current,
        { strokeDashoffset: -INNER_RING_CIRCUMFERENCE, rotate: 90, transformOrigin: "50% 50%" },
        { strokeDashoffset: 0, duration: 0.85, ease: "power2.out" },
        0.1,
      );
      tl.fromTo(wordmarkRef.current, { opacity: 0, y: 10, letterSpacing: "0.02em" }, { opacity: 1, y: 0, letterSpacing: "0.08em", duration: 0.45, ease: "power1.out" }, "-=0.35");
      tl.fromTo(taglineRef.current, { opacity: 0, y: 6 }, { opacity: 1, y: 0, duration: 0.4, ease: "power1.out" }, "-=0.15");
      // Sanfte Warte-Rotation (dieselbe Werteklasse wie die "Loading /
      // Skeleton"-Presets des `ui-ux-pro-max`-Skills): der äußere Ring
      // dreht sich langsam im, der innere gegen den Uhrzeigersinn — ein
      // spürbarer, aber ruhiger "arbeitet gerade"-Eindruck statt eines
      // starren Bilds. Läuft weiter, bis der Ausblend-Effekt unten sie
      // per `ctx.revert()` beendet.
      tl.to(ringRef.current, { rotation: "+=360", transformOrigin: "50% 50%", duration: 8, ease: "none", repeat: -1 }, "-=0.1");
      tl.to(innerRingRef.current, { rotation: "-=360", transformOrigin: "50% 50%", duration: 6, ease: "none", repeat: -1 }, "<");
    }, containerRef);

    return () => ctx.revert();
  }, [mounted, reducedMotion]);

  // Ausblenden, sobald die App wirklich bereit ist UND die
  // Mindestanzeigedauer verstrichen ist (siehe `MIN_VISIBLE_MS`-Moduldoku
  // oben).
  useEffect(() => {
    if (!ready || !mounted) return;

    const elapsed = performance.now() - mountedAtRef.current;
    const remaining = Math.max(0, MIN_VISIBLE_MS - elapsed);

    const timeoutId = window.setTimeout(() => {
      if (reducedMotion) {
        setMounted(false);
        return;
      }
      gsap.to(containerRef.current, {
        opacity: 0,
        scale: 1.03,
        duration: 0.35,
        ease: "power1.inOut",
        onComplete: () => setMounted(false),
      });
    }, remaining);

    return () => window.clearTimeout(timeoutId);
  }, [ready, mounted, reducedMotion]);

  if (!mounted) return null;

  return (
    <div
      ref={containerRef}
      aria-hidden="true"
      data-testid="startup-splash"
      className="apx-view-fade-in pointer-events-none fixed inset-0 z-[200] flex flex-col items-center justify-center gap-4 bg-bg-base"
    >
      <div className="relative flex items-center justify-center">
        <div
          ref={glowRef}
          className="absolute h-32 w-32 rounded-full bg-accent/20 blur-2xl"
          style={{ opacity: reducedMotion ? 1 : 0 }}
        />
        <svg width="96" height="96" viewBox="0 0 96 96" className="relative text-accent">
          <circle cx="48" cy="48" r={RING_RADIUS} fill="none" stroke="currentColor" strokeOpacity={0.12} strokeWidth={5} />
          <circle
            ref={ringRef}
            cx="48"
            cy="48"
            r={RING_RADIUS}
            fill="none"
            stroke="currentColor"
            strokeWidth={5}
            strokeLinecap="round"
            strokeDasharray={RING_CIRCUMFERENCE}
            strokeDashoffset={reducedMotion ? 0 : RING_CIRCUMFERENCE}
            transform="rotate(-90 48 48)"
          />
          <circle cx="48" cy="48" r={INNER_RING_RADIUS} fill="none" stroke="currentColor" strokeOpacity={0.1} strokeWidth={3} />
          <circle
            ref={innerRingRef}
            cx="48"
            cy="48"
            r={INNER_RING_RADIUS}
            fill="none"
            stroke="currentColor"
            strokeOpacity={0.65}
            strokeWidth={3}
            strokeLinecap="round"
            strokeDasharray={`${INNER_RING_CIRCUMFERENCE * 0.4} ${INNER_RING_CIRCUMFERENCE}`}
            strokeDashoffset={reducedMotion ? 0 : -INNER_RING_CIRCUMFERENCE}
            transform="rotate(90 48 48)"
          />
        </svg>
      </div>
      <div className="flex flex-col items-center gap-1">
        <span ref={wordmarkRef} className="text-xl font-semibold tracking-[0.08em] text-text-primary">
          Aperture X
        </span>
        <span ref={taglineRef} className="text-xs text-text-muted">
          Wird geladen …
        </span>
      </div>
    </div>
  );
}
