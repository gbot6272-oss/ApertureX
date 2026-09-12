import { useCallback, useEffect, useState } from "react";

import { MENU_CATEGORIES, useCommandRegistry, type AiFeatureStatus, type CommandCategory } from "../lib/commandRegistry";
import { useT, type TranslationKey } from "../lib/i18n";
import { selectFolderDialog } from "../lib/tauri";
import { useAppStore } from "../store";
import { BookDialog } from "./BookDialog";
import { WebDialog } from "./WebDialog";
import { ExportDialog } from "./ExportDialog";
import { ImportDialog } from "./ImportDialog";
import { PrintDialog } from "./PrintDialog";
import { SlideshowDialog } from "./SlideshowDialog";
import { VideoTimelineDialog } from "./VideoTimelineDialog";
import { TemplatesDialog } from "./TemplatesDialog";
import { LibraryOrganizeDialog } from "./LibraryOrganizeDialog";
import { BatchConsoleDialog } from "./BatchConsoleDialog";
import { StackingDialog } from "./StackingDialog";
import { ScriptPluginDialog } from "./ScriptPluginDialog";
import { ShareDialog } from "./ShareDialog";
import { TetherDialog } from "./TetherDialog";
import { MetadataDialog } from "./MetadataDialog";
import { StatsCacheDialog } from "./StatsCacheDialog";
import { CatalogDialog } from "./CatalogDialog";
import { Menu, type MenuSection } from "./ui/Menu";

/** Übersetzungsschlüssel je Overflow-Menü-Kategorie (Phase 18 Schritt 3).
 * `navigation`/`system` sind hier nur der Typvollständigkeit halber
 * vertreten — sie tauchen laut `MENU_CATEGORIES` nie im Menü auf. */
const CATEGORY_LABEL_KEY: Record<CommandCategory, TranslationKey> = {
  ai: "commands.category.ai",
  output: "header.group.output",
  templates: "header.group.templates",
  advanced: "header.group.advanced",
  analysis: "header.group.analysis",
  navigation: "header.viewGrid",
  system: "header.settings",
};

