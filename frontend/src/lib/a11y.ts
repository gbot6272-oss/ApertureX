import { useEffect, useRef } from "react";
import type { RefObject } from "react";

/**
 * Fokus-Falle für modale Dialoge (Phase 10 Schritt 6, siehe
 * `FEATURES.md` Barrierefreiheit — Tastaturbedienbarkeit). Solange
 * `active`, bleibt Tab/Shift+Tab innerhalb des Containers gefangen und der
 * Fokus springt beim Öffnen auf das erste fokussierbare Element; beim
 * Schließen kehrt der Fokus zum zuvor fokussierten Element zurück.
 *
 * **Bewusst als Stichprobe eingeführt statt in einem Rutsch auf alle
 * Dialoge ausgerollt**: `SettingsDialog.tsx`/`KeybindingsCheatsheet.tsx`
 * (beide neu in dieser Phase, keine bestehenden e2e-Tests) etablieren das
 * Muster; die ältesten, am dichtesten mit e2e-Tests belegten Dialoge
 * (Export/Print/Slideshow/Book/Web und die Phase-9-Dialoge) bleiben in
 * diesem Schritt unverändert — ihre Umstellung ohne begleitenden Test
 * dieser Phase (siehe ADR-0037) wäre ein Regressionsrisiko ohne
 * Frühwarnung vor der einmaligen vollen Suite in Schritt 12.
 */
export function useFocusTrap(containerRef: RefObject<HTMLElement | null>, active: boolean): void {
  const previouslyFocused = useRef<HTMLElement | null>(null);

  useEffect(() => {
    if (!active) return;
    const container = containerRef.current;
    if (!container) return;

    previouslyFocused.current = document.activeElement as HTMLElement | null;

    function focusable(): HTMLElement[] {
      if (!container) return [];
      return Array.from(container.querySelectorAll<HTMLElement>('a[href], button:not([disabled]), input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])')).filter(
        (el) => el.offsetParent !== null,
      );
    }

    const first = focusable()[0];
    first?.focus();

    function onKeyDown(event: KeyboardEvent) {
      if (event.key !== "Tab") return;
      const items = focusable();
      if (items.length === 0) return;
      const currentIndex = items.indexOf(document.activeElement as HTMLElement);
      if (event.shiftKey) {
        if (currentIndex <= 0) {
          event.preventDefault();
          items[items.length - 1]?.focus();
        }
      } else {
        if (currentIndex === items.length - 1 || currentIndex === -1) {
          event.preventDefault();
          items[0]?.focus();
        }
      }
    }

    container.addEventListener("keydown", onKeyDown);
    return () => {
      container.removeEventListener("keydown", onKeyDown);
      previouslyFocused.current?.focus();
    };
  }, [active, containerRef]);
}
