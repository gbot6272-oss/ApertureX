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

  return (
    <div className="flex flex-col gap-1">
      <div className="flex items-center justify-between text-xs text-text-secondary">
        <span>{spec.label}</span>
        <input
          type="number"
          aria-label={`${spec.label} (Zahlenwert)`}
          className="w-16 rounded border border-border bg-bg-base px-1 py-0.5 text-right text-text-primary"
          value={Math.round(value * 100) / 100}
          min={spec.min}
          max={spec.max}
          step={spec.fineStep}
          onChange={handleNumberInput}
          onBlur={onCommit}
        />
      </div>
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
        />
      </div>
    </div>
  );
}
