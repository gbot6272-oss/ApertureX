import { CalendarDays, CheckSquare, ChevronLeft, ChevronRight, ImageOff, RefreshCw, X } from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";
import { useShallow } from "zustand/react/shallow";

import { buildCalendar, countByDay, densityLevel, type CalendarMonth } from "../lib/calendarGrid";
import { previewUrl } from "../lib/media";
import { playCue } from "../lib/sound";
import { selectActivePhotos, useAppStore } from "../store";

/**
 * Kalenderansicht (Phase 32 F3, siehe `DECISIONS.md`).
 *
 * Der Katalog liess sich bisher nach Ordner, Sammlung, Person, Ort und
 * Regelwerk durchsehen — nur nicht nach der Zeit, obwohl das
 * Aufnahmedatum die Eigenschaft ist, nach der man sich am ehesten
 * erinnert. Diese Ansicht zeigt jeden Aufnahmetag als Zelle, nach Dichte
 * eingefärbt, und macht einen Tag mit einem Klick zur Auswahl.
 *
 * **Woher die Fotos kommen.** Aus `selectActivePhotos` — derselben Liste,
 * die Raster und Filmstreifen zeigen. Der Kalender ist damit eine Sicht
 * auf die aktuelle Auswahl (Ordner/Sammlung/Suchergebnis), keine zweite,
 * parallel gepflegte Katalogabfrage. Wer den Ordner wechselt, sieht
 * sofort dessen Kalender.
 *
 * **Warum kein Katalogfilter beim Tagesklick.** `FilterCriteriaDto`
 * (siehe `lib/tauri.ts`) kennt Bewertung, Flagge, Farbe und Kamera —
 * kein Datum. Ein Tagesklick füllt deshalb die vorhandene
 * Mehrfachauswahl (`setMultiSelection`) und zeigt den Tag im Panel
 * darunter, statt so zu tun, als gäbe es einen Datumsfilter im Backend.
 * Die Auswahl ist dieselbe, die Stapel-Bewertung, Export und
 * Sammlung-Hinzufügen ohnehin benutzen — der Tag ist damit sofort
 * weiterverarbeitbar.
 */

const WEEKDAYS = ["Mo", "Di", "Mi", "Do", "Fr", "Sa", "So"];

/** Hintergrund je Dichtestufe (siehe `densityLevel`). Bewusst über die
 * Akzentfarbe gestuft statt über eine feste Farbskala — die Akzentfarbe
 * ist in den Einstellungen frei wählbar (`App.tsx` setzt `--color-accent`),
 * eine hart kodierte grüne Skala würde daran vorbeilaufen. */
const DENSITY_CLASS: Record<0 | 1 | 2 | 3 | 4, string> = {
  0: "bg-bg-raised text-text-muted",
  1: "bg-accent/15 text-text-primary",
  2: "bg-accent/35 text-text-primary",
  3: "bg-accent/60 text-text-primary",
  4: "bg-accent/85 text-bg-base",
};

function formatDayLabel(day: string): string {
  const [year, month, dayOfMonth] = day.split("-").map(Number);
  const date = new Date(year ?? 1970, (month ?? 1) - 1, dayOfMonth ?? 1);
  return date.toLocaleDateString("de-DE", { weekday: "long", day: "numeric", month: "long", year: "numeric" });
}

