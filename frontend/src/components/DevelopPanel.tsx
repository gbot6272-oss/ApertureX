import { useEffect, useState } from "react";

import { BASIC_SLIDER_SPECS, readBasicField, WHITE_BALANCE_PRESETS, type CurvesAdjustment } from "../lib/edl";
import { useAppStore } from "../store";
import { CurveEditor } from "./CurveEditor";
import { DevelopSlider } from "./DevelopSlider";

const WHITE_BALANCE_KEYS = new Set(["temp_shift_kelvin", "tint_shift"]);

/** Die fünf Kurven-Kanäle (Phase 4 Schritt 4) — Anzeigereihenfolge wie in
 * `SPEC.md` §3.2 („RGB-Verbundkurve, R/G/B einzeln, Luminanz-Kurve"). */
const CURVE_CHANNEL_TABS: ReadonlyArray<{ key: keyof CurvesAdjustment; label: string }> = [
  { key: "rgb", label: "RGB" },
  { key: "red", label: "Rot" },
  { key: "green", label: "Grün" },
  { key: "blue", label: "Blau" },
  { key: "luminance", label: "Luminanz" },
];

/**
 * Das Entwickeln-Panel: die sieben Grundeinstellungs-Regler (Weißabgleich
 * zählt als einer, hat aber zwei Zahlenwerte — siehe `SPEC.md` §5) plus
 * Rückgängig/Wiederholen. Nur sichtbar, wenn `developPanelOpen` (siehe
 * `store/index.ts`, umgeschaltet über den "Entwickeln"-Knopf in
 * `Header.tsx`).
 *
 * Undo/Redo laufen direkt über `apx-catalog`s `edit_history` (siehe
 * `crates/apx-app/src/commands.rs`), nicht über eine separate
 * Frontend-Bibliothek wie ursprünglich in ADR-0018 vorgesehen — siehe die
 * Korrektur-Notiz dort: ein zusätzliches, lose synchronisiertes
 * Verlaufssystem im Frontend hätte keinen erkennbaren Vorteil gegenüber
 * der ohnehin schon vollständig getesteten Backend-Historie gebracht,
 * und Tauris IPC ist lokal schnell genug, um pro Undo/Redo-Klick einen
 * Roundtrip zu rechtfertigen.
 */
