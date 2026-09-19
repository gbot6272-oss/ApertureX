import { describe, expect, it } from "vitest";

import {
  DEFAULT_WATERMARK_TEMPLATE,
  percentOfShortEdge,
  placeAtPosition,
  templateToExportOptions,
  tileOrigins,
  type WatermarkTemplate,
} from "./watermarkTemplate";

describe("percentOfShortEdge", () => {
  it("nimmt die kürzere Kante, egal ob Hoch- oder Querformat", () => {
    expect(percentOfShortEdge(4000, 2000, 10)).toBe(200);
    expect(percentOfShortEdge(2000, 4000, 10)).toBe(200);
  });

  it("ergibt auf zwei Exportgrößen denselben Bildanteil", () => {
    const klein = percentOfShortEdge(800, 600, 25) / 600;
    const gross = percentOfShortEdge(8000, 6000, 25) / 6000;
    expect(klein).toBeCloseTo(gross, 6);
  });
});

describe("placeAtPosition", () => {
  it("setzt unten rechts den Rand von beiden Kanten ab", () => {
    const rect = placeAtPosition(100, 100, 20, 10, "bottom_right", 5);
    expect(rect).toEqual({ x: 75, y: 85, width: 20, height: 10 });
  });

  it("zentriert mittig unabhängig vom Rand", () => {
    const rect = placeAtPosition(100, 100, 20, 10, "center", 5);
    expect(rect.x).toBe(40);
    expect(rect.y).toBe(45);
  });

  it("legt oben links genau den Rand an", () => {
    expect(placeAtPosition(100, 100, 20, 10, "top_left", 5)).toMatchObject({ x: 5, y: 5 });
  });
});

describe("tileOrigins", () => {
  it("bedeckt das ganze Bild und ragt an beiden Seiten hinaus", () => {
    const origins = tileOrigins(100, 100, 30, 30, 0);
    const xs = origins.map((o) => o.x);
    expect(Math.min(...xs)).toBeLessThanOrEqual(0);
    expect(Math.max(...xs) + 30).toBeGreaterThanOrEqual(100);
  });

  it("zentriert das Muster — der Überstand ist links wie rechts gleich", () => {
    const origins = tileOrigins(100, 100, 30, 30, 0);
    const xs = origins.map((o) => o.x);
    const links = -Math.min(...xs);
    const rechts = Math.max(...xs) + 30 - 100;
    expect(links).toBeCloseTo(rechts, 6);
  });

  it("hält den Abstand zwischen zwei Kacheln ein", () => {
    const origins = tileOrigins(200, 50, 20, 20, 10);
    const xs = [...new Set(origins.map((o) => o.x))].sort((a, b) => a - b);
    expect(xs[1]! - xs[0]!).toBe(30);
  });

  it("ergibt für eine Kachel ohne Ausdehnung kein Muster", () => {
    expect(tileOrigins(100, 100, 0, 10, 0)).toEqual([]);
  });
});

describe("templateToExportOptions", () => {
  const base: WatermarkTemplate = { ...DEFAULT_WATERMARK_TEMPLATE, fontPath: "/schrift.ttf" };

  it("übersetzt eine Textvorlage samt relativer Platzierung", () => {
    const options = templateToExportOptions(base);
    expect(options.watermarkText).toBe("© Aperture X");
    expect(options.watermarkFontPath).toBe("/schrift.ttf");
    expect(options.watermarkSizePercent).toBe(5);
    expect(options.watermarkMarginPercent).toBe(3);
    expect(options.watermarkImagePath).toBeUndefined();
  });

  it("schickt ohne Schriftdatei gar nichts statt eines halben Auftrags", () => {
    expect(templateToExportOptions({ ...base, fontPath: "" })).toEqual({});
  });

  it("schickt ohne Text ebenfalls nichts", () => {
    expect(templateToExportOptions({ ...base, text: "   " })).toEqual({});
  });

  it("übersetzt eine Bildvorlage ohne Text- und Schriftfelder", () => {
    const options = templateToExportOptions({ ...base, mode: "image", imagePath: "/logo.png" });
    expect(options.watermarkImagePath).toBe("/logo.png");
    expect(options.watermarkText).toBeUndefined();
    expect(options.watermarkFontPath).toBeUndefined();
  });

  it("schickt ohne Bildpfad nichts", () => {
    expect(templateToExportOptions({ ...base, mode: "image", imagePath: "" })).toEqual({});
  });

  it("reicht die Kachelung durch", () => {
    const options = templateToExportOptions({ ...base, tile: true, rotationDegrees: -45 });
    expect(options.watermarkTile).toBe(true);
    expect(options.watermarkRotationDegrees).toBe(-45);
  });
});
