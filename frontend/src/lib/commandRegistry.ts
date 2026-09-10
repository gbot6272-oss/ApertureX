import { useMemo } from "react";

import { useAppStore } from "../store";
import { useT } from "./i18n";

/**
 * Zentrales Kommando-Register (Phase 18 Schritt 3, siehe `DECISIONS.md`
 * ADR-0046 Entwurfsentscheidung 4). Vorher pflegten `Header.tsx` (Zeile-2-
 * Knöpfe) und `CommandPalette.tsx` (`functionEntries`) zwei eigene, nur
 * zufällig deckungsgleiche Listen derselben Aktionen — jede neue Funktion
 * musste an zwei Stellen von Hand nachgetragen werden (siehe die in der
 * vorherigen Sitzung gefundene Ursache für das unauffindbare KI-Ausfüllen-
 * Download). `useCommandRegistry()` ist jetzt die einzige Quelle: die neue
 * schlanke Kopfleiste zieht daraus ihr Overflow-Menü (gruppiert nach
 * `category`), die Befehlspalette denselben Satz für ihre durchsuchbare
 * Liste (zusätzlich zu den dort weiterhin lokal berechneten Presets/Fotos/
 * Ordnern, die keine "Befehle", sondern Daten sind).
 *
 * Die neun in `Header.tsx` lokal gehaltenen Dialoge (siehe deren
 * Moduldoku) laufen weiterhin über die bestehende `pendingCommand`-Brücke
 * (`requestCommand`/`store/index.ts`) — dieses Register ruft für sie nur
 * `requestCommand(id)` auf, öffnet sie nicht selbst.
 */

export type CommandCategory = "output" | "templates" | "advanced" | "analysis" | "ai" | "navigation" | "system";

/** Sichtbarer Modellstatus für die neue "KI-Funktionen"-Kategorie —
 * behebt strukturell das in der letzten Sitzung gefundene
 * Auffindbarkeits-Problem ("Download-Hinweis nur sichtbar, wenn man
 * zufällig genau den richtigen verschachtelten Modus anwählt"). */
export type AiFeatureStatus = "ready" | "download-needed" | "not-available";

export interface CommandEntry {
  id: string;
  label: string;
  category: CommandCategory;
  /** Nur für `category === "ai"` gesetzt. */
  aiStatus?: AiFeatureStatus;
  disabled?: boolean;
  run: () => void;
}

/** Die vier Kategorien, die im neuen Overflow-Menü der Kopfleiste als
 * eigene Abschnitte erscheinen — dieselben wie die bisherigen Zeile-2-
 * Gruppen, plus die neue "ai"-Kategorie. `navigation`/`system` bleiben
 * absichtlich außen vor: die sechs Ansicht-Ziele sind bereits als
 * Segment-Gruppe in der Kopfleiste sichtbar, Einstellungen als eigenes
 * Icon — beides würde im Menü nur doppelt auftauchen. Sie bleiben aber
 * Teil des Registers, damit die Befehlspalette sie weiterhin findet. */
export const MENU_CATEGORIES: readonly CommandCategory[] = ["ai", "output", "templates", "advanced", "analysis"];

