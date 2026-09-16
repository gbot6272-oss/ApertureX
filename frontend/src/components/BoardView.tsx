import { FolderOpen, Layers, Plus, X } from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { useShallow } from "zustand/react/shallow";

import { previewUrl } from "../lib/media";
import { playCue } from "../lib/sound";
import type { PhotoDto } from "../lib/tauri";
import { selectActivePhotos, useAppStore } from "../store";

/**
 * Sammlungs-Board (Phase 32 F9).
 *
 * Sammlungen gibt es seit Phase 9, aber der einzige Weg hinein war
 * „Foto auswählen, Sammlung im Menü anklicken" — ein Vorgang, der
 * immer nur in eine Richtung und immer nur in eine Sammlung führt. Wer
 * einen Shooting-Ertrag auf „Auswahl", „Vielleicht" und „Kunde" verteilt,
 * arbeitet dabei blind: man sieht nie, was schon wo liegt.
 *
 * Das Board zeigt jede Sammlung als Spalte mit ihren Fotos und lässt
 * Bilder zwischen den Spalten ziehen. Links die Spalte „Ordner" mit den
 * Fotos des aktuellen Ordners, die noch in keiner Sammlung sind — der
 * Stapel, den man abarbeitet.
 *
 * **Ziehen verschiebt, es kopiert nicht.** Ein Foto kann technisch in
 * mehreren Sammlungen liegen; beim Sortieren ist das aber fast nie
 * gemeint — „Auswahl" und „Aussortiert" schließen sich aus. Wer ein Foto
 * zusätzlich in eine zweite Sammlung will, benutzt weiterhin den
 * bestehenden Weg über die Mehrfachauswahl.
 *
 * **Reines HTML5-Ziehen, keine Bibliothek** — dieselbe Linie wie beim
 * Verzicht auf eine i18n- oder Animations-Bibliothek: für Karten
 * zwischen Spalten reicht `dragstart`/`dragover`/`drop`.
 */

/** Spalten-Kennung: `null` ist die Ordner-Spalte (noch keiner Sammlung
 * zugeordnet), sonst die Sammlungs-ID. */
type ColumnId = string | null;

