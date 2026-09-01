import { useRef, useState } from "react";
import type { KeyboardEvent as ReactKeyboardEvent } from "react";

import { useFocusTrap } from "../lib/a11y";
import { useT } from "../lib/i18n";
import {
  FIXED_LOCAL_SHORTCUTS,
  KEYBINDING_ACTIONS,
  getBinding,
  normalizeEvent,
  resetBindings,
  setBinding,
} from "../lib/keybindings";

interface KeybindingsCheatsheetProps {
  open: boolean;
  onClose: () => void;
}

function displayKey(normalized: string): string {
  return normalized
    .split("+")
    .map((part) => (part === "mod" ? "Strg/Cmd" : part === "shift" ? "Shift" : part === "alt" ? "Alt" : part.length === 1 ? part.toUpperCase() : part))
    .join(" + ");
}

/**
 * Cheatsheet-Overlay bei `?` (Phase 10 Schritt 5) — listet alle über
 * `lib/keybindings.ts` umbelegbaren globalen Kürzel mit aktueller Belegung
 * plus (rein informativ, siehe dessen Moduldoku) die festen lokalen
 * Kürzel einzelner Komponenten. "Neu belegen" fängt den nächsten
 * Tastendruck ab und speichert ihn.
 */
export function KeybindingsCheatsheet({ open, onClose }: KeybindingsCheatsheetProps) {
  const t = useT();
  const [rebindingId, setRebindingId] = useState<string | null>(null);
  const [, forceRerender] = useState(0);
  const dialogRef = useRef<HTMLDivElement>(null);
  useFocusTrap(dialogRef, open);

  if (!open) return null;

  function handleRebindKeyDown(event: ReactKeyboardEvent, id: string) {
    event.preventDefault();
    if (event.key === "Escape") {
      setRebindingId(null);
      return;
    }
    setBinding(id, normalizeEvent(event));
    setRebindingId(null);
    forceRerender((n) => n + 1);
  }

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50" onClick={onClose}>
      <div
        ref={dialogRef}
        role="dialog"
        aria-modal="true"
        aria-label={t("cheatsheet.title")}
        className="flex max-h-[80vh] w-full max-w-lg flex-col rounded-lg border border-border bg-bg-raised shadow-xl"
        onClick={(event) => event.stopPropagation()}
      >
        <div className="flex items-center justify-between border-b border-border px-4 py-3">
          <h2 className="text-sm font-semibold">{t("cheatsheet.title")}</h2>
          <button type="button" onClick={onClose} className="text-text-secondary hover:text-text-primary" aria-label={t("settings.close")}>
            ✕
          </button>
        </div>

        <div className="flex-1 overflow-y-auto p-4 text-xs">
          <ul className="flex flex-col gap-1">
            {KEYBINDING_ACTIONS.map((action) => (
              <li key={action.id} className="flex items-center justify-between gap-2">
                <span className="text-text-secondary">{action.label}</span>
                {rebindingId === action.id ? (
                  <input
                    autoFocus
                    readOnly
                    value={t("cheatsheet.rebindPrompt")}
                    onKeyDown={(event) => handleRebindKeyDown(event, action.id)}
                    onBlur={() => setRebindingId(null)}
                    className="w-32 rounded border border-accent bg-bg-panel px-2 py-0.5 text-center"
                  />
                ) : (
                  <button
                    type="button"
                    onClick={() => setRebindingId(action.id)}
                    title={t("cheatsheet.rebindTitle")}
                    className="rounded border border-border bg-bg-panel px-2 py-0.5 font-mono hover:border-accent"
                  >
                    {displayKey(getBinding(action.id))}
                  </button>
                )}
              </li>
            ))}
          </ul>

          <button
            type="button"
            onClick={() => {
              resetBindings();
              forceRerender((n) => n + 1);
            }}
            className="mt-3 text-text-muted hover:text-text-primary"
          >
            {t("cheatsheet.resetAll")}
          </button>

          <h3 className="mt-4 mb-1 text-[10px] font-semibold uppercase tracking-wide text-text-muted">{t("cheatsheet.fixedLocalHeading")}</h3>
          <ul className="flex flex-col gap-1">
            {FIXED_LOCAL_SHORTCUTS.map((shortcut) => (
              <li key={shortcut.label} className="flex items-center justify-between gap-2">
                <span className="text-text-secondary">{shortcut.label}</span>
                <span className="rounded border border-border bg-bg-panel px-2 py-0.5 font-mono">{shortcut.key}</span>
              </li>
            ))}
          </ul>
        </div>
      </div>
    </div>
  );
}
