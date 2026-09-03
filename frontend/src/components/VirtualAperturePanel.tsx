import { DevelopSlider } from "./DevelopSlider";
import { VIRTUAL_APERTURE_SLIDER_SPECS } from "../lib/edl";
import { useAppStore } from "../store";

/**
 * KI-Tiefenschärfe-Simulator "Virtuelle Blende" (Phase 14 Schritt 8,
 * siehe `DECISIONS.md` ADR-0041 Nachtrag VIII) — Lightroom hat "keine
 * KI-Tiefenschätzung/synthetisches Bokeh", nur die deutlich gröbere
 * Laplace-Varianz-Heuristik in ApertureX selbst
 * (`stages::masks::relative_sharpness_map`, Phase 11 Schritt 7).
 *
 * Ablauf: einmal das opt-in MiDaS-v2.1-small-Modell herunterladen (MIT,
 * ~66 MB, lokal, SHA-256-geprüft), dann pro Foto "Fokuspunkt setzen" (ein
 * Bildklick, dasselbe Muster wie die `ClickRegion`-KI-Maske) und
 * "Tiefenkarte berechnen" — danach steuert der "Betrag"-Regler den
 * variablen Unschärferadius aus der Tiefendifferenz zum Fokuspunkt
 * (`stages::virtual_aperture`, rein CPU-seitig).
 */
export function VirtualAperturePanel() {
  const developPhotoId = useAppStore((s) => s.developPhotoId);
  const virtualAperture = useAppStore((s) => s.developEdl.virtual_aperture);
  const setVirtualApertureAmount = useAppStore((s) => s.setVirtualApertureAmount);
  const commitDevelopEdit = useAppStore((s) => s.commitDevelopEdit);
  const virtualApertureFocusPickerActive = useAppStore((s) => s.virtualApertureFocusPickerActive);
  const toggleVirtualApertureFocusPicker = useAppStore((s) => s.toggleVirtualApertureFocusPicker);
  const depthEstimating = useAppStore((s) => s.depthEstimating);
  const estimateDepthForCurrentPhoto = useAppStore((s) => s.estimateDepthForCurrentPhoto);
  const aiSettings = useAppStore((s) => s.aiSettings);
  const depthModelDownloading = useAppStore((s) => s.depthModelDownloading);
  const downloadDepthModel = useAppStore((s) => s.downloadDepthModel);
  const clearDepthModelPath = useAppStore((s) => s.clearDepthModelPath);

  if (!developPhotoId) return null;

  const hasModel = !!aiSettings?.depth_model_path;
  const hasDepthMap = !!virtualAperture.depth_map;

  return (
    <div className="flex flex-col gap-2">
      <p className="rounded border border-border px-2 py-1 text-xs text-text-secondary">
        {hasModel ? (
          <>
            Tiefenschätzungs-Modell installiert.{" "}
            <button type="button" onClick={() => void clearDepthModelPath()} className="text-text-muted underline hover:text-danger">
              Entfernen
            </button>
          </>
        ) : (
          <>
            Kein Modell installiert — MiDaS v2.1 small (MIT, ~66 MB, lokal, kein Cloud-Aufruf).{" "}
            <button
              type="button"
              disabled={depthModelDownloading}
              onClick={() => void downloadDepthModel()}
              className="text-accent underline disabled:cursor-not-allowed disabled:opacity-40"
            >
              {depthModelDownloading ? "Lädt herunter…" : "Herunterladen"}
            </button>
          </>
        )}
      </p>

      <div className="flex gap-2">
        <button
          type="button"
          onClick={toggleVirtualApertureFocusPicker}
          aria-pressed={virtualApertureFocusPickerActive}
          disabled={!hasModel}
          className={`flex-1 rounded border px-2 py-1 text-xs disabled:cursor-not-allowed disabled:opacity-50 ${
            virtualApertureFocusPickerActive ? "border-accent bg-accent/10 text-accent" : "border-border hover:border-accent"
          }`}
        >
          Fokuspunkt setzen
        </button>
        <button
          type="button"
          onClick={() => void estimateDepthForCurrentPhoto()}
          disabled={!hasModel || depthEstimating}
          className="flex-1 rounded border border-border px-2 py-1 text-xs hover:border-accent disabled:cursor-not-allowed disabled:opacity-50"
        >
          {depthEstimating ? "Berechnet…" : "Tiefenkarte berechnen"}
        </button>
      </div>

      {/* Dasselbe "Klicken Sie ins Bild…"-Hinweismuster wie
          `aiMaskClickPickerActive` in `MasksPanel.tsx` — der Knopftext
          bleibt statisch, nur `aria-pressed`/Rahmenfarbe zeigen den
          aktiven Picker an. */}
      {virtualApertureFocusPickerActive && <p className="text-xs text-accent">Klicken Sie ins Bild, um den Fokuspunkt zu setzen.</p>}

      {!hasDepthMap && <p className="text-xs text-text-muted">Noch keine Tiefenkarte berechnet.</p>}

      {VIRTUAL_APERTURE_SLIDER_SPECS.map((spec) => (
        <DevelopSlider
          key={spec.key}
          spec={spec}
          value={virtualAperture.amount * 100}
          onChange={(value) => setVirtualApertureAmount(value / 100)}
          onCommit={() => void commitDevelopEdit()}
        />
      ))}
    </div>
  );
}
