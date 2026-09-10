import { useAppStore } from "../store";

/**
 * Bewegungs-Tokens für JS (Phase 18, ADR-0046) — dieselben Werte wie
 * `index.css`s `--duration-*`-Variablen bzw. Tailwinds eigene, bereits
 * vorhandene `--ease-*`-Standardwerte (`ease-in/-out/-in-out`), hier
 * als Zahlen/Strings dupliziert statt per `getComputedStyle`
 * ausgelesen — für einen einfachen `setTimeout`-Wert (siehe
 * `usePrefersReducedMotion` unten, bzw. jede künftige Mount-/Unmount-
 * Verzögerung in `components/ui/Dialog.tsx`/`Sheet.tsx`) wäre ein
 * DOM-Auslesen unnötiger Umweg. Bei einer künftigen Wertänderung
 * **beide Stellen** (hier und `index.css`) zusammen anpassen.
 */
export const DURATION_FAST_MS = 120;
export const DURATION_BASE_MS = 200;
export const DURATION_SLOW_MS = 320;

/** Deckt sich mit Tailwinds `--ease-out` (siehe
 * `node_modules/tailwindcss/theme.css`) — für `Element.animate()`/
 * inline-`transition-timing-function`-Aufrufe aus JS, wo eine
 * Tailwind-Klasse nicht infrage kommt. */
export const EASE_OUT = "cubic-bezier(0, 0, 0.2, 1)";
export const EASE_IN_OUT = "cubic-bezier(0.4, 0, 0.2, 1)";

/**
 * Liest `uiSettings.reduced_motion` direkt aus dem Store. Nötig
 * **zusätzlich** zu `index.css`s CSS-`!important`-Bremse
 * (`.apx-reduce-motion`/`prefers-reduced-motion`, siehe dortige
 * Moduldoku): die CSS-Bremse zwingt nur CSS-getriebene
 * `transition`/`animation`-Dauern auf nahezu Null — eine JS-getimte
 * Verzögerung (z. B. ein `setTimeout(DURATION_BASE_MS)` vor dem
 * Entfernen eines Dialogs aus dem DOM nach seiner Ausblend-Animation)
 * bemerkt davon nichts und müsste sonst trotz aktivierter reduzierter
 * Bewegung die volle Dauer abwarten. Jede JS-Animationslogik in dieser
 * Phase muss diesen Hook statt eines hartcodierten Werts nutzen.
 */
export function usePrefersReducedMotion(): boolean {
  return useAppStore((s) => s.uiSettings?.reduced_motion ?? false);
}