export function CalendarView() {
  const photos = useAppStore(useShallow(selectActivePhotos));
  const selectPhoto = useAppStore((s) => s.selectPhoto);
  const setMultiSelection = useAppStore((s) => s.setMultiSelection);
  const setCenterView = useAppStore((s) => s.setCenterView);
  const selectedFolderId = useAppStore((s) => s.selectedFolderId);
  const rescanMetadata = useAppStore((s) => s.rescanMetadata);
  const metadataRescanBusy = useAppStore((s) => s.metadataRescanBusy);
  const metadataRescanResult = useAppStore((s) => s.metadataRescanResult);

  const [selectedDay, setSelectedDay] = useState<string | null>(null);
  const [hideEmptyMonths, setHideEmptyMonths] = useState(false);
  const scrollRef = useRef<HTMLElement | null>(null);
  const yearAnchors = useRef(new Map<number, HTMLElement>());

  const { months, maxPerDay, totalDated, undated, dayCount } = useMemo(() => {
    const days = countByDay(photos.map((p) => p.captured_at));
    const built = buildCalendar(days);
    const dated = days.reduce((sum, entry) => sum + entry.count, 0);
    return {
      months: built,
      maxPerDay: days.reduce((max, entry) => Math.max(max, entry.count), 0),
      totalDated: dated,
      undated: photos.length - dated,
      dayCount: days.length,
    };
  }, [photos]);

  const visibleMonths = useMemo(
    () => (hideEmptyMonths ? months.filter((month) => month.total > 0) : months),
    [months, hideEmptyMonths],
  );

  const years = useMemo(() => [...new Set(visibleMonths.map((month) => month.year))], [visibleMonths]);

  const busiestDay = useMemo(() => {
    let best: { day: string; count: number } | null = null;
    for (const month of months) {
      for (const cell of month.cells) {
        if (cell.day && cell.count > (best?.count ?? 0)) best = { day: cell.day, count: cell.count };
      }
    }
    return best;
  }, [months]);

  const photosOfDay = useMemo(() => {
    if (!selectedDay) return [];
    return photos.filter((photo) => {
      if (!photo.captured_at) return false;
      const date = new Date(photo.captured_at);
      if (Number.isNaN(date.getTime())) return false;
      const year = date.getFullYear().toString().padStart(4, "0");
      const month = (date.getMonth() + 1).toString().padStart(2, "0");
      const day = date.getDate().toString().padStart(2, "0");
      return `${year}-${month}-${day}` === selectedDay;
    });
  }, [photos, selectedDay]);

  // Ein Tag, der durch einen Ordnerwechsel aus der Liste gefallen ist,
  // bleibt sonst als leeres Panel stehen.
  useEffect(() => {
    if (selectedDay && photosOfDay.length === 0) setSelectedDay(null);
  }, [selectedDay, photosOfDay.length]);

  /** Alle belegten Tage in Reihenfolge — Grundlage für die Pfeiltasten
   * und die Vor-/Zurück-Knöpfe im Tagespanel. Über die Monate hinweg,
   * nicht je Monat: der Sprung vom 31. Januar auf den 3. Februar ist
   * genau der, den man beim Durchblättern will. */
  const occupiedDays = useMemo(() => {
    const list: string[] = [];
    for (const month of months) {
      for (const cell of month.cells) {
        if (cell.day && cell.count > 0) list.push(cell.day);
      }
    }
    return list;
  }, [months]);

  function stepDay(delta: number) {
    if (occupiedDays.length === 0) return;
    const index = selectedDay ? occupiedDays.indexOf(selectedDay) : -1;
    const next = index === -1 ? (delta > 0 ? 0 : occupiedDays.length - 1) : index + delta;
    if (next < 0 || next >= occupiedDays.length) return;
    chooseDay(occupiedDays[next]!);
  }

  function chooseDay(day: string) {
    playCue("select");
    setSelectedDay(day);
  }

  function scrollToYear(year: number) {
    yearAnchors.current.get(year)?.scrollIntoView({ behavior: "smooth", block: "start" });
  }

  function selectWholeDay() {
    if (photosOfDay.length === 0) return;
    setMultiSelection(photosOfDay.map((photo) => photo.id));
    playCue("select");
  }

  function openPhoto(photoId: string) {
    selectPhoto(photoId);
    setCenterView("viewer");
  }

  return (
    // `pt-16` wie in `PeopleView.tsx`: die schwebende Kopfzeile nimmt
    // keinen Platz im Dokumentfluss ein.
    <main ref={scrollRef} data-testid="calendar-view" className="flex flex-1 flex-col overflow-y-auto px-4 pt-16 pb-4">
      <header className="mb-4 flex flex-wrap items-end justify-between gap-3">
        <div>
          <h2 className="flex items-center gap-2 text-base font-semibold text-text-primary">
            <CalendarDays aria-hidden="true" className="size-4" />
            Kalender
          </h2>
          <p data-testid="calendar-summary" className="mt-1 text-xs text-text-secondary">
            {totalDated} {totalDated === 1 ? "Aufnahme" : "Aufnahmen"} an {dayCount} {dayCount === 1 ? "Tag" : "Tagen"}
            {busiestDay ? ` · stärkster Tag: ${formatDayLabel(busiestDay.day)} (${busiestDay.count})` : ""}
          </p>
          {undated > 0 && (
            // Bis Phase 34 las der JPEG/PNG/TIFF-Pfad gar kein
            // `DateTimeOriginal` (siehe `DECISIONS.md` ADR-0068) — fuer
            // einen so aufgebauten Katalog stand hier "alle ohne
            // Aufnahmedatum" ohne jeden Hinweis, was dagegen zu tun
            // waere. Das Nachlesen gehoert genau hierhin, wo der Mangel
            // sichtbar wird, nicht in einen Wartungsdialog, den man erst
            // suchen muss.
            <div className="mt-0.5 flex flex-wrap items-center gap-2 text-xs text-text-muted">
              <span className="flex items-center gap-1">
                <ImageOff aria-hidden="true" className="size-3" />
                {undated} ohne Aufnahmedatum — im Kalender nicht darstellbar
              </span>
              <button
                type="button"
                data-testid="calendar-rescan"
                disabled={metadataRescanBusy}
                onClick={() => {
                  playCue("press");
                  void rescanMetadata(selectedFolderId ?? undefined);
                }}
                className="apx-btn-liquid rounded border border-border px-2 py-0.5 text-[11px] text-text-secondary transition-colors duration-[var(--duration-fast)] hover:border-accent hover:text-text-primary disabled:opacity-50"
              >
                <RefreshCw aria-hidden="true" className={`mr-1 inline size-3 ${metadataRescanBusy ? "animate-spin" : ""}`} />
                {metadataRescanBusy ? "Liest Metadaten…" : "Aufnahmedaten nachlesen"}
              </button>
            </div>
          )}
          {metadataRescanResult && (
            <p data-testid="calendar-rescan-result" className="mt-0.5 text-xs text-text-secondary">
              {metadataRescanResult.dates_added > 0
                ? `${metadataRescanResult.dates_added} ${metadataRescanResult.dates_added === 1 ? "Foto hat" : "Fotos haben"} jetzt ein Aufnahmedatum.`
                : "Kein zusätzliches Aufnahmedatum gefunden — diese Dateien haben keins im EXIF."}
              {metadataRescanResult.unreadable > 0
                ? ` ${metadataRescanResult.unreadable} Datei(en) nicht lesbar.`
                : ""}
            </p>
          )}
        </div>

        <div className="flex flex-wrap items-center gap-3">
          <label className="flex items-center gap-1.5 text-xs text-text-secondary">
            <input
              type="checkbox"
              checked={hideEmptyMonths}
              onChange={(event) => setHideEmptyMonths(event.target.checked)}
              className="accent-accent"
            />
            Leere Monate ausblenden
          </label>

          <div aria-label="Dichte" className="flex items-center gap-1 text-[10px] text-text-muted">
            <span>weniger</span>
            {([0, 1, 2, 3, 4] as const).map((level) => (
              <span key={level} aria-hidden="true" className={`size-3 rounded-[2px] ${DENSITY_CLASS[level]}`} />
            ))}
            <span>mehr</span>
          </div>
        </div>
      </header>

      {years.length > 1 && (
        <nav aria-label="Jahr" data-testid="calendar-years" className="mb-4 flex flex-wrap gap-1">
          {years.map((year) => (
            <button
              key={year}
              type="button"
              onClick={() => scrollToYear(year)}
              className="apx-btn-liquid rounded border border-border px-2 py-0.5 text-xs text-text-secondary transition-colors duration-[var(--duration-fast)] hover:text-text-primary"
            >
              {year}
            </button>
          ))}
        </nav>
      )}

      {visibleMonths.length === 0 ? (
        <p className="text-sm text-text-muted">
          Keine Aufnahme in dieser Auswahl trägt ein Aufnahmedatum — der Kalender bleibt deshalb leer.
        </p>
      ) : (
        <div className="grid grid-cols-[repeat(auto-fill,minmax(230px,1fr))] gap-4">
          {visibleMonths.map((month) => (
            <MonthCard
              key={`${month.year}-${month.month}`}
              month={month}
              maxPerDay={maxPerDay}
              selectedDay={selectedDay}
              onChoose={chooseDay}
              registerYearAnchor={(element) => {
                if (element) yearAnchors.current.set(month.year, element);
              }}
              isFirstOfYear={visibleMonths.find((candidate) => candidate.year === month.year) === month}
            />
          ))}
        </div>
      )}

      {selectedDay && (
        <section
          aria-label={`Aufnahmen am ${formatDayLabel(selectedDay)}`}
          data-testid="calendar-day-panel"
          className="apx-notice-in sticky bottom-0 mt-4 rounded border border-border bg-bg-panel/95 p-3 backdrop-blur"
        >
          <div className="mb-2 flex flex-wrap items-center gap-2">
            <button
              type="button"
              aria-label="Vorheriger Tag mit Aufnahmen"
              onClick={() => stepDay(-1)}
              className="apx-btn-liquid rounded border border-border p-1 text-text-secondary hover:text-text-primary"
            >
              <ChevronLeft aria-hidden="true" className="size-3.5" />
            </button>
            <button
              type="button"
              aria-label="Nächster Tag mit Aufnahmen"
              onClick={() => stepDay(1)}
              className="apx-btn-liquid rounded border border-border p-1 text-text-secondary hover:text-text-primary"
            >
              <ChevronRight aria-hidden="true" className="size-3.5" />
            </button>

            <h3 className="text-sm font-semibold text-text-primary">{formatDayLabel(selectedDay)}</h3>
            <span className="text-xs text-text-secondary">
              {photosOfDay.length} {photosOfDay.length === 1 ? "Aufnahme" : "Aufnahmen"}
            </span>

            <button
              type="button"
              onClick={selectWholeDay}
              className="apx-btn-liquid ml-auto flex items-center gap-1.5 rounded border border-accent bg-accent/10 px-2 py-1 text-xs text-accent"
            >
              <CheckSquare aria-hidden="true" className="size-3.5" />
              Ganzen Tag auswählen
            </button>
            <button
              type="button"
              aria-label="Tagesauswahl schließen"
              onClick={() => setSelectedDay(null)}
              className="apx-btn-liquid rounded border border-border p-1 text-text-secondary hover:text-text-primary"
            >
              <X aria-hidden="true" className="size-3.5" />
            </button>
          </div>

          <ul className="flex gap-2 overflow-x-auto pb-1">
            {photosOfDay.map((photo) => (
              <li key={photo.id}>
                <button
                  type="button"
                  onClick={() => openPhoto(photo.id)}
                  title={photo.filename}
                  className="apx-btn-liquid block size-20 overflow-hidden rounded border border-border"
                >
                  <img src={previewUrl(photo.id)} alt={photo.filename} className="size-full object-cover" />
                </button>
              </li>
            ))}
          </ul>
        </section>
      )}
    </main>
  );
}

