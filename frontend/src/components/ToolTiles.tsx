import { type ReactNode } from "react";

/**
 * Der gemeinsame Kopf und die gemeinsame Kachel der beiden
 * Werkzeug-Panels (Kreativ aus Phase 27, Licht & Optik aus Phase 28).
 *
 * **Warum das ab Phase 28 nötig wurde:** mit zwölf weiteren Werkzeugen
 * stehen 22 Kacheln in zwei Panels. Ohne Hilfe wäre das genau die
 * Scroll-Wüste, die Phase 18 abgeschafft hat. Beide Panels bekommen
 * deshalb dasselbe Suchfeld, denselben „Nur aktive"-Schalter und
 * denselben Zurücksetzen-Knopf je Kachel — eine Fassung statt zweier,
 * die auseinanderlaufen können.
 */

/** Passt ein Werkzeug zur Suche? Sucht über Titel UND Wirkung, damit
 * „Himmel" auch das Werkzeug findet, das „Himmel" nur im Halbsatz
 * stehen hat. */
export function matchesToolQuery(title: string, hint: string, query: string): boolean {
  const needle = query.trim().toLowerCase();
  if (!needle) return true;
  return title.toLowerCase().includes(needle) || hint.toLowerCase().includes(needle);
}

export interface ToolToolbarProps {
  title: string;
  query: string;
  onQueryChange: (value: string) => void;
  onlyActive: boolean;
  onOnlyActiveChange: (value: boolean) => void;
  onReset: () => void;
  /** Wie viele Werkzeuge die aktuelle Filterung übrig lässt. */
  visibleCount: number;
  totalCount: number;
  /** Für `aria-controls`/`data-testid`-Eindeutigkeit, wenn beide Panels
   * gleichzeitig im DOM stehen. */
  idPrefix: string;
}

export function ToolToolbar({
  title,
  query,
  onQueryChange,
  onlyActive,
  onOnlyActiveChange,
  onReset,
  visibleCount,
  totalCount,
  idPrefix,
}: ToolToolbarProps) {
  return (
    <div className="flex flex-col gap-2">
      <div className="flex items-center justify-between gap-2">
        <h3 className="text-sm font-semibold text-text-primary">{title}</h3>
        <button
          type="button"
          onClick={onReset}
          className="rounded px-2 py-1 text-xs text-text-muted transition-colors duration-[var(--duration-fast)] hover:text-accent"
        >
          Alles zurücksetzen
        </button>
      </div>
      <div className="flex items-center gap-2">
        <input
          type="search"
          value={query}
          onChange={(event) => onQueryChange(event.target.value)}
          placeholder="Werkzeug suchen…"
          aria-label={`${title} durchsuchen`}
          data-testid={`${idPrefix}-search`}
          className="apx-glass min-w-0 flex-1 rounded border border-[var(--glass-border)] px-2 py-1 text-xs text-text-primary transition-[border-color] duration-[var(--duration-fast)] outline-none placeholder:text-text-muted focus:border-accent"
        />
        <button
          type="button"
          aria-pressed={onlyActive}
          onClick={() => onOnlyActiveChange(!onlyActive)}
          data-testid={`${idPrefix}-only-active`}
          className={`apx-btn-liquid shrink-0 rounded border px-2 py-1 text-xs whitespace-nowrap transition-colors duration-[var(--duration-fast)] ${
            onlyActive
              ? "apx-btn-liquid-active border-accent bg-accent/10 text-accent"
              : "border-border text-text-secondary hover:border-accent hover:text-text-primary"
          }`}
        >
          Nur aktive
        </button>
      </div>
      {visibleCount < totalCount && (
        <p aria-live="polite" className="text-[11px] text-text-muted">
          {visibleCount} von {totalCount} Werkzeugen
        </p>
      )}
    </div>
  );
}

export interface ToolTileProps {
  title: string;
  hint: string;
  active: boolean;
  /** Setzt NUR dieses Werkzeug zurück. */
  onReset: () => void;
  children: ReactNode;
}

export function ToolTile({ title, hint, active, onReset, children }: ToolTileProps) {
  return (
    <section
      aria-label={title}
      data-active={active ? "true" : "false"}
      className={`apx-glass group flex flex-col gap-2 rounded-lg border p-3 transition-[border-color,box-shadow] duration-[var(--duration-base)] hover:border-accent/50 hover:shadow-[var(--shadow-md)] ${
        active ? "border-accent/60" : "border-[var(--glass-border)]"
      }`}
    >
      <div className="flex items-baseline justify-between gap-2">
        <span className="text-sm font-medium text-text-primary">{title}</span>
        <span className="flex shrink-0 items-baseline gap-2">
          {active && (
            <span className="text-[10px] font-medium tracking-wide text-accent uppercase">aktiv</span>
          )}
          {/* Erscheint erst bei Hover oder Tastaturfokus — die Kachel
              bleibt ruhig, der Knopf ist aber dauerhaft in der
              Tab-Reihenfolge (Deckkraft entfernt ihn nicht daraus). */}
          <button
            type="button"
            onClick={onReset}
            aria-label={`${title} zurücksetzen`}
            className="rounded px-1 text-[11px] text-text-muted opacity-0 transition-[opacity,color] duration-[var(--duration-fast)] group-hover:opacity-100 hover:text-accent focus-visible:opacity-100"
          >
            ↺
          </button>
        </span>
      </div>
      <p className="text-xs text-text-muted">{hint}</p>
      {children}
    </section>
  );
}
