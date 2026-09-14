/**
 * Die fünf Tonwertzonen des Histogramms und welcher Regler zu ihnen
 * gehört (Phase 32 F2, siehe `DECISIONS.md`).
 *
 * Das Histogramm war bisher reine Anzeige. Im Ziehen liegt aber der
 * schnellste Weg zu einer Tonwertkorrektur: man sieht, wo die Werte
 * kleben, und schiebt genau dort — statt erst zu lesen, dann in die
 * Reglerliste zu greifen und den passenden Namen zu suchen.
 *
 * Die Zonengrenzen folgen der üblichen Aufteilung: die äußeren Zonen
 * sind schmal (sie treffen nur die Endpunkte), die Belichtung nimmt die
 * breite Mitte. Sie sind bewusst NICHT gleich breit — ein gleichmäßiges
 * Fünftel gäbe „Weiß" so viel Fläche wie der Belichtung, obwohl der
 * Weißpunkt nur die obersten Werte betrifft.
 */

/** Die Reglerfelder, die von hier aus bedient werden. */
export type HistogramZoneField = "blacks" | "shadows" | "exposure_ev" | "highlights" | "whites";

export interface HistogramZone {
  field: HistogramZoneField;
  label: string;
  /** Anteil der Histogrammbreite, links beginnend (0…1). */
  start: number;
  end: number;
  /**
   * Wie viel Reglerwert eine ganze Histogrammbreite ergibt.
   *
   * Die Belichtung rechnet in Blendenstufen und braucht deshalb eine
   * ganz andere Skala als die vier 0…100-Regler — ohne eigene Skala
   * wäre sie beim Ziehen entweder unbrauchbar träge oder unsteuerbar
   * sprunghaft.
   */
  unitsPerWidth: number;
}

export const HISTOGRAM_ZONES: readonly HistogramZone[] = [
  { field: "blacks", label: "Schwarz", start: 0, end: 0.12, unitsPerWidth: 200 },
  { field: "shadows", label: "Tiefen", start: 0.12, end: 0.35, unitsPerWidth: 200 },
  { field: "exposure_ev", label: "Belichtung", start: 0.35, end: 0.65, unitsPerWidth: 8 },
  { field: "highlights", label: "Lichter", start: 0.65, end: 0.88, unitsPerWidth: 200 },
  { field: "whites", label: "Weiß", start: 0.88, end: 1, unitsPerWidth: 200 },
];

/**
 * Welche Zone liegt an dieser waagerechten Position?
 *
 * `fraction` ist die Position im Histogramm (0 = linker Rand, 1 =
 * rechter). Außerhalb wird geklemmt statt `null` geliefert: wer knapp
 * neben das Histogramm greift, meint erkennbar die Randzone, und ein
 * ins Leere laufender Griff wäre nur ärgerlich.
 */
export function zoneAt(fraction: number): HistogramZone {
  const clamped = Math.min(1, Math.max(0, fraction));
  for (const zone of HISTOGRAM_ZONES) {
    if (clamped >= zone.start && clamped < zone.end) return zone;
  }
  // Genau 1.0 fällt aus jeder halboffenen Spanne heraus.
  return HISTOGRAM_ZONES[HISTOGRAM_ZONES.length - 1]!;
}

/**
 * Wertänderung aus einer waagerechten Zeigerbewegung.
 *
 * `deltaPixels` ist die zurückgelegte Strecke, `width` die
 * Histogrammbreite. Nach rechts ziehen hellt auf, nach links ab —
 * dieselbe Richtung wie bei jedem Regler der App.
 */
export function dragDelta(zone: HistogramZone, deltaPixels: number, width: number): number {
  if (width <= 0) return 0;
  return (deltaPixels / width) * zone.unitsPerWidth;
}
