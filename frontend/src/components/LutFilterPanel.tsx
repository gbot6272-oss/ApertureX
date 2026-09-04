import { DevelopSlider } from "./DevelopSlider";
import { LUT_FILTER_SLIDER_SPECS } from "../lib/edl";
import { useAppStore } from "../store";

/**
 * Filter-/LUT-Bibliothek (Phase 16 Schritt 1, siehe `DECISIONS.md`
 * ADR-0043). Ein Knopf öffnet einen Datei-Dialog für eine `.cube`-
 * 3D-LUT-Datei — anders als `SkinSmoothingPanel`/`StyleTransferPanel`
 * (je Foto berechnetes Ergebnis) ist ein importierter Filter
 * fotounabhängig: dieselbe `.cube`-Datei lässt sich unverändert auf
 * jedes andere Foto anwenden. Schritt 2 ergänzt eine durchsuchbare
 * Bibliothek mit Vorschau statt des reinen Datei-Dialogs hier.
 */
export function LutFilterPanel() {
  const developPhotoId = useAppStore((s) => s.developPhotoId);
  const lutFilterAdjustment = useAppStore((s) => s.developEdl.lut_filter);
  const lutFilterImporting = useAppStore((s) => s.lutFilterImporting);
  const importLutFilterForCurrentPhoto = useAppStore((s) => s.importLutFilterForCurrentPhoto);
  const clearLutFilter = useAppStore((s) => s.clearLutFilter);
  const setLutFilterStrength = useAppStore((s) => s.setLutFilterStrength);
  const commitDevelopEdit = useAppStore((s) => s.commitDevelopEdit);

  if (!developPhotoId) return null;

  return (
    <div className="flex flex-col gap-2">
      <button
        type="button"
        onClick={() => void importLutFilterForCurrentPhoto()}
        disabled={lutFilterImporting}
        className="rounded border border-accent bg-accent/10 px-2 py-1 text-xs text-accent disabled:cursor-not-allowed disabled:opacity-50"
      >
        {lutFilterImporting ? "Importiert…" : "Filter (.cube) importieren"}
      </button>

      {lutFilterAdjustment.lut ? (
        <>
          <p className="text-xs text-text-secondary">{lutFilterAdjustment.lut.name}</p>
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
    </div>
  );
}
