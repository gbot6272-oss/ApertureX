import { useEffect } from "react";
import { DevelopSlider } from "./DevelopSlider";
import { LUT_FILTER_BRUSH_RADIUS_SPEC, LUT_FILTER_BRUSH_STRENGTH_SPEC, LUT_FILTER_SLIDER_SPECS, type LutFilterData } from "../lib/edl";
import { useAppStore } from "../store";

/** Liest den Wert eines Rasterpunkts `(ri, gi, bi)` aus `lut.table`
 * (dieselbe Indizierung wie `apx_pipeline::lut_cube::ParsedLut::table`s
 * Moduldoku: r am schnellsten variierend). */
function gridPoint(lut: LutFilterData, ri: number, gi: number, bi: number): [number, number, number] {
  const idx = ((bi * lut.size + gi) * lut.size + ri) * 3;
  return [lut.table[idx] ?? 0, lut.table[idx + 1] ?? 0, lut.table[idx + 2] ?? 0];
}

/** Baut eine kleine CSS-Gradient-Vorschau aus der neutralen Diagonale
 * (`r === g === b`) eines Filters — reine Client-Berechnung aus den
 * bereits geladenen Rasterdaten, kein zusätzlicher Backend-Aufruf. Zeigt
 * anschaulich, wie der Filter Schatten/Mitten/Lichter einfärbt. */
function diagonalGradient(lut: LutFilterData): string {
  const steps = 6;
  const stops: string[] = [];
  for (let i = 0; i < steps; i += 1) {
    const t = i / (steps - 1);
    const gi = Math.round(t * (lut.size - 1));
    const [r, g, b] = gridPoint(lut, gi, gi, gi);
    stops.push(`rgb(${Math.round(r * 255)}, ${Math.round(g * 255)}, ${Math.round(b * 255)})`);
  }
  return `linear-gradient(to right, ${stops.join(", ")})`;
}

/**
 * Filter-/LUT-Bibliothek (Phase 16 Schritt 1+2, siehe `DECISIONS.md`
 * ADR-0043). Fünf eingebaute, selbst erstellte Looks (original erstellt,
 * kein externer Download — siehe `apx_pipeline::builtin_luts`s
 * Moduldoku für die Begründung) zum Ein-Klick-Anwenden, dazu ein
 * Datei-Dialog für eigene `.cube`-Dateien — anders als
 * `SkinSmoothingPanel`/`StyleTransferPanel` (je Foto berechnetes
 * Ergebnis) ist jeder Filter hier fotounabhängig: derselbe Filter lässt
 * sich unverändert auf jedes andere Foto anwenden.
 */
