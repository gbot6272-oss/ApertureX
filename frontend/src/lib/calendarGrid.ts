/**
 * Aufnahmen nach Tagen zählen und zu Monatsrastern anordnen
 * (Phase 32 F3, siehe `DECISIONS.md`).
 *
 * **Warum eine eigene Ansicht.** Der Katalog liess sich bisher nach
 * Ordner, Sammlung, Person, Ort und Regelwerk durchsehen — aber nicht
 * nach Zeit, obwohl das Aufnahmedatum die Eigenschaft ist, die jedes
 * Foto hat und nach der man sich am ehesten erinnert („der Ausflug im
 * letzten Mai").
 *
 * **Warum die lokale Zeitzone.** Das Aufnahmedatum kommt als
 * RFC-3339-Zeitstempel mit Zonenangabe. Nach UTC gruppiert landete eine
 * Abendaufnahme aus Mitteleuropa im Sommer schon am Folgetag — für den
 * Fotografen war es aber derselbe Abend. Gruppiert wird deshalb nach
 * dem Kalendertag in der Zeitzone des Rechners.
 */

export interface DayCount {
  /** `JJJJ-MM-TT` in lokaler Zeit. */
  day: string;
  count: number;
}

export interface CalendarCell {
  /** `null` = Füllzelle vor dem Monatsersten bzw. nach dem Letzten. */
  day: string | null;
  dayOfMonth: number | null;
  count: number;
}

export interface CalendarMonth {
  year: number;
  /** 1–12. */
  month: number;
  label: string;
  /** Immer ein Vielfaches von sieben, montagsbeginnend. */
  cells: CalendarCell[];
  total: number;
}

const MONTH_NAMES = [
  "Januar", "Februar", "März", "April", "Mai", "Juni",
  "Juli", "August", "September", "Oktober", "November", "Dezember",
];

/** `JJJJ-MM-TT` eines Datums in LOKALER Zeit (siehe Moduldoku). */
export function localDayKey(date: Date): string {
  const year = date.getFullYear().toString().padStart(4, "0");
  const month = (date.getMonth() + 1).toString().padStart(2, "0");
  const day = date.getDate().toString().padStart(2, "0");
  return `${year}-${month}-${day}`;
}

/**
 * Zählt Aufnahmen je Kalendertag.
 *
 * Fotos ohne Aufnahmedatum werden übersprungen statt auf „heute"
 * geraten — ein erfundenes Datum wäre im Kalender nicht als solches
 * erkennbar und würde den Tag verfälschen, an dem man tatsächlich
 * fotografiert hat.
 */
export function countByDay(capturedAt: ReadonlyArray<string | null | undefined>): DayCount[] {
  const counts = new Map<string, number>();
  for (const stamp of capturedAt) {
    if (!stamp) continue;
    const date = new Date(stamp);
    if (Number.isNaN(date.getTime())) continue;
    const key = localDayKey(date);
    counts.set(key, (counts.get(key) ?? 0) + 1);
  }
  return [...counts.entries()]
    .map(([day, count]) => ({ day, count }))
    .sort((a, b) => a.day.localeCompare(b.day));
}

/**
 * Baut das Raster eines Monats.
 *
 * Die Woche beginnt am Montag — die in Deutschland übliche Zählung.
 * `Date.getDay()` liefert Sonntag als 0, deshalb die Verschiebung.
 */
export function buildMonth(year: number, month: number, counts: ReadonlyMap<string, number>): CalendarMonth {
  const first = new Date(year, month - 1, 1);
  const daysInMonth = new Date(year, month, 0).getDate();
  const leading = (first.getDay() + 6) % 7;

  const cells: CalendarCell[] = [];
  for (let i = 0; i < leading; i += 1) cells.push({ day: null, dayOfMonth: null, count: 0 });

  let total = 0;
  for (let dayOfMonth = 1; dayOfMonth <= daysInMonth; dayOfMonth += 1) {
    const key = localDayKey(new Date(year, month - 1, dayOfMonth));
    const count = counts.get(key) ?? 0;
    total += count;
    cells.push({ day: key, dayOfMonth, count });
  }
  // Auf volle Wochen auffüllen, damit das Raster nicht ausfranst.
  while (cells.length % 7 !== 0) cells.push({ day: null, dayOfMonth: null, count: 0 });

  return { year, month, label: `${MONTH_NAMES[month - 1]} ${year}`, cells, total };
}

/**
 * Alle Monate zwischen dem ersten und dem letzten Aufnahmetag — auch
 * die leeren dazwischen.
 *
 * Lücken wegzulassen wäre irreführend: ein Kalender, der von März direkt
 * auf September springt, sieht aus wie ein lückenloser Zeitraum. Die
 * leeren Monate ZU zeigen macht die Pause sichtbar, und genau die ist
 * oft die Information.
 */
export function buildCalendar(days: readonly DayCount[]): CalendarMonth[] {
  if (days.length === 0) return [];
  const counts = new Map(days.map((entry) => [entry.day, entry.count]));

  const parse = (key: string) => {
    const [year, month] = key.split("-").map(Number);
    return { year: year ?? 1970, month: month ?? 1 };
  };
  const from = parse(days[0]!.day);
  const to = parse(days[days.length - 1]!.day);

  const months: CalendarMonth[] = [];
  let { year, month } = from;
  // Obergrenze gegen kaputte Daten: ein Foto mit dem Jahr 1899 und eines
  // mit 2999 würde sonst 13 000 Monate erzeugen und die Ansicht
  // einfrieren.
  for (let guard = 0; guard < 1200; guard += 1) {
    months.push(buildMonth(year, month, counts));
    if (year === to.year && month === to.month) break;
    month += 1;
    if (month > 12) {
      month = 1;
      year += 1;
    }
  }
  return months;
}

/**
 * Farbstufe einer Tageszelle (0–4), relativ zum stärksten Tag.
 *
 * Relativ statt absolut: wer an einem Tag 12 Fotos macht, soll denselben
 * Kontrast sehen wie jemand mit 1200 an seinem stärksten Tag.
 */
export function densityLevel(count: number, max: number): 0 | 1 | 2 | 3 | 4 {
  if (count <= 0 || max <= 0) return 0;
  const ratio = count / max;
  if (ratio > 0.66) return 4;
  if (ratio > 0.33) return 3;
  if (ratio > 0.1) return 2;
  return 1;
}
