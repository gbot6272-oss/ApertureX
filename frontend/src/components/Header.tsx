import { useCallback } from "react";

import { selectFolderDialog } from "../lib/tauri";
import { useAppStore } from "../store";

export function Header() {
  const importRunning = useAppStore((s) => s.importRunning);
  const importProgress = useAppStore((s) => s.importProgress);
  const importResult = useAppStore((s) => s.importResult);
  const startImport = useAppStore((s) => s.startImport);
  const cancelImport = useAppStore((s) => s.cancelImport);

  const handleImportClick = useCallback(async () => {
    const path = await selectFolderDialog();
    if (path) {
      await startImport(path);
    }
  }, [startImport]);

  const percent = importProgress && importProgress.total > 0 ? Math.round((importProgress.done / importProgress.total) * 100) : 0;

  return (
    <header className="flex h-12 shrink-0 items-center gap-4 border-b border-border bg-bg-raised px-4">
      <span className="font-semibold tracking-wide">Aperture X</span>

      <button
        type="button"
        onClick={() => void handleImportClick()}
        disabled={importRunning}
        className="rounded border border-border bg-bg-panel px-3 py-1 text-sm hover:border-accent disabled:cursor-not-allowed disabled:opacity-50"
      >
        Ordner importieren
      </button>

      {importRunning && (
        <>
          <div className="h-1.5 w-48 overflow-hidden rounded bg-bg-panel">
            <div className="h-full bg-accent transition-[width] duration-150" style={{ width: `${percent}%` }} />
          </div>
          <span className="max-w-xs truncate text-xs text-text-secondary">
            {importProgress ? `${importProgress.done} / ${importProgress.total}${importProgress.currentFile ? ` — ${importProgress.currentFile}` : ""}` : ""}
          </span>
          <button
            type="button"
            onClick={() => void cancelImport()}
            className="ml-auto shrink-0 rounded border border-danger px-2 py-1 text-xs text-danger hover:bg-danger/10"
          >
            Abbrechen
          </button>
        </>
      )}

      {!importRunning && importResult && (
        <span className="text-xs text-text-secondary">
          {importResult.cancelled ? "Import abgebrochen: " : "Import abgeschlossen: "}
          {importResult.imported} importiert · {importResult.skipped} übersprungen
          {importResult.errorCount > 0 ? ` · ${importResult.errorCount} Fehler` : ""}
        </span>
      )}

      <span className="ml-auto text-xs text-text-muted">Strg/Cmd+K — Befehlspalette</span>
    </header>
  );
}
