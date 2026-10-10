import { useEffect, useRef } from "react";

import { registerConfettiSink } from "../lib/celebrate";
import {
  cannonBursts,
  particleOpacity,
  partyColors,
  spawnBurst,
  stepParticles,
  type ConfettiParticle,
} from "../lib/confetti";
import { usePrefersReducedMotion } from "../lib/motion";
import { useAppStore } from "../store";

/**
 * Die eine Konfetti-Leinwand der App (Geburtstagsausgabe, siehe
 * `DECISIONS.md` ADR-0071) — die Physik steht in `lib/confetti.ts`,
 * hier wird nur gezeichnet.
 *
 * **Eine Leinwand, eine Schleife, app-weit.** Dieselbe Entscheidung wie
 * bei `lib/sound.ts`s Player-Singleton: wer feiern will, ruft
 * `lib/celebrate.ts`s `celebrate`/`burstAt`/`fireCannons` auf und
 * braucht weder eine eigene Leinwand noch ein eigenes
 * `requestAnimationFrame`. Mehrere Leinwände würden sich um
 * Compositor-Schichten streiten, mehrere Schleifen würden das
 * Teilchenbudget je Schleife statt app-weit auslegen — und genau das
 * Aufaddieren ist, was eine Deko-Animation in einem Fotoeditor
 * gefährlich macht.
 *
 * **Die Schleife läuft nur, wenn etwas zu zeichnen ist.** Kein
 * Dauer-`rAF`: [`stepParticles`] gibt eine leere Liste zurück, sobald
 * der letzte Ausbruch verflogen ist, und dann endet die Schleife und
 * die Leinwand wird einmal geleert. Im Ruhezustand kostet diese
 * Komponente nichts außer einem leeren `<canvas>`.
 *
 * **Abschaltung an drei Stellen**, absichtlich redundant: die
 * Einstellung `birthday_decorations`, `reduced_motion`/
 * `prefers-reduced-motion` (dort auch per CSS, siehe `index.css`s
 * `.apx-confetti-layer`-Regel) und der Kontrastmodus. Eine
 * Zugänglichkeitseinstellung, die nur an einer Stelle greift, greift
 * irgendwann nicht mehr.
 */

export function ConfettiLayer() {
  const canvasRef = useRef<HTMLCanvasElement | null>(null);
  const particlesRef = useRef<ConfettiParticle[]>([]);
  const frameRef = useRef<number | null>(null);
  const lastRef = useRef<number>(0);
  const reducedMotion = usePrefersReducedMotion();
  const decorations = useAppStore((s) => s.uiSettings?.birthday_decorations ?? false);
  const enabled = decorations && !reducedMotion;

  useEffect(() => {
    if (!enabled) return;

    const canvas = canvasRef.current;
    if (!canvas) return;

    // Gerätepixel-Skalierung nach dem Muster, das `lib/leafletHeatmap.ts`
    // in dieser App schon vormacht — ohne sie ist die Leinwand auf
    // einem HiDPI-Bildschirm oder bei UI-Skalierung über 100 % unscharf.
    function resize() {
      const dpr = Math.max(1, window.devicePixelRatio || 1);
      canvas!.width = Math.floor(window.innerWidth * dpr);
      canvas!.height = Math.floor(window.innerHeight * dpr);
      const ctx = canvas!.getContext("2d");
      ctx?.setTransform(dpr, 0, 0, dpr, 0, 0);
    }
    resize();
    window.addEventListener("resize", resize);

    function draw() {
      const ctx = canvas!.getContext("2d");
      if (!ctx) return;
      ctx.clearRect(0, 0, window.innerWidth, window.innerHeight);
      for (const p of particlesRef.current) {
        ctx.save();
        ctx.globalAlpha = particleOpacity(p);
        ctx.translate(p.x, p.y);
        // Die Stauchung per `cos(spin)` ist, was aus einem Rechteck ein
        // flatterndes Plättchen macht: von der Seite gesehen wird es
        // kurz zu einem Strich und verschwindet fast.
        ctx.rotate(p.spin * 0.35);
        ctx.fillStyle = p.color;
        ctx.fillRect(-p.size * Math.abs(Math.cos(p.spin)), -p.size * 0.5, p.size * 2 * Math.abs(Math.cos(p.spin)), p.size);
        ctx.restore();
      }
    }

    function tick(now: number) {
      const dt = lastRef.current === 0 ? 0 : (now - lastRef.current) / 1000;
      lastRef.current = now;
      particlesRef.current = stepParticles(particlesRef.current, dt);
      draw();
      if (particlesRef.current.length > 0) {
        frameRef.current = window.requestAnimationFrame(tick);
      } else {
        // Letzter Durchlauf: Leinwand geleert, Schleife beendet. Ab
        // hier kostet diese Komponente nichts mehr.
        frameRef.current = null;
        lastRef.current = 0;
      }
    }

    function start() {
      if (frameRef.current !== null) return;
      lastRef.current = 0;
      frameRef.current = window.requestAnimationFrame(tick);
    }

    const unregister = registerConfettiSink((bursts) => {
      const colors = partyColors();
      // Leere Liste = Kanonen-Begrüßung (siehe `fireCannons`): die
      // Geometrie hängt an der Fenstergröße und gehört deshalb hierher,
      // nicht an die Aufrufstelle.
      const effective =
        bursts.length === 0 ? cannonBursts(window.innerWidth, window.innerHeight, colors) : bursts;
      for (const burst of effective) {
        particlesRef.current = particlesRef.current.concat(
          spawnBurst(
            { ...burst, colors: burst.colors.length > 0 ? burst.colors : colors },
            particlesRef.current.length,
          ),
        );
      }
      if (particlesRef.current.length > 0) start();
    });

    return () => {
      unregister();
      window.removeEventListener("resize", resize);
      if (frameRef.current !== null) window.cancelAnimationFrame(frameRef.current);
      frameRef.current = null;
      particlesRef.current = [];
    };
  }, [enabled]);

  if (!enabled) return null;

  return (
    <canvas
      ref={canvasRef}
      aria-hidden="true"
      data-testid="confetti-layer"
      className="apx-confetti-layer"
    />
  );
}
