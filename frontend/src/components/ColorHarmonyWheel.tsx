import { useState } from "react";

import { HARMONY_LABELS, harmonyTargetHues, MIN_CHROMA_FOR_HARMONIZE } from "../lib/colorHarmony";
import type { HarmonyType } from "../lib/colorHarmony";
import { hueSaturationToPixelOffset } from "../lib/colorWheelMath";
import { useAppStore } from "../store";

const HARMONY_TYPES: readonly HarmonyType[] = ["complementary", "triadic", "splitComplementary", "analogous"];

const SIZE = 128;
const RADIUS = SIZE / 2;
/** CIE-LCh-`chroma` liegt praktisch etwa im Bereich `0..130` (siehe
 * `apx_ai::palette`s Moduldoku-Verweis auf `palette::Lch`) — auf diesen
 * Wert normiert, damit selbst kräftig gesättigte Fotofarben nicht über
 * den Radrand hinauslaufen. */
const CHROMA_NORMALIZATION = 100;
/** Selbst ein fast neutraler Farbton bekommt einen sichtbaren
 * Mindestabstand vom Zentrum — sonst würden alle wenig bunten
 * Palettenfarben unsichtbar auf einem Punkt übereinanderliegen. */
const MIN_DOT_RADIUS_FRACTION = 0.15;

/**
 * Farb-Harmonie-Rad: automatische Paletten-Extraktion (Phase 14
 * Schritt 7, siehe `DECISIONS.md` ADR-0041 Nachtrag VII) — Lightroom
 * Classic/CC-Color-Grading-Räder sind rein manuell, keine automatische
 * Paletten-Extraktion mit Harmonie-Vorschlag gefunden.
 *
 * "Palette extrahieren" ruft `apx_ai::palette` (k-means über CIE-Lab, ans
 * Backend delegiert) für das aktuell geöffnete Foto ab und zeigt die
 * dominanten Farben auf einem Rad an (Winkel = Farbton, Abstand vom
 * Mittelpunkt = Buntheit — dieselbe "0° oben, im Uhrzeigersinn"-
 * Konvention wie `ColorWheel.tsx`, per `hueSaturationToPixelOffset`
 * wiederverwendet). Ein Klick auf eine Palettenfarbe macht sie zur
 * Basis der Harmonie-Zielmarkierungen (kleine Ringe auf dem Radrand).
 * "Harmonisieren" verschiebt die Farbton-Regler der acht festen
 * HSL-Bänder additiv, damit die Palette auf die gewählte Harmonie
 * einrastet (`lib/colorHarmony.ts`s `computeHarmonizeShifts`).
 */
