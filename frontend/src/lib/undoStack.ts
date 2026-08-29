/**
 * Undo/Redo für Bibliotheks-Metadaten (Phase 3, Schritt 8.1, siehe
 * `DECISIONS.md` ADR-0027) — bewusst reines Frontend-Feature ohne neue
 * Backend-Verlaufstabelle: anders als beim Entwickeln (`edit_history`,
 * ADR-0014) gibt es für Bewertung/Flagge/Farbe/Schlagworte/Sammlungs-
 * mitgliedschaft keinen natürlichen Ort für einen Verlauf im Katalog — die
 * "Wahrheit" bleibt hier der zuletzt bekannte Frontend-Zustand.
 *
 * Reine, testbare Stack-Logik — der Store (`store/index.ts`) hält den
 * eigentlichen Zustand (`libraryUndoStack`/`libraryRedoStack`) und ruft
 * diese Funktionen auf. Bewusst **nicht** abgedeckt: Sammlung anlegen/
 * umbenennen/löschen (strukturelle Aktionen mit unklarer Undo-Semantik bei
 * Neuvergabe der ID) — siehe ADR-0027.
 */

export interface UndoEntry {
  /** Kurzer, menschenlesbarer Name der Aktion (z. B. "Bewertung"), für eine
   * spätere sichtbare Rückmeldung ("Rückgängig: Bewertung"). */
  label: string;
  undo: () => Promise<void>;
  redo: () => Promise<void>;
}

export interface UndoStacks {
  undoStack: UndoEntry[];
  redoStack: UndoEntry[];
}

export function emptyUndoStacks(): UndoStacks {
  return { undoStack: [], redoStack: [] };
}

/** Legt `entry` auf den Undo-Stack. Eine neue Aktion verwirft die per
 * [`undo`] erreichte Redo-"Zukunft" — gleiches Prinzip wie beim
 * Entwickeln-Verlauf (`DECISIONS.md` ADR-0014). Reine Funktion: gibt einen
 * neuen `UndoStacks`-Wert zurück, mutiert `stacks` nicht. */
export function pushUndo(stacks: UndoStacks, entry: UndoEntry): UndoStacks {
  return { undoStack: [...stacks.undoStack, entry], redoStack: [] };
}

/** Macht den obersten Undo-Eintrag rückgängig (führt `entry.undo()` aus)
 * und verschiebt ihn auf den Redo-Stack. No-op (liefert `stacks`
 * unverändert zurück), wenn der Undo-Stack leer ist. */
export async function undo(stacks: UndoStacks): Promise<UndoStacks> {
  const entry = stacks.undoStack.at(-1);
  if (!entry) return stacks;
  await entry.undo();
  return {
    undoStack: stacks.undoStack.slice(0, -1),
    redoStack: [...stacks.redoStack, entry],
  };
}

/** Wiederholt den obersten Redo-Eintrag (führt `entry.redo()` aus) und
 * verschiebt ihn zurück auf den Undo-Stack. No-op, wenn der Redo-Stack
 * leer ist. */
export async function redo(stacks: UndoStacks): Promise<UndoStacks> {
  const entry = stacks.redoStack.at(-1);
  if (!entry) return stacks;
  await entry.redo();
  return {
    undoStack: [...stacks.undoStack, entry],
    redoStack: stacks.redoStack.slice(0, -1),
  };
}
