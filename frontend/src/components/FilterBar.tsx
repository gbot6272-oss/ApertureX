import { useAppStore } from "../store";
import { COLOR_LABELS, COLOR_SWATCH } from "./RatingFlagColor";

/**
 * Filterleiste (Phase 3, Schritt 6): Suchfeld (`search_photos`, FTS5 über
 * Dateiname/Kamera/Objektiv) plus Attribut-Chips (Bewertung/Flagge/Farbe,
 * `filter_photos`, per UND kombiniert). Suche und Attributfilter sind
 * bewusst alternativ statt kombiniert (siehe `store/index.ts`s
 * `setLibraryFilterChip`/`runLibrarySearch`) — beide wirken über
 * `libraryResults` auf `selectActivePhotos`, das Raster und Filmstreifen
 * gemeinsam lesen.
 */
export function FilterBar() {
  const libraryQuery = useAppStore((s) => s.libraryQuery);
  const libraryFilter = useAppStore((s) => s.libraryFilter);
  const libraryResults = useAppStore((s) => s.libraryResults);
  const setLibraryQuery = useAppStore((s) => s.setLibraryQuery);
  const runLibrarySearch = useAppStore((s) => s.runLibrarySearch);
  const setLibraryFilterChip = useAppStore((s) => s.setLibraryFilterChip);
  const clearLibraryFilters = useAppStore((s) => s.clearLibraryFilters);

  const hasActiveFilter = libraryResults !== null;

  return (
    <div className="flex shrink-0 flex-wrap items-center gap-2 border-b border-border bg-bg-raised px-3 py-2">
      <input
        type="search"
        value={libraryQuery}
        onChange={(event) => setLibraryQuery(event.target.value)}
        onKeyDown={(event) => {
          if (event.key === "Enter") void runLibrarySearch();
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

      {hasActiveFilter && (
        <button type="button" onClick={clearLibraryFilters} className="ml-auto rounded border border-border px-2 py-1 text-xs hover:border-accent">
          Filter zurücksetzen
        </button>
      )}
    </div>
  );
}
