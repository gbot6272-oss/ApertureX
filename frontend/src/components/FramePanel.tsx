import { DevelopSlider } from "./DevelopSlider";
import { FRAME_SLIDER_SPECS, NEUTRAL_FRAME, type FrameAdjustment } from "../lib/edl";
import { useAppStore } from "../store";

/**
 * Rahmen und Passepartout (Phase 32 F8, siehe `apx-pipeline`s
 * `stages::frame` für die Bildseite).
 *
 * Drei Ränder von außen nach innen: Rahmenlinie, Passepartout,
 * Keylinie. Genau der Aufbau eines gerahmten Abzugs — und der Grund,
 * warum es drei getrennte Regler sind statt eines „Rahmenbreite"-
 * Reglers: die Keylinie ist der Strich, der ein helles Bild optisch vom
 * hellen Passepartout trennt, und sie muss dafür deutlich schmaler sein
 * als alles andere.
 *
 * **Der Rahmen vergrößert die Bildfläche nicht** — er wird
 * hineingezeichnet, das Foto rückt zusammen. Steht als Hinweis auch im
 * Panel, weil man es beim ersten Aufdrehen sonst für einen Fehler hält.
 */

const PRESETS: ReadonlyArray<{ name: string; hint: string; frame: FrameAdjustment }> = [
  {
    name: "Galerie",
    hint: "Breites weißes Passepartout, feine dunkle Keylinie — die übliche Ausstellungs-Rahmung.",
    frame: {
      mat_width: 12,
      mat_color: [1, 1, 1],
      border_width: 0.6,
      border_color: [0.15, 0.15, 0.15],
      inner_line_width: 0.25,
      inner_line_color: [0.2, 0.2, 0.2],
    },
  },
  {
    name: "Museum",
    hint: "Warmes Büttenweiß, kräftige Außenlinie.",
    frame: {
      mat_width: 15,
      mat_color: [0.96, 0.94, 0.89],
      border_width: 1.5,
      border_color: [0.1, 0.09, 0.08],
      inner_line_width: 0.3,
      inner_line_color: [0.55, 0.5, 0.42],
    },
  },
  {
    name: "Dunkel",
    hint: "Schwarzes Passepartout — lässt helle Bilder leuchten.",
    frame: {
      mat_width: 10,
      mat_color: [0.08, 0.08, 0.08],
      border_width: 0.5,
      border_color: [0.02, 0.02, 0.02],
      inner_line_width: 0.2,
      inner_line_color: [0.35, 0.35, 0.35],
    },
  },
];

function toHex(color: [number, number, number]): string {
  const part = (value: number) =>
    Math.round(Math.min(1, Math.max(0, value)) * 255)
      .toString(16)
      .padStart(2, "0");
  return `#${part(color[0])}${part(color[1])}${part(color[2])}`;
}

function fromHex(hex: string): [number, number, number] {
  const value = hex.replace("#", "");
  const channel = (index: number) => parseInt(value.slice(index * 2, index * 2 + 2), 16) / 255;
  return [channel(0), channel(1), channel(2)];
}

export function FramePanel() {
  const frame = useAppStore((s) => s.developEdl.frame);
  const setFrameWidth = useAppStore((s) => s.setFrameWidth);
  const setFrameColor = useAppStore((s) => s.setFrameColor);
  const applyFramePreset = useAppStore((s) => s.applyFramePreset);
  const commitDevelopEdit = useAppStore((s) => s.commitDevelopEdit);

  const active = frame.mat_width > 0 || frame.border_width > 0 || frame.inner_line_width > 0;

  return (
    <div className="flex flex-col gap-2" data-testid="frame-panel">
      <div className="flex flex-wrap gap-1" role="group" aria-label="Rahmen-Vorlagen">
        {PRESETS.map((preset) => (
          <button
            key={preset.name}
            type="button"
            title={preset.hint}
            onClick={() => {
              applyFramePreset(preset.frame);
              void commitDevelopEdit();
            }}
            className="apx-btn-liquid rounded border border-border px-2 py-0.5 text-[11px] text-text-secondary transition-colors duration-[var(--duration-fast)] hover:text-text-primary"
          >
            {preset.name}
          </button>
        ))}
        <button
          type="button"
          onClick={() => {
            applyFramePreset(NEUTRAL_FRAME);
            void commitDevelopEdit();
          }}
          disabled={!active}
          className="apx-btn-liquid rounded border border-border px-2 py-0.5 text-[11px] text-text-secondary disabled:opacity-40"
        >
          Ohne Rahmen
        </button>
      </div>

      {FRAME_SLIDER_SPECS.map((spec) => (
        <DevelopSlider
          key={spec.key}
          spec={spec}
          value={frame[spec.key as "mat_width" | "border_width" | "inner_line_width"]}
          onChange={(value) => setFrameWidth(spec.key as "mat_width" | "border_width" | "inner_line_width", value)}
          onCommit={() => void commitDevelopEdit()}
        />
      ))}

      <div className="grid grid-cols-3 gap-2">
        {(
          [
            ["mat_color", "Passepartout"],
            ["border_color", "Rahmenlinie"],
            ["inner_line_color", "Keylinie"],
          ] as const
        ).map(([key, label]) => (
          <label key={key} className="flex flex-col gap-1 text-[11px] text-text-secondary">
            {label}
            <input
              type="color"
              value={toHex(frame[key])}
              onChange={(event) => setFrameColor(key, fromHex(event.target.value))}
              onBlur={() => void commitDevelopEdit()}
              aria-label={`${label}-Farbe`}
              className="h-7 w-full cursor-pointer rounded border border-border bg-bg-base"
            />
          </label>
        ))}
      </div>

      <p className="text-[11px] text-text-muted">
        Der Rahmen wird in die vorhandene Bildfläche gezeichnet — das Foto wird dafür kleiner, die Ausgabegröße bleibt gleich.
        Breiten sind Prozent der kürzeren Bildkante, damit der Rand rundherum gleich dick ist.
      </p>
    </div>
  );
}