export function BoardView() {
  const collections = useAppStore((s) => s.collections);
  const collectionPhotos = useAppStore((s) => s.collectionPhotos);
  const folderPhotos = useAppStore(useShallow(selectActivePhotos));
  const loadAll = useAppStore((s) => s.loadAllCollectionPhotos);
  const refreshCollections = useAppStore((s) => s.refreshCollections);
  const createCollection = useAppStore((s) => s.createCollection);
  const movePhoto = useAppStore((s) => s.moveCollectionPhoto);
  const removePhoto = useAppStore((s) => s.removeCollectionPhoto);
  const selectPhoto = useAppStore((s) => s.selectPhoto);
  const selectedPhotoId = useAppStore((s) => s.selectedPhotoId);

  const [dragging, setDragging] = useState<{ photoId: string; from: ColumnId } | null>(null);
  const [hoveredColumn, setHoveredColumn] = useState<ColumnId | "none">("none");
  const [newName, setNewName] = useState("");

  useEffect(() => {
    void refreshCollections();
  }, [refreshCollections]);

  useEffect(() => {
    void loadAll();
  }, [loadAll, collections.length]);

  /** Fotos des Ordners, die in keiner Sammlung liegen. */
  const unassigned = useMemo(() => {
    const assigned = new Set<string>();
    for (const list of Object.values(collectionPhotos)) {
      for (const photo of list) assigned.add(photo.id);
    }
    return folderPhotos.filter((photo) => !assigned.has(photo.id));
  }, [folderPhotos, collectionPhotos]);

  async function handleDrop(target: ColumnId) {
    const payload = dragging;
    setDragging(null);
    setHoveredColumn("none");
    if (!payload) return;
    if (payload.from === target) return;
    playCue("drop");
    if (target === null) {
      // Zurück in die Ordner-Spalte heißt: aus der Sammlung nehmen.
      if (payload.from) await removePhoto(payload.photoId, payload.from);
      return;
    }
    await movePhoto(payload.photoId, payload.from, target);
  }

  async function addCollection() {
    const name = newName.trim();
    if (!name) return;
    await createCollection(name);
    setNewName("");
  }

  return (
    // `pt-16` wie in den anderen Vollflächen-Ansichten: die schwebende
    // Kopfzeile nimmt keinen Platz im Dokumentfluss ein.
    <main data-testid="board-view" className="flex flex-1 flex-col gap-3 overflow-hidden px-4 pt-16 pb-4">
      <header className="flex flex-wrap items-center gap-2">
        <h2 className="flex items-center gap-2 text-base font-semibold text-text-primary">
          <Layers aria-hidden="true" className="size-4" />
          Sammlungs-Board
        </h2>
        <span className="text-xs text-text-muted" data-testid="board-summary">
          {unassigned.length} unsortiert · {collections.length} {collections.length === 1 ? "Sammlung" : "Sammlungen"}
        </span>
        <span className="flex-1" />
        <input
          type="text"
          value={newName}
          onChange={(event) => setNewName(event.target.value)}
          onKeyDown={(event) => {
            if (event.key === "Enter") void addCollection();
          }}
          placeholder="Neue Sammlung…"
          aria-label="Name der neuen Sammlung"
          className="w-48 rounded border border-border bg-bg-base px-2 py-1 text-xs text-text-primary"
        />
        <button
          type="button"
          onClick={() => void addCollection()}
          disabled={!newName.trim()}
          className="apx-btn-liquid flex items-center gap-1 rounded border border-accent bg-accent/10 px-2 py-1 text-xs text-accent disabled:opacity-40"
        >
          <Plus aria-hidden="true" className="size-3.5" />
          Anlegen
        </button>
      </header>

      <div className="flex min-h-0 flex-1 gap-3 overflow-x-auto pb-2">
        <Column
          id={null}
          title="Ordner"
          hint="Fotos des aktuellen Ordners, die in keiner Sammlung liegen."
          photos={unassigned}
          icon={<FolderOpen aria-hidden="true" className="size-3.5" />}
          hovered={hoveredColumn === null}
          selectedPhotoId={selectedPhotoId}
          onDragStart={(photoId) => setDragging({ photoId, from: null })}
          onDragEnterColumn={() => setHoveredColumn(null)}
          onDrop={() => void handleDrop(null)}
          onPick={selectPhoto}
        />

        {collections.map((collection) => (
          <Column
            key={collection.id}
            id={collection.id}
            title={collection.name}
            photos={collectionPhotos[collection.id] ?? []}
            icon={<Layers aria-hidden="true" className="size-3.5" />}
            hovered={hoveredColumn === collection.id}
            selectedPhotoId={selectedPhotoId}
            onDragStart={(photoId) => setDragging({ photoId, from: collection.id })}
            onDragEnterColumn={() => setHoveredColumn(collection.id)}
            onDrop={() => void handleDrop(collection.id)}
            onPick={selectPhoto}
            onRemove={(photoId) => void removePhoto(photoId, collection.id)}
          />
        ))}

        {collections.length === 0 && (
          <p className="self-center text-xs text-text-muted">
            Noch keine Sammlung. Oben rechts eine anlegen — dann lassen sich Fotos hineinziehen.
          </p>
        )}
      </div>

      <p className="text-[11px] text-text-muted">
        Ziehen verschiebt: das Foto verlässt seine bisherige Sammlung. Zurück in die Ordner-Spalte gezogen, ist es in keiner
        Sammlung mehr.
      </p>
    </main>
  );
}

