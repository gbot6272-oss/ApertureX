import { useCallback, useState } from "react";

import { selectFolderDialog } from "../lib/tauri";
import { useAppStore } from "../store";
import { ExportDialog } from "./ExportDialog";
import { ImportDialog } from "./ImportDialog";
import { PrintDialog } from "./PrintDialog";

export function Header() {
  const importRunning = useAppStore((s) => s.importRunning);
  const importProgress = useAppStore((s) => s.importProgress);
  const importResult = useAppStore((s) => s.importResult);
  const startImport = useAppStore((s) => s.startImport);
  const cancelImport = useAppStore((s) => s.cancelImport);
  const developPanelOpen = useAppStore((s) => s.developPanelOpen);
  const toggleDevelopPanel = useAppStore((s) => s.toggleDevelopPanel);
  const selectedPhotoId = useAppStore((s) => s.selectedPhotoId);
  const multiSelectedIds = useAppStore((s) => s.multiSelectedIds);
  const centerView = useAppStore((s) => s.centerView);
  const toggleCenterView = useAppStore((s) => s.toggleCenterView);
  const metadataPanelOpen = useAppStore((s) => s.metadataPanelOpen);
  const toggleMetadataPanel = useAppStore((s) => s.toggleMetadataPanel);
  const exportDialogOpen = useAppStore((s) => s.exportDialogOpen);
  const openExportDialog = useAppStore((s) => s.openExportDialog);
  const closeExportDialog = useAppStore((s) => s.closeExportDialog);
  const printDialogOpen = useAppStore((s) => s.printDialogOpen);
  const openPrintDialog = useAppStore((s) => s.openPrintDialog);
  const closePrintDialog = useAppStore((s) => s.closePrintDialog);
  const [importDialogSource, setImportDialogSource] = useState<string | null>(null);

  const exportPhotoIds = multiSelectedIds.length > 0 ? multiSelectedIds : selectedPhotoId ? [selectedPhotoId] : [];

  const handleImportClick = useCallback(async () => {
    const path = await selectFolderDialog();
    if (path) {
      await startImport(path);
    }
  }, [startImport]);

  const handleImportWithTemplateClick = useCallback(async () => {
    const path = await selectFolderDialog();
    if (path) setImportDialogSource(path);
  }, []);

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

      <button
        type="button"
        onClick={() => void handleImportWithTemplateClick()}
        disabled={importRunning}
        className="rounded border border-border bg-bg-panel px-3 py-1 text-sm hover:border-accent disabled:cursor-not-allowed disabled:opacity-50"
        title="Import mit wählbarem Modus (Kopieren/Verschieben), Umbenennungsmuster und Presets"
      >
        Import mit Vorlage…
      </button>

      <ImportDialog open={importDialogSource !== null} sourcePath={importDialogSource ?? ""} onClose={() => setImportDialogSource(null)} />

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
          {importResult.duplicateCount > 0 ? ` · ${importResult.duplicateCount} Duplikate` : ""}
        </span>
      )}

      <button
        type="button"
        onClick={toggleCenterView}
        aria-pressed={centerView === "grid"}
        className={`ml-auto rounded border px-3 py-1 text-sm ${
          centerView === "grid" ? "border-accent bg-accent/10 text-accent" : "border-border bg-bg-panel hover:border-accent"
        }`}
      >
        Raster
      </button>

      <button
        type="button"
        onClick={toggleMetadataPanel}
        disabled={!selectedPhotoId && !metadataPanelOpen}
        aria-pressed={metadataPanelOpen}
        className={`rounded border px-3 py-1 text-sm disabled:cursor-not-allowed disabled:opacity-50 ${
          metadataPanelOpen ? "border-accent bg-accent/10 text-accent" : "border-border bg-bg-panel hover:border-accent"
        }`}
      >
        Info
      </button>

      <button
        type="button"
        onClick={toggleDevelopPanel}
        disabled={!selectedPhotoId && !developPanelOpen}
        aria-pressed={developPanelOpen}
        className={`rounded border px-3 py-1 text-sm disabled:cursor-not-allowed disabled:opacity-50 ${
          developPanelOpen ? "border-accent bg-accent/10 text-accent" : "border-border bg-bg-panel hover:border-accent"
        }`}
      >
        Entwickeln
      </button>

      <button
        type="button"
        onClick={openExportDialog}
        disabled={exportPhotoIds.length === 0}
        className="rounded border border-border bg-bg-panel px-3 py-1 text-sm hover:border-accent disabled:cursor-not-allowed disabled:opacity-50"
      >
        Exportieren…
      </button>

      <ExportDialog open={exportDialogOpen} photoIds={exportPhotoIds} onClose={closeExportDialog} />

      <button
        type="button"
        onClick={openPrintDialog}
        disabled={exportPhotoIds.length === 0}
        className="rounded border border-border bg-bg-panel px-3 py-1 text-sm hover:border-accent disabled:cursor-not-allowed disabled:opacity-50"
      >
        Drucken…
      </button>

      <PrintDialog open={printDialogOpen} photoIds={exportPhotoIds} onClose={closePrintDialog} />

      <span className="text-xs text-text-muted">Strg/Cmd+K — Befehlspalette</span>
    </header>
  );
}
