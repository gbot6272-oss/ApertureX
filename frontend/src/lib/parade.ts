/**
 * RGB-Parade (Phase 33 F6) — die drei Kanäle nebeneinander statt
 * übereinander, plus die Auswertung, für die eine Parade eigentlich da
 * ist.
 *
 * **Warum das mehr ist als „dieselbe Wellenform in drei Kästen".** Die
 * Wellenform aus Phase 14 legt Rot, Grün und Blau übereinander und
 * kombiniert sie per Maximum. Wo zwei Kanäle dicht beieinander liegen,
 * verdeckt einer den anderen — genau in dem Bereich, in dem ein
 * Farbstich entsteht. Nebeneinander sieht man dagegen sofort, dass der
 * Blaukanal in den Tiefen zehn Stufen höher anfängt als Rot. Diese
 * Aussage lässt sich aber auch ausrechnen, und das tut dieses Modul:
 * es liest die Schwarz- und Weißpunkte je Kanal und die mittleren
 * Kanalwerte in Tiefen/Mitten/Lichtern aus derselben Wellenform-
 * Datenstruktur und benennt den Stich.
 *
 * **Warum Perzentile statt Minimum und Maximum.** Ein einziger
 * ausgefressener Pixel — ein Sensorfehler, eine Spiegelung, ein
 * Staubkorn — verschiebt ein Minimum oder Maximum auf 0 bzw. 255, und
 * der Schwarzpunkt wäre bei jedem Foto 0. Das 0,5-Prozent-Perzentil
 * ignoriert solche Einzelfälle und meint, wo der Kanal *wirklich*
 * anfängt.
 *
 * Gerechnet wird über die bereits vorhandene [`Waveform`]-Struktur, ohne
 * den Bildpuffer ein zweites Mal zu lesen.
 */

import type { Waveform } from "./waveform";

export type Channel = "r" | "g" | "b";

/** Anteil der Pixel, der unter dem Schwarzpunkt bzw. über dem Weißpunkt
 * liegen darf. 0,5 % ist klein genug, dass eine echte Lichterfläche
 * nicht abgeschnitten wird, und groß genug, dass Einzelpixel keine Rolle
 * spielen. */
export const CLIP_FRACTION = 0.005;

/** Grenzen der drei Tonwertbereiche auf der 0..255-Skala. Die Drittelung
 * ist die übliche Lesart einer Parade („Tiefen, Mitten, Lichter") und
 * bewusst nicht an Zonen oder Gamma angelehnt — hier geht es um den
 * Vergleich der Kanäle *untereinander*, nicht um absolute Helligkeit. */
export const SHADOW_MAX = 85;
export const MIDTONE_MAX = 170;

export interface ChannelLevels {
  /** Wo der Kanal anfängt (0..255). */
  black: number;
  /** Wo er aufhört (0..255). */
  white: number;
  /** Mittlerer Wert in Tiefen/Mitten/Lichtern; `null`, wenn in diesem
   * Bereich keine Pixel liegen — ein erfundener Wert wäre schlimmer als
   * eine Lücke. */
  shadowMean: number | null;
  midMean: number | null;
  highlightMean: number | null;
  /** Mittlerer Wert über das ganze Bild. Immer vorhanden, solange das
   * Bild überhaupt Pixel hat — anders als die drei Bereichsmittel, die
   * leer sein können. Genau dafür ist er da: ein Motiv, das sich in
   * einem einzigen Tonwertbereich abspielt (Studiohintergrund, Himmel,
   * Nachtaufnahme), hätte sonst gar keine ablesbare Aussage. */
  fullMean: number | null;
}

export interface ParadeAnalysis {
  r: ChannelLevels;
  g: ChannelLevels;
  b: ChannelLevels;
}

/** Zählt die Pixel je Wert über alle Spalten hinweg zusammen. */
function valueHistogram(data: Uint32Array, columns: number, rows: number): Uint32Array {
  const totals = new Uint32Array(rows);
  for (let col = 0; col < columns; col += 1) {
    const base = col * rows;
    for (let value = 0; value < rows; value += 1) {
      totals[value] = (totals[value] ?? 0) + (data[base + value] ?? 0);
    }
  }
  return totals;
}

