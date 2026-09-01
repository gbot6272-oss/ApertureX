import type { KeyboardEvent as ReactKeyboardEvent } from "react";

/**
 * Zentrale Tastenkürzel-Zuordnung (Phase 10 Schritt 5, siehe `FEATURES.md`
 * UI-Anforderungen — vorher nur "grundlegende Tastenkürzel"). Deckt die
 * globalen, App-weiten Kürzel ab, die bisher als feste `if`-Kette in
 * `App.tsx` verdrahtet waren (Bildwechsel/Undo-Redo/Vollbild/Flaggen/
 * Palette/Escape) — **bewusst nicht** die lokalen Kürzel innerhalb
 * einzelner Komponenten (z. B. `Viewer.tsx`s Zoom-Zifferntasten,
 * `DevelopPanel.tsx`s eigener Ctrl/Cmd+Z-Handler, Kurven-/Masken-Editoren
 * mit `role="slider"`-Pfeiltasten-Feinjustierung): diese sind mit die am
 * dichtesten getesteten Pfade im Frontend, und diese Phase schreibt
 * bewusst keine neuen Tests, um Regressionen dort sofort abzufangen
 * (siehe `DECISIONS.md` ADR-0037) — sie bleiben fest, das Cheatsheet-
 * Overlay listet sie trotzdem informativ mit auf.
 */

export interface KeyBindingAction {
  id: string;
  label: string;
  /** Normalisierte Form: `mod` steht für Ctrl (Windows/Linux) bzw. Cmd
   * (macOS), Teile durch `+` getrennt, Buchstaben klein geschrieben. */
  defaultKey: string;
}

export const KEYBINDING_ACTIONS: KeyBindingAction[] = [
  { id: "prev-photo", label: "Vorheriges Foto", defaultKey: "arrowleft" },
  { id: "next-photo", label: "Nächstes Foto", defaultKey: "arrowright" },
  { id: "toggle-palette", label: "Befehlspalette öffnen/schließen", defaultKey: "mod+k" },
  { id: "cheatsheet", label: "Tastenkürzel-Übersicht anzeigen", defaultKey: "?" },
  { id: "close-overlay", label: "Palette/Overlay schließen", defaultKey: "escape" },
  { id: "undo", label: "Rückgängig (Bibliotheks-Metadaten)", defaultKey: "mod+z" },
  { id: "redo", label: "Wiederholen (Bibliotheks-Metadaten)", defaultKey: "mod+shift+z" },
  { id: "fullscreen", label: "Vollbild umschalten", defaultKey: "f" },
  { id: "flag-pick", label: "Als Favorit markieren", defaultKey: "p" },
  { id: "flag-reject", label: "Ablehnen markieren", defaultKey: "x" },
];

/** Rein informativ im Cheatsheet mit aufgeführt, hier fest (siehe
 * Moduldoku oben) — kein `defaultKey`-Eintrag in `KEYBINDING_ACTIONS`,
 * weil diese nicht über `matchesBinding` umbelegbar sind. */
export const FIXED_LOCAL_SHORTCUTS: { label: string; key: string }[] = [
  { label: "Bewertung 0–5 setzen", key: "0–5" },
  { label: "Zoom 100 % (im Viewer)", key: "1" },
  { label: "Rückgängig/Wiederholen (im Entwickeln-Panel)", key: "Strg/Cmd+Z, Strg/Cmd+Shift+Z" },
  { label: "Feinjustierung an Reglern/Kurvenpunkten", key: "Pfeiltasten (Shift = grob)" },
];

const STORAGE_KEY = "apx.keybindings.v1";

function readOverrides(): Record<string, string> {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) return {};
    const parsed: unknown = JSON.parse(raw);
    return typeof parsed === "object" && parsed !== null ? (parsed as Record<string, string>) : {};
  } catch {
    return {};
  }
}

function writeOverrides(overrides: Record<string, string>): void {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(overrides));
  } catch {
    // Reiner UI-Komfort, siehe workspaceLayout.ts.
  }
}

export function getBinding(id: string): string {
  const overrides = readOverrides();
  return overrides[id] ?? KEYBINDING_ACTIONS.find((a) => a.id === id)?.defaultKey ?? "";
}

export function setBinding(id: string, normalizedKey: string): void {
  const overrides = readOverrides();
  overrides[id] = normalizedKey;
  writeOverrides(overrides);
}

export function resetBindings(): void {
  writeOverrides({});
}

/** Baut aus einem `KeyboardEvent` dieselbe normalisierte Form wie
 * `defaultKey`/`setBinding` — Grundlage für sowohl `matchesBinding` als
 * auch das Aufzeichnen einer neuen Belegung im Cheatsheet-Overlay. */
export function normalizeEvent(event: KeyboardEvent | ReactKeyboardEvent): string {
  const parts: string[] = [];
  // Shift wird nur bei Buchstaben als eigener Modifikator geführt (z. B.
  // "mod+shift+z" vs. "mod+z") — bei Satzzeichen wie "?" erzeugt Shift auf
  // den meisten Layouts bereits das Zeichen selbst (`event.key === "?"`);
  // ein zusätzliches "shift+?" wäre redundant und würde nie zum
  // unveränderten Standard "?" passen.
  const isLetter = /^[a-zA-Z]$/.test(event.key);
  if (event.metaKey || event.ctrlKey) parts.push("mod");
  if (isLetter && event.shiftKey) parts.push("shift");
  if (event.altKey) parts.push("alt");
  parts.push(event.key.toLowerCase());
  return parts.join("+");
}

export function matchesBinding(event: KeyboardEvent, id: string): boolean {
  return normalizeEvent(event) === getBinding(id);
}
