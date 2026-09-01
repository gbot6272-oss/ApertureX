import { useEffect, useMemo, useState } from "react";

import { folderLabel } from "../lib/format";
import { buildChildrenByParent } from "../lib/folderTree";
import { selectFolderDialog } from "../lib/tauri";
import type { FolderDto } from "../lib/tauri";
import { useAppStore } from "../store";
import { PaletteFrame } from "./PaletteFrame";

interface FolderNodeProps {
  folder: FolderDto;
  depth: number;
  childrenOf: Map<string, FolderDto[]>;
}

function FolderNode({ folder, depth, childrenOf }: FolderNodeProps) {
  const selectedFolderId = useAppStore((s) => s.selectedFolderId);
  const selectFolder = useAppStore((s) => s.selectFolder);
  const relinkFolder = useAppStore((s) => s.relinkFolder);
  const children = childrenOf.get(folder.id) ?? [];

  async function handleRelink(event: React.MouseEvent) {
    event.stopPropagation();
    const newPath = await selectFolderDialog();
    if (newPath) {
      await relinkFolder(folder.id, newPath);
    }
  }

  return (
    <li>
      <button
        type="button"
        onClick={() => selectFolder(folder.id)}
        title={folder.path}
        style={{ paddingLeft: `${0.5 + depth * 1}rem` }}
        className={`flex w-full items-center justify-between rounded px-2 py-1.5 text-left text-sm hover:bg-bg-panel ${
          folder.id === selectedFolderId ? "bg-bg-panel text-text-primary" : "text-text-secondary"
        }`}
      >
        <span className="flex min-w-0 items-center gap-1.5">
          <span className="truncate">{folderLabel(folder.path)}</span>
          {folder.missing && (
            <span className="shrink-0 rounded bg-danger/20 px-1 text-[10px] font-semibold uppercase text-danger">
              fehlt
            </span>
          )}
        </span>
        <span className="ml-2 flex shrink-0 items-center gap-2">
          {folder.missing && (
            <span
              role="button"
              tabIndex={0}
              onClick={handleRelink}
              onKeyDown={(e) => e.key === "Enter" && handleRelink(e as unknown as React.MouseEvent)}
              className="text-xs text-accent hover:underline"
              title="Ordner an neuem Speicherort verknüpfen"
            >
              verknüpfen
            </span>
          )}
          <span className="text-xs text-text-muted">{folder.photo_count}</span>
        </span>
      </button>
      {children.length > 0 && (
        <ul className="space-y-0.5">
          {children.map((child) => (
            <FolderNode key={child.id} folder={child} depth={depth + 1} childrenOf={childrenOf} />
          ))}
        </ul>
      )}
    </li>
  );
}

/** Sammlungen-Abschnitt unterhalb des Ordnerbaums (Phase 3, Schritt 6) —
 * rein manuelle Sammlungen (siehe `DECISIONS.md` ADR-0023), Klick wählt sie
 * wie einen Ordner aus (`selectCollection`, gegenseitig ausschließend mit
 * `selectedFolderId`, siehe `store/index.ts`). */
function CollectionsSection() {
  const collections = useAppStore((s) => s.collections);
  const selectedCollectionId = useAppStore((s) => s.selectedCollectionId);
  const refreshCollections = useAppStore((s) => s.refreshCollections);
  const createCollection = useAppStore((s) => s.createCollection);
  const selectCollection = useAppStore((s) => s.selectCollection);
  const addSelectionToCollection = useAppStore((s) => s.addSelectionToCollection);
  const hasSelection = useAppStore((s) => s.selectedPhotoId !== null || s.multiSelectedIds.length > 0);
  const [creating, setCreating] = useState(false);
  const [newName, setNewName] = useState("");

  useEffect(() => {
    void refreshCollections();
  }, [refreshCollections]);

  async function handleCreate() {
    if (!newName.trim()) {
      setCreating(false);
      return;
    }
    await createCollection(newName);
    setNewName("");
    setCreating(false);
  }

  return (
    <div className="mt-4">
      <div className="flex items-center justify-between px-2 pb-2">
        <h2 className="text-xs font-semibold tracking-wide text-text-secondary uppercase">Sammlungen</h2>
        <button
          type="button"
          onClick={() => setCreating(true)}
          aria-label="Neue Sammlung"
          title="Neue Sammlung"
          className="text-text-muted hover:text-text-primary"
        >
          +
        </button>
      </div>

      {creating && (
        <div className="px-2 pb-2">
          <input
            type="text"
            autoFocus
            value={newName}
            onChange={(event) => setNewName(event.target.value)}
            onKeyDown={(event) => {
              if (event.key === "Enter") void handleCreate();
              if (event.key === "Escape") setCreating(false);
            }}
            onBlur={() => void handleCreate()}
            placeholder="Name der Sammlung…"
            className="w-full rounded border border-border bg-bg-panel px-2 py-1 text-sm"
          />
        </div>
      )}

      {collections.length === 0 && !creating && <p className="px-2 text-sm text-text-muted">Noch keine Sammlungen.</p>}

      <ul className="space-y-0.5">
        {collections.map((collection) => (
          <li key={collection.id} className="group flex items-center">
            <button
              type="button"
              onClick={() => selectCollection(collection.id)}
              className={`flex min-w-0 flex-1 items-center rounded px-2 py-1.5 text-left text-sm hover:bg-bg-panel ${
                collection.id === selectedCollectionId ? "bg-bg-panel text-text-primary" : "text-text-secondary"
              }`}
            >
              <span className="truncate">{collection.name}</span>
            </button>
            {hasSelection && (
              <button
                type="button"
                onClick={() => void addSelectionToCollection(collection.id)}
                aria-label="Auswahl zu dieser Sammlung hinzufügen"
                title="Auswahl zu dieser Sammlung hinzufügen"
                className="shrink-0 px-1.5 text-xs text-text-muted opacity-0 hover:text-accent group-hover:opacity-100"
              >
                +
              </button>
            )}
          </li>
        ))}
      </ul>
    </div>
  );
}

export function Sidebar() {
  const folders = useAppStore((s) => s.folders);
  const { roots, childrenOf } = useMemo(() => buildChildrenByParent(folders), [folders]);

  return (
    <PaletteFrame id="sidebar" side="left" defaultWidth={256} label="Ordner" className="border-r border-border bg-bg-raised p-2">
      <h2 className="px-2 pb-2 text-xs font-semibold tracking-wide text-text-secondary uppercase">Ordner</h2>

      {folders.length === 0 && <p className="px-2 text-sm text-text-muted">Noch keine Ordner importiert.</p>}

      <ul className="space-y-0.5">
        {roots.map((folder) => (
          <FolderNode key={folder.id} folder={folder} depth={0} childrenOf={childrenOf} />
        ))}
      </ul>

      <CollectionsSection />
    </PaletteFrame>
  );
}
