/**
 * Deutsch — Ausgangs-/Schlüsselsprache (siehe `lib/i18n.ts`s Moduldoku).
 * Jeder Wert hier ist character-für-character identisch mit dem vormals
 * hartkodierten deutschen Text an derselben Stelle.
 */
export const de = {
  // Header.tsx — Zeile 1 (Import + Ansicht)
  "header.importFolder": "Ordner importieren",
  "header.importWithTemplate": "Import mit Vorlage…",
  "header.importWithTemplateTitle": "Import mit wählbarem Modus (Kopieren/Verschieben), Umbenennungsmuster und Presets",
  "header.cancelImport": "Abbrechen",
  "header.viewGrid": "Raster",
  "header.viewMap": "Karte",
  "header.viewInfo": "Info",
  "header.viewDevelop": "Entwickeln",

  // Header.tsx — Zeile 2, Gruppenlabel + Module
  "header.group.output": "Ausgabe",
  "header.export": "Exportieren…",
  "header.print": "Drucken…",
  "header.slideshow": "Diashow…",
  "header.book": "Buch…",
  "header.web": "Web…",
  "header.group.templates": "Vorlagen",
  "header.templates": "Vorlagen…",
  "header.organize": "Organisieren…",
  "header.metadata": "Metadaten…",
  "header.group.advanced": "Fortgeschritten",
  "header.stacking": "Stacking…",
  "header.scriptPlugin": "Skript & Plugins…",
  "header.share": "Kollaboration…",
  "header.tether": "Tethering…",
  "header.group.analysis": "Analyse",
  "header.compare": "Vergleichen",
  "header.compareTitle": "Ausgewählte Fotos nebeneinander vergleichen",
  "header.versionsCompare": "Versionen vergleichen",
  "header.versionsCompareTitle": "Aktuelles Foto und seine virtuellen Kopien nebeneinander vergleichen (Phase 9 Schritt 7)",
  "header.secondaryDisplay": "Zweites Display…",
  "header.secondaryDisplayTitle": "Aktuelles Foto in einem zweiten Fenster anzeigen",
  "header.stats": "Statistik…",
  "header.settings": "Einstellungen…",
  "header.settingsTitle": "Theme, Sprache, UI-Skalierung, Barrierefreiheit (Phase 10)",
  "header.paletteHint": "Strg/Cmd+K — Befehlspalette",

  // Sidebar.tsx
  "sidebar.heading": "Ordner",
  "sidebar.empty": "Noch keine Ordner importiert.",

  // PresetsPanel.tsx
  "presets.heading": "Presets",

  // MetadataPanel.tsx
  "metadata.heading": "Metadaten",
  "metadata.noPhoto": "Kein Foto ausgewählt.",

  // SettingsDialog.tsx
  "settings.title": "Einstellungen",
  "settings.close": "Schließen",
  "settings.tab.display": "Anzeige",
  "settings.tab.language": "Sprache",
  "settings.loading": "Lädt …",
  "settings.theme": "Theme",
  "settings.themeDark": "Dunkel",
  "settings.themeLight": "Hell",
  "settings.accentColor": "Akzentfarbe",
  "settings.accentColorReset": "Zurücksetzen",
  "settings.uiScale": "UI-Skalierung: {percent}%",
  "settings.highContrast": "Kontrastmodus",
  "settings.reducedMotion": "Reduzierte Bewegung",
  "settings.resetWorkspace": "Arbeitsbereich zurücksetzen",
  "settings.resetWorkspaceTitle": "Setzt Breite und Eingeklappt-Status aller Paletten (Ordner/Presets/Metadaten) zurück",
  "settings.language": "Sprache",
  "settings.languageDe": "Deutsch",
  "settings.languageEn": "English",

  // KeybindingsCheatsheet.tsx
  "cheatsheet.title": "Tastenkürzel",
  "cheatsheet.rebindPrompt": "Taste drücken…",
  "cheatsheet.rebindTitle": "Neu belegen",
  "cheatsheet.resetAll": "Alle Tastenkürzel zurücksetzen",
  "cheatsheet.fixedLocalHeading": "Feste lokale Kürzel",
} as const;
