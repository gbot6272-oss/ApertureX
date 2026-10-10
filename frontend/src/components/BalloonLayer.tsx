import { usePrefersReducedMotion } from "../lib/motion";
import { useAppStore } from "../store";

/**
 * Luftballons im Hintergrund (Geburtstagsausgabe, siehe `DECISIONS.md`
 * ADR-0071).
 *
 * **Reines CSS, kein JavaScript pro Bild.** Sechs Elemente mit einer
 * `transform`-Keyframe-Animation (`index.css`s `apx-balloon-rise`) —
 * der Compositor bewegt sie ohne Layout- oder Zeichendurchlauf, der
 * Hauptthread merkt nichts davon. Dieselbe Entscheidung wie beim
 * Konfetti (eine Leinwand, eine Schleife): Deko darf in einem
 * Fotoeditor keine Rechenzeit kosten, die beim Dekodieren fehlt.
 *
 * **Sechs statt vieler.** Ein Dauerregen Luftballons wäre nach zwei
 * Minuten Arbeit unerträglich. Sechs, langsam, bei 22 % Deckkraft,
 * hinter allem — man sieht sie, wenn man hinschaut, und übersieht sie,
 * wenn man arbeitet.
 *
 * **Deckkraft und Lage sind Absicht:** `z-index: 0` und hinter dem
 * Inhalt, damit kein Ballon über einem Foto schwebt. Die Urteilsfläche
 * bleibt frei (dieselbe Überlegung wie bei `--color-bg-base` im
 * Theme-Block).
 */

/** Startposition (in Prozent der Breite), Verzögerung und Dauer je
 * Ballon. Von Hand verteilt statt zufällig: ein `Math.random()` beim
 * Mount würde bei jedem Start anders aussehen und bei jedem Re-Render
 * springen, und gleichmäßig verteilte Zufallswerte neigen zu
 * Häufchenbildung. */
const BALLOONS: ReadonlyArray<{ left: number; delay: number; duration: number; color: string; scale: number }> = [
  { left: 8, delay: 0, duration: 26, color: "var(--party-1)", scale: 1 },
  { left: 23, delay: 7, duration: 32, color: "var(--party-2)", scale: 0.8 },
  { left: 41, delay: 14, duration: 29, color: "var(--party-3)", scale: 1.15 },
  { left: 62, delay: 3, duration: 34, color: "var(--party-4)", scale: 0.9 },
  { left: 78, delay: 18, duration: 27, color: "var(--party-5)", scale: 1.05 },
  { left: 91, delay: 10, duration: 31, color: "var(--party-2)", scale: 0.75 },
];

export function BalloonLayer() {
  const decorations = useAppStore((s) => s.uiSettings?.birthday_decorations ?? false);
  const reducedMotion = usePrefersReducedMotion();
  if (!decorations || reducedMotion) return null;

  return (
    <div className="apx-balloon-layer" aria-hidden="true" data-testid="balloon-layer">
      {BALLOONS.map((balloon, index) => (
        <span
          key={index}
          className="apx-balloon"
          style={{
            left: `${balloon.left}%`,
            color: balloon.color,
            background: balloon.color,
            animationDelay: `-${balloon.delay}s`,
            animationDuration: `${balloon.duration}s`,
            scale: String(balloon.scale),
          }}
        />
      ))}
    </div>
  );
}