export function Header({ onOpenPalette }: { onOpenPalette: () => void }) {
  const t = useT();
  const importRunning = useAppStore((s) => s.importRunning);
  const importProgress = useAppStore((s) => s.importProgress);
  const importResult = useAppStore((s) => s.importResult);
  const startImport = useAppStore((s) => s.startImport);
  const cancelImport = useAppStore((s) => s.cancelImport);
  const setSettingsDialogOpen = useAppStore((s) => s.setSettingsDialogOpen);
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
  const closeExportDialog = useAppStore((s) => s.closeExportDialog);
  const printDialogOpen = useAppStore((s) => s.printDialogOpen);
  const closePrintDialog = useAppStore((s) => s.closePrintDialog);
  const slideshowDialogOpen = useAppStore((s) => s.slideshowDialogOpen);
  const closeSlideshowDialog = useAppStore((s) => s.closeSlideshowDialog);
  const videoTimelineDialogOpen = useAppStore((s) => s.videoTimelineDialogOpen);
  const closeVideoTimelineDialog = useAppStore((s) => s.closeVideoTimelineDialog);
  const bookDialogOpen = useAppStore((s) => s.bookDialogOpen);
  const closeBookDialog = useAppStore((s) => s.closeBookDialog);
  const webDialogOpen = useAppStore((s) => s.webDialogOpen);
  const closeWebDialog = useAppStore((s) => s.closeWebDialog);
  const [importDialogSource, setImportDialogSource] = useState<string | null>(null);
  const [templatesDialogOpen, setTemplatesDialogOpen] = useState(false);
  const [organizeDialogOpen, setOrganizeDialogOpen] = useState(false);
  const [batchConsoleDialogOpen, setBatchConsoleDialogOpen] = useState(false);
  const [stackingDialogOpen, setStackingDialogOpen] = useState(false);
  const [scriptPluginDialogOpen, setScriptPluginDialogOpen] = useState(false);
  const [shareDialogOpen, setShareDialogOpen] = useState(false);
  const [tetherDialogOpen, setTetherDialogOpen] = useState(false);
  const [metadataDialogOpen, setMetadataDialogOpen] = useState(false);
  const [statsDialogOpen, setStatsDialogOpen] = useState(false);
  const [catalogDialogOpen, setCatalogDialogOpen] = useState(false);
  const pendingCommand = useAppStore((s) => s.pendingCommand);
  const clearPendingCommand = useAppStore((s) => s.clearPendingCommand);

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

  // Brücke für die vollständige Befehlspalette (Phase 10 Schritt 4, siehe
  // `store/index.ts`s `pendingCommand`-Moduldoku): diese neun Dialoge sind
  // bewusst lokaler `useState` in dieser Komponente geblieben, die
  // Befehlspalette ist aber kein Kind von `Header.tsx` und kann sie daher
  // nicht direkt öffnen. Unverändert seit Phase 10 — das neue
  // Kommando-Register (Phase 18 Schritt 3) ruft für diese IDs weiterhin
  // nur `requestCommand(id)` auf statt sie selbst zu öffnen.
  useEffect(() => {
    if (!pendingCommand) return;
    switch (pendingCommand) {
      case "import":
        void handleImportClick();
        break;
      case "import-template":
        void handleImportWithTemplateClick();
        break;
      case "templates":
        setTemplatesDialogOpen(true);
        break;
      case "organize":
        setOrganizeDialogOpen(true);
        break;
      case "batch-console":
        setBatchConsoleDialogOpen(true);
        break;
      case "stacking":
        setStackingDialogOpen(true);
        break;
      case "script-plugin":
        setScriptPluginDialogOpen(true);
        break;
      case "share":
        setShareDialogOpen(true);
        break;
      case "tether":
        setTetherDialogOpen(true);
        break;
      case "metadata":
        setMetadataDialogOpen(true);
        break;
      case "stats":
        setStatsDialogOpen(true);
        break;
      case "catalog":
        setCatalogDialogOpen(true);
        break;
      default:
        return;
    }
    clearPendingCommand();
  }, [pendingCommand, clearPendingCommand, handleImportClick, handleImportWithTemplateClick]);

  // Zentrales Kommando-Register (Phase 18 Schritt 3, siehe
  // `lib/commandRegistry.ts`s Moduldoku + `DECISIONS.md` ADR-0046
  // Entwurfsentscheidung 4) — dieselbe Quelle, aus der auch
  // `CommandPalette.tsx` ihre Einträge zieht. Hier nur zum Aufbau des
  // Overflow-Menüs verwendet, gruppiert nach Kategorie.
  const commands = useCommandRegistry();
  const aiStatusLabel: Record<AiFeatureStatus, string> = {
    ready: t("commands.ai.status.ready"),
    "download-needed": t("commands.ai.status.downloadNeeded"),
    "not-available": t("commands.ai.status.notAvailable"),
  };
  const menuSections: MenuSection[] = MENU_CATEGORIES.map((category) => ({
    id: category,
    label: t(CATEGORY_LABEL_KEY[category]),
    items: commands
      .filter((entry) => entry.category === category)
      .map((entry) => ({
        id: entry.id,
        label: entry.label,
        disabled: entry.disabled,
        hint: entry.aiStatus ? aiStatusLabel[entry.aiStatus] : undefined,
        onSelect: entry.run,
      })),
  })).filter((section) => section.items.length > 0);

  return (
    // Eine einzige schlanke Zeile statt der vormaligen zwei Zeilen mit
    // ~30 Einzelknöpfen (Phase 18 Schritt 3, siehe `DECISIONS.md`
    // ADR-0046 Entwurfsentscheidung 5): Logo, Import, ein auffälliger
    // Such-/Befehlsknopf (macht die Befehlspalette zum primären statt
    // versteckten Werkzeug), die sechs Ansicht-Ziele als zusammenhängende
    // Segment-Gruppe, ein Overflow-Menü für die vier bisherigen
    // Zeile-2-Gruppen (jetzt aus dem Kommando-Register gespeist) und
    // Einstellungen. Kein Knopf wurde entfernt oder hinter mehr als
    // einer zusätzlichen Ebene versteckt — nur die dauerhaft sichtbare
    // Knopfzahl sinkt drastisch.
    <header className="apx-glass flex h-12 shrink-0 items-center gap-3 border-b border-[var(--glass-border)] px-4">
      <span className="shrink-0 font-semibold tracking-wide">Aperture X</span>

      <button
        type="button"
        data-tour="import"
        onClick={() => void handleImportClick()}
        disabled={importRunning}
        className="shrink-0 rounded border border-border bg-bg-panel px-3 py-1 text-sm transition-colors duration-[var(--duration-fast)] hover:border-accent disabled:cursor-not-allowed disabled:opacity-50"
      >
        {t("header.importFolder")}
      </button>

      <ImportDialog open={importDialogSource !== null} sourcePath={importDialogSource ?? ""} onClose={() => setImportDialogSource(null)} />

      <button
        type="button"
        data-tour="search"
        onClick={onOpenPalette}
        title={t("header.paletteHint")}
        className="flex shrink-0 items-center gap-2 rounded border border-border bg-bg-panel px-3 py-1 text-sm text-text-secondary transition-colors duration-[var(--duration-fast)] hover:border-accent hover:text-text-primary"
      >
        <span aria-hidden>⌘</span>
        {t("header.search")}
      </button>

      {importRunning && (
        <>
          <div className="h-1.5 w-40 shrink-0 overflow-hidden rounded bg-bg-panel">
            <div className="h-full bg-accent transition-[width] duration-150" style={{ width: `${percent}%` }} />
          </div>
          <span className="max-w-xs shrink-0 truncate text-xs text-text-secondary">
            {importProgress ? `${importProgress.done} / ${importProgress.total}${importProgress.currentFile ? ` — ${importProgress.currentFile}` : ""}` : ""}
          </span>
          <button
            type="button"
            onClick={() => void cancelImport()}
            className="shrink-0 rounded border border-danger px-2 py-1 text-xs text-danger hover:bg-danger/10"
          >
            {t("header.cancelImport")}
          </button>
        </>
      )}

      {!importRunning && importResult && (
        <span className="apx-notice-in hidden shrink-0 truncate text-xs text-text-secondary lg:inline">
          {importResult.cancelled ? "Import abgebrochen: " : "Import abgeschlossen: "}
          {importResult.imported} importiert · {importResult.skipped} übersprungen
          {importResult.errorCount > 0 ? ` · ${importResult.errorCount} Fehler` : ""}
          {importResult.duplicateCount > 0 ? ` · ${importResult.duplicateCount} Duplikate` : ""}
        </span>
      )}

      <nav aria-label="Ansicht" data-tour="views" className="ml-auto flex shrink-0 items-center gap-0.5 rounded border border-border bg-bg-panel p-0.5">
        <button
          type="button"
          onClick={toggleCenterView}
          aria-pressed={centerView === "grid"}
          className={`rounded px-3 py-1 text-sm transition-colors duration-[var(--duration-fast)] ${
            centerView === "grid" ? "bg-accent/10 text-accent" : "text-text-secondary hover:text-text-primary"
          }`}
        >
          {t("header.viewGrid")}
        </button>

        <button
          type="button"
          onClick={() => setCenterView(centerView === "overview" ? "viewer" : "overview")}
          aria-pressed={centerView === "overview"}
          className={`rounded px-3 py-1 text-sm transition-colors duration-[var(--duration-fast)] ${
            centerView === "overview" ? "bg-accent/10 text-accent" : "text-text-secondary hover:text-text-primary"
          }`}
        >
          {t("header.viewOverview")}
        </button>

        <button
          type="button"
          onClick={() => setCenterView(centerView === "map" ? "viewer" : "map")}
          aria-pressed={centerView === "map"}
          className={`rounded px-3 py-1 text-sm transition-colors duration-[var(--duration-fast)] ${
            centerView === "map" ? "bg-accent/10 text-accent" : "text-text-secondary hover:text-text-primary"
          }`}
        >
          {t("header.viewMap")}
        </button>

        <button
          type="button"
          onClick={() => setCenterView(centerView === "people" ? "viewer" : "people")}
          aria-pressed={centerView === "people"}
          className={`rounded px-3 py-1 text-sm transition-colors duration-[var(--duration-fast)] ${
            centerView === "people" ? "bg-accent/10 text-accent" : "text-text-secondary hover:text-text-primary"
          }`}
        >
          {t("header.viewPeople")}
        </button>

        <button
          type="button"
          onClick={toggleMetadataPanel}
          disabled={!selectedPhotoId && !metadataPanelOpen}
          aria-pressed={metadataPanelOpen}
          className={`rounded px-3 py-1 text-sm transition-colors duration-[var(--duration-fast)] disabled:cursor-not-allowed disabled:opacity-50 ${
            metadataPanelOpen ? "bg-accent/10 text-accent" : "text-text-secondary hover:text-text-primary"
          }`}
        >
          {t("header.viewInfo")}
        </button>

        <button
          type="button"
          data-tour="develop"
          onClick={toggleDevelopPanel}
          disabled={!selectedPhotoId && !developPanelOpen}
          aria-pressed={developPanelOpen}
          className={`rounded px-3 py-1 text-sm transition-colors duration-[var(--duration-fast)] disabled:cursor-not-allowed disabled:opacity-50 ${
            developPanelOpen ? "bg-accent/10 text-accent" : "text-text-secondary hover:text-text-primary"
          }`}
        >
          {t("header.viewDevelop")}
        </button>
      </nav>

      <Menu
        label={t("header.overflowMenu")}
        trigger={<span aria-hidden>⋯</span>}
        sections={menuSections}
      />

      <button
        type="button"
        onClick={() => setSettingsDialogOpen(true)}
        title={t("header.settingsTitle")}
        className="shrink-0 rounded border border-border bg-bg-panel px-3 py-1 text-sm transition-colors duration-[var(--duration-fast)] hover:border-accent"
      >
        {t("header.settings")}
      </button>

      {/* Die übrigen, in Zeile 2 vormals nebenstehenden Dialoge bleiben
          hier gerendert (Inhalt/Props/Store-Anbindung unverändert) — nur
          ihre Auslöser-Knöpfe sind jetzt Einträge im Kommando-Register/
          Overflow-Menü statt eigener sichtbarer Knöpfe. */}
      <ExportDialog open={exportDialogOpen} photoIds={exportPhotoIds} onClose={closeExportDialog} />
      <PrintDialog open={printDialogOpen} photoIds={exportPhotoIds} onClose={closePrintDialog} />
      <SlideshowDialog open={slideshowDialogOpen} photoIds={exportPhotoIds} onClose={closeSlideshowDialog} />
      <VideoTimelineDialog open={videoTimelineDialogOpen} photoIds={exportPhotoIds} onClose={closeVideoTimelineDialog} />
      <BookDialog open={bookDialogOpen} photoIds={exportPhotoIds} onClose={closeBookDialog} />
      <WebDialog open={webDialogOpen} photoIds={exportPhotoIds} onClose={closeWebDialog} />
      <TemplatesDialog open={templatesDialogOpen} photoIds={exportPhotoIds} onClose={() => setTemplatesDialogOpen(false)} />
      <LibraryOrganizeDialog open={organizeDialogOpen} onClose={() => setOrganizeDialogOpen(false)} />
      <BatchConsoleDialog open={batchConsoleDialogOpen} onClose={() => setBatchConsoleDialogOpen(false)} />
      <MetadataDialog open={metadataDialogOpen} onClose={() => setMetadataDialogOpen(false)} />
      <StackingDialog open={stackingDialogOpen} onClose={() => setStackingDialogOpen(false)} />
      <ScriptPluginDialog open={scriptPluginDialogOpen} onClose={() => setScriptPluginDialogOpen(false)} />
      <ShareDialog open={shareDialogOpen} onClose={() => setShareDialogOpen(false)} />
      <TetherDialog open={tetherDialogOpen} onClose={() => setTetherDialogOpen(false)} />
      <StatsCacheDialog open={statsDialogOpen} onClose={() => setStatsDialogOpen(false)} />
      <CatalogDialog open={catalogDialogOpen} onClose={() => setCatalogDialogOpen(false)} />
    </header>
  );
}
