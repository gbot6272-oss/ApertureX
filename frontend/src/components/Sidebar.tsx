import { useMemo } from "react";

import { folderLabel } from "../lib/format";
import { buildChildrenByParent } from "../lib/folderTree";
import { selectFolderDialog } from "../lib/tauri";
import type { FolderDto } from "../lib/tauri";
import { useAppStore } from "../store";

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

export function Sidebar() {
  const folders = useAppStore((s) => s.folders);
  const { roots, childrenOf } = useMemo(() => buildChildrenByParent(folders), [folders]);

  return (
    <aside className="w-64 shrink-0 overflow-y-auto border-r border-border bg-bg-raised p-2">
      <h2 className="px-2 pb-2 text-xs font-semibold tracking-wide text-text-secondary uppercase">Ordner</h2>

      {folders.length === 0 && <p className="px-2 text-sm text-text-muted">Noch keine Ordner importiert.</p>}

      <ul className="space-y-0.5">
        {roots.map((folder) => (
          <FolderNode key={folder.id} folder={folder} depth={0} childrenOf={childrenOf} />
        ))}
      </ul>
    </aside>
  );
}
