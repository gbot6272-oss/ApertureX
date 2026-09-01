import { useRef, useState } from "react";

import { hueSaturationToPixelOffset, pixelOffsetToHueSaturation } from "../lib/colorWheelMath";
import type { ColorGradingWheel } from "../lib/edl";
import { DevelopSlider } from "./DevelopSlider";

interface ColorWheelProps {
  wheel: ColorGradingWheel;
  onChange: (next: ColorGradingWheel) => void;
  onCommit: () => void;
  /** Beschriftet sowohl das Rad selbst als auch seinen Luminanz-Regler
   * (`Luminanz (${label})`) — die vier Räder sind gleichzeitig sichtbar,
   * ein bloßes "Luminanz" wäre mit den anderen Reglern gleichen Namens
   * mehrdeutig (siehe `DevelopPanel.tsx`s HSL-/Grundeinstellungs-Regler). */
  label: string;
}

const SIZE = 96;
const RADIUS = SIZE / 2;

function clamp(value: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, value));
}

/**
 * Ein einzelnes Farbrad für Color Grading (`SPEC.md` §3.2 „Color
 * Grading") — Winkel = Farbton (0° oben, im Uhrzeigersinn), Abstand vom
 * Mittelpunkt = Sättigung. Der feste konische Farbverlauf im Hintergrund
 * ist nur die Auswahlfläche, kein Live-Vorschau der gewählten Farbe
 * (wie bei vergleichbaren Werkzeugen in echten Fotoeditoren auch) — der
 * kleine Punkt markiert die aktuelle Auswahl.
 */
export function ColorWheel({ wheel, onChange, onCommit, label }: ColorWheelProps) {
  const wheelRef = useRef<HTMLDivElement>(null);
  const [dragging, setDragging] = useState(false);

  function pixelToHueSat(clientX: number, clientY: number): { hue_degrees: number; saturation: number } {
    const rect = wheelRef.current?.getBoundingClientRect();
    if (!rect) return { hue_degrees: wheel.hue_degrees, saturation: wheel.saturation };
    const cx = rect.left + rect.width / 2;
    const cy = rect.top + rect.height / 2;
    return pixelOffsetToHueSaturation(clientX - cx, clientY - cy, rect.width / 2);
  }

  function handlePointerDown(event: React.PointerEvent<HTMLDivElement>) {
    event.currentTarget.setPointerCapture(event.pointerId);
    setDragging(true);
    onChange({ ...wheel, ...pixelToHueSat(event.clientX, event.clientY) });
  }

  function handlePointerMove(event: React.PointerEvent<HTMLDivElement>) {
    if (!dragging) return;
    onChange({ ...wheel, ...pixelToHueSat(event.clientX, event.clientY) });
  }

  function handlePointerUp() {
    if (dragging) {
      setDragging(false);
      onCommit();
    }
  }

  function handleKeyDown(event: React.KeyboardEvent<HTMLDivElement>) {
    const hueStep = event.shiftKey ? 10 : 2;
    const satStep = (event.shiftKey ? 0.1 : 0.02);
    if (event.key === "ArrowRight") {
      event.preventDefault();
      onChange({ ...wheel, hue_degrees: (wheel.hue_degrees + hueStep + 360) % 360 });
    } else if (event.key === "ArrowLeft") {
      event.preventDefault();
      onChange({ ...wheel, hue_degrees: (wheel.hue_degrees - hueStep + 360) % 360 });
    } else if (event.key === "ArrowUp") {
      event.preventDefault();
      onChange({ ...wheel, saturation: clamp(wheel.saturation + satStep, 0, 1) });
    } else if (event.key === "ArrowDown") {
      event.preventDefault();
      onChange({ ...wheel, saturation: clamp(wheel.saturation - satStep, 0, 1) });
    }
  }

  function handleKeyUp(event: React.KeyboardEvent<HTMLDivElement>) {
    if (event.key.startsWith("Arrow")) onCommit();
  }

  const { dx, dy } = hueSaturationToPixelOffset(wheel.hue_degrees, wheel.saturation, RADIUS);
  const puckX = RADIUS + dx;
  const puckY = RADIUS + dy;

  return (
    <div className="flex flex-col items-center gap-1">
      <span className="text-xs text-text-secondary">{label}</span>
      <div
        ref={wheelRef}
        role="slider"
        tabIndex={0}
        aria-label={`${label}-Farbrad`}
        aria-valuenow={Math.round(wheel.hue_degrees)}
        onPointerDown={handlePointerDown}
        onPointerMove={handlePointerMove}
        onPointerUp={handlePointerUp}
        onKeyDown={handleKeyDown}
        onKeyUp={handleKeyUp}
        className="relative cursor-pointer rounded-full border border-border focus:outline focus:outline-2 focus:outline-accent"
        style={{
          width: SIZE,
          height: SIZE,
          background:
            "radial-gradient(circle, white 0%, transparent 70%), conic-gradient(from 0deg, hsl(0,100%,50%), hsl(60,100%,50%), hsl(120,100%,50%), hsl(180,100%,50%), hsl(240,100%,50%), hsl(300,100%,50%), hsl(360,100%,50%))",
        }}
      >
        <div
          className="pointer-events-none absolute h-2.5 w-2.5 -translate-x-1/2 -translate-y-1/2 rounded-full border border-white bg-black/50"
          style={{ left: puckX, top: puckY }}
        />
      </div>
      <DevelopSlider
        spec={{ key: "luminance", label: `Luminanz (${label})`, min: -100, max: 100, fineStep: 1, coarseStep: 10, neutral: 0 }}
        value={wheel.luminance}
        onChange={(value) => onChange({ ...wheel, luminance: value })}
        onCommit={onCommit}
      />
    </div>
  );
}
