import { type ComponentProps, useCallback, useEffect, useRef } from "react";

import { usePrefersReducedMotion } from "../../lib/motion";
import { cn } from "../../lib/utils";

/**
 * Animierter Punktraster-Loader (Phase 23, siehe DECISIONS.md
 * ADR-0051) — fast wörtlich portiert aus einem vom Nutzer beigelegten
 * Komponenten-Vorbild (dependency-frei: reines React + CSS, keine
 * neue Laufzeitabhängigkeit nötig). Einzige Anpassung gegenüber dem
 * Original: `usePrefersReducedMotion()` erzwingt `isPlaying=false`
 * (nur das erste Bild wird statisch gezeigt) statt jede
 * Aufrufstelle einzeln dafür verantwortlich zu machen — dieselbe
 * durchgehende Barrierefreiheits-Pflicht wie bei jeder anderen
 * Bewegung in dieser App (siehe ADR-0046).
 */
type DotLoaderProps = {
  frames: number[][];
  dotClassName?: string;
  isPlaying?: boolean;
  duration?: number;
  repeatCount?: number;
  onComplete?: () => void;
} & ComponentProps<"div">;

export const DotLoader = ({
  frames,
  isPlaying = true,
  duration = 100,
  dotClassName,
  className,
  repeatCount = -1,
  onComplete,
  ...props
}: DotLoaderProps) => {
  const prefersReducedMotion = usePrefersReducedMotion();
  const effectivelyPlaying = isPlaying && !prefersReducedMotion;
  const gridRef = useRef<HTMLDivElement>(null);
  const currentIndex = useRef(0);
  const repeats = useRef(0);
  const interval = useRef<ReturnType<typeof setInterval> | null>(null);

  const applyFrameToDots = useCallback(
    (dots: HTMLDivElement[], frameIndex: number) => {
      const frame = frames[frameIndex];
      if (!frame) return;

      dots.forEach((dot, index) => {
        dot.classList.toggle("active", frame.includes(index));
      });
    },
    [frames],
  );

  useEffect(() => {
    currentIndex.current = 0;
    repeats.current = 0;
  }, [frames]);

  useEffect(() => {
    const dotElements = gridRef.current?.children;
    if (!dotElements) return;
    const dots = Array.from(dotElements) as HTMLDivElement[];

    if (effectivelyPlaying) {
      if (currentIndex.current >= frames.length) {
        currentIndex.current = 0;
      }
      interval.current = setInterval(() => {
        applyFrameToDots(dots, currentIndex.current);
        if (currentIndex.current + 1 >= frames.length) {
          if (repeatCount !== -1 && repeats.current + 1 >= repeatCount) {
            if (interval.current) clearInterval(interval.current);
            onComplete?.();
          }
          repeats.current++;
        }
        currentIndex.current = (currentIndex.current + 1) % frames.length;
      }, duration);
    } else {
      if (interval.current) clearInterval(interval.current);
      // Reduzierte Bewegung / `isPlaying=false`: erstes Bild statisch
      // zeigen statt einer leeren Punktfläche.
      applyFrameToDots(dots, 0);
    }

    return () => {
      if (interval.current) clearInterval(interval.current);
    };
  }, [frames, effectivelyPlaying, applyFrameToDots, duration, repeatCount, onComplete]);

  return (
    <div {...props} ref={gridRef} className={cn("grid w-fit grid-cols-7 gap-0.5", className)}>
      {Array.from({ length: 49 }).map((_, i) => (
        <div key={i} className={cn("h-1.5 w-1.5 rounded-sm", dotClassName)} />
      ))}
    </div>
  );
};

/**
 * Ein kleines "wanderndes Punktpaar", das im 3×3-Ausschnitt (Zeilen/
 * Spalten 2–4) des 7×7-Rasters einmal rundherum läuft — die einzige
 * Bild-Sequenz, die diese App bisher braucht (kompakter Kreislauf statt
 * des Spielfeld-Musters aus dem Vorbild), daher hier zentral exportiert
 * statt an jeder Verwendungsstelle neu berechnet (`GlobalBusyIndicator.tsx`,
 * `InlineSpinner` unten).
 */
const SPINNER_RING = [2, 3, 4, 11, 18, 17, 16, 9];
export const RING_SPINNER_FRAMES: number[][] = SPINNER_RING.map((_, i) => [SPINNER_RING[i]!, SPINNER_RING[(i + 1) % SPINNER_RING.length]!]);

/**
 * Winziger Inline-Spinner für einen Knopf-/Link-Text während einer
 * laufenden Operation (Phase 23 Nachtrag, siehe DECISIONS.md
 * ADR-0051-Nachtrag) — z. B. `{loading ? <><InlineSpinner /> Berechnet…</> : "Anwenden"}`.
 * Rendert reine `<div>`-Elemente ohne Textinhalt, verändert daher den
 * per Text berechneten zugänglichen Namen des umschließenden Knopfs
 * nicht (wichtig für die bestehenden Playwright-`getByRole`-Selektoren).
 */
export function InlineSpinner({ className }: { className?: string }) {
  return (
    <DotLoader
      aria-hidden="true"
      frames={RING_SPINNER_FRAMES}
      duration={90}
      className={cn("inline-grid align-middle", className)}
      dotClassName="bg-current/20 [&.active]:bg-current size-0.5"
    />
  );
}
