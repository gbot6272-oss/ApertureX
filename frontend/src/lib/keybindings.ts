import type { KeyboardEvent as ReactKeyboardEvent } from "react";

/**
 * Zentrale Tastenkürzel-Zuordnung (Phase 10 Schritt 5, siehe `FEATURES.md`
 * UI-Anforderungen — vorher nur "grundlegende Tastenkürzel"). Deckt die
 * globalen, App-weiten Kürzel ab, die bisher als feste `if`-Kette in
 * `App.tsx` verdrahtet waren (Bildwechsel/Undo-Redo/Vollbild/Flaggen/
 * Palette/Escape).
 *
 * **Nachtrag Phase 11 Schritt 11** (siehe `DECISIONS.md` ADR-0038): die in
 * Phase 10 Schritt 5 bewusst zurückgestellten lokalen Kürzel — `Viewer.tsx`s
 * Zoom-Zifferntasten (`zoom-fit`/`zoom-100`) und `DevelopPanel.tsx`s eigener
 * Ctrl/Cmd+Z-Handler (wiederverwendet dieselben `undo`/`redo`-IDs wie die
 * Bibliotheks-Metadaten, da beide Kontexte sich gegenseitig ausschließen —
 * `App.tsx` reicht Ctrl/Cmd+Z nur weiter, wenn das Entwickeln-Panel
 * geschlossen ist) sind jetzt ebenfalls über `matchesBinding` umbelegbar.
 * **Weiterhin bewusst fest** (Kurven-/Masken-Editoren mit
 * `role="slider"`-Pfeiltasten-Feinjustierung, die Bewertungs-Zifferntasten
 * 0–5, die eine parametrisierte Ziffernreihe statt einer einzelnen festen
 * Aktion sind): diese bleiben die am dichtesten getesteten Pfade im
 * Frontend und sind laut Nutzervorgabe für diese Phase auf maximal einen
 * neuen Test pro Schritt beschränkt (siehe `DECISIONS.md` ADR-0038-
 * Nachtrag) — das Cheatsheet-Overlay listet sie weiterhin informativ mit.
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
  { id: "undo", label: "Rückgängig (Bibliotheks-Metadaten, oder Entwickeln bei geöffnetem Panel)", defaultKey: "mod+z" },
  { id: "redo", label: "Wiederholen (Bibliotheks-Metadaten, oder Entwickeln bei geöffnetem Panel)", defaultKey: "mod+shift+z" },
  { id: "fullscreen", label: "Vollbild umschalten", defaultKey: "f" },
  { id: "flag-pick", label: "Als Favorit markieren", defaultKey: "p" },
  { id: "flag-reject", label: "Ablehnen markieren", defaultKey: "x" },
  { id: "zoom-fit", label: "Zoom einpassen (im Viewer)", defaultKey: "0" },
  { id: "zoom-100", label: "Zoom 100 % (im Viewer)", defaultKey: "1" },
];

/** Rein informativ im Cheatsheet mit aufgeführt, hier fest (siehe
 * Moduldoku oben) — kein `defaultKey`-Eintrag in `KEYBINDING_ACTIONS`,
 * weil diese nicht über `matchesBinding` umbelegbar sind. */
export const FIXED_LOCAL_SHORTCUTS: { label: string; key: string }[] = [
  { label: "Bewertung 0–5 setzen", key: "0–5" },
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