export function useCommandRegistry(): CommandEntry[] {
  const t = useT();

  const requestCommand = useAppStore((s) => s.requestCommand);
  const toggleCenterView = useAppStore((s) => s.toggleCenterView);
  const setCenterView = useAppStore((s) => s.setCenterView);
  const toggleMetadataPanel = useAppStore((s) => s.toggleMetadataPanel);
  const toggleDevelopPanel = useAppStore((s) => s.toggleDevelopPanel);
  const developPanelOpen = useAppStore((s) => s.developPanelOpen);
  const setRepairDraftMode = useAppStore((s) => s.setRepairDraftMode);
  const setCanvasExtendDialogOpen = useAppStore((s) => s.setCanvasExtendDialogOpen);
  const openExportDialog = useAppStore((s) => s.openExportDialog);
  const openPrintDialog = useAppStore((s) => s.openPrintDialog);
  const openSlideshowDialog = useAppStore((s) => s.openSlideshowDialog);
  const openBookDialog = useAppStore((s) => s.openBookDialog);
  const openWebDialog = useAppStore((s) => s.openWebDialog);
  const openVideoTimelineDialog = useAppStore((s) => s.openVideoTimelineDialog);
  const setSettingsDialogOpen = useAppStore((s) => s.setSettingsDialogOpen);
  const selectedPhotoId = useAppStore((s) => s.selectedPhotoId);
  const multiSelectedIds = useAppStore((s) => s.multiSelectedIds);
  const openCompareView = useAppStore((s) => s.openCompareView);
  const openVersionsCompareView = useAppStore((s) => s.openVersionsCompareView);
  const openSecondaryDisplay = useAppStore((s) => s.openSecondaryDisplay);
  const aiSettings = useAppStore((s) => s.aiSettings);

  const exportPhotoIds = multiSelectedIds.length > 0 ? multiSelectedIds : selectedPhotoId ? [selectedPhotoId] : [];

  function openDevelopPanel() {
    if (!developPanelOpen) toggleDevelopPanel();
  }

  return useMemo<CommandEntry[]>(() => {
    const entries: CommandEntry[] = [
      // Navigation — bereits als Segment-Gruppe in der Kopfleiste sichtbar,
      // hier nur damit die Befehlspalette sie ebenfalls findet.
      { id: "fn:view-grid", label: t("header.viewGrid"), category: "navigation", run: toggleCenterView },
      { id: "fn:view-overview", label: t("header.viewOverview"), category: "navigation", run: () => setCenterView("overview") },
      { id: "fn:view-map", label: t("header.viewMap"), category: "navigation", run: () => setCenterView("map") },
      { id: "fn:view-people", label: t("header.viewPeople"), category: "navigation", run: () => setCenterView("people") },
      { id: "fn:view-info", label: t("header.viewInfo"), category: "navigation", run: toggleMetadataPanel },
      { id: "fn:view-develop", label: t("header.viewDevelop"), category: "navigation", run: toggleDevelopPanel },

      // Ausgabe
      { id: "fn:export", label: t("header.export"), category: "output", disabled: exportPhotoIds.length === 0, run: openExportDialog },
      { id: "fn:print", label: t("header.print"), category: "output", disabled: exportPhotoIds.length === 0, run: openPrintDialog },
      { id: "fn:slideshow", label: t("header.slideshow"), category: "output", disabled: exportPhotoIds.length === 0, run: openSlideshowDialog },
      {
        id: "fn:video-timeline",
        label: t("header.videoTimeline"),
        category: "output",
        disabled: exportPhotoIds.length === 0,
        run: openVideoTimelineDialog,
      },
      { id: "fn:book", label: t("header.book"), category: "output", disabled: exportPhotoIds.length === 0, run: openBookDialog },
      { id: "fn:web", label: t("header.web"), category: "output", disabled: exportPhotoIds.length === 0, run: openWebDialog },

      // Vorlagen & Organisation
      { id: "fn:import-template", label: t("header.importWithTemplate"), category: "templates", run: () => requestCommand("import-template") },
      { id: "fn:templates", label: t("header.templates"), category: "templates", run: () => requestCommand("templates") },
      { id: "fn:organize", label: t("header.organize"), category: "templates", run: () => requestCommand("organize") },
      { id: "fn:batch-console", label: t("header.batchConsole"), category: "templates", run: () => requestCommand("batch-console") },
      { id: "fn:metadata", label: t("header.metadata"), category: "templates", run: () => requestCommand("metadata") },

      // Fortgeschritten
      { id: "fn:stacking", label: t("header.stacking"), category: "advanced", run: () => requestCommand("stacking") },
      { id: "fn:script-plugin", label: t("header.scriptPlugin"), category: "advanced", run: () => requestCommand("script-plugin") },
      { id: "fn:share", label: t("header.share"), category: "advanced", run: () => requestCommand("share") },
      { id: "fn:tether", label: t("header.tether"), category: "advanced", run: () => requestCommand("tether") },

      // Analyse
      {
        id: "fn:compare",
        label: t("header.compare"),
        category: "analysis",
        disabled: exportPhotoIds.length < 2,
        run: () => openCompareView(exportPhotoIds),
      },
      {
        id: "fn:versions-compare",
        label: t("header.versionsCompare"),
        category: "analysis",
        disabled: !selectedPhotoId,
        run: () => void openVersionsCompareView(),
      },
      {
        id: "fn:secondary-display",
        label: t("header.secondaryDisplay"),
        category: "analysis",
        disabled: !selectedPhotoId,
        run: () => selectedPhotoId && void openSecondaryDisplay(selectedPhotoId),
      },
      { id: "fn:stats", label: t("header.stats"), category: "analysis", run: () => requestCommand("stats") },
      { id: "fn:catalog", label: t("header.catalog"), category: "analysis", run: () => requestCommand("catalog") },

      // System — nicht im Overflow-Menü (eigenes Icon/Tastenkürzel), aber
      // weiterhin über die Befehlspalette auffindbar.
      { id: "fn:settings", label: t("header.settings"), category: "system", run: () => setSettingsDialogOpen(true) },
      { id: "fn:onboarding", label: t("commands.onboarding"), category: "system", run: () => requestCommand("onboarding") },
      { id: "fn:cheatsheet", label: t("commands.cheatsheet"), category: "system", run: () => requestCommand("cheatsheet-overlay") },

      // KI-Funktionen — neue Kategorie (Entwurfsentscheidung 4): jedes
      // Opt-in-KI-Feature an einer Stelle, mit sichtbarem Modellstatus,
      // statt in einem verschachtelten Modus versteckt.
      {
        id: "ai:inpaint",
        label: t("commands.ai.inpaint"),
        category: "ai",
        aiStatus: aiSettings?.inpainting_model_path ? "ready" : "download-needed",
        run: () => {
          openDevelopPanel();
          setRepairDraftMode("AiInpaint");
        },
      },
      {
        id: "ai:outpaint",
        label: t("commands.ai.outpaint"),
        category: "ai",
        aiStatus: aiSettings?.inpainting_model_path ? "ready" : "download-needed",
        run: () => setCanvasExtendDialogOpen(true),
      },
      {
        id: "ai:skin-smoothing",
        label: t("commands.ai.skinSmoothing"),
        category: "ai",
        aiStatus: "ready",
        run: openDevelopPanel,
      },
      {
        id: "ai:sky-replace",
        label: t("commands.ai.skyReplace"),
        category: "ai",
        aiStatus: "ready",
        run: openDevelopPanel,
      },
      {
        id: "ai:style-transfer",
        label: t("commands.ai.styleTransfer"),
        category: "ai",
        aiStatus: Object.keys(aiSettings?.style_transfer_model_paths ?? {}).length > 0 ? "ready" : "download-needed",
        run: openDevelopPanel,
      },
      {
        id: "ai:depth",
        label: t("commands.ai.depth"),
        category: "ai",
        aiStatus: aiSettings?.depth_model_path ? "ready" : "download-needed",
        run: openDevelopPanel,
      },
      {
        id: "ai:remove-background",
        label: t("commands.ai.removeBackground"),
        category: "ai",
        aiStatus: aiSettings?.selfie_segmentation_model_path ? "ready" : "download-needed",
        run: () => setCenterView("viewer"),
      },
      {
        id: "ai:subtitles",
        label: t("commands.ai.subtitles"),
        category: "ai",
        aiStatus: !aiSettings?.subtitles_feature_compiled ? "not-available" : aiSettings.whisper_model_path ? "ready" : "download-needed",
        run: openVideoTimelineDialog,
      },
      {
        id: "ai:people",
        label: t("commands.ai.people"),
        category: "ai",
        aiStatus: !aiSettings?.people_feature_compiled
          ? "not-available"
          : aiSettings.people_landmark_model_path && aiSettings.people_encoder_model_path
            ? "ready"
            : "download-needed",
        run: () => setCenterView("people"),
      },
    ];

    return entries;
  }, [
    t,
    toggleCenterView,
    setCenterView,
    toggleMetadataPanel,
    toggleDevelopPanel,
    developPanelOpen,
    setRepairDraftMode,
    setCanvasExtendDialogOpen,
    exportPhotoIds,
    openExportDialog,
    openPrintDialog,
    openSlideshowDialog,
    openVideoTimelineDialog,
    openBookDialog,
    openWebDialog,
    requestCommand,
    openCompareView,
    openVersionsCompareView,
    selectedPhotoId,
    openSecondaryDisplay,
    setSettingsDialogOpen,
    aiSettings,
  ]);
}
