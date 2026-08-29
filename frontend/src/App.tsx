import { useEffect, useState } from "react";

import { CommandPalette } from "./components/CommandPalette";
import { DevelopPanel } from "./components/DevelopPanel";
import { ErrorBanner } from "./components/ErrorBanner";
import { Filmstrip } from "./components/Filmstrip";
import { Header } from "./components/Header";
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
 * (Ordnerbaum), Mitte (Viewer), unten (Filmstreifen) — plus Befehlspalette
 * und grundlegende Tastenkürzel. Ein-/ausklappbare, breitenziehbare
 * Paletten mit speicherbarem Arbeitsbereich-Preset sind Phase 3
 * (`FEATURES.md`, UI-Anforderungen).
 */
export default function App() {
  useImportEvents();

  const refreshFolders = useAppStore((s) => s.refreshFolders);
  const refreshCatalogStatus = useAppStore((s) => s.refreshCatalogStatus);
  const stepSelection = useAppStore((s) => s.stepSelection);
  const [paletteOpen, setPaletteOpen] = useState(false);

  useEffect(() => {
    void refreshFolders();
    void refreshCatalogStatus();
  }, [refreshFolders, refreshCatalogStatus]);

  useEffect(() => {
    function handleKeyDown(event: KeyboardEvent) {
      if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "k") {
        event.preventDefault();
        setPaletteOpen((open) => !open);
        return;
      }

      const target = event.target as HTMLElement | null;
      const isEditable = target !== null && (target.tagName === "INPUT" || target.tagName === "TEXTAREA");
      if (isEditable) return;

      if (event.key === "ArrowLeft") {
        stepSelection(-1);
      } else if (event.key === "ArrowRight") {
        stepSelection(1);
      } else if (event.key === "Escape") {
        setPaletteOpen(false);
      } else if (event.key.toLowerCase() === "f") {
        void toggleFullscreen();
      }
    }

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [stepSelection]);

  return (
    <div className="flex h-screen flex-col bg-bg-base text-text-primary">
      <Header />
      <ErrorBanner />
      <div className="flex flex-1 overflow-hidden">
        <Sidebar />
        <Viewer />
        <DevelopPanel />
      </div>
      <Filmstrip />
      <CommandPalette open={paletteOpen} onClose={() => setPaletteOpen(false)} />
    </div>
  );
}
