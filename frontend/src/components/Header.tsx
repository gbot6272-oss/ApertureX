import { useCallback, useState } from "react";

import { selectFolderDialog } from "../lib/tauri";
import { useAppStore } from "../store";
import { BookDialog } from "./BookDialog";
import { WebDialog } from "./WebDialog";
import { ExportDialog } from "./ExportDialog";
import { ImportDialog } from "./ImportDialog";
import { PrintDialog } from "./PrintDialog";
import { SlideshowDialog } from "./SlideshowDialog";
import { TemplatesDialog } from "./TemplatesDialog";
import { LibraryOrganizeDialog } from "./LibraryOrganizeDialog";
import { StackingDialog } from "./StackingDialog";
import { ScriptPluginDialog } from "./ScriptPluginDialog";
import { ShareDialog } from "./ShareDialog";
import { TetherDialog } from "./TetherDialog";
import { MetadataDialog } from "./MetadataDialog";
import { StatsCacheDialog } from "./StatsCacheDialog";

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
  const setCenterView = useAppStore((s) => s.setCenterView);
  const metadataPanelOpen = useAppStore((s) => s.metadataPanelOpen);
  const toggleMetadataPanel = useAppStore((s) => s.toggleMetadataPanel);
  const exportDialogOpen = useAppStore((s) => s.exportDialogOpen);
  const openExportDialog = useAppStore((s) => s.openExportDialog);
  const closeExportDialog = useAppStore((s) => s.closeExportDialog);
  const printDialogOpen = useAppStore((s) => s.printDialogOpen);
  const openPrintDialog = useAppStore((s) => s.openPrintDialog);
  const closePrintDialog = useAppStore((s) => s.closePrintDialog);
  const slideshowDialogOpen = useAppStore((s) => s.slideshowDialogOpen);
  const openSlideshowDialog = useAppStore((s) => s.openSlideshowDialog);
  const closeSlideshowDialog = useAppStore((s) => s.closeSlideshowDialog);
  const bookDialogOpen = useAppStore((s) => s.bookDialogOpen);
  const openBookDialog = useAppStore((s) => s.openBookDialog);
  const closeBookDialog = useAppStore((s) => s.closeBookDialog);
  const webDialogOpen = useAppStore((s) => s.webDialogOpen);
  const openWebDialog = useAppStore((s) => s.openWebDialog);
  const closeWebDialog = useAppStore((s) => s.closeWebDialog);
  const [importDialogSource, setImportDialogSource] = useState<string | null>(null);
  const [templatesDialogOpen, setTemplatesDialogOpen] = useState(false);
  const [organizeDialogOpen, setOrganizeDialogOpen] = useState(false);
  const [stackingDialogOpen, setStackingDialogOpen] = useState(false);
  const [scriptPluginDialogOpen, setScriptPluginDialogOpen] = useState(false);
  const [shareDialogOpen, setShareDialogOpen] = useState(false);
  const [tetherDialogOpen, setTetherDialogOpen] = useState(false);
  const [metadataDialogOpen, setMetadataDialogOpen] = useState(false);
  const [statsDialogOpen, setStatsDialogOpen] = useState(false);
  const openCompareView = useAppStore((s) => s.openCompareView);
  const openVersionsCompareView = useAppStore((s) => s.openVersionsCompareView);
  const openSecondaryDisplay = useAppStore((s) => s.openSecondaryDisplay);

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
    <header className="flex h-12 shrink-0 items-center gap-4 overflow-x-auto border-b border-border bg-bg-raised px-4">
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
          <div className="h-1.5 w-48 shrink-0 overflow-hidden rounded bg-bg-panel">
            <div className="h-full bg-accent transition-[width] duration-150" style={{ width: `${percent}%` }} />
          </div>
          <span className="max-w-xs shrink-0 truncate text-xs text-text-secondary">
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
        onClick={() => setCenterView(centerView === "map" ? "viewer" : "map")}
        aria-pressed={centerView === "map"}
        className={`rounded border px-3 py-1 text-sm ${
          centerView === "map" ? "border-accent bg-accent/10 text-accent" : "border-border bg-bg-panel hover:border-accent"
        }`}
      >
        Karte
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

      <button
        type="button"
        onClick={openSlideshowDialog}
        disabled={exportPhotoIds.length === 0}
        className="rounded border border-border bg-bg-panel px-3 py-1 text-sm hover:border-accent disabled:cursor-not-allowed disabled:opacity-50"
      >
        Diashow…
      </button>

      <SlideshowDialog open={slideshowDialogOpen} photoIds={exportPhotoIds} onClose={closeSlideshowDialog} />

      <button
        type="button"
        onClick={openBookDialog}
        disabled={exportPhotoIds.length === 0}
        className="rounded border border-border bg-bg-panel px-3 py-1 text-sm hover:border-accent disabled:cursor-not-allowed disabled:opacity-50"
      >
        Buch…
      </button>

      <BookDialog open={bookDialogOpen} photoIds={exportPhotoIds} onClose={closeBookDialog} />

      <button
        type="button"
        onClick={openWebDialog}
        disabled={exportPhotoIds.length === 0}
        className="rounded border border-border bg-bg-panel px-3 py-1 text-sm hover:border-accent disabled:cursor-not-allowed disabled:opacity-50"
      >
        Web…
      </button>

      <WebDialog open={webDialogOpen} photoIds={exportPhotoIds} onClose={closeWebDialog} />

      <button
        type="button"
        onClick={() => setTemplatesDialogOpen(true)}
        className="rounded border border-border bg-bg-panel px-3 py-1 text-sm hover:border-accent"
      >
        Vorlagen…
      </button>

      <TemplatesDialog open={templatesDialogOpen} photoIds={exportPhotoIds} onClose={() => setTemplatesDialogOpen(false)} />

      <button
        type="button"
        onClick={() => setOrganizeDialogOpen(true)}
        className="rounded border border-border bg-bg-panel px-3 py-1 text-sm hover:border-accent"
      >
        Organisieren…
      </button>

      <LibraryOrganizeDialog open={organizeDialogOpen} onClose={() => setOrganizeDialogOpen(false)} />

      <button
        type="button"
        onClick={() => setStackingDialogOpen(true)}
        className="rounded border border-border bg-bg-panel px-3 py-1 text-sm hover:border-accent"
      >
        Stacking…
      </button>

      <StackingDialog open={stackingDialogOpen} onClose={() => setStackingDialogOpen(false)} />

      <button
        type="button"
        onClick={() => setScriptPluginDialogOpen(true)}
        className="rounded border border-border bg-bg-panel px-3 py-1 text-sm hover:border-accent"
      >
        Skript &amp; Plugins…
      </button>

      <ScriptPluginDialog open={scriptPluginDialogOpen} onClose={() => setScriptPluginDialogOpen(false)} />

      <button
        type="button"
        onClick={() => setShareDialogOpen(true)}
        className="rounded border border-border bg-bg-panel px-3 py-1 text-sm hover:border-accent"
      >
        Kollaboration…
      </button>

      <ShareDialog open={shareDialogOpen} onClose={() => setShareDialogOpen(false)} />

      <button
        type="button"
        onClick={() => setTetherDialogOpen(true)}
        className="rounded border border-border bg-bg-panel px-3 py-1 text-sm hover:border-accent"
      >
        Tethering…
      </button>

      <TetherDialog open={tetherDialogOpen} onClose={() => setTetherDialogOpen(false)} />

      <button
        type="button"
        onClick={() => setMetadataDialogOpen(true)}
        className="rounded border border-border bg-bg-panel px-3 py-1 text-sm hover:border-accent"
      >
        Metadaten…
      </button>

      <MetadataDialog open={metadataDialogOpen} onClose={() => setMetadataDialogOpen(false)} />

      <button
        type="button"
        onClick={() => openCompareView(exportPhotoIds)}
        disabled={exportPhotoIds.length < 2}
        title="Ausgewählte Fotos nebeneinander vergleichen"
        className="rounded border border-border bg-bg-panel px-3 py-1 text-sm hover:border-accent disabled:opacity-40"
      >
        Vergleichen
      </button>

      <button
        type="button"
        onClick={() => void openVersionsCompareView()}
        disabled={!selectedPhotoId}
        title="Aktuelles Foto und seine virtuellen Kopien nebeneinander vergleichen (Phase 9 Schritt 7)"
        className="rounded border border-border bg-bg-panel px-3 py-1 text-sm hover:border-accent disabled:opacity-40"
      >
        Versionen vergleichen
      </button>

      <button
        type="button"
        onClick={() => selectedPhotoId && void openSecondaryDisplay(selectedPhotoId)}
        disabled={!selectedPhotoId}
        title="Aktuelles Foto in einem zweiten Fenster anzeigen"
        className="rounded border border-border bg-bg-panel px-3 py-1 text-sm hover:border-accent disabled:opacity-40"
      >
        Zweites Display…
      </button>

      <button
        type="button"
        onClick={() => setStatsDialogOpen(true)}
        className="rounded border border-border bg-bg-panel px-3 py-1 text-sm hover:border-accent"
      >
        Statistik…
      </button>

      <StatsCacheDialog open={statsDialogOpen} onClose={() => setStatsDialogOpen(false)} />

      <span className="text-xs text-text-muted">Strg/Cmd+K — Befehlspalette</span>
    </header>
  );
}
