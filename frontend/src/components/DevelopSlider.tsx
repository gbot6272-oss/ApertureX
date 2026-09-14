import { useEffect, useRef } from "react";
import type { ChangeEvent, CSSProperties, KeyboardEvent, PointerEvent } from "react";

import { applyArrowStep, clampSliderValue, type SliderSpec } from "../lib/edl";
import { playCue } from "../lib/sound";
import { useAppStore } from "../store";

interface DevelopSliderProps {
  spec: SliderSpec;
  value: number;
  /** Live-Zwischenwert beim Ziehen/Tippen — noch nicht dauerhaft
   * gespeichert. */
  onChange: (value: number) => void;
  /** Der aktuelle Wert soll dauerhaft gespeichert werden (Loslassen,
   * Doppelklick-Reset, Direkteingabe abgeschlossen) — siehe `SPEC.md` §4. */
  onCommit: () => void;
}

/**
 * Ein einzelner Entwickeln-Regler nach der in `SPEC.md` §4 vorgegebenen
 * Bedienkonvention: Doppelklick = Zurücksetzen, Direkteingabe über das
 * Zahlenfeld, Pfeiltasten = Feinschritt, Umschalt+Pfeiltasten =
 * Grobschritt. Eine gemeinsame Komponente statt sieben (bzw. acht,
 * Weißabgleich hat zwei Werte) einzelner Implementierungen.
 */
