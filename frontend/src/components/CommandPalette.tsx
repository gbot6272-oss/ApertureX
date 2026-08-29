import { useEffect, useMemo, useState } from "react";

import { folderLabel } from "../lib/format";
import { useAppStore } from "../store";

interface PaletteEntry {
  id: string;
  label: string;
  hint?: string;
  run: () => void;
}

/**
 * Befehlspalette auf Strg/Cmd+K — laut `PHASE1_PROMPT.md` Abschnitt 7 in
 * Phase 1 nur als "Grundgerüst, findet Ordner und Befehle". Der volle
 * Ausbau (jede Funktion und jedes Preset) kommt mit dem Preset-System in
 * Phase 5.
 */
export function CommandPalette({ open, onClose }: { open: boolean; onClose: () => void }) {
  const [query, setQuery] = useState("");
  const folders = useAppStore((s) => s.folders);
  const selectFolder = useAppStore((s) => s.selectFolder);
  const cancelImport = useAppStore((s) => s.cancelImport);
  const importRunning = useAppStore((s) => s.importRunning);

  useEffect(() => {
    if (open) setQuery("");
  }, [open]);

  const entries = useMemo<PaletteEntry[]>(() => {
    const commandEntries: PaletteEntry[] = importRunning ? [{ id: "cmd:cancel-import", label: "Import abbrechen", run: () => void cancelImport() }] : [];
    const folderEntries: PaletteEntry[] = folders.map((folder) => ({
      id: `folder:${folder.id}`,
      label: folderLabel(folder.path),
      hint: folder.path,
      run: () => selectFolder(folder.id),
    }));
    return [...commandEntries, ...folderEntries];
  }, [folders, importRunning, selectFolder, cancelImport]);

  const filtered = entries.filter((entry) => entry.label.toLowerCase().includes(query.toLowerCase()));

  if (!open) return null;

  return (
    <div className="fixed inset-0 z-50 flex items-start justify-center bg-black/50 pt-32" onClick={onClose}>
      <div className="w-full max-w-lg rounded-lg border border-border bg-bg-raised shadow-xl" onClick={(event) => event.stopPropagation()}>
        <input
          autoFocus
          value={query}
          onChange={(event) => setQuery(event.target.value)}
          onKeyDown={(event) => {
            if (event.key === "Escape") onClose();
            if (event.key === "Enter" && filtered[0]) {
              filtered[0].run();
              onClose();
            }
          }}
          placeholder="Ordner oder Befehl suchen…"
          className="w-full border-b border-border bg-transparent px-4 py-3 text-sm outline-none placeholder:text-text-muted"
        />
        <ul className="max-h-80 overflow-y-auto p-1">
          {filtered.length === 0 && <li className="px-3 py-2 text-sm text-text-muted">Keine Treffer.</li>}
          {filtered.map((entry) => (
            <li key={entry.id}>
              <button
                type="button"
                onClick={() => {
                  entry.run();
                  onClose();
                }}
                className="flex w-full flex-col rounded px-3 py-2 text-left text-sm hover:bg-bg-panel"
              >
                <span>{entry.label}</span>
                {entry.hint && <span className="text-xs text-text-muted">{entry.hint}</span>}
              </button>
            </li>
          ))}
        </ul>
      </div>
    </div>
  );
}
