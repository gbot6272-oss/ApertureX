/**
 * Gemeinsame Registerkarten-Leiste (Phase 18 Schritt 4, siehe
 * `DECISIONS.md` ADR-0046) — von `DevelopPanel.tsx` und `MasksPanel.tsx`
 * genutzt, um die vormals durchgehende Fieldset-Scroll-Spalte in
 * benannte Gruppen zu teilen. Bewusst generisch/minimal (kein Routing,
 * kein Lazy-Mount) — der aktive Tab ist reiner `useState` der
 * aufrufenden Komponente, dieselbe Größenordnung wie die bereits
 * bestehenden lokalen Tab-Leisten (`CURVE_CHANNEL_TABS` u. Ä.).
 */
export interface TabItem<T extends string> {
  id: T;
  label: string;
}

export function TabBar<T extends string>({
  tabs,
  active,
  onChange,
  label,
}: {
  tabs: ReadonlyArray<TabItem<T>>;
  active: T;
  onChange: (id: T) => void;
  label: string;
}) {
  return (
    <div role="tablist" aria-label={label} className="flex flex-wrap gap-0.5 rounded border border-border bg-bg-panel p-0.5">
      {tabs.map((tab) => (
        <button
          key={tab.id}
          type="button"
          role="tab"
          aria-selected={active === tab.id}
          onClick={() => onChange(tab.id)}
          className={`flex-1 rounded px-2 py-1 text-xs transition-colors duration-[var(--duration-fast)] ${
            active === tab.id ? "bg-accent/10 text-accent" : "text-text-secondary hover:text-text-primary"
          }`}
        >
          {tab.label}
        </button>
      ))}
    </div>
  );
}
