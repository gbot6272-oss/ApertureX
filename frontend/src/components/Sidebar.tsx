import { folderLabel } from "../lib/format";
import { useAppStore } from "../store";

export function Sidebar() {
  const folders = useAppStore((s) => s.folders);
  const selectedFolderId = useAppStore((s) => s.selectedFolderId);
  const selectFolder = useAppStore((s) => s.selectFolder);

  return (
    <aside className="w-64 shrink-0 overflow-y-auto border-r border-border bg-bg-raised p-2">
      <h2 className="px-2 pb-2 text-xs font-semibold tracking-wide text-text-secondary uppercase">Ordner</h2>

      {folders.length === 0 && <p className="px-2 text-sm text-text-muted">Noch keine Ordner importiert.</p>}

      <ul className="space-y-0.5">
        {folders.map((folder) => (
          <li key={folder.id}>
            <button
              type="button"
              onClick={() => selectFolder(folder.id)}
              title={folder.path}
              className={`flex w-full items-center justify-between rounded px-2 py-1.5 text-left text-sm hover:bg-bg-panel ${
                folder.id === selectedFolderId ? "bg-bg-panel text-text-primary" : "text-text-secondary"
              }`}
            >
              <span className="truncate">{folderLabel(folder.path)}</span>
              <span className="ml-2 shrink-0 text-xs text-text-muted">{folder.photo_count}</span>
            </button>
          </li>
        ))}
      </ul>
    </aside>
  );
}
