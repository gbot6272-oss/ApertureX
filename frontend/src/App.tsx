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
import { KeybindingsCheatsheet } from "./components/KeybindingsCheatsheet";
import { MapView } from "./components/MapView";
import { MasksPanel } from "./components/MasksPanel";
import { MetadataPanel } from "./components/MetadataPanel";
import { PresetsPanel } from "./components/PresetsPanel";
import { SettingsDialog } from "./components/SettingsDialog";
import { Sidebar } from "./components/Sidebar";
import { Viewer } from "./components/Viewer";
import { useImportEvents } from "./hooks/useImportEvents";
import { matchesBinding } from "./lib/keybindings";
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
  const uiSettings = useAppStore((s) => s.uiSettings);
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
  const [cheatsheetOpen, setCheatsheetOpen] = useState(false);

  useEffect(() => {
    void refreshFolders();
    void refreshCatalogStatus();
    void loadUiSettings();
  }, [refreshFolders, refreshCatalogStatus, loadUiSettings]);

  // Barrierefreiheit (Phase 10 Schritt 6): Kontrastmodus/UI-Skalierung/
  // reduzierte Bewegung wirken app-weit auf `<html>`, nicht nur innerhalb
  // dieser Komponente — deshalb hier statt in `SettingsDialog.tsx`
  // angewendet, das nur die Werte schreibt. Theme (Dark/Hell/Akzentfarbe)
  // folgt in Schritt 7 nach demselben Muster.
  useEffect(() => {
    const root = document.documentElement;
    if (uiSettings?.high_contrast) {
      root.setAttribute("data-contrast", "high");
    } else {
      root.removeAttribute("data-contrast");
    }
    root.classList.toggle("apx-reduce-motion", uiSettings?.reduced_motion ?? false);
    root.style.fontSize = uiSettings ? `${uiSettings.ui_scale_percent}%` : "";
  }, [uiSettings]);

  useEffect(() => {
    function handleKeyDown(event: KeyboardEvent) {
      // Vollständig belegbare Tastenkürzel (Phase 10 Schritt 5, siehe
      // `lib/keybindings.ts`) — dieselbe Verzweigung wie zuvor, jetzt über
      // `matchesBinding` statt fest verdrahteter `event.key`-Vergleiche,
      // damit jede hier behandelte Aktion im Cheatsheet-Overlay (`?`)
      // umbelegbar ist. Reihenfolge/Wächter (mod+k vor dem
      // Editierbar-Ausschluss, Undo/Redo nur bei geschlossenem
      // Entwickeln-Panel) sind unverändert aus der vorherigen Fassung
      // übernommen.
      if (matchesBinding(event, "toggle-palette")) {
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
      if (!developPanelOpen && matchesBinding(event, "redo")) {
        event.preventDefault();
        void redoLibraryAction();
        return;
      }
      if (!developPanelOpen && matchesBinding(event, "undo")) {
        event.preventDefault();
        void undoLibraryAction();
        return;
      }

      if (matchesBinding(event, "prev-photo")) {
        stepSelection(-1);
      } else if (matchesBinding(event, "next-photo")) {
        stepSelection(1);
      } else if (matchesBinding(event, "cheatsheet")) {
        setCheatsheetOpen((open) => !open);
      } else if (matchesBinding(event, "close-overlay")) {
        setCheatsheetOpen(false);
        setPaletteOpen(false);
      } else if (matchesBinding(event, "fullscreen")) {
        void toggleFullscreen();
      } else if (selectedPhotoId && /^[0-5]$/.test(event.key)) {
        // Bewertungs-Tastenkürzel (Lightroom-Konvention), siehe
        // `PLAN.md` Phase 3, Schritt 6 — bewusst nicht Teil von
        // `lib/keybindings.ts`: eine parametrisierte Ziffernreihe statt
        // einer einzelnen festen Aktion.
        void setPhotoRating(selectedPhotoId, Number(event.key));
      } else if (selectedPhotoId && matchesBinding(event, "flag-pick")) {
        void setPhotoFlag(selectedPhotoId, 1);
      } else if (selectedPhotoId && matchesBinding(event, "flag-reject")) {
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
      <KeybindingsCheatsheet open={cheatsheetOpen} onClose={() => setCheatsheetOpen(false)} />
    </div>
  );
}
