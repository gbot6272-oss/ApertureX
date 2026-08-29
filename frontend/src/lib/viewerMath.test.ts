import { describe, expect, it } from "vitest";

import { clampZoom, computeBaseScale, imageOrigin, nextZoomStep, panForZoomAtCursor, ZOOM_STEPS } from "./viewerMath";

describe("computeBaseScale", () => {
  it("fit: waehlt den kleineren Skalierungsfaktor, damit das ganze Bild sichtbar bleibt", () => {
    // Bild breiter als hoch im Verhaeltnis zum Container -> Breite ist der
    // limitierende Faktor.
    const scale = computeBaseScale("fit", 1000, 500, 2000, 500);
    expect(scale).toBeCloseTo(0.5); // 1000/2000
  });

  it("fill: waehlt den groesseren Skalierungsfaktor, damit der Container komplett bedeckt ist", () => {
    const scale = computeBaseScale("fill", 1000, 500, 2000, 500);
    expect(scale).toBeCloseTo(1); // 500/500
  });

  it("liefert 1 als sicheren Fallback fuer ungueltige Masse", () => {
    expect(computeBaseScale("fit", 0, 500, 100, 100)).toBe(1);
    expect(computeBaseScale("fit", 500, 500, 0, 100)).toBe(1);
  });
});

describe("clampZoom", () => {
  it("begrenzt auf den erlaubten Bereich", () => {
    expect(clampZoom(0.0001)).toBeGreaterThan(0);
    expect(clampZoom(1000)).toBeLessThanOrEqual(32);
    expect(clampZoom(2)).toBe(2);
  });
});

describe("nextZoomStep", () => {
  const fitScale = 0.3;

  it("steigt zur naechsthoeheren festen Stufe", () => {
    expect(nextZoomStep(1, 1, fitScale)).toBe(2);
    expect(nextZoomStep(2, 1, fitScale)).toBe(4);
  });

  it("faellt zur naechstniedrigeren Stufe, inklusive Einpassen-Skalierung", () => {
    expect(nextZoomStep(2, -1, fitScale)).toBe(1);
    expect(nextZoomStep(1, -1, fitScale)).toBe(fitScale);
  });

  it("bleibt am oberen/unteren Ende der Leiter stehen", () => {
    // Unter `noUncheckedIndexedAccess` liefert der Index `| undefined` —
    // hier per Konstruktion unmöglich (ZOOM_STEPS ist nicht leer), daher
    // der Fallback nur zur Typisierung, nie zur eigentlichen Logik.
    const maxStep = ZOOM_STEPS[ZOOM_STEPS.length - 1] ?? fitScale;
    expect(nextZoomStep(maxStep, 1, fitScale)).toBe(maxStep);
    expect(nextZoomStep(fitScale, -1, fitScale)).toBe(fitScale);
  });
});

describe("imageOrigin", () => {
  it("zentriert das Bild ohne Pan-Versatz", () => {
    // Container 200x100, Bild bei scale=1 100x50 -> zentriert bei (50, 25).
    const origin = imageOrigin(200, 100, 100, 50, 1, { x: 0, y: 0 });
    expect(origin).toEqual({ x: 50, y: 25 });
  });

  it("addiert den Pan-Versatz auf die zentrierte Position", () => {
    const origin = imageOrigin(200, 100, 100, 50, 1, { x: 10, y: -5 });
    expect(origin).toEqual({ x: 60, y: 20 });
  });
});

describe("panForZoomAtCursor", () => {
  it("haelt den Bildpunkt unter dem Cursor beim Zoomen fest", () => {
    const containerW = 200;
    const containerH = 100;
    const imgW = 100;
    const imgH = 50;
    const oldScale = 1;
    const newScale = 2;
    const oldPan = { x: 0, y: 0 };

    // Cursor exakt auf der linken oberen Bildecke (Origin bei scale=1: 50,25).
    const cursor = { x: 50, y: 25 };
    const newPan = panForZoomAtCursor(cursor, containerW, containerH, imgW, imgH, oldScale, newScale, oldPan);

    const newOrigin = imageOrigin(containerW, containerH, imgW, imgH, newScale, newPan);
    // Der Bildpunkt (0,0) lag unter dem Cursor und muss es nach dem Zoom
    // immer noch sein.
    expect(newOrigin.x).toBeCloseTo(cursor.x);
    expect(newOrigin.y).toBeCloseTo(cursor.y);
  });

  it("ist ein Nullwechsel, wenn sich die Skalierung nicht aendert", () => {
    const pan = { x: 12, y: -7 };
    const result = panForZoomAtCursor({ x: 30, y: 40 }, 200, 100, 100, 50, 1, 1, pan);
    expect(result.x).toBeCloseTo(pan.x);
    expect(result.y).toBeCloseTo(pan.y);
  });
});
