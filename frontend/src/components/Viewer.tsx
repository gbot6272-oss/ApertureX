import { formatShutter } from "../lib/format";
import { previewUrl } from "../lib/media";
import { useAppStore } from "../store";

/**
 * Zeigt das ausgewählte Foto an. Bewusst noch ohne Canvas/Zoom/Pan — das
 * kommt in Schritt 9 ("Viewer") dazu und ersetzt das `<img>` hier durch
 * eine interaktive Canvas-2D-Darstellung. Für Schritt 8 reicht eine
 * echte, funktionierende Anzeige (nutzt bereits den in Schritt 7 gebauten
 * Protokoll-Handler) mit Metadaten-Leiste.
 */
export function Viewer() {
  const selectedFolderId = useAppStore((s) => s.selectedFolderId);
  const selectedPhotoId = useAppStore((s) => s.selectedPhotoId);
  const photos = useAppStore((s) => (selectedFolderId ? s.photosByFolder[selectedFolderId] : undefined));
  const photo = photos?.find((p) => p.id === selectedPhotoId);

  return (
    <main className="relative flex flex-1 items-center justify-center overflow-hidden bg-bg-base">
      {!photo && <p className="text-sm text-text-muted">Kein Foto ausgewählt.</p>}

      {photo && (
        <>
          <img key={photo.id} src={previewUrl(photo.id, 1)} alt={photo.filename} className="max-h-full max-w-full object-contain" />

          <div className="absolute right-3 bottom-3 rounded bg-bg-raised/90 px-3 py-2 text-xs text-text-secondary backdrop-blur">
            <div className="font-medium text-text-primary">{photo.filename}</div>
            <div>
              {[photo.camera_make, photo.camera_model].filter(Boolean).join(" ")}
              {photo.lens ? ` · ${photo.lens}` : ""}
            </div>
            <div>
              {[photo.iso ? `ISO ${photo.iso}` : null, photo.aperture ? `f/${photo.aperture}` : null, photo.shutter ? formatShutter(photo.shutter) : null, photo.focal_length ? `${Math.round(photo.focal_length)}mm` : null]
                .filter(Boolean)
                .join(" · ")}
            </div>
            {photo.width && photo.height && (
              <div>
                {photo.width} × {photo.height}
              </div>
            )}
          </div>
        </>
      )}
    </main>
  );
}
