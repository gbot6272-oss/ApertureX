import { useEffect, useRef, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";

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
import { OnboardingDialog } from "./components/OnboardingDialog";
import { PeopleView } from "./components/PeopleView";
import { PresetsPanel } from "./components/PresetsPanel";
import { SettingsDialog } from "./components/SettingsDialog";
import { Sidebar } from "./components/Sidebar";
import { StartupSplash } from "./components/StartupSplash";
import { VideoPlayer } from "./components/VideoPlayer";
import { Viewer } from "./components/Viewer";
import { ShutdownOverlay } from "./components/ShutdownOverlay";
import { useImportEvents } from "./hooks/useImportEvents";
import { matchesBinding } from "./lib/keybindings";
import { usePrefersReducedMotion } from "./lib/motion";
import { applySoundSettings, playCue, useAccordionSounds, useUnlockSoundOnFirstInteraction } from "./lib/sound";
import { useAppStore } from "./store";

const SHUTDOWN_TRANSITION_MS = 420;

/**
 * Beenden-Übergang (Phase 19, siehe `ShutdownOverlay.tsx`s Moduldoku) —
 * fängt das erste Schließen-Ereignis des Fensters ab (`preventDefault`),
 * zeigt kurz die Übergangsfläche + spielt einen Sound, dann schließt das
 * Fenster tatsächlich. `confirmedRef` verhindert eine Endlosschleife:
 * das zweite (selbst ausgelöste) `close()` darf nicht erneut abgefangen
 * werden.
 *
 * `getCurrentWindow()` liest synchron `window.__TAURI_INTERNALS__.
 * metadata.currentWindow.label` (siehe `@tauri-apps/api/window`s
 * Quelltext) — sowohl in Playwright-Tests (`tauri-mock.ts` setzt kein
 * `metadata`-Feld) als auch im per `vite preview` ohne Tauri-Hülle
 * geöffneten Browser-Tab ist dieses Feld nicht vorhanden, der Aufruf
 * wirft dann sofort. Ein ungefangener Wurf in einem `useEffect` reißt
 * in React die ganze Baum-Wurzel mit (genau das brach beim ersten
 * Versuch dieser Funktion die komplette Test-Suite) — deshalb hier
 * bewusst in `try`/`catch`: fehlt der echte Tauri-Fensterkontext, bleibt
 * dieser Hook ein folgenloser No-op, statt die App zum Absturz zu
 * bringen.
 */
function useShutdownTransition(): boolean {
  const [closing, setClosing] = useState(false);
  const confirmedRef = useRef(false);
  const reducedMotion = usePrefersReducedMotion();

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    try {
      void getCurrentWindow()
        .onCloseRequested((event) => {
          if (confirmedRef.current) return;
          event.preventDefault();
          setClosing(true);
          playCue("stop");
          const delay = reducedMotion ? 0 : SHUTDOWN_TRANSITION_MS;
          setTimeout(() => {
            confirmedRef.current = true;
            try {
              void getCurrentWindow().close();
            } catch {
              // Siehe Moduldoku oben.
            }
          }, delay);
        })
        .then((fn) => {
          unlisten = fn;
        })
        .catch(() => {
          // Kein echtes Tauri-Fenster (Browser-Vorschau/Tests) — siehe
          // Moduldoku oben.
        });
    } catch {
      // Siehe Moduldoku oben.
    }
    return () => unlisten?.();
  }, [reducedMotion]);

  return closing;
}

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
  const saveUiSettings = useAppStore((s) => s.saveUiSettings);
  const settingsDialogOpen = useAppStore((s) => s.settingsDialogOpen);
  const setSettingsDialogOpen = useAppStore((s) => s.setSettingsDialogOpen);
  const pendingCommand = useAppStore((s) => s.pendingCommand);
  const clearPendingCommand = useAppStore((s) => s.clearPendingCommand);
  const [onboardingOpen, setOnboardingOpen] = useState(false);
  const onboardingAutoShown = useRef(false);
  const stepSelection = useAppStore((s) => s.stepSelection);
  const centerView = useAppStore((s) => s.centerView);
  const selectedPhotoId = useAppStore((s) => s.selectedPhotoId);
  // Video als Katalog-Asset (Phase 16 Schritt 5, siehe `DECISIONS.md`
  // ADR-0043): dieselbe Foto-Nachschlage-Konvention wie `Viewer.tsx` —
  // entscheidet, ob der Einzel-Element-Fallback-Zweig unten `Viewer`
  // (Foto) oder `VideoPlayer` (Video) zeigt. Bewusst KEIN eigener
  // `centerView`-Wert: es gibt in dieser App keine automatische
  // Grid→Einzelansicht-Navigation beim Öffnen (ADR-0037) — die einzige
  // Stelle, die zwischen der Einzel-Element-Ansicht und den
  // Alternativ-Ansichten (Raster/Karte/Personen) unterscheidet, ist
  // bereits dieser Fallback-Zweig, den es nur content-bewusst zu machen
  // gilt.
  const selectedFolderId = useAppStore((s) => s.selectedFolderId);
  const activeFolderPhotos = useAppStore((s) => (selectedFolderId ? s.photosByFolder[selectedFolderId] : undefined));
  const selectedPhotoIsVideo = activeFolderPhotos?.find((p) => p.id === selectedPhotoId)?.media_kind === "video";
  const setPhotoRating = useAppStore((s) => s.setPhotoRating);
  const setPhotoFlag = useAppStore((s) => s.setPhotoFlag);
  const developPanelOpen = useAppStore((s) => s.developPanelOpen);
  const undoLibraryAction = useAppStore((s) => s.undoLibraryAction);
  const redoLibraryAction = useAppStore((s) => s.redoLibraryAction);
  const [paletteOpen, setPaletteOpen] = useState(false);
  const [cheatsheetOpen, setCheatsheetOpen] = useState(false);
  // Start-Ladeschirm (Phase 19, siehe `StartupSplash.tsx`s Moduldoku) —
  // "bereit" heißt: die beiden für den ersten sinnvollen Bildschirm
  // nötigen Ladevorgänge sind durch. `refreshFolders` fließt bewusst
  // nicht mit ein: dessen Ergebnis zeigt sich ohnehin erst in der
  // Seitenleiste, ein Warten darauf würde den Splash unnötig verlängern.
  const catalogStatus = useAppStore((s) => s.catalogStatus);
  const appReady = uiSettings !== null && catalogStatus !== null;
  const shuttingDown = useShutdownTransition();

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

    // Theme + benutzerdefinierte Akzentfarbe (Phase 10 Schritt 7).
    root.setAttribute("data-theme", uiSettings?.theme ?? "dark");
    if (uiSettings?.accent_color) {
      root.style.setProperty("--color-accent", uiSettings.accent_color);
    } else {
      root.style.removeProperty("--color-accent");
    }

    // UI-Sounds (Phase 19, siehe `DECISIONS.md` ADR-0047) — derselbe
    // "Store schreibt, dieser Effekt wendet an"-Mechanismus wie oben.
    if (uiSettings) {
      applySoundSettings(uiSettings);
    }
  }, [uiSettings]);

  // Web-Audio darf laut Browser-Autoplay-Policy erst nach einer echten
  // Zeiger-/Tastatur-Interaktion starten (siehe `lib/sound.ts`).
  useUnlockSoundOnFirstInteraction();
  // App-weiter Akkordeon-Sound (siehe `lib/sound.ts`s Moduldoku).
  useAccordionSounds();

  // Onboarding (Phase 10 Schritt 9): einmaliges automatisches Erstanzeigen
  // über uiSettings.onboarding_seen, sobald die Einstellungen tatsächlich
  // geladen sind (nicht bei jedem Rerender — `onboardingAutoShown` schützt
  // davor, den Dialog erneut zu öffnen, nachdem der Nutzer ihn geschlossen
  // hat, obwohl das Backend-Feld erst mit dem Schließen selbst auf `true`
  // wechselt).
  useEffect(() => {
    if (uiSettings && !uiSettings.onboarding_seen && !onboardingAutoShown.current) {
      onboardingAutoShown.current = true;
      setOnboardingOpen(true);
    }
  }, [uiSettings]);

  function closeOnboarding() {
    setOnboardingOpen(false);
    if (uiSettings && !uiSettings.onboarding_seen) {
      void saveUiSettings({ ...uiSettings, onboarding_seen: true });
    }
  }

  // Befehlspalette-Brücke fürs erneute Aufrufen (siehe store/index.ts
  // pendingCommand-Moduldoku) — Header.tsx ignoriert unbekannte
  // Befehls-IDs (fällt auf `default: return` ohne `clearPendingCommand()`
  // zurück), diese Komponente behandelt deshalb nur "onboarding".
  useEffect(() => {
    if (pendingCommand === "onboarding") {
      setOnboardingOpen(true);
      clearPendingCommand();
    } else if (pendingCommand === "cheatsheet-overlay") {
      setCheatsheetOpen(true);
      clearPendingCommand();
    }
  }, [pendingCommand, clearPendingCommand]);

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
        playCue("redo");
        void redoLibraryAction();
        return;
      }
      if (!developPanelOpen && matchesBinding(event, "undo")) {
        event.preventDefault();
        playCue("undo");
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
      <StartupSplash ready={appReady} />
      <ShutdownOverlay visible={shuttingDown} />
      <Header onOpenPalette={() => setPaletteOpen(true)} />
      <ErrorBanner />
      {(centerView === "grid" || centerView === "overview") && <FilterBar />}
      <div className="flex flex-1 overflow-hidden">
        <Sidebar />
        <PresetsPanel />
        {/* Sanfter Überblend-Wechsel beim centerView-Wechsel (Phase 18
            Schritt 6, siehe `DECISIONS.md` ADR-0046) — `key={centerView}`
            erzwingt einen frischen Mount nur bei einem echten
            Ansicht-Wechsel (Foto-/Video-Wechsel innerhalb der "viewer"-
            Ansicht bleibt unter demselben Schlüssel, löst also keine
            wiederholte Überblendung aus). `index.css`s
            `.apx-view-fade-in` respektiert `prefers-reduced-motion`/
            `uiSettings.reduced_motion` automatisch mit. */}
        <div key={centerView} className="apx-view-fade-in flex flex-1 overflow-hidden">
          {centerView === "grid" ? (
            <GridView />
          ) : centerView === "overview" ? (
            <GridView variant="overview" />
          ) : centerView === "map" ? (
            <MapView />
          ) : centerView === "people" ? (
            <PeopleView />
          ) : selectedPhotoIsVideo ? (
            <VideoPlayer />
          ) : (
            <Viewer />
          )}
        </div>
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
      <OnboardingDialog open={onboardingOpen} onClose={closeOnboarding} />
    </div>
  );
}
