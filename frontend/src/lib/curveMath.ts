import type { CurveChannel, CurvePoint } from "./edl";

/**
 * Spiegelt `crates/apx-pipeline/src/stages/curves.rs`s Fritsch-Carlson-
 * monotone-kubische-Spline — nur für die Live-Vorschau im
 * `CurveEditor`. Maßgeblich für die tatsächlich gerenderten Pixel ist
 * ausschließlich die Rust-Seite (siehe `curves.rs`s Moduldoku); diese
 * TS-Fassung dient nur der Kurvendarstellung im Editor selbst.
 *
 * Die `at*`-Hilfsfunktionen unten geben unter `noUncheckedIndexedAccess`
 * einen Ersatzwert zurück, statt `undefined` zuzulassen — bei korrekter
 * Nutzung (mindestens zwei Punkte, Schleifen innerhalb ihrer eigenen
 * Länge) werden diese Rückfälle nie tatsächlich gebraucht, halten den
 * Code aber ohne Behauptungs-Operatoren (`!`) typsicher.
 */
function clamp01(value: number): number {
  return Math.min(1, Math.max(0, value));
}

function atNumber(values: number[], index: number): number {
  return values[index] ?? 0;
}

function atPoint(points: CurvePoint[], index: number): CurvePoint {
  return points[index] ?? { input: 0, output: 0 };
}

export function evaluatePointsCurve(points: readonly CurvePoint[], x: number): number {
  const sorted = [...points].sort((a, b) => a.input - b.input);
  const unique: CurvePoint[] = [];
  for (const point of sorted) {
    const last = unique[unique.length - 1];
    if (last && Math.abs(last.input - point.input) < 1e-6) {
      unique[unique.length - 1] = point;
    } else {
      unique.push(point);
    }
  }
  if (unique.length < 2) return clamp01(x);

  const n = unique.length;
  const secants: number[] = [];
  for (let i = 0; i < n - 1; i++) {
    const a = atPoint(unique, i);
    const b = atPoint(unique, i + 1);
    const dx = Math.max(b.input - a.input, 1e-6);
    secants.push((b.output - a.output) / dx);
  }

  const tangents: number[] = new Array(n).fill(0) as number[];
  tangents[0] = atNumber(secants, 0);
  tangents[n - 1] = atNumber(secants, n - 2);
  for (let i = 1; i < n - 1; i++) {
    const s0 = atNumber(secants, i - 1);
    const s1 = atNumber(secants, i);
    tangents[i] = s0 * s1 <= 0 ? 0 : (s0 + s1) / 2;
  }
  for (let i = 0; i < n - 1; i++) {
    const s = atNumber(secants, i);
    if (s === 0) {
      tangents[i] = 0;
      tangents[i + 1] = 0;
      continue;
    }
    if (atNumber(tangents, i) / s < 0) tangents[i] = 0;
    if (atNumber(tangents, i + 1) / s < 0) tangents[i + 1] = 0;
    const alpha = atNumber(tangents, i) / s;
    const beta = atNumber(tangents, i + 1) / s;
    const sumSq = alpha * alpha + beta * beta;
    if (sumSq > 9) {
      const tau = 3 / Math.sqrt(sumSq);
      tangents[i] = tau * alpha * s;
      tangents[i + 1] = tau * beta * s;
    }
  }

  const first = atPoint(unique, 0);
  const last = atPoint(unique, n - 1);
  if (x <= first.input) return clamp01(first.output);
  if (x >= last.input) return clamp01(last.output);

  let segment = 0;
  for (let i = 0; i < n - 1; i++) {
    const a = atPoint(unique, i);
    const b = atPoint(unique, i + 1);
    if (x >= a.input && x <= b.input) {
      segment = i;
      break;
    }
  }
  const p0 = atPoint(unique, segment);
  const p1 = atPoint(unique, segment + 1);
  const h = Math.max(p1.input - p0.input, 1e-6);
  const t = (x - p0.input) / h;
  const h00 = 2 * t ** 3 - 3 * t ** 2 + 1;
  const h10 = t ** 3 - 2 * t ** 2 + t;
  const h01 = -2 * t ** 3 + 3 * t ** 2;
  const h11 = t ** 3 - t ** 2;
  return clamp01(h00 * p0.output + h10 * h * atNumber(tangents, segment) + h01 * p1.output + h11 * h * atNumber(tangents, segment + 1));
}

const PARAMETRIC_SIGMA = 0.25;

/** Spiegelt `curves.rs`s `build_parametric_lut` (dieselbe Gauß-Gewichtung
 * um vier feste Tonwertzonen, siehe dort für die Begründung). */
export function evaluateParametricCurve(shadows: number, darks: number, lights: number, highlights: number, x: number): number {
  const regions: Array<[number, number]> = [
    [0, shadows],
    [1 / 3, darks],
    [2 / 3, lights],
    [1, highlights],
  ];
  let delta = 0;
  for (const [center, amount] of regions) {
    const d = x - center;
    const weight = Math.exp(-(d * d) / (2 * PARAMETRIC_SIGMA * PARAMETRIC_SIGMA));
    delta += (amount / 100) * weight * 0.3;
  }
  return clamp01(x + delta);
}

export function evaluateCurveChannel(channel: CurveChannel, x: number): number {
  if (channel.kind === "Points") return evaluatePointsCurve(channel.points, x);
  return evaluateParametricCurve(channel.shadows, channel.darks, channel.lights, channel.highlights, x);
}
