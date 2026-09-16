import { describe, expect, it } from "vitest";

import { estimateExportSize, scaledSize, totalBytes } from "./exportEstimate";

const SOURCE = { width: 6000, height: 4000 };

describe("scaledSize", () => {
  it("begrenzt die längste Kante und behält das Seitenverhältnis", () => {
    const size = scaledSize(SOURCE, { maxEdge: 3000 });
    expect(size).toEqual({ width: 3000, height: 2000 });
  });

  it("skaliert niemals hoch — eine Grenze ist eine Obergrenze", () => {
    expect(scaledSize({ width: 800, height: 600 }, { maxEdge: 4000 })).toEqual({ width: 800, height: 600 });
  });

  it("wendet beide Grenzen an, die kleinere gewinnt", () => {
    // 24 MP Quelle: maxEdge 3000 ergäbe 6 MP, maxMegapixels 12 ergäbe
    // 12 MP — die Kantengrenze ist strenger und muss gewinnen.
    const size = scaledSize(SOURCE, { maxEdge: 3000, maxMegapixels: 12 });
    expect(size).toEqual({ width: 3000, height: 2000 });
  });

  it("wendet die Megapixel-Grenze an, wenn sie die strengere ist", () => {
    const size = scaledSize(SOURCE, { maxEdge: 6000, maxMegapixels: 6 });
    expect(size.width * size.height).toBeLessThanOrEqual(6_000_000);
    expect(size.width / size.height).toBeCloseTo(1.5, 2);
  });

  it("liefert 0×0 statt NaN für eine Quelle ohne Maße", () => {
    expect(scaledSize({ width: 0, height: 0 }, { maxEdge: 1000 })).toEqual({ width: 0, height: 0 });
  });
});

describe("estimateExportSize", () => {
  it("rechnet unkomprimiertes TIFF exakt", () => {
    const estimate = estimateExportSize({
      source: { width: 1000, height: 1000 },
      limits: {},
      format: "tiff",
      quality: 90,
      bitDepth16: false,
    });
    expect(estimate.exact).toBe(true);
    expect(estimate.bytes).toBe(1000 * 1000 * 3 + 2048);
    expect(estimate.lowBytes).toBe(estimate.highBytes);
  });

  it("verdoppelt TIFF bei 16 Bit je Kanal", () => {
    const common = { source: { width: 1000, height: 1000 }, limits: {}, format: "tiff" as const, quality: 90 };
    const eight = estimateExportSize({ ...common, bitDepth16: false });
    const sixteen = estimateExportSize({ ...common, bitDepth16: true });
    expect(sixteen.bytes - 2048).toBe((eight.bytes - 2048) * 2);
  });

  it("wächst mit der Qualität, und zwar oben herum stärker", () => {
    const at = (quality: number) =>
      estimateExportSize({ source: SOURCE, limits: {}, format: "jpeg", quality, bitDepth16: false }).bytes;
    expect(at(60)).toBeGreaterThan(at(40));
    // Der Sprung von 90 auf 100 ist größer als der von 40 auf 50 —
    // genau die Nichtlinearität, um die es geht.
    expect(at(100) - at(90)).toBeGreaterThan(at(50) - at(40));
  });

  it("gibt AVIF als deutlich kleiner an als JPEG bei gleicher Qualität", () => {
    const common = { source: SOURCE, limits: {}, quality: 85, bitDepth16: false };
    const jpeg = estimateExportSize({ ...common, format: "jpeg" });
    const avif = estimateExportSize({ ...common, format: "avif" });
    expect(avif.bytes).toBeLessThan(jpeg.bytes * 0.6);
  });

  it("weist für verlustbehaftete Formate eine Spanne aus statt einer Scheingenauigkeit", () => {
    const estimate = estimateExportSize({ source: SOURCE, limits: {}, format: "jpeg", quality: 85, bitDepth16: false });
    expect(estimate.exact).toBe(false);
    expect(estimate.lowBytes).toBeLessThan(estimate.bytes);
    expect(estimate.highBytes).toBeGreaterThan(estimate.bytes);
  });

  it("berücksichtigt die Größenbegrenzung", () => {
    const full = estimateExportSize({ source: SOURCE, limits: {}, format: "jpeg", quality: 85, bitDepth16: false });
    const small = estimateExportSize({
      source: SOURCE,
      limits: { maxEdge: 1500 },
      format: "jpeg",
      quality: 85,
      bitDepth16: false,
    });
    expect(small.width).toBe(1500);
    expect(small.bytes).toBeLessThan(full.bytes / 3);
  });

  it("liefert für eine leere Quelle null Byte statt NaN", () => {
    const estimate = estimateExportSize({
      source: { width: 0, height: 0 },
      limits: {},
      format: "jpeg",
      quality: 85,
      bitDepth16: false,
    });
    expect(estimate.bytes).toBe(0);
  });
});

describe("totalBytes", () => {
  it("multipliziert mit der Fotoanzahl", () => {
    const estimate = estimateExportSize({ source: SOURCE, limits: {}, format: "jpeg", quality: 85, bitDepth16: false });
    expect(totalBytes(estimate, 10)).toBe(estimate.bytes * 10);
    expect(totalBytes(estimate, 0)).toBe(0);
  });
});
