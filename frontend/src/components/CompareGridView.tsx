import { previewUrl } from "../lib/media";
import { useAppStore } from "../store";
import { FlagToggle, RatingStars } from "./RatingFlagColor";

/** Feste Zoom-Stufen für den gemeinsamen Zoom-Regler (Phase 9 Schritt 7). */
const ZOOM_LEVELS = [1, 1.5, 2, 3] as const;

/**
 * Vergleichsansicht (Phase 9 Schritt 3, erweitert in Phase 9 Schritt 7 um
 * einen gemeinsamen Zoom — siehe `PLAN.md`/`DECISIONS.md` ADR-0035) — bis
 * zu 9 Fotos (oder, über `openVersionsCompareView`, ein Foto plus seine
 * virtuellen Kopien) nebeneinander, mit direkter Bewertungs-/Flaggen-
 * Bedienung je Kachel.
 *
 * **Bewusste Vereinfachung**: zeigt die bereits generierte Standard-
 * Vorschau (`PreviewLevel.Standard`, JPEG) statt eines live gerenderten
 * Entwickeln-Stands — reflektiert trotzdem den jeweils committeten
 * Bearbeitungsstand jeder Kachel (auch bei virtuellen Kopien, die eine
 * eigene, unabhängige `edit_history` haben, siehe
 * `apx-catalog::repository::photos::create_virtual_copy`s Moduldoku), da
 * der Vorschau-Cache selbst schon je `photo_id` gerendert wird.
 *
 * **Synchronisierter Zoom** (`compareViewZoom`, Schritt 7): ein einziger
 * gemeinsamer Skalierungsfaktor für alle Kacheln (`transform: scale(...)`)
 * statt unabhängigem Zoom je Kachel — echtes Pan-Sync (bei dem alle
 * Kacheln zusätzlich denselben Bildausschnitt verfolgen, wie Lightrooms
 * Vergleichsansicht) bräuchte eine gemeinsame Pointer-Drag-Zustands-
 * maschine über bis zu neun `<img>`-Elemente; für den Beurteilungs-
 * Anwendungsfall (Detailschärfe bei gleicher Vergrößerung vergleichen)
 * reicht der gemeinsame Skalierungsfaktor, echtes Pan-Sync bleibt eine
 * spätere Erweiterung.
 */
export function CompareGridView() {
  const photoIds = useAppStore((s) => s.compareViewPhotoIds);
  const selectedFolderId = useAppStore((s) => s.selectedFolderId);
  const photosInFolder = useAppStore((s) => (selectedFolderId ? s.photosByFolder[selectedFolderId] : undefined));
  const closeCompareView = useAppStore((s) => s.closeCompareView);
  const setPhotoRating = useAppStore((s) => s.setPhotoRating);
  const setPhotoFlag = useAppStore((s) => s.setPhotoFlag);
  const zoom = useAppStore((s) => s.compareViewZoom);
  const setCompareViewZoom = useAppStore((s) => s.setCompareViewZoom);

  if (photoIds.length === 0) return null;

  const photos = photoIds.map((id) => photosInFolder?.find((p) => p.id === id)).filter((p): p is NonNullable<typeof p> => p !== undefined);

  const columns = photos.length <= 2 ? photos.length : photos.length <= 4 ? 2 : 3;

  return (
    <div className="fixed inset-0 z-50 flex flex-col bg-bg-base" aria-label="Vergleichsansicht">
      <div className="flex shrink-0 items-center justify-between border-b border-border px-3 py-2">
        <h2 className="text-sm font-semibold text-text-primary">Vergleichsansicht — {photos.length} Fotos</h2>
        <div className="flex items-center gap-2">
          <span className="text-xs text-text-secondary" id="compare-zoom-label">
            Zoom (synchronisiert)
          </span>
          <div className="flex gap-1" role="group" aria-labelledby="compare-zoom-label">
            {ZOOM_LEVELS.map((level) => (
              <button
                key={level}
                type="button"
                onClick={() => setCompareViewZoom(level)}
                aria-pressed={zoom === level}
                className={`rounded border px-2 py-0.5 text-xs ${zoom === level ? "border-accent bg-accent/10 text-accent" : "border-border text-text-secondary hover:border-accent"}`}
              >
                {level}×
              </button>
            ))}
          </div>
          <button type="button" onClick={closeCompareView} className="rounded border border-border px-2 py-1 text-xs hover:border-accent">
            Schließen
          </button>
        </div>
      </div>
      <div className="grid flex-1 gap-2 overflow-auto p-2" style={{ gridTemplateColumns: `repeat(${columns}, 1fr)` }}>
        {photos.map((photo) => (
          <div key={photo.id} className="flex flex-col overflow-hidden rounded border border-border bg-bg-panel">
            <div className="min-h-0 flex-1 overflow-hidden">
              <img
                src={previewUrl(photo.id, 1)}
                alt={photo.filename}
                className="h-full w-full object-contain transition-transform"
                style={{ transform: `scale(${zoom})` }}
              />
            </div>
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
