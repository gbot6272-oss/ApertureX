import { useEffect, useMemo, useState } from "react";
import { useShallow } from "zustand/react/shallow";

import { folderLabel } from "../lib/format";
import { selectActivePhotos, useAppStore } from "../store";

interface PaletteEntry {
  id: string;
  label: string;
  hint?: string;
  run: () => void;
}

/**
 * Vollständige Befehlspalette (Phase 10 Schritt 4, siehe `FEATURES.md`
 * UI-Anforderungen — vorher nur "Grundgerüst": Ordner + ein Befehl).
 * Vier Quellen durchsuchbarer Einträge: alle Header-Funktionen (Dialoge
 * über `store`-Actionen direkt, die neun `Header.tsx`-lokalen Dialoge
 * über die `pendingCommand`-Brücke, siehe `store/index.ts`s Moduldoku),
 * alle Presets (wendet das Preset auf das aktuelle Entwickeln-Foto an),
 * alle Fotos des aktuell gewählten Ordners (wählt das Foto aus), sowie
 * die bereits vorhandenen Ordner-Einträge.
 */
export function CommandPalette({ open, onClose }: { open: boolean; onClose: () => void }) {
  const [query, setQuery] = useState("");
  const folders = useAppStore((s) => s.folders);
  const selectFolder = useAppStore((s) => s.selectFolder);
  const cancelImport = useAppStore((s) => s.cancelImport);
  const importRunning = useAppStore((s) => s.importRunning);
  const requestCommand = useAppStore((s) => s.requestCommand);
  const toggleCenterView = useAppStore((s) => s.toggleCenterView);
  const setCenterView = useAppStore((s) => s.setCenterView);
  const toggleMetadataPanel = useAppStore((s) => s.toggleMetadataPanel);
  const toggleDevelopPanel = useAppStore((s) => s.toggleDevelopPanel);
  const openExportDialog = useAppStore((s) => s.openExportDialog);
  const openPrintDialog = useAppStore((s) => s.openPrintDialog);
  const openSlideshowDialog = useAppStore((s) => s.openSlideshowDialog);
  const openBookDialog = useAppStore((s) => s.openBookDialog);
  const openWebDialog = useAppStore((s) => s.openWebDialog);
  const setSettingsDialogOpen = useAppStore((s) => s.setSettingsDialogOpen);
  const selectedPhotoId = useAppStore((s) => s.selectedPhotoId);
  const multiSelectedIds = useAppStore((s) => s.multiSelectedIds);
  const openCompareView = useAppStore((s) => s.openCompareView);
  const openVersionsCompareView = useAppStore((s) => s.openVersionsCompareView);
  const openSecondaryDisplay = useAppStore((s) => s.openSecondaryDisplay);
  const presets = useAppStore((s) => s.presets);
  const applyPreset = useAppStore((s) => s.applyPreset);
  const photos = useAppStore(useShallow(selectActivePhotos));
  const selectPhoto = useAppStore((s) => s.selectPhoto);

  const exportPhotoIds = multiSelectedIds.length > 0 ? multiSelectedIds : selectedPhotoId ? [selectedPhotoId] : [];

  useEffect(() => {
    if (open) setQuery("");
  }, [open]);

  const entries = useMemo<PaletteEntry[]>(() => {
    const commandEntries: PaletteEntry[] = importRunning ? [{ id: "cmd:cancel-import", label: "Import abbrechen", run: () => void cancelImport() }] : [];

    const functionEntries: PaletteEntry[] = [
      { id: "fn:import", label: "Ordner importieren", run: () => requestCommand("import") },
      { id: "fn:import-template", label: "Import mit Vorlage…", run: () => requestCommand("import-template") },
      { id: "fn:view-grid", label: "Raster", run: toggleCenterView },
      { id: "fn:view-map", label: "Karte", run: () => setCenterView("map") },
      { id: "fn:view-info", label: "Info", run: toggleMetadataPanel },
      { id: "fn:view-develop", label: "Entwickeln", run: toggleDevelopPanel },
      { id: "fn:export", label: "Exportieren…", run: openExportDialog },
      { id: "fn:print", label: "Drucken…", run: openPrintDialog },
      { id: "fn:slideshow", label: "Diashow…", run: openSlideshowDialog },
      { id: "fn:book", label: "Buch…", run: openBookDialog },
      { id: "fn:web", label: "Web…", run: openWebDialog },
      { id: "fn:templates", label: "Vorlagen…", run: () => requestCommand("templates") },
      { id: "fn:organize", label: "Organisieren…", run: () => requestCommand("organize") },
      { id: "fn:stacking", label: "Stacking…", run: () => requestCommand("stacking") },
      { id: "fn:script-plugin", label: "Skript & Plugins…", run: () => requestCommand("script-plugin") },
      { id: "fn:share", label: "Kollaboration…", run: () => requestCommand("share") },
      { id: "fn:tether", label: "Tethering…", run: () => requestCommand("tether") },
      { id: "fn:metadata", label: "Metadaten…", run: () => requestCommand("metadata") },
      { id: "fn:compare", label: "Vergleichen", run: () => openCompareView(exportPhotoIds) },
      { id: "fn:versions-compare", label: "Versionen vergleichen", run: () => void openVersionsCompareView() },
      {
        id: "fn:secondary-display",
        label: "Zweites Display…",
        run: () => selectedPhotoId && void openSecondaryDisplay(selectedPhotoId),
      },
      { id: "fn:stats", label: "Statistik…", run: () => requestCommand("stats") },
      { id: "fn:settings", label: "Einstellungen…", run: () => setSettingsDialogOpen(true) },
      { id: "fn:onboarding", label: "Erste Schritte anzeigen", run: () => requestCommand("onboarding") },
      { id: "fn:cheatsheet", label: "Tastenkürzel-Übersicht anzeigen", run: () => requestCommand("cheatsheet-overlay") },
    ];

    const presetEntries: PaletteEntry[] = presets.map((preset) => ({
      id: `preset:${preset.id}`,
      label: preset.name,
      hint: "Preset anwenden",
      run: () => void applyPreset(preset.id),
    }));

    const photoEntries: PaletteEntry[] = photos.map((photo) => ({
      id: `photo:${photo.id}`,
      label: photo.filename,
      hint: "Foto auswählen",
      run: () => selectPhoto(photo.id),
    }));

    const folderEntries: PaletteEntry[] = folders.map((folder) => ({
      id: `folder:${folder.id}`,
      label: folderLabel(folder.path),
      hint: folder.path,
      run: () => selectFolder(folder.id),
    }));

    return [...commandEntries, ...functionEntries, ...presetEntries, ...photoEntries, ...folderEntries];
  }, [
    folders,
    importRunning,
    selectFolder,
    cancelImport,
    requestCommand,
    toggleCenterView,
    setCenterView,
    toggleMetadataPanel,
    toggleDevelopPanel,
    openExportDialog,
    openPrintDialog,
    openSlideshowDialog,
    openBookDialog,
    openWebDialog,
    setSettingsDialogOpen,
    openCompareView,
    openVersionsCompareView,
    openSecondaryDisplay,
    selectedPhotoId,
    exportPhotoIds,
    presets,
    applyPreset,
    photos,
    selectPhoto,
  ]);

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
          placeholder="Befehl, Preset, Foto oder Ordner suchen…"
          className="w-full border-b border-border bg-transparent px-4 py-3 text-sm outline-none placeholder:text-text-muted"
        />
        <ul className="max-h-80 overflow-y-auto p-1">
          {filtered.length === 0 && <li className="px-3 py-2 text-sm text-text-muted">Keine Treffer.</li>}
          {filtered.slice(0, 50).map((entry) => (
            <li key={entry.id}>
              <button
                type="button"
                onClick={() => {
                  entry.run();
                  onClose();
                }}
                className="flex w-full items-center justify-between gap-2 rounded px-3 py-1.5 text-left text-sm hover:bg-bg-panel"
              >
                <span className="truncate">{entry.label}</span>
                {entry.hint && <span className="shrink-0 text-xs text-text-muted">{entry.hint}</span>}
              </button>
            </li>
          ))}
        </ul>
      </div>
    </div>
  );
}
