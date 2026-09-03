import { useEffect, useState } from "react";

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
 *
 * Seit Phase 13 Schritt 8 (siehe `DECISIONS.md` ADR-0040-Nachtrag VI)
 * zusätzlich ein oberer Abschnitt für echte Personen-Wiedererkennung:
 * echte 128-dimensionale Gesichts-Embeddings statt der Hautton-Heuristik
 * — additiv, ersetzt die Gruppen unten nicht (die bleiben als Fallback
 * bestehen, wenn die Modelle nicht heruntergeladen sind oder diese Build
 * ohne das `people`-Cargo-Feature kompiliert wurde).
 */
export function PeopleView() {
  const peopleGroups = useAppStore((s) => s.peopleGroups);
  const peopleGroupsLoading = useAppStore((s) => s.peopleGroupsLoading);
  const loadPeopleGroups = useAppStore((s) => s.loadPeopleGroups);
  const selectPhoto = useAppStore((s) => s.selectPhoto);

  const aiSettings = useAppStore((s) => s.aiSettings);
  const loadAiSettings = useAppStore((s) => s.loadAiSettings);
  const peopleModelsDownloading = useAppStore((s) => s.peopleModelsDownloading);
  const downloadPeopleModels = useAppStore((s) => s.downloadPeopleModels);
  const clearPeopleModelPaths = useAppStore((s) => s.clearPeopleModelPaths);

  const people = useAppStore((s) => s.people);
  const peopleLoading = useAppStore((s) => s.peopleLoading);
  const refreshPeople = useAppStore((s) => s.refreshPeople);
  const personPhotos = useAppStore((s) => s.personPhotos);
  const loadPhotosForPerson = useAppStore((s) => s.loadPhotosForPerson);
  const renamePerson = useAppStore((s) => s.renamePerson);
  const deletePerson = useAppStore((s) => s.deletePerson);

  const selectedPhotoId = useAppStore((s) => s.selectedPhotoId);
  const facesForSelectedPhoto = useAppStore((s) => s.facesForSelectedPhoto);
  const facesLoading = useAppStore((s) => s.facesLoading);
  const loadFacesForSelectedPhoto = useAppStore((s) => s.loadFacesForSelectedPhoto);
  const detectingFaces = useAppStore((s) => s.detectingFaces);
  const detectFacesForSelectedPhoto = useAppStore((s) => s.detectFacesForSelectedPhoto);
  const assignFaceToPerson = useAppStore((s) => s.assignFaceToPerson);
  const unassignFace = useAppStore((s) => s.unassignFace);

  const [expandedPersonId, setExpandedPersonId] = useState<string | null>(null);
  const [nameDrafts, setNameDrafts] = useState<Record<string, string>>({});

  useEffect(() => {
    void loadPeopleGroups();
    void loadAiSettings();
    void refreshPeople();
    // eslint-disable-next-line react-hooks/exhaustive-deps -- lädt gezielt nur beim Öffnen der Ansicht neu
  }, []);

  useEffect(() => {
    void loadFacesForSelectedPhoto();
    // eslint-disable-next-line react-hooks/exhaustive-deps -- lädt gezielt nur bei Fotowechsel neu
  }, [selectedPhotoId]);

  const modelsReady = Boolean(aiSettings?.people_landmark_model_path && aiSettings?.people_encoder_model_path);

  function toggleExpanded(personId: string) {
    if (expandedPersonId === personId) {
      setExpandedPersonId(null);
      return;
    }
    setExpandedPersonId(personId);
    if (!personPhotos[personId]) void loadPhotosForPerson(personId);
  }

  return (
    <main className="flex flex-1 flex-col gap-4 overflow-y-auto p-4">
      <section aria-label="Echte Personen-Wiedererkennung" className="rounded border border-border p-3">
        <h3 className="mb-2 text-sm font-semibold text-text-primary">Echte Personen-Wiedererkennung</h3>

        {!aiSettings?.people_feature_compiled && (
          <p className="text-xs text-text-muted">
            Diese Aperture-X-Build wurde ohne echte Personen-Wiedererkennung kompiliert (Systembibliothek `libdlib` fehlt in dieser
            Umgebung) — nur die Hautton-Gruppierung unten steht zur Verfügung.
          </p>
        )}

        {aiSettings?.people_feature_compiled && !modelsReady && (
          <div className="flex flex-col gap-2">
            <p className="text-xs text-text-muted">
              Erkennt echte Gesichter über ein 128-dimensionales Embedding statt grober Hautton-Blobs — lädt zwei gemeinfreie
              `dlib`-Modelldateien herunter (~31 MB, dlib.net).
            </p>
            <button
              type="button"
              onClick={() => void downloadPeopleModels()}
              disabled={peopleModelsDownloading}
              className="w-fit rounded border border-accent bg-accent/10 px-2 py-1 text-xs text-accent disabled:cursor-not-allowed disabled:opacity-50"
            >
              {peopleModelsDownloading ? "Lädt herunter…" : "Modelle herunterladen"}
            </button>
          </div>
        )}

        {aiSettings?.people_feature_compiled && modelsReady && (
          <div className="flex flex-col gap-3">
            <div className="flex items-center justify-between">
              <button
                type="button"
                onClick={() => void detectFacesForSelectedPhoto()}
                disabled={!selectedPhotoId || detectingFaces}
                className="rounded border border-accent bg-accent/10 px-2 py-1 text-xs text-accent disabled:cursor-not-allowed disabled:opacity-50"
              >
                {detectingFaces ? "Erkennt…" : "Gesichter im aktuellen Foto erkennen"}
              </button>
              <button type="button" onClick={() => void clearPeopleModelPaths()} className="text-xs text-text-muted hover:text-danger">
                Modelle entfernen
              </button>
            </div>

            {selectedPhotoId && (
              <div>
                <p className="mb-1 text-xs text-text-muted">
                  {facesLoading ? "Lädt Gesichter…" : `${facesForSelectedPhoto.length} Gesicht(er) im aktuellen Foto`}
                </p>
                <ul className="flex flex-col gap-1">
                  {facesForSelectedPhoto.map((face) => {
                    const person = people.find((p) => p.id === face.person_id);
                    return (
                      <li key={face.id} className="flex items-center justify-between gap-2 rounded border border-border px-2 py-1 text-xs">
                        <span>{person ? person.name ?? "(unbenannt)" : "Nicht zugeordnet"}</span>
                        <div className="flex gap-1">
                          {!face.person_id && (
                            <button
                              type="button"
                              onClick={() => void assignFaceToPerson(face.id, null)}
                              className="rounded border border-border px-1.5 py-0.5 hover:border-accent"
                            >
                              Als neue Person markieren
                            </button>
                          )}
                          {face.person_id && (
                            <button
                              type="button"
                              onClick={() => void unassignFace(face.id)}
                              className="rounded border border-border px-1.5 py-0.5 hover:border-danger"
                            >
                              Zuordnung aufheben
                            </button>
                          )}
                        </div>
                      </li>
                    );
                  })}
                </ul>
              </div>
            )}

            <div>
              <p className="mb-1 text-xs font-semibold text-text-secondary">
                {peopleLoading ? "Lädt Personen…" : `Personen (${people.length})`}
              </p>
              <ul className="flex flex-col gap-2">
                {people.map((person) => (
                  <li key={person.id} className="rounded border border-border p-2">
                    <div className="flex items-center gap-2">
                      <input
                        type="text"
                        value={nameDrafts[person.id] ?? person.name ?? ""}
                        onChange={(e) => setNameDrafts((prev) => ({ ...prev, [person.id]: e.target.value }))}
                        onBlur={() => {
                          const draft = nameDrafts[person.id];
                          if (draft !== undefined && draft !== (person.name ?? "")) void renamePerson(person.id, draft || null);
                        }}
                        placeholder="(unbenannt)"
                        className="min-w-0 flex-1 rounded border border-border bg-bg-panel px-2 py-1 text-xs"
                      />
                      <button type="button" onClick={() => toggleExpanded(person.id)} className="rounded border border-border px-1.5 py-0.5 text-xs hover:border-accent">
                        {expandedPersonId === person.id ? "Fotos verbergen" : "Fotos zeigen"}
                      </button>
                      <button type="button" onClick={() => void deletePerson(person.id)} className="rounded border border-border px-1.5 py-0.5 text-xs hover:border-danger">
                        Löschen
                      </button>
                    </div>
                    {expandedPersonId === person.id && (
                      <div className="mt-2 flex flex-wrap gap-2">
                        {(personPhotos[person.id] ?? []).map((photo) => (
                          <button
                            key={photo.id}
                            type="button"
                            onClick={() => selectPhoto(photo.id)}
                            title={photo.filename}
                            className="h-16 w-16 shrink-0 overflow-hidden rounded border-2 border-transparent hover:border-accent"
                          >
                            <img src={previewUrl(photo.id, 0)} alt={photo.filename} className="h-full w-full object-cover" loading="lazy" />
                          </button>
                        ))}
                      </div>
                    )}
                  </li>
                ))}
                {people.length === 0 && <li className="text-xs text-text-muted">Noch keine Person erkannt — oben ein Foto öffnen und „Gesichter erkennen" klicken.</li>}
              </ul>
            </div>
          </div>
        )}
      </section>

      <section aria-label="Hautton-Gruppierung (Fallback)">
        <h3 className="mb-2 text-sm font-semibold text-text-primary">Hautton-Gruppierung (Fallback)</h3>
        {peopleGroupsLoading && <p className="text-sm text-text-muted">Gruppiert…</p>}
        {!peopleGroupsLoading && peopleGroups.length === 0 && (
          <p className="text-sm text-text-muted">
            Keine Gruppen gefunden — die Hautton-Heuristik erkennt entweder keine Gesichtsregionen oder findet keine zwei Fotos mit
            ähnlicher Blob-Signatur.
          </p>
        )}
        <div className="flex flex-col gap-4">
          {peopleGroups.map((group, index) => (
            <section key={index} aria-label={`Gruppe ${index + 1}`}>
              <h4 className="mb-2 text-xs text-text-muted">Gruppe {index + 1} ({group.length} Fotos)</h4>
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
        </div>
      </section>
    </main>
  );
}
