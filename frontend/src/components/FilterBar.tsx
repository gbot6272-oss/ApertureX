import { SORT_FIELDS, sortFieldLabel } from "../lib/sortPhotos";
import { useAppStore } from "../store";
import { COLOR_LABELS, COLOR_SWATCH } from "./RatingFlagColor";

/**
 * Filterleiste (Phase 3, Schritt 6, erweitert in Schritt 8): Suchfeld
 * (`search_and_filter_photos`, FTS5 über Dateiname/Kamera/Objektiv) plus
 * Attribut-Chips (Bewertung/Flagge/Farbe/Kameramodell) — beide sind
 * kombinierbar (per UND, siehe `store/index.ts`s
 * `runLibrarySearchAndFilter`/`setLibraryFilterChip` sowie `DECISIONS.md`
 * ADR-0027, das die frühere ADR-0026-Entscheidung "bewusst alternativ"
 * zurücknimmt). Dazu ein "Duplikate anzeigen"-Knopf (Schritt 8.2) und eine
 * Sortierauswahl (Schritt 8.3) — alle wirken über `libraryResults` auf
 * `selectActivePhotos`, das Raster und Filmstreifen gemeinsam lesen.
 */
export function FilterBar() {
  const libraryQuery = useAppStore((s) => s.libraryQuery);
  const libraryFilter = useAppStore((s) => s.libraryFilter);
  const libraryResults = useAppStore((s) => s.libraryResults);
  const setLibraryQuery = useAppStore((s) => s.setLibraryQuery);
  const runLibrarySearchAndFilter = useAppStore((s) => s.runLibrarySearchAndFilter);
  const setLibraryFilterChip = useAppStore((s) => s.setLibraryFilterChip);
  const clearLibraryFilters = useAppStore((s) => s.clearLibraryFilters);
  const showDuplicatePhotos = useAppStore((s) => s.showDuplicatePhotos);
  const librarySortField = useAppStore((s) => s.librarySortField);
  const librarySortDirection = useAppStore((s) => s.librarySortDirection);
  const setLibrarySort = useAppStore((s) => s.setLibrarySort);

  const hasActiveFilter = libraryResults !== null;

  return (
    <div className="flex shrink-0 flex-wrap items-center gap-2 border-b border-border bg-bg-raised px-3 py-2">
      <input
        type="search"
        value={libraryQuery}
        onChange={(event) => setLibraryQuery(event.target.value)}
        onKeyDown={(event) => {
          if (event.key === "Enter") void runLibrarySearchAndFilter();
        }}
        placeholder="Suche (Dateiname, Kamera, Objektiv)…"
        className="w-64 rounded border border-border bg-bg-panel px-2 py-1 text-sm"
      />

      <div className="flex items-center gap-1" role="group" aria-label="Nach Bewertung filtern">
        {[1, 2, 3, 4, 5].map((value) => (
          <button
            key={value}
            type="button"
            onClick={() => void setLibraryFilterChip({ rating_at_least: libraryFilter.rating_at_least === value ? undefined : value })}
            aria-pressed={libraryFilter.rating_at_least === value}
            title={`Bewertung ${value}+`}
            className={`rounded border px-1.5 py-0.5 text-xs ${
              libraryFilter.rating_at_least === value ? "border-accent bg-accent/10 text-accent" : "border-border text-text-secondary hover:border-accent"
            }`}
          >
            {value}★+
          </button>
        ))}
      </div>

      <div className="flex items-center gap-1" role="group" aria-label="Nach Flagge filtern">
        <button
          type="button"
          onClick={() => void setLibraryFilterChip({ flag: libraryFilter.flag === 1 ? undefined : 1 })}
          aria-pressed={libraryFilter.flag === 1}
          className={`rounded border px-1.5 py-0.5 text-xs ${
            libraryFilter.flag === 1 ? "border-accent bg-accent/10 text-accent" : "border-border text-text-secondary hover:border-accent"
          }`}
        >
          Pick
        </button>
        <button
          type="button"
          onClick={() => void setLibraryFilterChip({ flag: libraryFilter.flag === -1 ? undefined : -1 })}
          aria-pressed={libraryFilter.flag === -1}
          className={`rounded border px-1.5 py-0.5 text-xs ${
            libraryFilter.flag === -1 ? "border-danger bg-danger/10 text-danger" : "border-border text-text-secondary hover:border-danger"
          }`}
        >
          Reject
        </button>
      </div>

      <div className="flex items-center gap-1" role="group" aria-label="Nach Farbe filtern">
        {COLOR_LABELS.map((color) => (
          <button
            key={color}
            type="button"
            onClick={() => void setLibraryFilterChip({ color_label: libraryFilter.color_label === color ? undefined : color })}
            aria-pressed={libraryFilter.color_label === color}
            title={color}
            className={`h-4 w-4 rounded-full border ${libraryFilter.color_label === color ? "ring-2 ring-text-primary" : "opacity-60 hover:opacity-100"}`}
            style={{ backgroundColor: COLOR_SWATCH[color] }}
          />
        ))}
      </div>

      <input
        type="text"
        defaultValue={libraryFilter.camera_model ?? ""}
        key={libraryFilter.camera_model ?? ""}
        onKeyDown={(event) => {
          if (event.key !== "Enter") return;
          const value = event.currentTarget.value.trim();
          void setLibraryFilterChip({ camera_model: value || undefined });
        }}
        placeholder="Kameramodell…"
        aria-label="Nach Kameramodell filtern"
        className="w-40 rounded border border-border bg-bg-panel px-2 py-1 text-xs"
      />

      <button
        type="button"
        onClick={() => void showDuplicatePhotos()}
        title="Fotos mit identischem Inhalt (exakter Hash-Vergleich) anzeigen"
        className="rounded border border-border px-2 py-1 text-xs text-text-secondary hover:border-accent"
      >
        Duplikate anzeigen
      </button>

      <div className="flex items-center gap-1" role="group" aria-label="Sortierung">
        <label htmlFor="library-sort-field" className="sr-only">
          Sortieren nach
        </label>
        <select
          id="library-sort-field"
          value={librarySortField}
          onChange={(event) => setLibrarySort(event.target.value as typeof librarySortField, librarySortDirection)}
          className="rounded border border-border bg-bg-panel px-2 py-1 text-xs"
        >
          {SORT_FIELDS.map((field) => (
            <option key={field} value={field}>
              {sortFieldLabel(field)}
            </option>
          ))}
        </select>
        <button
          type="button"
          onClick={() => setLibrarySort(librarySortField, librarySortDirection === "asc" ? "desc" : "asc")}
          aria-label={librarySortDirection === "asc" ? "Aufsteigend sortiert, absteigend sortieren" : "Absteigend sortiert, aufsteigend sortieren"}
          title={librarySortDirection === "asc" ? "Aufsteigend" : "Absteigend"}
          className="rounded border border-border px-2 py-1 text-xs text-text-secondary hover:border-accent"
        >
          {librarySortDirection === "asc" ? "↑" : "↓"}
        </button>
      </div>

      {hasActiveFilter && (
        <button type="button" onClick={clearLibraryFilters} className="ml-auto rounded border border-border px-2 py-1 text-xs hover:border-accent">
          Filter zurücksetzen
        </button>
      )}
    </div>
  );
}