export function DevelopPanel() {
  const open = useAppStore((s) => s.developPanelOpen);
  const basic = useAppStore((s) => s.developEdl.basic);
  const setBasicField = useAppStore((s) => s.setBasicField);
  const commitDevelopEdit = useAppStore((s) => s.commitDevelopEdit);
  const undoDevelop = useAppStore((s) => s.undoDevelop);
  const redoDevelop = useAppStore((s) => s.redoDevelop);
  const selectedPhotoId = useAppStore((s) => s.selectedPhotoId);
  const lastLatencyMs = useAppStore((s) => s.developLastLatencyMs);
  const wbPickerActive = useAppStore((s) => s.wbPickerActive);
  const toggleWbPicker = useAppStore((s) => s.toggleWbPicker);
  const applyWhiteBalancePreset = useAppStore((s) => s.applyWhiteBalancePreset);
  const curves = useAppStore((s) => s.developEdl.curves);
  const setCurveChannel = useAppStore((s) => s.setCurveChannel);
  const [activeCurveChannel, setActiveCurveChannel] = useState<keyof CurvesAdjustment>("rgb");

  useEffect(() => {
    if (!open) return;

    function handleKeyDown(event: KeyboardEvent) {
      const target = event.target as HTMLElement | null;
      if (target && (target.tagName === "INPUT" || target.tagName === "TEXTAREA")) return;
      if (!(event.metaKey || event.ctrlKey) || event.key.toLowerCase() !== "z") return;

      event.preventDefault();
      if (event.shiftKey) {
        void redoDevelop();
      } else {
        void undoDevelop();
      }
    }

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [open, undoDevelop, redoDevelop]);

  if (!open) return null;

  const whiteBalanceSpecs = BASIC_SLIDER_SPECS.filter((spec) => WHITE_BALANCE_KEYS.has(spec.key));
  const toneSpecs = BASIC_SLIDER_SPECS.filter((spec) => !WHITE_BALANCE_KEYS.has(spec.key));

  return (
    <aside className="flex w-72 shrink-0 flex-col gap-4 overflow-y-auto border-l border-border bg-bg-raised p-3" aria-label="Entwickeln">
      <div className="flex items-center justify-between">
        <h2 className="text-sm font-semibold text-text-primary">Entwickeln</h2>
        <div className="flex gap-1">
          <button
            type="button"
            onClick={() => void undoDevelop()}
            disabled={!selectedPhotoId}
            aria-label="Rückgängig"
            title="Rückgängig (Strg/Cmd+Z)"
            className="rounded px-2 py-1 text-xs hover:bg-bg-panel disabled:cursor-not-allowed disabled:opacity-40"
          >
            ↶
          </button>
          <button
            type="button"
            onClick={() => void redoDevelop()}
            disabled={!selectedPhotoId}
            aria-label="Wiederholen"
            title="Wiederholen (Strg/Cmd+Umschalt+Z)"
            className="rounded px-2 py-1 text-xs hover:bg-bg-panel disabled:cursor-not-allowed disabled:opacity-40"
          >
            ↷
          </button>
        </div>
      </div>

      {!selectedPhotoId && <p className="text-xs text-text-muted">Kein Foto ausgewählt.</p>}

      {selectedPhotoId && lastLatencyMs !== null && (
        <p className="text-xs text-text-muted" title="Ende-zu-Ende-Antwortzeit der letzten Regler-Vorschau (IPC + Dekodierung/Rendern, ohne Neuzeichnen im Browser) — siehe PLAN.md Phase 2 Schritt 7">
          Letztes Rendering: {Math.round(lastLatencyMs)} ms
        </p>
      )}

      {selectedPhotoId && (
        <>
          <fieldset className="flex flex-col gap-3">
            <legend className="mb-1 text-xs font-medium text-text-secondary">Weißabgleich</legend>

            <div className="flex items-center gap-2">
              <select
                aria-label="Weißabgleich-Preset"
                defaultValue=""
                onChange={(event) => {
                  if (event.target.value) applyWhiteBalancePreset(event.target.value);
                  event.target.value = "";
                }}
                className="flex-1 rounded border border-border bg-bg-panel px-2 py-1 text-xs"
              >
                <option value="" disabled>
                  Preset wählen…
                </option>
                {WHITE_BALANCE_PRESETS.map((preset) => (
                  <option key={preset.key} value={preset.key}>
                    {preset.label}
                  </option>
                ))}
              </select>
              <button
                type="button"
                onClick={toggleWbPicker}
                aria-pressed={wbPickerActive}
                title="Weißabgleich-Pipette: ins Bild klicken, um einen neutralen Punkt zu setzen"
                className={`rounded border px-2 py-1 text-xs ${
                  wbPickerActive ? "border-accent bg-accent/10 text-accent" : "border-border bg-bg-panel hover:border-accent"
                }`}
              >
                Pipette
              </button>
            </div>
            {wbPickerActive && <p className="text-xs text-accent">Klicken Sie in einen neutral-grauen Bildpunkt.</p>}

            {whiteBalanceSpecs.map((spec) => (
              <DevelopSlider
                key={spec.key}
                spec={spec}
                value={readBasicField(basic, spec.key)}
                onChange={(value) => setBasicField(spec.key, value)}
                onCommit={() => void commitDevelopEdit()}
              />
            ))}
          </fieldset>

          <div className="flex flex-col gap-3">
            {toneSpecs.map((spec) => (
              <DevelopSlider
                key={spec.key}
                spec={spec}
                value={readBasicField(basic, spec.key)}
                onChange={(value) => setBasicField(spec.key, value)}
                onCommit={() => void commitDevelopEdit()}
              />
            ))}
          </div>

          <fieldset className="flex flex-col gap-2">
            <legend className="mb-1 text-xs font-medium text-text-secondary">Kurven</legend>
            <div className="flex flex-wrap gap-1">
              {CURVE_CHANNEL_TABS.map((tab) => (
                <button
                  key={tab.key}
                  type="button"
                  onClick={() => setActiveCurveChannel(tab.key)}
                  aria-pressed={activeCurveChannel === tab.key}
                  className={`rounded border px-2 py-1 text-xs ${
                    activeCurveChannel === tab.key ? "border-accent bg-accent/10 text-accent" : "border-border bg-bg-panel hover:border-accent"
                  }`}
                >
                  {tab.label}
                </button>
              ))}
            </div>
            <CurveEditor
              key={activeCurveChannel}
              channel={curves[activeCurveChannel]}
              onChange={(next) => setCurveChannel(activeCurveChannel, next)}
              onCommit={() => void commitDevelopEdit()}
            />
          </fieldset>
        </>
      )}
    </aside>
  );
}