export function LutFilterPanel() {
  const developPhotoId = useAppStore((s) => s.developPhotoId);
  const lutFilterAdjustment = useAppStore((s) => s.developEdl.lut_filter);
  const lutFilterImporting = useAppStore((s) => s.lutFilterImporting);
  const importLutFilterForCurrentPhoto = useAppStore((s) => s.importLutFilterForCurrentPhoto);
  const clearLutFilter = useAppStore((s) => s.clearLutFilter);
  const setLutFilterStrength = useAppStore((s) => s.setLutFilterStrength);
  const builtinLutFilters = useAppStore((s) => s.builtinLutFilters);
  const loadBuiltinLutFilters = useAppStore((s) => s.loadBuiltinLutFilters);
  const applyBuiltinLutFilter = useAppStore((s) => s.applyBuiltinLutFilter);
  const commitDevelopEdit = useAppStore((s) => s.commitDevelopEdit);
  const lutFilterBrushActive = useAppStore((s) => s.lutFilterBrushActive);
  const toggleLutFilterBrushActive = useAppStore((s) => s.toggleLutFilterBrushActive);
  const lutFilterDraftRadius = useAppStore((s) => s.lutFilterDraftRadius);
  const lutFilterDraftStrength = useAppStore((s) => s.lutFilterDraftStrength);
  const setLutFilterDraftField = useAppStore((s) => s.setLutFilterDraftField);
  const removeLutFilterStroke = useAppStore((s) => s.removeLutFilterStroke);

  useEffect(() => {
    void loadBuiltinLutFilters();
  }, [loadBuiltinLutFilters]);

  if (!developPhotoId) return null;

  return (
    <div className="flex flex-col gap-2">
      <p className="text-xs font-medium text-text-secondary">Bibliothek</p>
      <div className="flex flex-col gap-1.5">
        {(builtinLutFilters ?? []).map((lut, index) => {
          const active = lutFilterAdjustment.lut?.name === lut.name;
          return (
            <button
              key={lut.name}
              type="button"
              onClick={() => applyBuiltinLutFilter(index)}
              aria-pressed={active}
              className={`flex items-center gap-2 rounded border px-1.5 py-1 text-left text-xs ${
                active ? "border-accent bg-accent/10 text-accent" : "border-border text-text-secondary hover:border-accent"
              }`}
            >
              <span className="h-4 w-8 flex-none rounded-sm border border-border/60" style={{ background: diagonalGradient(lut) }} />
              {lut.name}
            </button>
          );
        })}
        {builtinLutFilters === null && <p className="text-xs text-text-muted">Lädt…</p>}
      </div>

      <button
        type="button"
        onClick={() => void importLutFilterForCurrentPhoto()}
        disabled={lutFilterImporting}
        className="rounded border border-accent bg-accent/10 px-2 py-1 text-xs text-accent disabled:cursor-not-allowed disabled:opacity-50"
      >
        {lutFilterImporting ? "Importiert…" : "Eigene .cube-Datei importieren"}
      </button>

      {lutFilterAdjustment.lut ? (
        <>
          <p className="text-xs text-text-secondary">Aktiv: {lutFilterAdjustment.lut.name}</p>
          <button type="button" onClick={clearLutFilter} className="text-left text-xs text-text-muted underline hover:text-danger">
            Entfernen
          </button>
        </>
      ) : (
        <p className="text-xs text-text-muted">Kein Filter gewählt.</p>
      )}

      {LUT_FILTER_SLIDER_SPECS.map((spec) => (
        <DevelopSlider
          key={spec.key}
          spec={spec}
          value={lutFilterAdjustment.strength * 100}
          onChange={(value) => setLutFilterStrength(value / 100)}
          onCommit={() => void commitDevelopEdit()}
        />
      ))}

      {/* Punktuelle Anwendung (Phase 16 Schritt 3, ADR-0043) — leere
          `strokes` heißen "im ganzen Bild", nicht-leere beschränken den
          gewählten Filter auf die gemalten Bereiche (siehe
          `stages::lut_filter`s Moduldoku). */}
      <button
        type="button"
        aria-pressed={lutFilterBrushActive}
        onClick={toggleLutFilterBrushActive}
        disabled={!lutFilterAdjustment.lut}
        className={`rounded border px-2 py-1 text-xs disabled:cursor-not-allowed disabled:opacity-50 ${
          lutFilterBrushActive ? "border-accent bg-accent/20 text-accent" : "border-border text-text-secondary"
        }`}
      >
        Filter-Pinsel {lutFilterBrushActive ? "(aktiv)" : ""}
      </button>
      {lutFilterBrushActive && <p className="text-xs text-text-muted">Strich im Bild ziehen, um den Filter nur dort anzuwenden.</p>}

      <DevelopSlider
        spec={LUT_FILTER_BRUSH_RADIUS_SPEC}
        value={lutFilterDraftRadius * 100}
        onChange={(value) => setLutFilterDraftField("radius", value / 100)}
        onCommit={() => {}}
      />
      <DevelopSlider
        spec={LUT_FILTER_BRUSH_STRENGTH_SPEC}
        value={lutFilterDraftStrength * 100}
        onChange={(value) => setLutFilterDraftField("strength", value / 100)}
        onCommit={() => {}}
      />

      {lutFilterAdjustment.strokes.length > 0 && (
        <ul className="flex flex-col gap-1 text-xs text-text-secondary">
          {lutFilterAdjustment.strokes.map((_stroke, index) => (
            <li key={index} className="flex items-center justify-between rounded border border-border px-2 py-1">
              <span>Strich {index + 1}</span>
              <button type="button" onClick={() => removeLutFilterStroke(index)} className="text-text-muted hover:text-danger">
                Entfernen
              </button>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
