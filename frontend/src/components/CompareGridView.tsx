import { previewUrl } from "../lib/media";
import { useAppStore } from "../store";
import { FlagToggle, RatingStars } from "./RatingFlagColor";

/**
 * Vergleichsansicht (Phase 9 Schritt 3, siehe `PLAN.md`/`DECISIONS.md`
 * ADR-0035) — bis zu 9 Fotos nebeneinander für schnelles Sichten/Culling
 * einer Auswahl, mit direkter Bewertungs-/Flaggen-Bedienung je Kachel.
 *
 * **Bewusste Vereinfachung**: zeigt die bereits generierte Standard-
 * Vorschau (`PreviewLevel.Standard`, JPEG) statt eines live gerenderten
 * Entwickeln-Stands, und ohne synchronisierten Zoom/Pan zwischen den
 * Kacheln (anders als `ReferenceView.tsx`s WebGL-Renderer, der genau
 * *ein* Arbeitsbild gegen ein Referenzbild vergleicht) — für den
 * Sichtungs-/Auswahl-Anwendungsfall reicht ein schneller, statischer
 * Überblick; ein echter Live-Renderer-Vergleich mehrerer Bilder wäre ein
 * Mehrfaches der GPU-Kosten des normalen Viewers.
 */
export function CompareGridView() {
  const photoIds = useAppStore((s) => s.compareViewPhotoIds);
  const selectedFolderId = useAppStore((s) => s.selectedFolderId);
  const photosInFolder = useAppStore((s) => (selectedFolderId ? s.photosByFolder[selectedFolderId] : undefined));
  const closeCompareView = useAppStore((s) => s.closeCompareView);
  const setPhotoRating = useAppStore((s) => s.setPhotoRating);
  const setPhotoFlag = useAppStore((s) => s.setPhotoFlag);

  if (photoIds.length === 0) return null;

  const photos = photoIds.map((id) => photosInFolder?.find((p) => p.id === id)).filter((p): p is NonNullable<typeof p> => p !== undefined);

  const columns = photos.length <= 2 ? photos.length : photos.length <= 4 ? 2 : 3;

  return (
    <div className="fixed inset-0 z-50 flex flex-col bg-bg-base" aria-label="Vergleichsansicht">
      <div className="flex shrink-0 items-center justify-between border-b border-border px-3 py-2">
        <h2 className="text-sm font-semibold text-text-primary">Vergleichsansicht — {photos.length} Fotos</h2>
        <button type="button" onClick={closeCompareView} className="rounded border border-border px-2 py-1 text-xs hover:border-accent">
          Schließen
        </button>
      </div>
      <div className="grid flex-1 gap-2 overflow-auto p-2" style={{ gridTemplateColumns: `repeat(${columns}, 1fr)` }}>
        {photos.map((photo) => (
          <div key={photo.id} className="flex flex-col overflow-hidden rounded border border-border bg-bg-panel">
            <img src={previewUrl(photo.id, 1)} alt={photo.filename} className="min-h-0 flex-1 object-contain" />
            <div className="flex shrink-0 items-center justify-between gap-2 border-t border-border px-2 py-1">
              <span className="truncate text-xs text-text-secondary" title={photo.filename}>
                {photo.filename}
              </span>
              <div className="flex items-center gap-2">
                <RatingStars rating={photo.rating} onChange={(rating) => void setPhotoRating(photo.id, rating)} compact />
                <FlagToggle flag={photo.flag} onChange={(flag) => void setPhotoFlag(photo.id, flag)} compact />
              </div>
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}
