/**
 * Generischer, beliebig tief verschachtelbarer UND/ODER-Regelbaum (Phase 13
 * Schritt 7, siehe `DECISIONS.md` ADR-0040-Nachtrag V) — löst die zuvor
 * ausschließlich flache, UND-verknüpfte Regelliste sowohl bei bedingten
 * Presets (`lib/presets.ts`) als auch bei intelligenten Sammlungen
 * (`apx_catalog::FilterNode`, `LibraryOrganizeDialog.tsx`) ab. Dieses Modul
 * kennt die konkreten Blatt-Bedingungen nicht (`TLeaf` ist generisch) —
 * `RuleTreeEditor.tsx` rendert den Baum unabhängig vom Blatt-Vokabular,
 * jede Stelle liefert nur ihren eigenen Blatt-Editor mit.
 *
 * Das JSON-Schema ist absichtlich so gewählt, dass es 1:1 zum
 * Rust-Gegenstück (`apx_catalog::FilterNode`, `#[serde(tag = "type")]`)
 * passt: `{"type":"condition","condition":{...}}` /
 * `{"type":"group","operator":"and"|"or","children":[...]}`.
 */
export type RuleNode<TLeaf> =
  | { type: "condition"; condition: TLeaf }
  | { type: "group"; operator: "and" | "or"; children: RuleNode<TLeaf>[] };

export function conditionNode<TLeaf>(condition: TLeaf): RuleNode<TLeaf> {
  return { type: "condition", condition };
}

export function groupNode<TLeaf>(operator: "and" | "or" = "and", children: RuleNode<TLeaf>[] = []): RuleNode<TLeaf> {
  return { type: "group", operator, children };
}

/** Wertet den Baum rekursiv aus — `evaluateLeaf` prüft eine einzelne
 * Bedingung gegen den jeweiligen Auswertungskontext (Fotometadaten für
 * bedingte Presets, Katalogfelder für intelligente Sammlungen). Eine
 * Gruppe ohne Kinder ist für UND vakuos wahr, für ODER vakuos falsch —
 * dieselbe Konvention wie das Rust-Gegenstück `FilterNode::matches`. */
export function evaluateRuleNode<TLeaf>(node: RuleNode<TLeaf>, evaluateLeaf: (leaf: TLeaf) => boolean): boolean {
  if (node.type === "condition") return evaluateLeaf(node.condition);
  if (node.children.length === 0) return node.operator === "and";
  return node.operator === "and"
    ? node.children.every((child) => evaluateRuleNode(child, evaluateLeaf))
    : node.children.some((child) => evaluateRuleNode(child, evaluateLeaf));
}

/** Zählt die Blattbedingungen im Baum — für kleine UI-Hinweise wie
 * „3 Bedingungen" ohne die Gruppenstruktur mitzuzählen. */
export function countLeaves<TLeaf>(node: RuleNode<TLeaf>): number {
  if (node.type === "condition") return 1;
  return node.children.reduce((sum, child) => sum + countLeaves(child), 0);
}