export function ColorHarmonyWheel() {
  const developPhotoId = useAppStore((s) => s.developPhotoId);
  const colorPalette = useAppStore((s) => s.colorPalette);
  const colorPaletteLoading = useAppStore((s) => s.colorPaletteLoading);
  const extractColorPaletteForCurrentPhoto = useAppStore((s) => s.extractColorPaletteForCurrentPhoto);
  const harmonizeToTarget = useAppStore((s) => s.harmonizeToTarget);

  const [harmony, setHarmony] = useState<HarmonyType>("complementary");
  const [baseIndex, setBaseIndex] = useState(0);

  if (!developPhotoId) return null;

  const palette = colorPalette ?? [];
  const base = palette[Math.min(baseIndex, Math.max(0, palette.length - 1))];
  const targets = base ? harmonyTargetHues(base.hue_degrees, harmony) : [];

  return (
    <div className="flex flex-col gap-2">
      <div className="flex flex-wrap gap-1">
        {HARMONY_TYPES.map((type) => (
          <button
            key={type}
            type="button"
            onClick={() => setHarmony(type)}
            aria-pressed={harmony === type}
            className={`rounded border px-2 py-1 text-xs ${harmony === type ? "border-accent bg-accent/10 text-accent" : "border-border bg-bg-panel hover:border-accent"}`}
          >
            {HARMONY_LABELS[type]}
          </button>
        ))}
      </div>

      <div className="flex items-center gap-3">
        <div
          className="relative shrink-0 rounded-full border border-border"
          style={{
            width: SIZE,
            height: SIZE,
            background:
              "radial-gradient(circle, white 0%, transparent 70%), conic-gradient(from 0deg, hsl(0,100%,50%), hsl(60,100%,50%), hsl(120,100%,50%), hsl(180,100%,50%), hsl(240,100%,50%), hsl(300,100%,50%), hsl(360,100%,50%))",
          }}
        >
          {targets.map((targetHue, index) => {
            const { dx, dy } = hueSaturationToPixelOffset(targetHue, 1, RADIUS - 4);
            return (
              <div
                key={index}
                title={`Harmonie-Ziel: ${Math.round(targetHue)}°`}
                className="pointer-events-none absolute h-3 w-3 -translate-x-1/2 -translate-y-1/2 rounded-full border-2 border-white"
                style={{ left: RADIUS + dx, top: RADIUS + dy }}
              />
            );
          })}
          {palette.map((color, index) => {
            const chromaFraction = Math.max(MIN_DOT_RADIUS_FRACTION, Math.min(1, color.chroma / CHROMA_NORMALIZATION));
            const { dx, dy } = hueSaturationToPixelOffset(color.hue_degrees, chromaFraction, RADIUS - 6);
            const isBase = index === baseIndex;
            return (
              <button
                key={index}
                type="button"
                title={`R ${color.r} · G ${color.g} · B ${color.b} — ${Math.round(color.percentage * 100)}%`}
                onClick={() => setBaseIndex(index)}
                className={`absolute h-4 w-4 -translate-x-1/2 -translate-y-1/2 rounded-full border-2 ${isBase ? "border-accent" : "border-white/70"}`}
                style={{ left: RADIUS + dx, top: RADIUS + dy, backgroundColor: `rgb(${color.r}, ${color.g}, ${color.b})` }}
              />
            );
          })}
        </div>

        <div className="flex flex-1 flex-col gap-1 text-xs">
          {palette.length === 0 && !colorPaletteLoading && <p className="text-text-muted">Noch keine Palette extrahiert.</p>}
          {colorPaletteLoading && <p className="text-text-muted">Extrahiere Palette…</p>}
          {palette.map((color, index) => (
            <button
              key={index}
              type="button"
              onClick={() => setBaseIndex(index)}
              className={`flex items-center gap-2 rounded border px-1.5 py-0.5 text-left ${index === baseIndex ? "border-accent bg-accent/10" : "border-border hover:border-accent"}`}
            >
              <span className="h-3 w-3 shrink-0 rounded-full border border-border" style={{ backgroundColor: `rgb(${color.r}, ${color.g}, ${color.b})` }} />
              <span className="text-text-secondary">{Math.round(color.percentage * 100)}%</span>
            </button>
          ))}
        </div>
      </div>

      <div className="flex gap-2">
        <button
          type="button"
          onClick={() => void extractColorPaletteForCurrentPhoto()}
          disabled={colorPaletteLoading}
          className="flex-1 rounded border border-border px-2 py-1 text-xs hover:border-accent disabled:cursor-not-allowed disabled:opacity-50"
        >
          {colorPaletteLoading ? "Extrahiere…" : "Palette extrahieren"}
        </button>
        <button
          type="button"
          onClick={() => base && harmonizeToTarget(harmony, base.hue_degrees)}
          disabled={!base || base.chroma < MIN_CHROMA_FOR_HARMONIZE}
          title="Verschiebt die HSL-Farbton-Regler so, dass die Palette auf die gewählte Harmonie einrastet"
          className="flex-1 rounded border border-accent bg-accent/10 px-2 py-1 text-xs text-accent disabled:cursor-not-allowed disabled:opacity-50"
        >
          Harmonisieren
        </button>
      </div>
    </div>
  );
}
