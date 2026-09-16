/**
 * Abschätzung von Ausgabegröße und Dateigröße vor dem Export
 * (Phase 32 F10).
 *
 * **Warum eine Schätzung und keine Messung.** Die exakte Größe steht
 * erst fest, wenn das Bild gerendert und kodiert ist — bei 300
 * ausgewählten Fotos wäre das der ganze Export, nur eben zweimal. Die
 * Ansage vorher soll die Frage beantworten „passt das noch auf die
 * Karte / in den E-Mail-Anhang", und dafür reicht eine Größenordnung mit
 * ehrlich ausgewiesener Unsicherheit.
 *
 * **Woher die Zahlen kommen.** Für verlustfreie, unkomprimierte Formate
 * ist die Rechnung exakt (Pixel × Kanäle × Bytes je Kanal). Für
 * verlustbehaftete Formate steckt die Erfahrung in einer
 * Bits-pro-Pixel-Kurve je Qualitätsstufe; die Werte sind die üblichen
 * Größenordnungen für fotografische Motive. Bei einem Bild mit sehr
 * glatten Flächen (Himmel, Studio-Hintergrund) liegt das Ergebnis
 * darunter, bei feinem Laub darüber — deshalb gibt
 * [`estimateExportSize`] zusätzlich eine Spanne zurück statt einer
 * Zahl, die Genauigkeit vortäuscht.
 */

export type EstimateFormat = "jpeg" | "png" | "tiff" | "webp" | "avif" | "psd" | "jxl";

export interface SourceSize {
  width: number;
  height: number;
}

export interface SizeLimits {
  /** Längste Kante in Pixeln, `undefined` = keine Begrenzung. */
  maxEdge?: number;
  /** Megapixel-Obergrenze, `undefined` = keine Begrenzung. */
  maxMegapixels?: number;
}

export interface EstimateInput {
  source: SourceSize;
  limits: SizeLimits;
  format: EstimateFormat;
  /** 1..100, nur für verlustbehaftete Formate. */
  quality: number;
  /** 16 Bit je Kanal statt 8 — nur PNG/TIFF. */
  bitDepth16: boolean;
}

export interface Estimate {
  width: number;
  height: number;
  megapixels: number;
  /** Erwartete Dateigröße in Byte. */
  bytes: number;
  /** Untere und obere Grenze der Erwartung — siehe Moduldoku. */
  lowBytes: number;
  highBytes: number;
  /** `true`, wenn die Rechnung exakt ist (unkomprimiert). */
  exact: boolean;
}

/**
 * Wendet Kantenlängen- und Megapixel-Grenze an.
 *
 * Beide Grenzen wirken zusammen, die kleinere gewinnt — dieselbe Regel
 * wie im Backend (`apx_export::engine`), das ebenfalls beide anwendet,
 * wenn beide gesetzt sind. Das Seitenverhältnis bleibt erhalten, und
 * kleiner wird nie hochskaliert: eine Grenze ist eine Obergrenze, keine
 * Zielgröße.
 */
export function scaledSize(source: SourceSize, limits: SizeLimits): SourceSize {
  if (source.width <= 0 || source.height <= 0) return { width: 0, height: 0 };
  let factor = 1;

  if (limits.maxEdge && limits.maxEdge > 0) {
    const longest = Math.max(source.width, source.height);
    factor = Math.min(factor, limits.maxEdge / longest);
  }
  if (limits.maxMegapixels && limits.maxMegapixels > 0) {
    const pixels = source.width * source.height;
    const target = limits.maxMegapixels * 1_000_000;
    if (pixels > target) factor = Math.min(factor, Math.sqrt(target / pixels));
  }

  return {
    width: Math.max(1, Math.round(source.width * factor)),
    height: Math.max(1, Math.round(source.height * factor)),
  };
}

/**
 * Bits je Pixel für ein verlustbehaftetes Format bei gegebener Qualität.
 *
 * Die Kurve ist bewusst nicht linear: zwischen Qualität 90 und 100 wächst
 * die Datei stark (das Format speichert zunehmend Rauschen mit), zwischen
 * 40 und 70 kaum. Stützstellen mit linearer Interpolation dazwischen —
 * eine Formel würde eine Genauigkeit vortäuschen, die es nicht gibt.
 */
