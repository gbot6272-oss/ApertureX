/**
 * Differenzbild zweier Aufnahmen (Phase 34 F3, siehe `DECISIONS.md`
 * ADR-0070).
 *
 * **Wozu.** Zwei virtuelle Kopien nebeneinander sehen fast gleich aus —
 * was genau unterscheidet sie? Dieselbe Frage bei zwei Aufnahmen
 * derselben Szene: hat sich der Ast bewegt, hat jemand geblinzelt? Das
 * Auge im Nebeneinander-Vergleich ist dafür schlecht; ein verstärktes
 * Differenzbild beantwortet es sofort.
 *
 * **Warum Verstärkung nötig ist.** Die interessanten Unterschiede liegen
 * oft bei 1–3 von 255. Ungestreckt wäre das Differenzbild schwarz und
 * damit nutzlos. Der Faktor streckt sie in den sichtbaren Bereich; was
 * danach über 255 liegt, wird abgeschnitten — das ist richtig so, denn
 * jenseits davon ist die Aussage ohnehin nur noch „hier ist viel
 * anders".
 *
 * **Warum die Helligkeits-Betriebsart.** Bei einer reinen
 * Belichtungsänderung unterscheiden sich alle drei Kanäle gleichmäßig;
 * das kanalweise Differenzbild ist dann grau und sagt wenig. Die
 * Luma-Betriebsart zeigt stattdessen die Struktur des Unterschieds. Die
 * kanalweise Betriebsart bleibt daneben, weil sie einen reinen
 * Farbstich sichtbar macht, den Luma gerade wegmittelt.
 */

export type DifferenceMode = "channels" | "luma";

export interface DifferenceOptions {
  /** Streckung der Differenz, >= 1. */
  amplify: number;
  mode: DifferenceMode;
}

export const DEFAULT_AMPLIFY = 8;
export const MAX_AMPLIFY = 64;

/** Rec.709 — dieselben Gewichte wie in `exposure_match.rs`. */
function luma(r: number, g: number, b: number): number {
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}

function clampByte(value: number): number {
  if (value < 0) return 0;
  if (value > 255) return 255;
  return Math.round(value);
}

/**
 * Baut das verstärkte Differenzbild von `a` und `b` (beide RGBA8,
 * gleiche Pixelzahl).
 *
 * Gibt `null` zurück, wenn die Puffer unterschiedlich groß sind: zwei
 * verschieden große Bilder pixelweise zu vergleichen ergäbe keinen Sinn,
 * und ein stillschweigendes Zuschneiden würde einen Unterschied
 * vortäuschen, der nur aus der Verschiebung stammt.
 *
 * Der Alphakanal des Ergebnisses ist immer undurchsichtig — ein
 * Differenzbild ist eine Aussage über jedes Pixel, auch über die, an
 * denen eines der beiden Bilder durchsichtig war.
 */
export function differenceImage(
  a: Uint8ClampedArray | Uint8Array,
  b: Uint8ClampedArray | Uint8Array,
  options: DifferenceOptions,
): Uint8ClampedArray | null {
  if (a.length !== b.length || a.length % 4 !== 0) return null;
  const amplify = Math.max(1, options.amplify);
  const out = new Uint8ClampedArray(a.length);
  for (let i = 0; i < a.length; i += 4) {
    if (options.mode === "luma") {
      const diff = Math.abs(luma(a[i]!, a[i + 1]!, a[i + 2]!) - luma(b[i]!, b[i + 1]!, b[i + 2]!));
      const value = clampByte(diff * amplify);
      out[i] = value;
      out[i + 1] = value;
      out[i + 2] = value;
    } else {
      out[i] = clampByte(Math.abs(a[i]! - b[i]!) * amplify);
      out[i + 1] = clampByte(Math.abs(a[i + 1]! - b[i + 1]!) * amplify);
      out[i + 2] = clampByte(Math.abs(a[i + 2]! - b[i + 2]!) * amplify);
    }
    out[i + 3] = 255;
  }
  return out;
}

/**
 * Wie stark unterscheiden sich die beiden Bilder insgesamt? 0 = keine
 * Abweichung, 1 = maximal.
 *
 * Bewusst der Mittelwert der UNVERSTÄRKTEN Differenz: die Verstärkung
 * ist eine Sichthilfe, keine Messung. Eine Zahl, die sich mit dem Regler
 * ändert, wäre keine Aussage über die Bilder.
 */
export function differenceAmount(
  a: Uint8ClampedArray | Uint8Array,
  b: Uint8ClampedArray | Uint8Array,
): number | null {
  if (a.length !== b.length || a.length % 4 !== 0 || a.length === 0) return null;
  let sum = 0;
  for (let i = 0; i < a.length; i += 4) {
    sum += Math.abs(a[i]! - b[i]!) + Math.abs(a[i + 1]! - b[i + 1]!) + Math.abs(a[i + 2]! - b[i + 2]!);
  }
  const pixels = a.length / 4;
  return sum / (pixels * 3 * 255);
}
