/**
 * Minimale `cn()`-Hilfsfunktion (Phase 23, siehe DECISIONS.md
 * ADR-0051) — bewusst ohne `clsx`/`tailwind-merge` als neue
 * Abhängigkeit: dieses Projekt kombiniert Tailwind-Klassen bisher
 * immer per Template-String/Array-`join`, `cn()` ist nur eine
 * aufgeräumte Variante desselben Musters (filtert falsy Werte raus),
 * kompatibel mit dem Signatur-Vorbild, das portierte shadcn-artige
 * Komponenten (`components/ui/DotLoader.tsx`) erwarten. Kein
 * Klassen-Konflikt-Merging wie `tailwind-merge` — bei den bisher
 * hier verwendeten, disjunkten Klassenlisten nicht nötig.
 */
export function cn(...inputs: Array<string | false | null | undefined>): string {
  return inputs.filter(Boolean).join(" ");
}