function levelsFrom(totals: Uint32Array): ChannelLevels {
  let total = 0;
  for (let value = 0; value < totals.length; value += 1) total += totals[value] ?? 0;
  if (total === 0) {
    return {
      black: 0,
      white: 0,
      shadowMean: null,
      midMean: null,
      highlightMean: null,
      fullMean: null,
    };
  }

  const threshold = total * CLIP_FRACTION;

  let seen = 0;
  let black = 0;
  for (let value = 0; value < totals.length; value += 1) {
    seen += totals[value] ?? 0;
    if (seen > threshold) {
      black = value;
      break;
    }
  }

  seen = 0;
  let white = totals.length - 1;
  for (let value = totals.length - 1; value >= 0; value -= 1) {
    seen += totals[value] ?? 0;
    if (seen > threshold) {
      white = value;
      break;
    }
  }

  const rangeMean = (from: number, to: number): number | null => {
    let count = 0;
    let sum = 0;
    for (let value = from; value <= to; value += 1) {
      const n = totals[value] ?? 0;
      count += n;
      sum += n * value;
    }
    return count === 0 ? null : sum / count;
  };

  return {
    black,
    white,
    shadowMean: rangeMean(0, SHADOW_MAX),
    midMean: rangeMean(SHADOW_MAX + 1, MIDTONE_MAX),
    highlightMean: rangeMean(MIDTONE_MAX + 1, totals.length - 1),
    fullMean: rangeMean(0, totals.length - 1),
  };
}

export function analyzeParade(waveform: Waveform): ParadeAnalysis {
  const { columns, rows } = waveform;
  return {
    r: levelsFrom(valueHistogram(waveform.r, columns, rows)),
    g: levelsFrom(valueHistogram(waveform.g, columns, rows)),
    b: levelsFrom(valueHistogram(waveform.b, columns, rows)),
  };
}

/** Ab welcher Abweichung zwischen dem höchsten und dem niedrigsten
 * Kanalmittel von einem Farbstich die Rede ist. Fünf Stufen auf 255 sind
 * am Bildschirm gerade noch erkennbar; darunter wäre die Meldung
 * Rauschen. */
export const CAST_THRESHOLD = 5;

export interface CastReading {
  /** `null` = kein nennenswerter Stich. */
  channel: Channel | null;
  /** Abstand zwischen dem stärksten und dem schwächsten Kanal. */
  delta: number;
}

/**
 * Welcher Kanal in einem Tonwertbereich dominiert.
 *
 * Fehlt ein Kanalwert (kein Pixel in diesem Bereich), wird kein Stich
 * gemeldet: aus zwei von drei Kanälen lässt sich kein Farbstich ablesen.
 */
export function readCast(
  values: readonly [number | null, number | null, number | null],
): CastReading {
  if (values.some((value) => value === null)) return { channel: null, delta: 0 };
  const [r, g, b] = values as [number, number, number];
  const max = Math.max(r, g, b);
  const min = Math.min(r, g, b);
  const delta = max - min;
  if (delta < CAST_THRESHOLD) return { channel: null, delta };
  const channel: Channel = max === r ? "r" : max === g ? "g" : "b";
  return { channel, delta };
}

/** Der Stich je Tonwertbereich plus der Gesamtstich — die eigentliche
 * Ausgabe der Parade.
 *
 * `overall` ist nicht das Mittel der drei Bereiche, sondern über alle
 * Pixel gerechnet: ein Motiv, das nur einen Bereich belegt, liefert in
 * den anderen beiden keine Aussage, hat aber sehr wohl einen Farbstich. */
export function readCasts(analysis: ParadeAnalysis): {
  shadows: CastReading;
  midtones: CastReading;
  highlights: CastReading;
  overall: CastReading;
} {
  return {
    overall: readCast([analysis.r.fullMean, analysis.g.fullMean, analysis.b.fullMean]),
    shadows: readCast([analysis.r.shadowMean, analysis.g.shadowMean, analysis.b.shadowMean]),
    midtones: readCast([analysis.r.midMean, analysis.g.midMean, analysis.b.midMean]),
    highlights: readCast([
      analysis.r.highlightMean,
      analysis.g.highlightMean,
      analysis.b.highlightMean,
    ]),
  };
}
