import { describe, expect, it } from "vitest";

import { FULL_CROP, originalToView, pointFromEvent, shortLabel, viewToOriginal } from "./notePins";

describe("originalToView", () => {
  it("lässt Koordinaten ohne Beschnitt unverändert", () => {
    expect(originalToView({ x: 0.25, y: 0.75 }, FULL_CROP)).toEqual({ x: 0.25, y: 0.75 });
  });

  it("rechnet in den Ausschnitt um", () => {
    // Ausschnitt = rechte untere Hälfte. Die Mitte des Originals ist
    // damit die linke obere Ecke des Ausschnitts.
    const crop = { x: 0.5, y: 0.5, width: 0.5, height: 0.5 };
    expect(originalToView({ x: 0.5, y: 0.5 }, crop)).toEqual({ x: 0, y: 0 });
    expect(originalToView({ x: 0.75, y: 0.75 }, crop)).toEqual({ x: 0.5, y: 0.5 });
  });

  it("meldet null statt den Punkt an den Rand zu kleben", () => {
    const crop = { x: 0.5, y: 0.5, width: 0.5, height: 0.5 };
    expect(originalToView({ x: 0.1, y: 0.6 }, crop)).toBeNull();
    expect(originalToView({ x: 0.6, y: 0.1 }, crop)).toBeNull();
  });

  it("überlebt einen entarteten Ausschnitt ohne Division durch null", () => {
    expect(originalToView({ x: 0.5, y: 0.5 }, { x: 0, y: 0, width: 0, height: 1 })).toBeNull();
  });
});

describe("viewToOriginal", () => {
  it("ist die Umkehrung von originalToView", () => {
    const crop = { x: 0.2, y: 0.1, width: 0.6, height: 0.4 };
    const original = { x: 0.5, y: 0.3 };
    const view = originalToView(original, crop);
    expect(view).not.toBeNull();
    const back = viewToOriginal(view!, crop);
    expect(back.x).toBeCloseTo(original.x, 10);
    expect(back.y).toBeCloseTo(original.y, 10);
  });

  it("begrenzt auf das Bild", () => {
    expect(viewToOriginal({ x: 2, y: -1 }, FULL_CROP)).toEqual({ x: 1, y: 0 });
  });
});

describe("pointFromEvent", () => {
  const rect = { left: 100, top: 50, width: 200, height: 100 } as DOMRect;

  it("normiert die Klickposition auf die Elementfläche", () => {
    expect(pointFromEvent(rect, 200, 100)).toEqual({ x: 0.5, y: 0.5 });
    expect(pointFromEvent(rect, 100, 50)).toEqual({ x: 0, y: 0 });
    expect(pointFromEvent(rect, 300, 150)).toEqual({ x: 1, y: 1 });
  });

  it("klemmt einen Klick knapp neben das Element auf den Rand", () => {
    expect(pointFromEvent(rect, 90, 40)).toEqual({ x: 0, y: 0 });
  });

  it("liefert den Ursprung statt NaN, wenn das Element noch keine Größe hat", () => {
    // Kommt beim ersten Rendern vor, bevor das Bild geladen ist.
    expect(pointFromEvent({ left: 0, top: 0, width: 0, height: 0 } as DOMRect, 10, 10)).toEqual({ x: 0, y: 0 });
  });
});

describe("shortLabel", () => {
  it("lässt kurze Texte unverändert", () => {
    expect(shortLabel("Staubfleck")).toBe("Staubfleck");
  });

  it("kürzt lange Texte mit Auslassungszeichen", () => {
    const label = shortLabel("Diese Notiz ist deutlich zu lang für einen Pin", 20);
    expect(label).toHaveLength(20);
    expect(label.endsWith("…")).toBe(true);
  });

  it("macht aus Zeilenumbrüchen einfache Leerzeichen", () => {
    expect(shortLabel("Erste Zeile\n\nZweite")).toBe("Erste Zeile Zweite");
  });
});