export function DevelopSlider({ spec, value, onChange, onCommit }: DevelopSliderProps) {
  const setDevelopLiveDragging = useAppStore((s) => s.setDevelopLiveDragging);

  // Der Wheel-Listener unten wird EINMAL gehängt (siehe dort). Damit er
  // trotzdem immer den aktuellen Wert und die aktuellen Rückrufe sieht,
  // laufen beide über Refs statt über die Abhängigkeitsliste.
  const valueRef = useRef(value);
  const onChangeRef = useRef(onChange);
  const onCommitRef = useRef(onCommit);
  valueRef.current = value;
  onChangeRef.current = onChange;
  onCommitRef.current = onCommit;

  function handleSliderChange(event: ChangeEvent<HTMLInputElement>) {
    onChange(Number(event.target.value));
  }

  // Verkleinert die Live-Vorschau-Auflösung in `Viewer.tsx`, solange
  // tatsächlich am Schieberegler gezogen wird (siehe `developIsLiveDragging`-
  // Moduldoku im Store, `DECISIONS.md` ADR-0048) — Klicks auf die Leiste
  // ohne Ziehen lösen `pointerdown` zwar auch aus, sind aber nur ein
  // einzelner Regler-Tick, kein Performance-Problem.
  function handlePointerDown(event: PointerEvent<HTMLInputElement>) {
    if (event.button !== 0) return;
    setDevelopLiveDragging(true);
  }

  // "release" statt "select" (Phase 20, siehe `DECISIONS.md` ADR-0048):
  // ein Regler, der losgelassen wird, ist kein Auswahl-Vorgang — `uisfx`
  // bietet mit "release" ("A pressed control springs back") den
  // semantisch passenden Cue. Zuvor hatte KEINER der ~40 Entwickeln-
  // Regler einen eigenen Sound, obwohl sie die mit Abstand am
  // häufigsten benutzten Bedienelemente der App sind — direkte Ursache
  // der Nutzer-Rückmeldung "es gibt maximal 3 Sounds".
  function handleCommit() {
    setDevelopLiveDragging(false);
    playCue("release");
    onCommit();
  }

  function handleDoubleClick() {
    onChange(spec.neutral);
    handleCommit();
  }

  function handleKeyDown(event: KeyboardEvent<HTMLInputElement>) {
    if (event.key === "ArrowRight" || event.key === "ArrowUp") {
      event.preventDefault();
      onChange(applyArrowStep(value, 1, spec, event.shiftKey));
    } else if (event.key === "ArrowLeft" || event.key === "ArrowDown") {
      event.preventDefault();
      onChange(applyArrowStep(value, -1, spec, event.shiftKey));
    }
  }

  function handleKeyUp(event: KeyboardEvent<HTMLInputElement>) {
    if (event.key.startsWith("Arrow")) {
      onCommit();
    }
  }

  // Mausrad über dem Regler ändert den Wert — die Bedienung, die jeder
  // andere Foto-Editor anbietet und die hier fehlte. `onWheel` allein
  // genügt nicht: React hängt Wheel-Listener passiv ein, `preventDefault`
  // greift dort nicht, und die Palette würde unter dem Zeiger
  // mitscrollen. Deshalb ein eigener, nicht-passiver Listener auf dem
  // Element.
  const sliderRef = useRef<HTMLInputElement>(null);
  const wheelCommitTimer = useRef<number | null>(null);
  useEffect(() => {
    const element = sliderRef.current;
    if (!element) return;

    function handleWheel(event: WheelEvent) {
      event.preventDefault();
      const direction = event.deltaY < 0 ? 1 : -1;
      // Umschalt = Grobschritt, wie schon bei den Pfeiltasten — eine
      // Bedienkonvention, zwei Eingabewege.
      onChangeRef.current(applyArrowStep(valueRef.current, direction, spec, event.shiftKey));

      // Erst wenn das Rad zur Ruhe kommt, wird gespeichert. Sonst
      // schriebe jede einzelne Rasterung einen eigenen Verlaufseintrag.
      if (wheelCommitTimer.current !== null) window.clearTimeout(wheelCommitTimer.current);
      wheelCommitTimer.current = window.setTimeout(() => {
        wheelCommitTimer.current = null;
        onCommitRef.current();
      }, 220);
    }

    element.addEventListener("wheel", handleWheel, { passive: false });
    return () => {
      element.removeEventListener("wheel", handleWheel);
      if (wheelCommitTimer.current !== null) window.clearTimeout(wheelCommitTimer.current);
    };
    // `spec` ist je Regler konstant; Wert und Rückrufe kommen über Refs
    // herein, damit der Listener nicht bei jedem Tick neu gehängt wird.
  }, [spec]);

  function handleNumberInput(event: ChangeEvent<HTMLInputElement>) {
    const parsed = Number(event.target.value);
    if (!Number.isNaN(parsed)) {
      onChange(clampSliderValue(parsed, spec));
    }
  }

  // "Bessere Regler" (Phase 22, siehe `DECISIONS.md` ADR-0050): der Regler
  // trug bis dahin gar keine eigene Optik — reines, unstilisiertes
  // Browser-`<input type="range">` (grauer Balken, runder Punkt), während
  // praktisch jede andere Fläche der App längst eigene Tokens/Liquid-
  // Glass-Optik bekommen hatte. `--range-progress` (per CSS-Eigenschaft
  // gesetzt, ausgewertet von `.apx-range` in `index.css`) füllt den
  // Balken bis zum aktuellen Wert farbig — dieselbe Berechnung, die auch
  // die Neutral-Markierung unten positioniert.
  const progressPercent = spec.max === spec.min ? 0 : ((value - spec.min) / (spec.max - spec.min)) * 100;
  const neutralPercent = spec.max === spec.min ? null : ((spec.neutral - spec.min) / (spec.max - spec.min)) * 100;
  const showNeutralTick = neutralPercent !== null && neutralPercent > 0.5 && neutralPercent < 99.5;

  // Phase 31 Schritt 2: eine Zeile statt zwei. Vorher standen
  // Beschriftung und Zahlenfeld in einer Zeile und der Regler in einer
  // zweiten darunter — zusammen rund 42px je Regler. Bei etwa vierzig
  // Reglern im Entwickeln-Panel ist das der Grund, warum man für die
  // Grundeinstellungen scrollen muss. Jetzt Beschriftung | Regler | Zahl
  // nebeneinander, rund 28px.
  //
  // Die Beschriftung wird abgeschnitten statt umzubrechen (ein Umbruch
  // machte die Zeile wieder hoch und damit die ganze Änderung zunichte);
  // `title` zeigt den vollen Text, und `aria-label` am Regler trägt ihn
  // ohnehin vollständig für Screenreader.
  return (
    <div className="grid grid-cols-[minmax(0,5.5rem)_1fr_auto] items-center gap-2">
      <span className="truncate text-xs text-text-secondary" title={spec.label}>
        {spec.label}
      </span>
      <div className="relative flex items-center">
        {/* Neutral-Markierung: zeigt, wohin ein Doppelklick zurücksetzt —
            nur wenn der Neutralwert nicht ohnehin an einem Rand liegt
            (dort wäre eine Markierung direkt auf dem Regler-Rand unnütz). */}
        {showNeutralTick && (
          <div
            aria-hidden="true"
            className="pointer-events-none absolute h-2.5 w-px -translate-x-1/2 bg-text-muted/50"
            style={{ left: `${neutralPercent}%` }}
          />
        )}
        <input
          type="range"
          aria-label={spec.label}
          className="apx-range w-full"
          style={{ "--range-progress": `${progressPercent}%` } as CSSProperties}
          min={spec.min}
          max={spec.max}
          step={spec.fineStep}
          value={value}
          onChange={handleSliderChange}
          onDoubleClick={handleDoubleClick}
          onKeyDown={handleKeyDown}
          onKeyUp={handleKeyUp}
          onPointerDown={handlePointerDown}
          onPointerUp={handleCommit}
          ref={sliderRef}
        />
      </div>
      <input
        type="number"
        aria-label={`${spec.label} (Zahlenwert)`}
        className="w-14 rounded border border-border bg-bg-base px-1 py-0.5 text-right text-xs text-text-primary"
        value={Math.round(value * 100) / 100}
        min={spec.min}
        max={spec.max}
        step={spec.fineStep}
        onChange={handleNumberInput}
        onBlur={onCommit}
      />
    </div>
  );
}