interface MonthCardProps {
  month: CalendarMonth;
  maxPerDay: number;
  selectedDay: string | null;
  onChoose: (day: string) => void;
  registerYearAnchor: (element: HTMLElement | null) => void;
  isFirstOfYear: boolean;
}

function MonthCard({ month, maxPerDay, selectedDay, onChoose, registerYearAnchor, isFirstOfYear }: MonthCardProps) {
  const gridRef = useRef<HTMLDivElement | null>(null);

  /** Pfeiltasten bewegen den Fokus im Monatsraster, wie man es von einem
   * Kalender erwartet — links/rechts einen Tag, hoch/runter eine Woche.
   * Die Zellen sind echte `<button>`, `Tab` funktioniert also ohnehin;
   * die Pfeiltasten sparen nur die 30 Tabs bis ans Monatsende. */
  function onKeyDown(event: React.KeyboardEvent<HTMLDivElement>) {
    const steps: Record<string, number> = { ArrowLeft: -1, ArrowRight: 1, ArrowUp: -7, ArrowDown: 7 };
    const step = steps[event.key];
    if (step === undefined) return;
    const cells = [...(gridRef.current?.querySelectorAll<HTMLButtonElement>("button[data-day]") ?? [])];
    const index = cells.findIndex((cell) => cell === document.activeElement);
    if (index === -1) return;
    const target = cells[index + step];
    if (!target) return;
    event.preventDefault();
    target.focus();
  }

  return (
    <section
      ref={isFirstOfYear ? registerYearAnchor : undefined}
      aria-label={month.label}
      data-testid="calendar-month"
      className="rounded border border-border bg-bg-raised p-2"
    >
      <div className="mb-1.5 flex items-baseline justify-between">
        <h3 className="text-xs font-semibold text-text-primary">{month.label}</h3>
        <span className="text-[10px] text-text-muted">{month.total > 0 ? month.total : "—"}</span>
      </div>

      <div aria-hidden="true" className="mb-1 grid grid-cols-7 gap-0.5 text-center text-[10px] text-text-muted">
        {WEEKDAYS.map((weekday) => (
          <span key={weekday}>{weekday}</span>
        ))}
      </div>

      <div ref={gridRef} onKeyDown={onKeyDown} className="grid grid-cols-7 gap-0.5">
        {month.cells.map((cell, index) =>
          cell.day === null ? (
            <span key={`pad-${index}`} aria-hidden="true" className="aspect-square rounded-[3px]" />
          ) : (
            <button
              key={cell.day}
              type="button"
              data-day={cell.day}
              data-count={cell.count}
              disabled={cell.count === 0}
              onClick={() => onChoose(cell.day!)}
              aria-pressed={selectedDay === cell.day}
              aria-label={`${cell.dayOfMonth}. ${month.label}: ${cell.count} ${cell.count === 1 ? "Aufnahme" : "Aufnahmen"}`}
              className={`aspect-square rounded-[3px] text-[10px] leading-none transition-colors duration-[var(--duration-fast)] disabled:cursor-default ${
                DENSITY_CLASS[densityLevel(cell.count, maxPerDay)]
              } ${selectedDay === cell.day ? "ring-2 ring-accent ring-offset-1 ring-offset-bg-raised" : ""}`}
            >
              {cell.dayOfMonth}
            </button>
          ),
        )}
      </div>
    </section>
  );
}
