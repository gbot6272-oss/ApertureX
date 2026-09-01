import { useEffect, useState } from "react";

import { CommandPalette } from "./components/CommandPalette";
import { CompareGridView } from "./components/CompareGridView";
import { HistoryTimelineDialog } from "./components/HistoryTimelineDialog";
import { DevelopPanel } from "./components/DevelopPanel";
import { ErrorBanner } from "./components/ErrorBanner";
import { FilterBar } from "./components/FilterBar";
import { Filmstrip } from "./components/Filmstrip";
import { GridView } from "./components/GridView";
import { Header } from "./components/Header";
import { MapView } from "./components/MapView";
import { MasksPanel } from "./components/MasksPanel";
import { MetadataPanel } from "./components/MetadataPanel";
import { PresetsPanel } from "./components/PresetsPanel";
import { SettingsDialog } from "./components/SettingsDialog";
import { Sidebar } from "./components/Sidebar";
import { Viewer } from "./components/Viewer";
import { useImportEvents } from "./hooks/useImportEvents";
import { useAppStore } from "./store";

async function toggleFullscreen(): Promise<void> {
  if (document.fullscreenElement) {
    await document.exitFullscreen();
  } else {
    await document.documentElement.requestFullscreen();
  }
}

/**
 * Grundlayout aus `PHASE1_PROMPT.md` Abschnitt 7: Kopfzeile, linke Spalte
 * (Ordnerbaum/Sammlungen), Mitte (Viewer oder Raster, ab Phase 3 Schritt 6
 * umschaltbar), unten (Filmstreifen) — plus Befehlspalette und
 * grundlegende Tastenkürzel. Ein-/ausklappbare, breitenziehbare Paletten
 * mit speicherbarem Arbeitsbereich-Preset bleiben eine spätere
 * Ausbaustufe (`FEATURES.md`, UI-Anforderungen).
 */
export default function App() {
  useImportEvents();

  const refreshFolders = useAppStore((s) => s.refreshFolders);
  const refreshCatalogStatus = useAppStore((s) => s.refreshCatalogStatus);
  const loadUiSettings = useAppStore((s) => s.loadUiSettings);
  const settingsDialogOpen = useAppStore((s) => s.settingsDialogOpen);
  const setSettingsDialogOpen = useAppStore((s) => s.setSettingsDialogOpen);
  const stepSelection = useAppStore((s) => s.stepSelection);
  const centerView = useAppStore((s) => s.centerView);
  const selectedPhotoId = useAppStore((s) => s.selectedPhotoId);
  const setPhotoRating = useAppStore((s) => s.setPhotoRating);
  const setPhotoFlag = useAppStore((s) => s.setPhotoFlag);
  const developPanelOpen = useAppStore((s) => s.developPanelOpen);
  const undoLibraryAction = useAppStore((s) => s.undoLibraryAction);
  const redoLibraryAction = useAppStore((s) => s.redoLibraryAction);
  const [paletteOpen, setPaletteOpen] = useState(false);

  useEffect(() => {
    void refreshFolders();
    void refreshCatalogStatus();
    void loadUiSettings();
  }, [refreshFolders, refreshCatalogStatus, loadUiSettings]);

  useEffect(() => {
    function handleKeyDown(event: KeyboardEvent) {
      if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "k") {
        event.preventDefault();
        setPaletteOpen((open) => !open);
        return;
      }

      const target = event.target as HTMLElement | null;
      const isEditable =
        target !== null &&
        (target.tagName === "INPUT" ||
          target.tagName === "TEXTAREA" ||
          // Eigene interaktive Regel-Widgets (`ColorWheel.tsx`s Farbrad,
          // `CurveEditor.tsx`s Kurvenpunkte) sind `role="slider"`-Elemente
          // ohne natives Eingabe-Tag und behandeln Pfeiltasten selbst
          // (Feinjustierung von Farbton/Sättigung bzw. Kurvenpunkten) — ohne
          // diesen Ausschluss würde der globale Foto-Navigations-Kurzbefehl
          // (`stepSelection`, unten) parallel feuern und über
          // `loadDevelopStateForPhoto` die gerade vorgenommene Änderung
          // wieder überschreiben.
          target.closest('[role="slider"]') !== null);
      if (isEditable) return;

      // Rückgängig/Wiederholen für Bibliotheks-Metadaten (Schritt 8.1,
      // `DECISIONS.md` ADR-0027) — nur, wenn das Entwickeln-Panel nicht
      // offen ist: das hat schon seinen eigenen lokalen Ctrl/Cmd+Z-Handler
      // (siehe `DevelopPanel.tsx`), sonst würden beide Aktionen auf
      // denselben Tastendruck reagieren.
      if (!developPanelOpen && (event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "z") {
        event.preventDefault();
        if (event.shiftKey) {
          void redoLibraryAction();
        } else {
          void undoLibraryAction();
        }
        return;
      }

      if (event.key === "ArrowLeft") {
        stepSelection(-1);
      } else if (event.key === "ArrowRight") {
        stepSelection(1);
      } else if (event.key === "Escape") {
        setPaletteOpen(false);
      } else if (event.key.toLowerCase() === "f") {
        void toggleFullscreen();
      } else if (selectedPhotoId && /^[0-5]$/.test(event.key)) {
        // Bewertungs-Tastenkürzel (Lightroom-Konvention), siehe
        // `PLAN.md` Phase 3, Schritt 6.
        void setPhotoRating(selectedPhotoId, Number(event.key));
      } else if (selectedPhotoId && event.key.toLowerCase() === "p") {
        void setPhotoFlag(selectedPhotoId, 1);
      } else if (selectedPhotoId && event.key.toLowerCase() === "x") {
        void setPhotoFlag(selectedPhotoId, -1);
      }
    }

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [stepSelection, selectedPhotoId, setPhotoRating, setPhotoFlag, developPanelOpen, undoLibraryAction, redoLibraryAction]);

  return (
    <div className="flex h-screen flex-col bg-bg-base text-text-primary">
      <Header />
      <ErrorBanner />
      {centerView === "grid" && <FilterBar />}
      <div className="flex flex-1 overflow-hidden">
        <Sidebar />
        <PresetsPanel />
        {centerView === "grid" ? <GridView /> : centerView === "map" ? <MapView /> : <Viewer />}
        <MetadataPanel />
        {/* Rechte Werkzeug-Palette (Phase 10 Schritt 2): Entwickeln- und
            Masken-Panel bleiben zwei unabhängig sichtbare/aufklappbare
            Bereiche (nicht exklusiv verdeckende Reiter — viele bestehende
            e2e-Tests bedienen Entwickeln- und Maskenregler im selben
            Ablauf), aber unter einer gemeinsamen visuellen Außenhülle statt
            zweier lose nebeneinanderstehender <aside>s. */}
        <div className="flex shrink-0">
          <DevelopPanel />
          <MasksPanel />
        </div>
      </div>
      <Filmstrip />
      <CompareGridView />
      <HistoryTimelineDialog />
      <CommandPalette open={paletteOpen} onClose={() => setPaletteOpen(false)} />
      <SettingsDialog open={settingsDialogOpen} onClose={() => setSettingsDialogOpen(false)} />
    </div>
  );
}