function bitsPerPixel(format: EstimateFormat, quality: number): number {
  const q = Math.min(100, Math.max(1, quality));
  // Stützstellen: [Qualität, Bits je Pixel] für JPEG.
  const jpeg: Array<[number, number]> = [
    [10, 0.25],
    [40, 0.7],
    [60, 1.1],
    [75, 1.6],
    [85, 2.3],
    [95, 4.0],
    [100, 7.5],
  ];
  const base = interpolate(jpeg, q);
  switch (format) {
    case "jpeg":
      return base;
    // WebP und AVIF erreichen dieselbe wahrgenommene Qualität mit
    // deutlich weniger Daten — die Faktoren sind die gängigen
    // Größenordnungen aus den Formatvergleichen.
    case "webp":
      return base * 0.7;
    case "avif":
      return base * 0.45;
    case "jxl":
      return base * 0.6;
    default:
      return base;
  }
}

function interpolate(points: Array<[number, number]>, x: number): number {
  if (x <= points[0]![0]) return points[0]![1];
  const last = points[points.length - 1]!;
  if (x >= last[0]) return last[1];
  for (let i = 1; i < points.length; i += 1) {
    const [x1, y1] = points[i]!;
    const [x0, y0] = points[i - 1]!;
    if (x <= x1) {
      const t = (x - x0) / (x1 - x0);
      return y0 + (y1 - y0) * t;
    }
  }
  return last[1];
}

/** Kopfdaten/Metadaten, die auch ein winziges Bild mitschleppt. */
const HEADER_BYTES = 2 * 1024;

export function estimateExportSize(input: EstimateInput): Estimate {
  const { width, height } = scaledSize(input.source, input.limits);
  const pixels = width * height;
  const megapixels = pixels / 1_000_000;

  if (pixels === 0) {
    return { width: 0, height: 0, megapixels: 0, bytes: 0, lowBytes: 0, highBytes: 0, exact: true };
  }

  const bytesPerChannel = input.bitDepth16 ? 2 : 1;

  switch (input.format) {
    case "tiff": {
      // Unkomprimiertes RGB — hier ist die Rechnung exakt.
      const bytes = pixels * 3 * bytesPerChannel + HEADER_BYTES;
      return { width, height, megapixels, bytes, lowBytes: bytes, highBytes: bytes, exact: true };
    }
    case "psd": {
      // Photoshop schreibt ein einzelnes, unkomprimiertes Bild plus
      // Vorschau — praktisch dieselbe Rechnung wie TIFF, aber mit
      // Aufschlag, deshalb keine exakte Angabe.
      const bytes = Math.round(pixels * 3 * bytesPerChannel * 1.05) + HEADER_BYTES;
      return { width, height, megapixels, bytes, lowBytes: Math.round(bytes * 0.95), highBytes: Math.round(bytes * 1.2), exact: false };
    }
    case "png": {
      // Verlustfrei komprimiert: bei Fotos bringt PNG typisch 20–40 %
      // gegenüber roh, bei flächigen Bildern deutlich mehr — daher die
      // weite Spanne.
      const raw = pixels * 3 * bytesPerChannel;
      const bytes = Math.round(raw * 0.7) + HEADER_BYTES;
      return { width, height, megapixels, bytes, lowBytes: Math.round(raw * 0.35), highBytes: Math.round(raw * 0.95), exact: false };
    }
    default: {
      const bpp = bitsPerPixel(input.format, input.quality);
      const bytes = Math.round((pixels * bpp) / 8) + HEADER_BYTES;
      // ±40 %: die Bandbreite zwischen glattem Himmel und feinem Laub.
      return {
        width,
        height,
        megapixels,
        bytes,
        lowBytes: Math.round(bytes * 0.6),
        highBytes: Math.round(bytes * 1.4),
        exact: false,
      };
    }
  }
}

/** Summiert eine Schätzung über mehrere gleich große Fotos. */
export function totalBytes(estimate: Estimate, photoCount: number): number {
  return estimate.bytes * Math.max(0, photoCount);
}
