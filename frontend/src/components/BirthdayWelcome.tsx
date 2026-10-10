import { useEffect, useRef, useState } from "react";
import gsap from "gsap";

import { fireCannons } from "../lib/celebrate";
import { usePrefersReducedMotion } from "../lib/motion";
import { playCue } from "../lib/sound";
import { useAppStore } from "../store";


/**
 * Die Begrüßung beim Öffnen (Geburtstagsausgabe, siehe `DECISIONS.md`
 * ADR-0071): geblurrter Hintergrund, große Gratulation, zwei
 * Konfettikanonen.
 *
 * **Warum das die Bedienung nicht blockiert.** Dieselbe Bauweise wie
 * `StartupSplash.tsx`, und zwar aus demselben hart erarbeiteten Grund:
 * die Fläche ist `pointer-events-none`, nur der Schließen-Knopf nimmt
 * Klicks an. Ein Vollbild-Overlay, das Zeigerereignisse abfängt, würde
 * in den ersten Sekunden jeden Klick schlucken — bei einer Begrüßung,
 * die bei *jedem* Start erscheint, wäre das kein Schmuck mehr, sondern
 * ein Fehler. Wer sofort anfangen will, klickt durch die Begrüßung
 * hindurch auf seinen Ordner, und sie geht von selbst.
 *
 * **Warum sie von selbst geht.** [`VISIBLE_MS`] ist lang genug für den
 * Moment und kurz genug, dass niemand wartet. Escape und der Knopf
 * schließen sofort; ein `pointerdown` irgendwo schließt ebenfalls (der
 * Klick selbst erreicht trotzdem sein Ziel, weil die Fläche keine
 * Ereignisse abfängt und der Lauscher am `window` hängt).
 *
 * **Der Hintergrund wird geblurrt, nicht verdeckt.** `backdrop-filter`
 * auf einer nur leicht abgedunkelten Fläche: man sieht die eigene
 * Bibliothek dahinter weich liegen, statt auf eine undurchsichtige
 * Platte zu schauen. Das ist der Unterschied zwischen "die App
 * gratuliert" und "ein Dialog ist im Weg".
 */

/** Wie lange die Begrüßung ohne Zutun stehen bleibt. */
const VISIBLE_MS = 5200;
/** Dauer des Ausblendens. */
const FADE_MS = 520;

interface BirthdayWelcomeProps {
  /** Erst zeigen, wenn die App geladen ist — sonst gratuliert sie vor
   * einem leeren Fenster, und der Start-Ladeschirm liegt ohnehin
   * darüber (`z-[200]` gegen `z-[160]` hier). */
  ready: boolean;
}

export function BirthdayWelcome({ ready }: BirthdayWelcomeProps) {
  const decorations = useAppStore((s) => s.uiSettings?.birthday_decorations ?? false);
  const reducedMotion = usePrefersReducedMotion();
  const [phase, setPhase] = useState<"waiting" | "shown" | "gone">("waiting");
  const containerRef = useRef<HTMLDivElement | null>(null);
  const titleRef = useRef<HTMLHeadingElement | null>(null);
  const subtitleRef = useRef<HTMLParagraphElement | null>(null);

  // Starten, sobald die App bereit ist. `phase` ist der Wächter gegen
  // ein zweites Erscheinen, wenn `uiSettings` später erneut durch den
  // Store läuft.
  useEffect(() => {
    if (!ready || !decorations || phase !== "waiting") return;
    setPhase("shown");
  }, [ready, decorations, phase]);

  // Einblenden, Kanonen, Sound.
  useEffect(() => {
    if (phase !== "shown") return;

    playCue("success");
    if (!reducedMotion) fireCannons();

    if (reducedMotion) return;
    const ctx = gsap.context(() => {
      const tl = gsap.timeline();
      tl.fromTo(containerRef.current, { opacity: 0 }, { opacity: 1, duration: 0.45, ease: "power2.out" }, 0);
      tl.fromTo(
        titleRef.current,
        { opacity: 0, scale: 0.86, y: 18 },
        // `back.out` gibt dem Text den kurzen Überschwung, der ihn wie
        // einen Zuruf wirken lässt statt wie eine Einblendung.
        { opacity: 1, scale: 1, y: 0, duration: 0.95, ease: "back.out(1.5)" },
        0.12,
      );
      tl.fromTo(subtitleRef.current, { opacity: 0, y: 12 }, { opacity: 1, y: 0, duration: 0.5, ease: "power1.out" }, "-=0.35");
    }, containerRef);
    return () => ctx.revert();
  }, [phase, reducedMotion]);

  // Automatisches Ausblenden.
  useEffect(() => {
    if (phase !== "shown") return;
    const timeout = window.setTimeout(() => dismiss(), VISIBLE_MS);
    return () => window.clearTimeout(timeout);
    // eslint-disable-next-line react-hooks/exhaustive-deps -- `dismiss` ist stabil (nur `setPhase`).
  }, [phase]);

  // Escape und ein Klick irgendwo schließen sofort. Beide Lauscher
  // hängen am `window`, nicht an der Fläche — die fängt bewusst keine
  // Ereignisse ab (siehe Moduldoku).
  useEffect(() => {
    if (phase !== "shown") return;
    function onKey(event: KeyboardEvent) {
      if (event.key === "Escape") dismiss();
    }
    function onPointer() {
      dismiss();
    }
    window.addEventListener("keydown", onKey);
    window.addEventListener("pointerdown", onPointer);
    return () => {
      window.removeEventListener("keydown", onKey);
      window.removeEventListener("pointerdown", onPointer);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps -- siehe oben.
  }, [phase]);

  function dismiss() {
    if (reducedMotion) {
      setPhase("gone");
      return;
    }
    gsap.to(containerRef.current, {
      opacity: 0,
      duration: FADE_MS / 1000,
      ease: "power1.inOut",
      onComplete: () => setPhase("gone"),
    });
  }

  if (phase !== "shown") return null;

  return (
    <div
      ref={containerRef}
      data-testid="birthday-welcome"
      // `pointer-events-none` ist hier keine Feinheit, sondern die
      // Zusage, dass diese Fläche niemandem im Weg steht (siehe
      // Moduldoku). Der Knopf unten holt sich die Ereignisse zurück.
      className="pointer-events-none fixed inset-0 z-[160] flex flex-col items-center justify-center gap-5 bg-bg-overlay px-6 text-center backdrop-blur-xl"
      style={{ opacity: reducedMotion ? 1 : 0 }}
      role="status"
      aria-live="polite"
    >
      <h1
        ref={titleRef}
        data-testid="birthday-welcome-title"
        className="apx-birthday-title max-w-[22ch] text-balance text-5xl font-bold leading-[1.05] tracking-tight sm:text-6xl md:text-7xl"
        style={{ opacity: reducedMotion ? 1 : 0 }}
      >
        Happy Birthday Papa
      </h1>
      <p
        ref={subtitleRef}
        className="max-w-[46ch] text-sm text-text-secondary sm:text-base"
        style={{ opacity: reducedMotion ? 1 : 0 }}
      >
        Diese Fassung von Aperture X ist für dich gebaut — mit drei neuen Looks, einer Einführung und deiner Kamera
        schon eingestellt.
      </p>
      <button
        type="button"
        onClick={dismiss}
        data-testid="birthday-welcome-dismiss"
        className="apx-btn-liquid pointer-events-auto rounded-full border border-accent/60 px-5 py-2 text-sm text-accent"
      >
        Los geht's
      </button>
    </div>
  );
}
