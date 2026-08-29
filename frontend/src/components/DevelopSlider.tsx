import type { ChangeEvent, KeyboardEvent } from "react";

import { applyArrowStep, clampSliderValue, type SliderSpec } from "../lib/edl";

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
  function handleSliderChange(event: ChangeEvent<HTMLInputElement>) {
    onChange(Number(event.target.value));
  }

  function handleDoubleClick() {
    onChange(spec.neutral);
    onCommit();
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
      <input
        type="range"
        aria-label={spec.label}
        className="w-full"
        min={spec.min}
        max={spec.max}
        step={spec.fineStep}
        value={value}
        onChange={handleSliderChange}
        onDoubleClick={handleDoubleClick}
        onKeyDown={handleKeyDown}
        onKeyUp={handleKeyUp}
        onPointerUp={onCommit}
      />
    </div>
  );
}
