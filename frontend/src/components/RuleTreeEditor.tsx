import { groupNode } from "../lib/ruleTree";
import type { RuleNode } from "../lib/ruleTree";

interface RuleTreeEditorProps<TLeaf> {
  node: RuleNode<TLeaf>;
  onChange: (next: RuleNode<TLeaf>) => void;
  makeDefaultLeaf: () => TLeaf;
  renderLeaf: (leaf: TLeaf, onChange: (next: TLeaf) => void) => React.ReactNode;
  /** Nur die oberste Ebene (vom Aufrufer übergeben) darf sich nicht selbst
   * entfernen — verschachtelte Gruppen/Bedingungen bekommen ein `onRemove`
   * von ihrem Elternknoten gereicht. */
  onRemoveSelf?: () => void;
  depth?: number;
}

/**
 * Generischer, rekursiver Editor für einen [`RuleNode`]-Baum (Phase 13
 * Schritt 7, siehe `DECISIONS.md` ADR-0040-Nachtrag V) — kennt das
 * Blatt-Vokabular nicht (`renderLeaf`/`makeDefaultLeaf` kommen vom
 * Aufrufer), damit dieselbe Komponente sowohl bedingte Presets
 * (`SavePresetDialog.tsx`, Blätter = EXIF-Bedingungen) als auch
 * intelligente Sammlungen (`LibraryOrganizeDialog.tsx`, Blätter =
 * Katalogfeld-Bedingungen) bedient. Jede Gruppe zeigt einen UND/ODER-
 * Umschalter und kann beliebig viele Kinder haben — sowohl weitere
 * Bedingungen als auch verschachtelte Untergruppen.
 */
export function RuleTreeEditor<TLeaf>({ node, onChange, makeDefaultLeaf, renderLeaf, onRemoveSelf, depth = 0 }: RuleTreeEditorProps<TLeaf>) {
  if (node.type === "condition") {
    return (
      <div className="flex flex-wrap items-center gap-1 text-xs">
        {renderLeaf(node.condition, (nextLeaf) => onChange({ type: "condition", condition: nextLeaf }))}
        {onRemoveSelf && (
          <button type="button" onClick={onRemoveSelf} aria-label="Bedingung entfernen" className="shrink-0 text-danger">
            ×
          </button>
        )}
      </div>
    );
  }

  // `node` bleibt nach der obigen Prüfung zwar für TypeScript im
  // unmittelbaren Funktionskörper auf die Gruppen-Variante eingeengt, aber
  // nicht mehr innerhalb der unten deklarierten Closures (TS verwirft die
  // Control-Flow-Einengung über Funktionsgrenzen hinweg) — `group` als
  // eigene `const` behält die Einengung dauerhaft.
  const group = node;

  function updateChild(index: number, next: RuleNode<TLeaf>) {
    const children = [...group.children];
    children[index] = next;
    onChange({ ...group, children });
  }

  function removeChild(index: number) {
    onChange({ ...group, children: group.children.filter((_, i) => i !== index) });
  }

  function addCondition() {
    onChange({ ...group, children: [...group.children, { type: "condition", condition: makeDefaultLeaf() }] });
  }

  function addGroup() {
    onChange({ ...group, children: [...group.children, groupNode<TLeaf>("and", [{ type: "condition", condition: makeDefaultLeaf() }])] });
  }

  return (
    <div className={depth > 0 ? "flex flex-col gap-1 rounded border border-border p-2" : "flex flex-col gap-1"}>
      <div className="flex items-center gap-2">
        <select
          aria-label="Verknüpfung"
          value={group.operator}
          onChange={(event) => onChange({ ...group, operator: event.target.value as "and" | "or" })}
          className="rounded border border-border bg-bg-panel px-1 py-0.5 text-xs font-medium"
        >
          <option value="and">UND</option>
          <option value="or">ODER</option>
        </select>
        {onRemoveSelf && (
          <button type="button" onClick={onRemoveSelf} aria-label="Gruppe entfernen" className="shrink-0 text-xs text-danger">
            Gruppe entfernen
          </button>
        )}
      </div>

      {group.children.length === 0 && <p className="text-xs text-text-muted">Keine Bedingungen — trifft immer zu.</p>}

      {group.children.map((child, index) => (
        <RuleTreeEditor
          key={index}
          node={child}
          onChange={(next) => updateChild(index, next)}
          makeDefaultLeaf={makeDefaultLeaf}
          renderLeaf={renderLeaf}
          onRemoveSelf={() => removeChild(index)}
          depth={depth + 1}
        />
      ))}

      <div className="flex gap-1">
        <button type="button" onClick={addCondition} className="self-start rounded border border-border px-2 py-1 text-xs text-text-secondary hover:bg-bg-panel">
          + Bedingung
        </button>
        <button type="button" onClick={addGroup} className="self-start rounded border border-border px-2 py-1 text-xs text-text-secondary hover:bg-bg-panel">
          + Untergruppe
        </button>
      </div>
    </div>
  );
}
