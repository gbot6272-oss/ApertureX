import { useMemo, useRef, useState } from "react";

import { evaluateCurveChannel } from "../lib/curveMath";
import { CURVE_PRESETS, identityCurve, PARAMETRIC_CURVE_SLIDER_SPECS, type CurveChannel, type CurvePoint } from "../lib/edl";
import { DevelopSlider } from "./DevelopSlider";

interface CurveEditorProps {
  channel: CurveChannel;
  onChange: (next: CurveChannel) => void;
  onCommit: () => void;
}

const SIZE = 160; // SVG-Einheiten = CSS-Pixel (kein viewBox-Skalierungsfaktor nötig)
const SAMPLE_COUNT = 40;
/** Mindestabstand zwischen benachbarten Punkten auf der Eingabe-Achse —
 * verhindert, dass zwei Punkte exakt aufeinanderfallen (bricht sonst die
 * Monotonie-Annahme der Spline). */
const MIN_INPUT_GAP = 0.02;

function clamp(value: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, value));
}

/**
 * Editor für eine einzelne Gradationskurve (`SPEC.md` §3.2 „Kurven") —
 * Punktkurve mit ziehbaren Kontrollpunkten (SVG statt `<canvas>`: echte,
 * fokussierbare Elemente pro Punkt machen Tastaturbedienung und
 * Playwright-Tests deutlich einfacher, bei identischem visuellem
 * Ergebnis) oder parametrische Kurve (vier Regler, wiederverwendet
 * `DevelopSlider`). Ein Wechsel zwischen beiden Modi setzt den Kanal auf
 * neutral zurück statt zu versuchen, eine Punktkurve in Regler-Werte
 * umzurechnen oder umgekehrt (bewusste Vereinfachung).
 */
