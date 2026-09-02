import { useCallback, useEffect, useState } from "react";

/**
 * Ein-/ausklappbare, breitenziehbare Paletten + Arbeitsbereich-Preset
 * (Phase 10 Schritt 3, siehe `FEATURES.md` UI-Anforderungen — bisher
 * "Nicht begonnen"). Reiner UI-Komfort, bewusst nur in `localStorage`
 * persistiert statt in der Backend-`UiSettings`-Datei (siehe
 * `apx_core::settings`): anders als Theme/Sprache/Skalierung muss der
 * Arbeitsbereichs-Zuschnitt nicht App-weit/Backend-seitig gelten, er ist
 * reiner Anzeigezustand des jeweiligen Fensters.
 */

const STORAGE_KEY = "apx.workspaceLayout.v1";

export interface PanelLayout {
  width: number;
  collapsed: boolean;
}

type LayoutState = Record<string, PanelLayout>;

function readAll(): LayoutState {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) return {};
    const parsed: unknown = JSON.parse(raw);
    return typeof parsed === "object" && parsed !== null ? (parsed as LayoutState) : {};
  } catch {
    return {};
  }
}

function writeAll(state: LayoutState): void {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(state));
  } catch {
    // localStorage kann in privaten Fenstern/mit deaktiviertem Site-Data
    // werfen — reiner UI-Komfort, ein fehlgeschlagenes Speichern ist kein
    // Fehlerfall, der dem Nutzer gemeldet werden müsste.
  }
}

/** Setzt den Arbeitsbereich (alle bekannten Paletten) auf die
 * Standardbreiten zurück, die `useWorkspacePanel` beim ersten Aufruf je
 * Palette übergeben bekommt — implementiert als vollständiges Löschen des
 * gespeicherten Zustands, jede Palette fällt dann auf ihren `defaultWidth`
 * zurück. */
export function resetWorkspaceLayout(): void {
  writeAll({});
  window.dispatchEvent(new Event("apx:workspace-layout-reset"));
}

const MIN_WIDTH = 180;
const MAX_WIDTH = 480;

export function useWorkspacePanel(id: string, defaultWidth: number) {
  const [layout, setLayout] = useState<PanelLayout>(() => readAll()[id] ?? { width: defaultWidth, collapsed: false });

  useEffect(() => {
    function onReset() {
      setLayout({ width: defaultWidth, collapsed: false });
    }
    window.addEventListener("apx:workspace-layout-reset", onReset);
    return () => window.removeEventListener("apx:workspace-layout-reset", onReset);
  }, [defaultWidth]);

  const persist = useCallback(
    (next: PanelLayout) => {
      setLayout(next);
      const all = readAll();
      all[id] = next;
      writeAll(all);
    },
    [id],
  );

  const toggleCollapsed = useCallback(() => {
    persist({ ...layout, collapsed: !layout.collapsed });
  }, [layout, persist]);

  const setWidth = useCallback(
    (width: number) => {
      persist({ ...layout, width: Math.min(MAX_WIDTH, Math.max(MIN_WIDTH, width)) });
    },
    [layout, persist],
  );

  return { width: layout.width, collapsed: layout.collapsed, toggleCollapsed, setWidth };
}
