import { Layers, SlidersHorizontal } from "lucide-react";
import { useEffect, useState } from "react";

import { SORT_FIELDS, sortFieldLabel } from "../lib/sortPhotos";
import { useShallow } from "zustand/react/shallow";

import { stacksInView } from "../lib/stackGrouping";
import { selectActivePhotos } from "../store";
import type { AspectFilter } from "../lib/tauri";
import { useAppStore } from "../store";
import { COLOR_LABELS, COLOR_SWATCH } from "./RatingFlagColor";

/**
 * Filterleiste (Phase 3, Schritt 6, erweitert in Schritt 8): Suchfeld
 * (`search_and_filter_photos`, FTS5 über Dateiname/Kamera/Objektiv/Titel/
 * Beschriftung/Urheber/Copyright plus Schlagworte und Bildnotizen, siehe
 * `DECISIONS.md` ADR-0070) plus
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

  const filterPresets = useAppStore((s) => s.filterPresets);
  const refreshFilterPresets = useAppStore((s) => s.refreshFilterPresets);
  const saveCurrentFilterAsPreset = useAppStore((s) => s.saveCurrentFilterAsPreset);
  const applyFilterPreset = useAppStore((s) => s.applyFilterPreset);
  const deleteFilterPreset = useAppStore((s) => s.deleteFilterPreset);
  const [newPresetName, setNewPresetName] = useState("");
  // Erweiterte Filter (Phase 33 F7) hinter einem Schalter statt
  // dauerhaft sichtbar: sechs weitere Eingabefelder in der ohnehin schon
  // vollen Zeile hätten aus der Leiste eine Wand gemacht. Ist einer der
  // neuen Filter gesetzt, klappt der Bereich von selbst auf — sonst wäre
  // ein aktiver Filter unsichtbar, und das ist die schlimmste Sorte
  // Filter.
  // Stapel im Raster (Phase 33 F10): ein Sammel-Schalter, weil ein
  // Stapel-Abzeichen je Kachel zwar zum gezielten Aufklappen taugt, aber
  // nicht zum „zeig mir mal alles".
  const stacks = useAppStore((s) => s.stacks);
  // `selectActivePhotos` sortiert und liefert damit bei jedem Aufruf
  // ein neues Array — ohne `useShallow` hielte Zustand das für eine
  // Änderung und liefe in eine Endlosschleife (genau das hat hier beim
  // ersten Versuch die ganze Oberfläche lahmgelegt).
  const activePhotos = useAppStore(useShallow(selectActivePhotos));
  const expandedStackIds = useAppStore((s) => s.expandedStackIds);
  const setAllStacksExpanded = useAppStore((s) => s.setAllStacksExpanded);
  const stackIdsInView = stacksInView(activePhotos, stacks);
  const allExpanded = stackIdsInView.length > 0 && stackIdsInView.every((id) => expandedStackIds.includes(id));

  const [advancedOpen, setAdvancedOpen] = useState(false);
  const advancedActive =
    libraryFilter.lens !== undefined ||
    libraryFilter.iso_min !== undefined ||
    libraryFilter.iso_max !== undefined ||
    libraryFilter.captured_from !== undefined ||
    libraryFilter.captured_to !== undefined ||
    libraryFilter.aspect !== undefined ||
    libraryFilter.media_kind !== undefined;
  const showAdvanced = advancedOpen || advancedActive;

  /** Ein lokales Datum (`YYYY-MM-DD`) als Unix-Sekunden. `endOfDay`
   * schiebt auf 23:59:59 — ohne das fiele der gewählte Endtag selbst aus
   * dem Zeitraum heraus, was niemand meint, der „bis zum 5. Mai" sagt. */
  const dateToUnix = (value: string, endOfDay: boolean): number | undefined => {
    if (!value) return undefined;
    const date = new Date(`${value}T${endOfDay ? "23:59:59" : "00:00:00"}`);
    const seconds = Math.floor(date.getTime() / 1000);
    return Number.isNaN(seconds) ? undefined : seconds;
  };

  /** Unix-Sekunden zurück in ein lokales `YYYY-MM-DD` fürs Eingabefeld. */
  const unixToDate = (value: number | undefined): string => {
    if (value === undefined) return "";
    const date = new Date(value * 1000);
    if (Number.isNaN(date.getTime())) return "";
    const pad = (n: number) => String(n).padStart(2, "0");
    return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}`;
  };

  useEffect(() => {
    void refreshFilterPresets();
  }, [refreshFilterPresets]);

  const hasActiveFilter = libraryResults !== null;

  return (
    // Phase 25 Nachtrag III: eigenes `pt-12` statt der vorherigen
    // symmetrischen `py-2` — dieselbe Begründung wie `ErrorBanner.tsx`s
    // aktueller Moduldoku: die jetzt schwebende Kopfzeile (`Header.tsx`)
    // nimmt keinen Platz im Dokumentfluss mehr ein, diese Leiste ist
    // dadurch potenziell das oberste Flusselement und muss die 48px
    // selbst zurückgewinnen. `PaletteFrame.tsx` lässt seine eigene
    // Kopfzeilen-Kompensation genau in den Ansichten weg, in denen
    // diese Leiste erscheint (Raster/Übersicht) — sie übernimmt die
    // Kompensation hier stellvertretend für die ganze Zeile darunter.
    <div className="flex shrink-0 flex-wrap items-center gap-2 border-b border-border bg-bg-raised px-3 pt-12 pb-2">
      <input
        type="search"
        value={libraryQuery}
        onChange={(event) => setLibraryQuery(event.target.value)}
        onKeyDown={(event) => {
          if (event.key === "Enter") void runLibrarySearchAndFilter();
        }}
        // Der Platzhalter nennt seit Phase 34 F1 nicht mehr nur drei
        // Felder: die Suche deckt jetzt auch die selbst gepflegten
        // Textfelder, Schlagworte und Bildnotizen ab. Ein Suchfeld, das
        // weniger verspricht als es kann, wird für das Gefundene nicht
        // benutzt — dieselbe Auffindbarkeits-Linie wie ADR-0046.
        placeholder="Suche (Dateiname, Kamera, Titel, Schlagwort, Notiz…)"
        title="Durchsucht Dateiname, Kamerahersteller und -modell, Objektiv, Titel, Beschriftung, Urheber, Copyright, Schlagworte und Bildnotizen"
        className="w-72 rounded border border-border bg-bg-panel px-2 py-1 text-sm"
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

      {stackIdsInView.length > 0 && (
        <button
          type="button"
          onClick={() => setAllStacksExpanded(!allExpanded)}
          aria-pressed={allExpanded}
          data-testid="toggle-all-stacks"
          className={`flex items-center gap-1 rounded border px-2 py-1 text-xs ${
            allExpanded ? "border-accent text-accent" : "border-border text-text-secondary hover:border-accent"
          }`}
        >
          <Layers aria-hidden="true" className="size-3.5" />
          {allExpanded
            ? `${stackIdsInView.length} Stapel einklappen`
            : `${stackIdsInView.length} Stapel aufklappen`}
        </button>
      )}

      <button
        type="button"
        onClick={() => setAdvancedOpen((open) => !open)}
        aria-pressed={showAdvanced}
        aria-label="Erweiterte Filter"
        className={`flex items-center gap-1 rounded border px-2 py-1 text-xs ${
          advancedActive ? "border-accent text-accent" : "border-border text-text-secondary hover:border-accent"
        }`}
      >
        <SlidersHorizontal aria-hidden="true" className="size-3.5" />
        Mehr
      </button>

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

      <div className="flex items-center gap-1" role="group" aria-label="Filter-Presets">
        <select
          value=""
          onChange={(event) => {
            if (event.target.value) void applyFilterPreset(event.target.value);
          }}
          className="rounded border border-border bg-bg-panel px-1 py-1 text-xs"
          aria-label="Filter-Preset anwenden"
        >
          <option value="">Filter-Preset…</option>
          {filterPresets.map((preset) => (
            <option key={preset.id} value={preset.id}>
              {preset.name}
            </option>
          ))}
        </select>
        {filterPresets.length > 0 && (
          <select
            value=""
            onChange={(event) => {
              if (event.target.value) void deleteFilterPreset(event.target.value);
            }}
            className="rounded border border-border bg-bg-panel px-1 py-1 text-xs"
            aria-label="Filter-Preset löschen"
            title="Filter-Preset löschen"
          >
            <option value="">Löschen…</option>
            {filterPresets.map((preset) => (
              <option key={preset.id} value={preset.id}>
                {preset.name}
              </option>
            ))}
          </select>
        )}
        <input
          type="text"
          value={newPresetName}
          onChange={(event) => setNewPresetName(event.target.value)}
          placeholder="Neues Preset…"
          className="w-28 rounded border border-border bg-bg-panel px-2 py-1 text-xs"
        />
        <button
          type="button"
          onClick={() => {
            void saveCurrentFilterAsPreset(newPresetName);
            setNewPresetName("");
          }}
          disabled={!newPresetName.trim()}
          className="rounded border border-border px-2 py-1 text-xs hover:border-accent disabled:opacity-40"
        >
          Speichern
        </button>
      </div>

      {hasActiveFilter && (
        <button type="button" onClick={clearLibraryFilters} className="ml-auto rounded border border-border px-2 py-1 text-xs hover:border-accent">
          Filter zurücksetzen
        </button>
      )}

      {showAdvanced && (
        <div
          className="flex w-full flex-wrap items-center gap-2 border-t border-border pt-2"
          role="group"
          aria-label="Erweiterte Filter"
          data-testid="advanced-filters"
        >
          <input
            type="text"
            defaultValue={libraryFilter.lens ?? ""}
            key={`lens-${libraryFilter.lens ?? ""}`}
            onKeyDown={(event) => {
              if (event.key !== "Enter") return;
              const value = event.currentTarget.value.trim();
              void setLibraryFilterChip({ lens: value || undefined });
            }}
            placeholder="Objektiv…"
            aria-label="Nach Objektiv filtern"
            className="w-40 rounded border border-border bg-bg-panel px-2 py-1 text-xs"
          />

          <span className="flex items-center gap-1 text-xs text-text-secondary">
            ISO
            <input
              type="number"
              min={0}
              value={libraryFilter.iso_min ?? ""}
              onChange={(event) => {
                const value = event.target.value;
                void setLibraryFilterChip({ iso_min: value === "" ? undefined : Number(value) });
              }}
              aria-label="ISO mindestens"
              className="w-20 rounded border border-border bg-bg-panel px-2 py-1 text-xs"
            />
            bis
            <input
              type="number"
              min={0}
              value={libraryFilter.iso_max ?? ""}
              onChange={(event) => {
                const value = event.target.value;
                void setLibraryFilterChip({ iso_max: value === "" ? undefined : Number(value) });
              }}
              aria-label="ISO höchstens"
              className="w-20 rounded border border-border bg-bg-panel px-2 py-1 text-xs"
            />
          </span>

          <span className="flex items-center gap-1 text-xs text-text-secondary">
            Aufgenommen
            <input
              type="date"
              value={unixToDate(libraryFilter.captured_from)}
              onChange={(event) => void setLibraryFilterChip({ captured_from: dateToUnix(event.target.value, false) })}
              aria-label="Aufgenommen ab"
              className="rounded border border-border bg-bg-panel px-2 py-1 text-xs"
            />
            bis
            <input
              type="date"
              value={unixToDate(libraryFilter.captured_to)}
              onChange={(event) => void setLibraryFilterChip({ captured_to: dateToUnix(event.target.value, true) })}
              aria-label="Aufgenommen bis"
              className="rounded border border-border bg-bg-panel px-2 py-1 text-xs"
            />
          </span>

          <select
            value={libraryFilter.aspect ?? ""}
            onChange={(event) =>
              void setLibraryFilterChip({ aspect: (event.target.value || undefined) as AspectFilter | undefined })
            }
            aria-label="Seitenverhältnis"
            className="rounded border border-border bg-bg-panel px-2 py-1 text-xs"
          >
            <option value="">Jedes Format</option>
            <option value="landscape">Querformat</option>
            <option value="portrait">Hochformat</option>
            <option value="square">Quadratisch</option>
          </select>

          <select
            value={libraryFilter.media_kind ?? ""}
            onChange={(event) =>
              void setLibraryFilterChip({
                media_kind: (event.target.value || undefined) as "photo" | "video" | undefined,
              })
            }
            aria-label="Medienart"
            className="rounded border border-border bg-bg-panel px-2 py-1 text-xs"
          >
            <option value="">Fotos und Videos</option>
            <option value="photo">Nur Fotos</option>
            <option value="video">Nur Videos</option>
          </select>
        </div>
      )}
    </div>
  );
}