export function CurveEditor({ channel, onChange, onCommit }: CurveEditorProps) {
  const svgRef = useRef<SVGSVGElement>(null);
  const [draggingIndex, setDraggingIndex] = useState<number | null>(null);
  /** Für die numerische Punkteingabe (`SPEC.md` §3.2: „frei setzbare
   * Punkte mit numerischer Eingabe") — welcher Punkt zuletzt fokussiert/
   * angeklickt wurde. Reines Anzeige-Detail, kein Teil des EDL. */
  const [selectedIndex, setSelectedIndex] = useState<number | null>(null);

  const pathD = useMemo(() => {
    const segments: string[] = [];
    for (let i = 0; i <= SAMPLE_COUNT; i++) {
      const x = i / SAMPLE_COUNT;
      const y = evaluateCurveChannel(channel, x);
      segments.push(`${i === 0 ? "M" : "L"} ${(x * SIZE).toFixed(2)} ${((1 - y) * SIZE).toFixed(2)}`);
    }
    return segments.join(" ");
  }, [channel]);

  function updatePoint(index: number, newInput: number, newOutput: number) {
    if (channel.kind !== "Points") return;
    const newPoints = channel.points.map((p, i) => (i === index ? { input: newInput, output: newOutput } : p));
    onChange({ kind: "Points", points: newPoints });
  }

  function pixelToUnit(clientX: number, clientY: number): { x: number; y: number } {
    const rect = svgRef.current?.getBoundingClientRect();
    if (!rect) return { x: 0, y: 0 };
    return {
      x: clamp((clientX - rect.left) / SIZE, 0, 1),
      y: clamp(1 - (clientY - rect.top) / SIZE, 0, 1),
    };
  }

  function handlePointPointerDown(index: number) {
    return (event: React.PointerEvent<SVGCircleElement>) => {
      event.currentTarget.setPointerCapture(event.pointerId);
      setDraggingIndex(index);
      setSelectedIndex(index);
    };
  }

  function handlePointPointerMove(index: number) {
    return (event: React.PointerEvent<SVGCircleElement>) => {
      if (draggingIndex !== index || channel.kind !== "Points") return;
      const { x, y } = pixelToUnit(event.clientX, event.clientY);
      const points = channel.points;
      const isEndpoint = index === 0 || index === points.length - 1;
      const point = points[index];
      if (!point) return;
      if (isEndpoint) {
        updatePoint(index, point.input, y);
        return;
      }
      const prev = points[index - 1];
      const next = points[index + 1];
      const minInput = prev ? prev.input + MIN_INPUT_GAP : 0;
      const maxInput = next ? next.input - MIN_INPUT_GAP : 1;
      updatePoint(index, clamp(x, minInput, maxInput), y);
    };
  }

  function handlePointPointerUp(index: number) {
    return () => {
      if (draggingIndex === index) {
        setDraggingIndex(null);
        onCommit();
      }
    };
  }

  function handlePointKeyDown(index: number) {
    return (event: React.KeyboardEvent<SVGCircleElement>) => {
      if (channel.kind !== "Points") return;
      const points = channel.points;
      const point = points[index];
      if (!point) return;
      const isEndpoint = index === 0 || index === points.length - 1;
      const step = event.shiftKey ? 0.05 : 0.01;

      if (event.key === "ArrowUp") {
        event.preventDefault();
        updatePoint(index, point.input, clamp(point.output + step, 0, 1));
      } else if (event.key === "ArrowDown") {
        event.preventDefault();
        updatePoint(index, point.input, clamp(point.output - step, 0, 1));
      } else if (event.key === "ArrowRight" && !isEndpoint) {
        event.preventDefault();
        const next = points[index + 1];
        const maxInput = next ? next.input - MIN_INPUT_GAP : 1;
        updatePoint(index, clamp(point.input + step, 0, maxInput), point.output);
      } else if (event.key === "ArrowLeft" && !isEndpoint) {
        event.preventDefault();
        const prev = points[index - 1];
        const minInput = prev ? prev.input + MIN_INPUT_GAP : 0;
        updatePoint(index, clamp(point.input - step, minInput, 1), point.output);
      } else if ((event.key === "Delete" || event.key === "Backspace") && !isEndpoint) {
        event.preventDefault();
        onChange({ kind: "Points", points: points.filter((_, i) => i !== index) });
        onCommit();
        setSelectedIndex(null);
      }
    };
  }

  function handlePointKeyUp(event: React.KeyboardEvent<SVGCircleElement>) {
    if (event.key.startsWith("Arrow")) onCommit();
  }

  function handleBackgroundClick(event: React.MouseEvent<SVGRectElement>) {
    if (channel.kind !== "Points" || draggingIndex !== null) return;
    const { x, y } = pixelToUnit(event.clientX, event.clientY);
    const newPoint: CurvePoint = { input: x, output: y };
    const newPoints = [...channel.points, newPoint].sort((a, b) => a.input - b.input);
    onChange({ kind: "Points", points: newPoints });
    onCommit();
    setSelectedIndex(newPoints.indexOf(newPoint));
  }

  const selectedPoint = channel.kind === "Points" && selectedIndex !== null ? channel.points[selectedIndex] : undefined;

  function applyPreset(key: string) {
    const preset = CURVE_PRESETS.find((p) => p.key === key);
    if (!preset) return;
    onChange({ kind: "Points", points: preset.points.map((p) => ({ ...p })) });
    onCommit();
  }

  return (
    <div className="flex flex-col gap-2">
      <div className="flex items-center gap-2">
        <button
          type="button"
          onClick={() => {
            if (channel.kind !== "Points") {
              onChange(identityCurve());
              onCommit();
            }
          }}
          aria-pressed={channel.kind === "Points"}
          className={`rounded border px-2 py-1 text-xs ${
            channel.kind === "Points" ? "border-accent bg-accent/10 text-accent" : "border-border bg-bg-panel hover:border-accent"
          }`}
        >
          Punkte
        </button>
        <button
          type="button"
          onClick={() => {
            if (channel.kind !== "Parametric") {
              onChange({ kind: "Parametric", shadows: 0, darks: 0, lights: 0, highlights: 0 });
              onCommit();
            }
          }}
          aria-pressed={channel.kind === "Parametric"}
          className={`rounded border px-2 py-1 text-xs ${
            channel.kind === "Parametric" ? "border-accent bg-accent/10 text-accent" : "border-border bg-bg-panel hover:border-accent"
          }`}
        >
          Parametrisch
        </button>
        <select
          aria-label="Kurven-Preset"
          defaultValue=""
          onChange={(event) => {
            if (event.target.value) applyPreset(event.target.value);
            event.target.value = "";
          }}
          className="ml-auto rounded border border-border bg-bg-panel px-2 py-1 text-xs"
        >
          <option value="" disabled>
            Preset…
          </option>
          {CURVE_PRESETS.map((preset) => (
            <option key={preset.key} value={preset.key}>
              {preset.label}
            </option>
          ))}
        </select>
      </div>

      <svg ref={svgRef} width={SIZE} height={SIZE} className="rounded border border-border bg-bg-base" role="img" aria-label="Kurven-Diagramm">
        {[0.25, 0.5, 0.75].map((fraction) => (
          <g key={fraction} className="text-border" stroke="currentColor" strokeWidth={0.5}>
            <line x1={fraction * SIZE} y1={0} x2={fraction * SIZE} y2={SIZE} />
            <line x1={0} y1={fraction * SIZE} x2={SIZE} y2={fraction * SIZE} />
          </g>
        ))}
        <line x1={0} y1={SIZE} x2={SIZE} y2={0} className="text-text-muted" stroke="currentColor" strokeWidth={0.5} strokeDasharray="2,2" />

        {channel.kind === "Points" && <rect x={0} y={0} width={SIZE} height={SIZE} fill="transparent" onClick={handleBackgroundClick} />}

        <path d={pathD} fill="none" className="text-accent" stroke="currentColor" strokeWidth={1.5} />

        {channel.kind === "Points" &&
          channel.points.map((point, index) => (
            <circle
              // eslint-disable-next-line react/no-array-index-key -- Punkte haben keine stabile ID, der Index ist die Identität dieser Liste.
              key={index}
              cx={point.input * SIZE}
              cy={(1 - point.output) * SIZE}
              r={4}
              tabIndex={0}
              role="slider"
              aria-label={`Kurvenpunkt ${index + 1}`}
              aria-valuenow={point.output}
              className="cursor-pointer text-accent focus:outline focus:outline-2 focus:outline-accent"
              fill="currentColor"
              onPointerDown={handlePointPointerDown(index)}
              onPointerMove={handlePointPointerMove(index)}
              onPointerUp={handlePointPointerUp(index)}
              onFocus={() => setSelectedIndex(index)}
              onKeyDown={handlePointKeyDown(index)}
              onKeyUp={handlePointKeyUp}
            />
          ))}
      </svg>

      {channel.kind === "Points" && selectedIndex !== null && selectedPoint && (
        <div className="flex items-center gap-2 text-xs text-text-secondary">
          <label className="flex items-center gap-1">
            Eingabe
            <input
              type="number"
              aria-label="Kurvenpunkt Eingabe"
              className="w-16 rounded border border-border bg-bg-base px-1 py-0.5 text-right text-text-primary disabled:opacity-40"
              value={Math.round(selectedPoint.input * 1000) / 1000}
              min={0}
              max={1}
              step={0.01}
              disabled={selectedIndex === 0 || selectedIndex === channel.points.length - 1}
              onChange={(event) => {
                const parsed = Number(event.target.value);
                if (Number.isNaN(parsed)) return;
                const prev = channel.points[selectedIndex - 1];
                const next = channel.points[selectedIndex + 1];
                const minInput = prev ? prev.input + MIN_INPUT_GAP : 0;
                const maxInput = next ? next.input - MIN_INPUT_GAP : 1;
                updatePoint(selectedIndex, clamp(parsed, minInput, maxInput), selectedPoint.output);
              }}
              onBlur={onCommit}
            />
          </label>
          <label className="flex items-center gap-1">
            Ausgabe
            <input
              type="number"
              aria-label="Kurvenpunkt Ausgabe"
              className="w-16 rounded border border-border bg-bg-base px-1 py-0.5 text-right text-text-primary"
              value={Math.round(selectedPoint.output * 1000) / 1000}
              min={0}
              max={1}
              step={0.01}
              onChange={(event) => {
                const parsed = Number(event.target.value);
                if (Number.isNaN(parsed)) return;
                updatePoint(selectedIndex, selectedPoint.input, clamp(parsed, 0, 1));
              }}
              onBlur={onCommit}
            />
          </label>
        </div>
      )}

      {channel.kind === "Parametric" && (
        <div className="flex flex-col gap-3">
          {PARAMETRIC_CURVE_SLIDER_SPECS.map((spec) => {
            const key = spec.key as "shadows" | "darks" | "lights" | "highlights";
            return (
              <DevelopSlider
                key={spec.key}
                spec={spec}
                value={channel[key]}
                onChange={(value) => onChange({ ...channel, [key]: value })}
                onCommit={onCommit}
              />
            );
          })}
        </div>
      )}
    </div>
  );
}