interface ColumnProps {
  id: ColumnId;
  title: string;
  hint?: string;
  photos: PhotoDto[];
  icon: React.ReactNode;
  hovered: boolean;
  selectedPhotoId: string | null;
  onDragStart: (photoId: string) => void;
  onDragEnterColumn: () => void;
  onDrop: () => void;
  onPick: (photoId: string) => void;
  onRemove?: (photoId: string) => void;
}

function Column({
  id,
  title,
  hint,
  photos,
  icon,
  hovered,
  selectedPhotoId,
  onDragStart,
  onDragEnterColumn,
  onDrop,
  onPick,
  onRemove,
}: ColumnProps) {
  return (
    <section
      aria-label={title}
      data-testid="board-column"
      data-column-id={id ?? "unassigned"}
      onDragOver={(event) => {
        // Ohne `preventDefault` lehnt der Browser das Ablegen ab — der
        // häufigste Grund, warum HTML5-Drag-and-Drop "nicht geht".
        event.preventDefault();
        onDragEnterColumn();
      }}
      onDrop={(event) => {
        event.preventDefault();
        onDrop();
      }}
      className={`flex w-56 shrink-0 flex-col rounded-lg border p-2 transition-colors duration-[var(--duration-fast)] ${
        hovered ? "border-accent bg-accent/5" : "border-border bg-bg-panel"
      }`}
    >
      <div className="mb-2">
        <h3 className="flex items-center gap-1.5 text-xs font-semibold text-text-primary">
          {icon}
          <span className="truncate">{title}</span>
          <span className="ml-auto tabular-nums text-text-muted">{photos.length}</span>
        </h3>
        {hint && <p className="mt-0.5 text-[10px] text-text-muted">{hint}</p>}
      </div>

      <ul className="flex min-h-0 flex-1 flex-col gap-1.5 overflow-y-auto">
        {photos.map((photo) => (
          <li key={photo.id}>
            <div
              draggable
              onDragStart={(event) => {
                // Der Nutzdatensatz liegt im React-Zustand; `dataTransfer`
                // braucht trotzdem irgendeinen Inhalt, sonst starten
                // manche Browser den Zieh-Vorgang gar nicht erst.
                event.dataTransfer.setData("text/plain", photo.id);
                event.dataTransfer.effectAllowed = "move";
                onDragStart(photo.id);
              }}
              onClick={() => onPick(photo.id)}
              role="button"
              tabIndex={0}
              onKeyDown={(event) => {
                if (event.key === "Enter" || event.key === " ") {
                  event.preventDefault();
                  onPick(photo.id);
                }
              }}
              data-testid="board-card"
              data-photo-id={photo.id}
              title={photo.filename}
              // Eigenes Label: ohne dieses setzt sich der Name der Karte
              // aus ihrem Inhalt zusammen — also auch aus der
              // Beschriftung des Entfernen-Knopfes darin, und beide
              // Elemente hörten auf denselben Namen.
              aria-label={photo.filename}
              className={`flex cursor-grab items-center gap-2 rounded border p-1 text-left transition-colors duration-[var(--duration-fast)] ${
                photo.id === selectedPhotoId ? "border-accent" : "border-border hover:border-accent/60"
              }`}
            >
              <img src={previewUrl(photo.id)} alt="" draggable={false} className="size-10 shrink-0 rounded object-cover" />
              <span className="min-w-0 flex-1 truncate text-[11px] text-text-secondary">{photo.filename}</span>
              {onRemove && (
                <button
                  type="button"
                  onClick={(event) => {
                    event.stopPropagation();
                    onRemove(photo.id);
                  }}
                  aria-label={`${photo.filename} aus ${title} entfernen`}
                  className="rounded p-0.5 text-text-muted hover:text-danger"
                >
                  <X aria-hidden="true" className="size-3" />
                </button>
              )}
            </div>
          </li>
        ))}
        {photos.length === 0 && (
          <li className="rounded border border-dashed border-border p-3 text-center text-[11px] text-text-muted">
            Fotos hierher ziehen
          </li>
        )}
      </ul>
    </section>
  );
}
