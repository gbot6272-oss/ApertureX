import { useEffect, useRef, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";

import { ErrorBoundary } from "./components/ErrorBoundary";
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
import { BoardView } from "./components/BoardView";
import { CalendarView } from "./components/CalendarView";
import { MapView } from "./components/MapView";
import { MasksPanel } from "./components/MasksPanel";
import { MetadataPanel } from "./components/MetadataPanel";
import { OnboardingTour } from "./components/OnboardingTour";
import { GlobalBusyIndicator } from "./components/GlobalBusyIndicator";
import { GlassDistortionFilter } from "./components/ui/GlassDistortionFilter";
import { PeopleView } from "./components/PeopleView";
import { PresetsPanel } from "./components/PresetsPanel";
import { SettingsDialog } from "./components/SettingsDialog";
import { Sidebar } from "./components/Sidebar";
import { StartupSplash } from "./components/StartupSplash";
import { BalloonLayer } from "./components/BalloonLayer";
import { BirthdayWelcome } from "./components/BirthdayWelcome";
import { ConfettiLayer } from "./components/ConfettiLayer";
import { VideoPlayer } from "./components/VideoPlayer";
import { Viewer } from "./components/Viewer";
import { ShutdownOverlay } from "./components/ShutdownOverlay";
import { useImportEvents } from "./hooks/useImportEvents";
import { matchesBinding } from "./lib/keybindings";
import {
  applyEffectLevel,
  decideLevel,
  probeFrameTimes,
  resolveLevel,
  type AppliedEffectLevel,
} from "./lib/effects";
import { usePrefersReducedMotion } from "./lib/motion";
import { applySoundSettings, playCue, useAccordionSounds, useButtonPressSounds, useUnlockSoundOnFirstInteraction } from "./lib/sound";
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
  const refreshStacks = useAppStore((s) => s.refreshStacks);
  const uiSettings = useAppStore((s) => s.uiSettings);
  const saveUiSettings = useAppStore((s) => s.saveUiSettings);
  const settingsDialogOpen = useAppStore((s) => s.settingsDialogOpen);
  const setSettingsDialogOpen = useAppStore((s) => s.setSettingsDialogOpen);
  const pendingCommand = useAppStore((s) => s.pendingCommand);
  const clearPendingCommand = useAppStore((s) => s.clearPendingCommand);
  const [onboardingOpen, setOnboardingOpen] = useState(false);
  const onboardingAutoShown = useRef(false);
  /** Ergebnis der Bildzeit-Messung (ADR-0073, Anfangswert nach
   * ADR-0076). Zählt nur bei `effect_level === "auto"`; die drei
   * festen Stufen gehen vor.
   *
   * Startwert `"reduced"` und nicht `"full"`: die volle Stufe ist
   * gemessen die mit Abstand teuerste Konfiguration (SVG-Verzerrung auf
   * acht Glasflächen, dahinter dauerhaft steigende Ballons), und sie
   * sieht der reduzierten fast gleich. Bei `auto` darf das niemand
   * ungefragt bezahlen — wer die Verzerrung will, wählt „Voll". */
  const [measuredLevel, setMeasuredLevel] = useState<AppliedEffectLevel>("reduced");
  const effectProbeDone = useRef(false);
  /** Nur die Effektstufe, nicht das ganze Einstellungsobjekt — siehe
   * die Abhängigkeitsliste der Messung unten (ADR-0076). */
  const effectSetting = useAppStore((s) => s.uiSettings?.effect_level);
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
  const trashSelectedPhotos = useAppStore((s) => s.trashSelectedPhotos);
  const compareViewOpen = useAppStore((s) => s.compareViewPhotoIds.length > 0);
  const developPanelOpen = useAppStore((s) => s.developPanelOpen);
  const undoLibraryAction = useAppStore((s) => s.undoLibraryAction);
  const redoLibraryAction = useAppStore((s) => s.redoLibraryAction);
  // Fokus-Modus + "Lichter aus" (Phase 26, siehe `DECISIONS.md` ADR-0056).
  const focusMode = useAppStore((s) => s.focusMode);
  const toggleFocusMode = useAppStore((s) => s.toggleFocusMode);
  const lightsOut = useAppStore((s) => s.lightsOut);
  const cycleLightsOut = useAppStore((s) => s.cycleLightsOut);
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
    // Stapel (Phase 33 F10): das Raster gruppiert danach, also müssen
    // sie beim Start da sein — bisher wurden sie nur geladen, wenn man
    // den Organisieren-Dialog öffnete.
    void refreshStacks();
  }, [refreshFolders, refreshCatalogStatus, loadUiSettings, refreshStacks]);

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
    // Effektstufe (ADR-0073) — `measuredLevel` ist das Ergebnis der
    // Messung unten und zählt nur, wenn die Einstellung `auto` ist.
    applyEffectLevel(resolveLevel(uiSettings?.effect_level ?? "auto", measuredLevel), root);
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
    // `measuredLevel` gehört in die Liste: sonst bliebe das Ergebnis
    // der Messung bis zur nächsten Änderung an den Einstellungen
    // liegen — also genau in dem Fall nutzlos, für den es gedacht ist.
  }, [uiSettings, measuredLevel]);

  // Web-Audio darf laut Browser-Autoplay-Policy erst nach einer echten
  // Zeiger-/Tastatur-Interaktion starten (siehe `lib/sound.ts`).
  useUnlockSoundOnFirstInteraction();
  // App-weiter Akkordeon-Sound (siehe `lib/sound.ts`s Moduldoku).
  useAccordionSounds();
  // App-weites Tastenanschlag-Geräusch für jeden Knopf (siehe
  // `lib/sound.ts`s Moduldoku — Phase 20 ADR-0048-Nachtrag).
  useButtonPressSounds();

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

  // Bildzeit-Messung für die Effektstufe (ADR-0073).
  //
  // Läuft genau einmal je Sitzung und erst, wenn die Oberfläche steht
  // — während des Starts wäre jede Messung von Layout, Schriftladen
  // und dem ersten Katalogzugriff verfälscht. `probeFrameTimes` hängt
  // sich dafür selbst eine bewegte Last hinter die Glasflächen: ohne
  // die wäre alles zwischengespeichert und jedes Gerät sähe schnell
  // aus, auch das, auf dem es dann ruckelt.
  //
  // Nur bei `auto`: wer eine feste Stufe gewählt hat, soll nicht
  // heimlich überstimmt werden — und die Messung selbst kostet kurz
  // Rechenzeit, die ohne Zweck niemand zahlen muss.
  //
  // Gemessen wird in bis zu zwei Runden. Ein einziger Durchgang stuft
  // bei maessiger Überschreitung nur um eine Stufe herunter — reicht
  // die nicht, bliebe das Gerät ohne Nachmessung dauerhaft zu langsam,
  // und der Nutzer saesse wieder vor einer zähen App, die ihm gerade
  // erst versichert hat, sie habe sich angepasst. Zwei Runden genügen,
  // weil von „voll" aus höchstens zwei Stufen nach unten gehen.
  useEffect(() => {
    if (effectProbeDone.current) return;
    if (effectSetting !== "auto") return;

    let cancelled = false;
    const timer = window.setTimeout(() => {
      // Das Flag erst HIER setzen, nicht beim Planen.
      //
      // Vorher stand es oben, direkt hinter der Abfrage — und damit
      // lief die Messung in der Praxis nie: ändert sich `uiSettings`
      // innerhalb der 2,5 Sekunden (beim Start der Regelfall), räumt
      // die Aufräumfunktion den Timer ab, der Effekt läuft erneut und
      // steigt in der ersten Zeile sofort wieder aus. Die Automatik war
      // dann für den Rest der Sitzung tot, und die App blieb auf der
      // teuersten Stufe stehen (ADR-0076).
      effectProbeDone.current = true;
      void (async () => {
        let level: AppliedEffectLevel = "reduced";
        for (let round = 0; round < 2; round += 1) {
          const times = await probeFrameTimes(40);
          if (cancelled) return;
          const next = decideLevel(times, level);
          if (next === level) break;
          level = next;
          setMeasuredLevel(level);
          // Die zweite Runde muss gegen die NEUE Stufe messen. React
          // hat das Attribut nach `setMeasuredLevel` noch nicht
          // geschrieben; ein Bild Pause reicht, damit der Browser die
          // geänderten Regeln angewendet hat.
          await new Promise((resolve) => window.requestAnimationFrame(resolve));
        }
      })();
    }, 2500);

    return () => {
      cancelled = true;
      window.clearTimeout(timer);
    };
    // Nur das eine Feld, das hier zählt — nicht das ganze
    // Einstellungsobjekt. Jede unbeteiligte Änderung daran hätte den
    // Timer sonst erneut abgeräumt.
  }, [effectSetting]);

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
        // Escape schließt zuerst jede offene Überlagerung (unverändert).
        // Ist **nichts** überhaupt offen — auch keiner der ~25 lokal in
        // `Header.tsx` gehaltenen Dialoge, die dieser Komponente nicht
        // als State bekannt sind, deshalb die DOM-Abfrage statt eines
        // weiteren Store-/Props-Felds —, öffnet Escape stattdessen die
        // Befehlspalette als Schnellmenü (Phase 22, siehe `DECISIONS.md`
        // ADR-0050: Nutzerwunsch "Menü, wenn man ESC drückt", Einstellungen
        // sind dort bereits als Befehl gelistet, siehe `commandRegistry.ts`).
        const anyOverlayOpen = cheatsheetOpen || paletteOpen || settingsDialogOpen || onboardingOpen || document.querySelector('[role="dialog"]') !== null;
        setCheatsheetOpen(false);
        setPaletteOpen(false);
        if (!anyOverlayOpen) {
          event.preventDefault();
          setPaletteOpen(true);
        }
      } else if (matchesBinding(event, "fullscreen")) {
        void toggleFullscreen();
      } else if (matchesBinding(event, "focus-mode")) {
        // Phase 26 (siehe `DECISIONS.md` ADR-0056) — beide Modi laufen
        // bewusst durch dieselbe `matchesBinding`-Kette wie alle anderen
        // globalen Kürzel, sind also über das Cheatsheet umbelegbar und
        // durch den `isEditable`-Wächter oben automatisch aus
        // Eingabefeldern/Reglern herausgehalten.
        event.preventDefault();
        playCue(focusMode ? "expand" : "collapse");
        toggleFocusMode();
      } else if (matchesBinding(event, "lights-out")) {
        event.preventDefault();
        cycleLightsOut();
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
      } else if (selectedPhotoId && compareViewOpen && matchesBinding(event, "trash-selection")) {
        // In der Vergleichsansicht heißt Entf seit Phase 31 Schritt 8
        // „aus dem Vergleich nehmen" — das hat dort den Vorrang, und der
        // Vergleich hat seinen eigenen Tastatur-Handler dafür. Hier nur
        // aus dem Weg gehen, statt zusätzlich in den Papierkorb zu
        // werfen: zwei Bedeutungen auf einer Taste wären ein Datenverlust
        // mit Ansage.
      } else if (selectedPhotoId && matchesBinding(event, "trash-selection")) {
        // Kein Rückfrage-Dialog: das Foto landet im Papierkorb, nicht im
        // Nichts (Phase 33 F1). Die Rückfrage gehört an die eine Stelle,
        // an der wirklich etwas verloren geht — das Leeren des
        // Papierkorbs.
        event.preventDefault();
        void trashSelectedPhotos("manual");
      }
    }

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [
    stepSelection,
    selectedPhotoId,
    setPhotoRating,
    setPhotoFlag,
    trashSelectedPhotos,
    compareViewOpen,
    developPanelOpen,
    undoLibraryAction,
    redoLibraryAction,
    cheatsheetOpen,
    paletteOpen,
    settingsDialogOpen,
    onboardingOpen,
  ]);

  return (
    <div className="flex h-screen flex-col bg-bg-base text-text-primary">
      <GlassDistortionFilter />
      {/* Geburtstagsausgabe (siehe `DECISIONS.md` ADR-0071) — drei
          Schichten, alle rein dekorativ und alle ohne
          Zeigerereignisse: Luftballons hinter dem Inhalt
          (`z-index: 0`), Konfetti darüber (`z-index: 150`), die
          Begrüßung dazwischen (`z-[160]`, aber `pointer-events-none`).
          Der Start-Ladeschirm bleibt mit `z-[200]` über allem. Jede
          Schicht prüft `birthday_decorations` und `reduced_motion`
          selbst und rendert sonst `null` — kein Element, keine
          Animation, keine Schleife. */}
      <BalloonLayer />
      <ConfettiLayer />
      <BirthdayWelcome ready={appReady} />
      <StartupSplash ready={appReady} />
      <ShutdownOverlay visible={shuttingDown} />
      <Header onOpenPalette={() => setPaletteOpen(true)} />
      <ErrorBanner />
      {(centerView === "grid" || centerView === "overview") && <FilterBar />}
      {/* Fokus-Modus (Phase 26, siehe `DECISIONS.md` ADR-0056): die
          angedockten Paletten und der Filmstreifen werden nicht nur
          versteckt, sondern gar nicht erst gerendert — sonst liefen ihre
          Effekte (Vorschau-Nachladen, Histogramm-Zeichnen) im
          ausgeblendeten Zustand weiter. Ihr jeweils eigener Ein-/
          Ausklapp-Zustand (`useWorkspacePanel`, localStorage) bleibt
          davon unberührt und kommt beim Verlassen unverändert zurück. */}
      <div className="flex flex-1 overflow-hidden">
        {!focusMode && <Sidebar />}
        {!focusMode && <PresetsPanel />}
        {/* Sanfter Überblend-Wechsel beim centerView-Wechsel (Phase 18
            Schritt 6, siehe `DECISIONS.md` ADR-0046) — `key={centerView}`
            erzwingt einen frischen Mount nur bei einem echten
            Ansicht-Wechsel (Foto-/Video-Wechsel innerhalb der "viewer"-
            Ansicht bleibt unter demselben Schlüssel, löst also keine
            wiederholte Überblendung aus). `index.css`s
            `.apx-view-fade-in` respektiert `prefers-reduced-motion`/
            `uiSettings.reduced_motion` automatisch mit. */}
        {/* Bei aktivem "Lichter aus" hebt sich allein der Bildbereich über
            die Dimm-Ebene (`z-20`, siehe unten) — genau das ist der Sinn
            des Modus: die Umgebung verschwindet, das Foto bleibt hell.
            Bleibt unter der Kopfzeile (`z-30`), deren `backdrop-filter`
            dadurch die abgedunkelte Umgebung abtastet und selbst dunkel
            wird. Ohne aktiven Modus bewusst KEINE Stapelkontext-Klasse
            (siehe ADR-0055: ein unnötig gesetzter `z-index` hat hier
            schon einmal das Overflow-Menü verdeckt). */}
        <div key={centerView} className={`apx-view-fade-in flex flex-1 overflow-hidden ${lightsOut === "off" ? "" : "relative z-[25]"}`}>
          {/* Eigene Fehlergrenze je Bereich (ADR-0068): stuerzt eine
              Ansicht ab, bleiben Kopfleiste, Seitenleiste und Paletten
              bedienbar — statt dass React den ganzen Baum abhaengt und ein
              weisses Fenster ohne Ausweg zurueckbleibt. Der `key` sorgt
              nebenbei dafuer, dass ein Ansichtswechsel die Grenze
              zuruecksetzt. */}
          <ErrorBoundary scope="region" label="Diese Ansicht">
          {centerView === "grid" ? (
            <GridView />
          ) : centerView === "overview" ? (
            <GridView variant="overview" />
          ) : centerView === "map" ? (
            <MapView />
          ) : centerView === "people" ? (
            <PeopleView />
          ) : centerView === "calendar" ? (
            <CalendarView />
          ) : centerView === "board" ? (
            <BoardView />
          ) : selectedPhotoIsVideo ? (
            <VideoPlayer />
          ) : (
            <Viewer />
          )}
          </ErrorBoundary>
        </div>
        {!focusMode && <MetadataPanel />}
        {/* Rechte Werkzeug-Palette (Phase 10 Schritt 2): Entwickeln- und
            Masken-Panel bleiben zwei unabhängig sichtbare/aufklappbare
            Bereiche (nicht exklusiv verdeckende Reiter — viele bestehende
            e2e-Tests bedienen Entwickeln- und Maskenregler im selben
            Ablauf), aber unter einer gemeinsamen visuellen Außenhülle statt
            zweier lose nebeneinanderstehender <aside>s. */}
        {!focusMode && (
          <div className="flex shrink-0">
            <ErrorBoundary scope="region" label="Das Entwickeln-Panel">
              <DevelopPanel />
            </ErrorBoundary>
            <ErrorBoundary scope="region" label="Das Masken-Panel">
              <MasksPanel />
            </ErrorBoundary>
          </div>
        )}
      </div>
      {!focusMode && <Filmstrip />}
      {/* "Lichter aus" (Phase 26): eine reine Sichtebene ÜBER der
          gesamten App, aber UNTER der Kopfzeile (`z-30`) und unter
          Dialogen (`z-50`) — der zentrale Bildbereich bleibt dadurch
          unangetastet hell, während Paletten/Filmstreifen/Filterleiste
          abgedunkelt werden. `pointer-events-none`: der Modus dimmt nur
          optisch, er sperrt die Bedienung nicht (sonst wäre er ohne
          Tastatur nicht mehr verlassbar). */}
      {lightsOut !== "off" && (
        <div
          aria-hidden
          data-testid="lights-out-overlay"
          className="pointer-events-none fixed inset-0 z-20 transition-[background-color] duration-[var(--duration-base)]"
          style={{ backgroundColor: lightsOut === "dim" ? "rgb(0 0 0 / 55%)" : "rgb(0 0 0 / 92%)" }}
        />
      )}
      <CompareGridView />
      <HistoryTimelineDialog />
      <CommandPalette open={paletteOpen} onClose={() => setPaletteOpen(false)} />
      <SettingsDialog open={settingsDialogOpen} onClose={() => setSettingsDialogOpen(false)} />
      <KeybindingsCheatsheet open={cheatsheetOpen} onClose={() => setCheatsheetOpen(false)} />
      <OnboardingTour open={onboardingOpen} onClose={closeOnboarding} />
      <GlobalBusyIndicator />
    </div>
  );
}
