/** Alles, was zum Aufbau eines Baums über `parent_id` gebraucht wird —
 * sowohl `FolderDto` (Bibliothek, ab Phase 1) als auch `PresetFolderDto`
 * (ab Phase 5, siehe `DECISIONS.md` ADR-0031) erfüllen das strukturell. */
interface HasParent {
  id: string;
  parent_id: string | null;
}

/** Baut aus einer flachen Liste (`FolderDto[]` oder `PresetFolderDto[]`)
 * eine Baumstruktur über `parent_id` — der Baum existiert nur hier im
 * Frontend, in der Datenbank ist `parent_id` nur eine Spalte ohne
 * UI-Nutzung (siehe `PLAN.md` Phase 3, Schritt 5). Ein Eintrag gilt als
 * Wurzel, wenn `parent_id` fehlt oder auf keinen bekannten Eintrag
 * zeigt (defensiv gegen inkonsistente Daten, statt ihn stillschweigend zu
 * verlieren).
 */
export function buildChildrenByParent<T extends HasParent>(
  items: T[],
): {
  roots: T[];
  childrenOf: Map<string, T[]>;
} {
  const knownIds = new Set(items.map((item) => item.id));
  const childrenOf = new Map<string, T[]>();
  const roots: T[] = [];

  for (const item of items) {
    if (item.parent_id && knownIds.has(item.parent_id)) {
      const siblings = childrenOf.get(item.parent_id) ?? [];
      siblings.push(item);
      childrenOf.set(item.parent_id, siblings);
    } else {
      roots.push(item);
    }
  }
  return { roots, childrenOf };
}
