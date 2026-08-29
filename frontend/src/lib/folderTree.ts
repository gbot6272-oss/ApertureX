import type { FolderDto } from "./tauri";

/** Baut aus der flachen `FolderDto[]`-Liste (siehe `commands.rs::list_folders`)
 * eine Baumstruktur über `parent_id` — der Ordnerbaum existiert nur hier im
 * Frontend, im Katalog ist `folders.parent_id` seit Phase 1 nur eine
 * Spalte ohne UI-Nutzung (siehe `PLAN.md` Phase 3, Schritt 5). Ein Ordner
 * gilt als Wurzel, wenn `parent_id` fehlt oder auf keinen bekannten Ordner
 * zeigt (defensiv gegen inkonsistente Daten, statt ihn stillschweigend zu
 * verlieren).
 */
export function buildChildrenByParent(folders: FolderDto[]): {
  roots: FolderDto[];
  childrenOf: Map<string, FolderDto[]>;
} {
  const knownIds = new Set(folders.map((f) => f.id));
  const childrenOf = new Map<string, FolderDto[]>();
  const roots: FolderDto[] = [];

  for (const folder of folders) {
    if (folder.parent_id && knownIds.has(folder.parent_id)) {
      const siblings = childrenOf.get(folder.parent_id) ?? [];
      siblings.push(folder);
      childrenOf.set(folder.parent_id, siblings);
    } else {
      roots.push(folder);
    }
  }
  return { roots, childrenOf };
}
