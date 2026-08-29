import { previewUrl } from "../lib/media";
import { useAppStore } from "../store";

/**
 * Horizontaler Filmstreifen. Noch **nicht** virtualisiert — bei sehr
 * vielen Fotos (Ziel: 50.000, siehe `PHASE1_PROMPT.md` Abschnitt 7) würde
 * das ruckeln. Die Virtualisierung mit `@tanstack/react-virtual` ist
 * Schritt 10 im Plan; Schritt 8 liefert erst das Layout und die
 * Grundfunktion (Anzeige, Auswahl per Klick).
 */
export function Filmstrip() {
  const selectedFolderId = useAppStore((s) => s.selectedFolderId);
  const selectedPhotoId = useAppStore((s) => s.selectedPhotoId);
  const photos = useAppStore((s) => (selectedFolderId ? s.photosByFolder[selectedFolderId] : undefined)) ?? [];
  const selectPhoto = useAppStore((s) => s.selectPhoto);

  if (photos.length === 0) {
    return (
      <footer className="flex h-24 shrink-0 items-center justify-center border-t border-border bg-bg-raised text-sm text-text-muted">
        {selectedFolderId ? "Keine Fotos in diesem Ordner." : "Wähle links einen Ordner."}
      </footer>
    );
  }

  return (
    <footer className="flex h-24 shrink-0 gap-1 overflow-x-auto border-t border-border bg-bg-raised p-1">
      {photos.map((photo) => (
        <button
          key={photo.id}
          type="button"
          onClick={() => selectPhoto(photo.id)}
          title={photo.filename}
          className={`h-full shrink-0 overflow-hidden rounded border-2 ${photo.id === selectedPhotoId ? "border-accent" : "border-transparent hover:border-border"}`}
        >
          <img src={previewUrl(photo.id, 0)} alt={photo.filename} className="h-full w-auto object-cover" loading="lazy" />
        </button>
      ))}
    </footer>
  );
}
