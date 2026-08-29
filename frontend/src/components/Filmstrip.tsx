import { useVirtualizer } from "@tanstack/react-virtual";
import { useRef } from "react";
import { useShallow } from "zustand/react/shallow";

import { previewUrl } from "../lib/media";
import { resolveSelectionMode, selectActivePhotos, useAppStore } from "../store";

// Feste Zellbreite statt individueller Seitenverhältnisse — hält die
// Virtualisierung einfach und schnell auch bei sehr vielen Fotos (Ziel:
// 50.000, siehe PHASE1_PROMPT.md Abschnitt 9) und entspricht dem üblichen
// Filmstreifen-Look (gleich große Zellen, Vorschau per `object-cover`
// zugeschnitten).
const CELL_WIDTH = 96;
const CELL_GAP = 4;

/**
 * Virtualisierter Filmstreifen (`@tanstack/react-virtual`): unabhängig
 * von der Gesamtanzahl der Fotos werden nur die im Container sichtbaren
 * Zellen plus ein kleiner Überhang (`overscan`) tatsächlich ins DOM
 * gerendert — das hält das Scrollen auch bei 50.000 Einträgen flüssig.
 */
export function Filmstrip() {
  const hasActiveContext = useAppStore(
    (s) => s.selectedFolderId !== null || s.selectedCollectionId !== null || s.libraryResults !== null,
  );
  const selectedPhotoId = useAppStore((s) => s.selectedPhotoId);
  const multiSelectedIds = useAppStore((s) => s.multiSelectedIds);
  // Geteilter Fotoliste-Zustand mit `GridView` (siehe `DECISIONS.md`
  // ADR-0024) — zeigt je nach aktivem Kontext den Ordner, die Sammlung
  // oder ein Such-/Filterergebnis, nicht mehr nur `photosByFolder` direkt.
  // `useShallow` statt einer bloßen Referenzprüfung — siehe `GridView.tsx`
  // für die Begründung (sonst Endlos-Rerender bei leerer Auswahl).
  const photos = useAppStore(useShallow(selectActivePhotos));
  const togglePhotoSelection = useAppStore((s) => s.togglePhotoSelection);

  const scrollRef = useRef<HTMLDivElement>(null);
  const virtualizer = useVirtualizer({
    count: photos.length,
    getScrollElement: () => scrollRef.current,
    horizontal: true,
    estimateSize: () => CELL_WIDTH + CELL_GAP,
    overscan: 8,
  });

  if (photos.length === 0) {
    return (
      <footer className="flex h-24 shrink-0 items-center justify-center border-t border-border bg-bg-raised text-sm text-text-muted">
        {hasActiveContext ? "Keine Fotos zum Anzeigen." : "Wähle links einen Ordner."}
      </footer>
    );
  }

  return (
    <footer ref={scrollRef} className="h-24 shrink-0 overflow-x-auto overflow-y-hidden border-t border-border bg-bg-raised">
      <div style={{ width: virtualizer.getTotalSize(), height: "100%", position: "relative" }}>
        {virtualizer.getVirtualItems().map((item) => {
          const photo = photos[item.index];
          if (!photo) return null;
          return (
            <button
              key={photo.id}
              type="button"
              onClick={(event) => togglePhotoSelection(photo.id, resolveSelectionMode(event))}
              // Siehe PHASE1_PROMPT.md Abschnitt 9, Akzeptanzkriterium 8:
              // eine außerhalb der App gelöschte Datei wird beim nächsten
              // Öffnen des Ordners als `missing` markiert (Backend:
              // `crate::reconcile`) — hier sichtbar gemacht, statt eine
              // stillschweigend tote DTO-Eigenschaft zu bleiben.
              title={photo.missing ? `${photo.filename} (Datei fehlt)` : photo.filename}
              style={{
                position: "absolute",
                left: item.start,
                top: 4,
                width: CELL_WIDTH,
                height: "calc(100% - 8px)",
              }}
              className={`relative overflow-hidden rounded border-2 ${
                photo.id === selectedPhotoId ? "border-accent" : multiSelectedIds.includes(photo.id) ? "border-accent/50" : "border-transparent hover:border-border"
              } ${photo.missing ? "opacity-40" : ""}`}
            >
              <img src={previewUrl(photo.id, 0)} alt={photo.filename} className="h-full w-full object-cover" loading="lazy" />
              {photo.missing && (
                <span className="absolute right-1 bottom-1 rounded bg-bg-base/80 px-1 text-[10px] leading-tight text-danger">fehlt</span>
              )}
            </button>
          );
        })}
      </div>
    </footer>
  );
}
