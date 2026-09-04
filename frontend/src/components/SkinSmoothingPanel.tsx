import { DevelopSlider } from "./DevelopSlider";
import { SKIN_SMOOTHING_SLIDER_SPECS } from "../lib/edl";
import { useAppStore } from "../store";

/**
 * Automatisches Hautglätten (Phase 15 Schritt 5, siehe `DECISIONS.md`
 * ADR-0042 — Lightroom hat kein automatisches, gesichtserkennungs-
 * gestütztes Hautglätten, nur den manuellen Anpassungspinsel). Erkennt
 * Gesichter selbst (`apx_ai::faces::detect_face_regions`) — kein
 * manuelles Maskieren nötig, das unterscheidet es bewusst vom
 * bestehenden Reparaturpinsel/Anpassungspinsel. Ein Knopf löst die
 * Berechnung aus, der Deckkraft-Regler blendet anschließend linear
 * zwischen dem unveränderten Foto und dem vollen Ergebnis.
 */
export function SkinSmoothingPanel() {
  const developPhotoId = useAppStore((s) => s.developPhotoId);
  const skinSmoothingAdjustment = useAppStore((s) => s.developEdl.skin_smoothing);
  const skinSmoothing = useAppStore((s) => s.skinSmoothing);
  const smoothSkinForCurrentPhoto = useAppStore((s) => s.smoothSkinForCurrentPhoto);
  const clearSkinSmoothing = useAppStore((s) => s.clearSkinSmoothing);
  const setSkinSmoothingAmount = useAppStore((s) => s.setSkinSmoothingAmount);
  const commitDevelopEdit = useAppStore((s) => s.commitDevelopEdit);

  if (!developPhotoId) return null;

  return (
    <div className="flex flex-col gap-2">
      <button
        type="button"
        onClick={() => void smoothSkinForCurrentPhoto()}
        disabled={skinSmoothing}
        className="rounded border border-accent bg-accent/10 px-2 py-1 text-xs text-accent disabled:cursor-not-allowed disabled:opacity-50"
      >
        {skinSmoothing ? "Glättet…" : "Haut automatisch glätten"}
      </button>

      {skinSmoothingAdjustment.patch ? (
        <button type="button" onClick={clearSkinSmoothing} className="text-left text-xs text-text-muted underline hover:text-danger">
          Entfernen
        </button>
      ) : (
        <p className="text-xs text-text-muted">Noch keine Hautglättung berechnet.</p>
      )}

      {SKIN_SMOOTHING_SLIDER_SPECS.map((spec) => (
        <DevelopSlider
          key={spec.key}
          spec={spec}
          value={skinSmoothingAdjustment.amount * 100}
          onChange={(value) => setSkinSmoothingAmount(value / 100)}
          onCommit={() => void commitDevelopEdit()}
        />
      ))}
    </div>
  );
}
