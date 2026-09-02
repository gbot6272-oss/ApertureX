import { useEffect } from "react";

import { previewUrl } from "../lib/media";
import { useAppStore } from "../store";

/**
 * Personenansicht (Phase 11 Schritt 5, siehe `DECISIONS.md` ADR-0038):
 * zeigt die vom Backend gebildeten Gruppen (`list_people_groups`, siehe
 * dessen Moduldoku) als Reihen. **Ehrlich begrenzt**: die Gruppierung
 * beruht auf einer groben Hautton-Blob-Signatur (Anzahl/Fläche
 * zusammenhängender Regionen), nicht auf echter Gesichts-Identifizierung
 * — der Titel „Gruppe" ist deshalb bewusst neutral gehalten statt einen
 * Personennamen vorzutäuschen.
 */
export function PeopleView() {
  const peopleGroups = useAppStore((s) => s.peopleGroups);
  const peopleGroupsLoading = useAppStore((s) => s.peopleGroupsLoading);
  const loadPeopleGroups = useAppStore((s) => s.loadPeopleGroups);
  const selectPhoto = useAppStore((s) => s.selectPhoto);

  useEffect(() => {
    void loadPeopleGroups();
    // eslint-disable-next-line react-hooks/exhaustive-deps -- lädt gezielt nur beim Öffnen der Ansicht neu
  }, []);

  return (
    <main className="flex flex-1 flex-col gap-4 overflow-y-auto p-4">
      {peopleGroupsLoading && <p className="text-sm text-text-muted">Gruppiert…</p>}
      {!peopleGroupsLoading && peopleGroups.length === 0 && (
        <p className="text-sm text-text-muted">
          Keine Gruppen gefunden — die Hautton-Heuristik erkennt entweder keine Gesichtsregionen oder findet keine zwei Fotos mit
          ähnlicher Blob-Signatur.
        </p>
      )}
      {peopleGroups.map((group, index) => (
        <section key={index} aria-label={`Gruppe ${index + 1}`}>
          <h3 className="mb-2 text-xs text-text-muted">Gruppe {index + 1} ({group.length} Fotos)</h3>
          <div className="flex flex-wrap gap-2">
            {group.map((photo) => (
              <button
                key={photo.id}
                type="button"
                onClick={() => selectPhoto(photo.id)}
                title={photo.filename}
                className="h-24 w-24 shrink-0 overflow-hidden rounded border-2 border-transparent hover:border-accent"
              >
                <img src={previewUrl(photo.id, 0)} alt={photo.filename} className="h-full w-full object-cover" loading="lazy" />
              </button>
            ))}
          </div>
        </section>
      ))}
    </main>
  );
}
