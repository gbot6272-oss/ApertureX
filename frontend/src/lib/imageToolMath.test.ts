import { describe, expect, it } from "vitest";

import {
  isNearHandle,
  normalizedRadiusToScreen,
  normalizedToScreen,
  screenToNormalized,
  type ViewTransform,
} from "./imageToolMath";

/** Ein hineingezoomtes, verschobenes Bild — der Fall, in dem eine
 * falsche Umrechnung überhaupt erst auffällt. */
const zoomed: ViewTransform = {
  origin: { x: -120, y: -60 },
  scale: 2.5,
  imgW: 400,
  imgH: 300,
};

const fitted: ViewTransform = {
  origin: { x: 50, y: 20 },
  scale: 1,
  imgW: 400,
  imgH: 300,
};

describe("Bild-Overlay-Mathematik (Phase 30)", () => {
  it("bildet die Bildecken auf die erwarteten Bildschirmpunkte ab", () => {
    expect(normalizedToScreen(0, 0, fitted)).toEqual({ x: 50, y: 20 });
    expect(normalizedToScreen(1, 1, fitted)).toEqual({ x: 450, y: 320 });
    expect(normalizedToScreen(0.5, 0.5, fitted)).toEqual({ x: 250, y: 170 });
  });

  it("ist in beide Richtungen umkehrbar — auch gezoomt und verschoben", () => {
    for (const view of [fitted, zoomed]) {
      const cases: [number, number][] = [
        [0, 0],
        [1, 1],
        [0.25, 0.75],
        [0.5, 0.5],
        [0.13, 0.87],
      ];
      for (const [u, v] of cases) {
        const screen = normalizedToScreen(u, v, view);
        const back = screenToNormalized(screen.x, screen.y, view);
        expect(back.x).toBeCloseTo(u, 6);
        expect(back.y).toBeCloseTo(v, 6);
      }
    }
  });

  it("klemmt einen Zeiger ausserhalb des Bildes auf den Rand", () => {
    // Beim Ziehen rutscht der Zeiger regelmaessig ueber den Bildrand.
    const far = screenToNormalized(-9999, 9999, zoomed);
    expect(far).toEqual({ x: 0, y: 1 });
  });

  it("misst Radien an der KUERZEREN Bildkante, nicht an der Breite", () => {
    // 400x300 -> kuerzere Kante 300. Ein Radius 0,5 ist also 150 Bildpixel.
    expect(normalizedRadiusToScreen(0.5, fitted)).toBeCloseTo(150, 6);
    // Beim Zoomen skaliert er mit.
    expect(normalizedRadiusToScreen(0.5, zoomed)).toBeCloseTo(375, 6);
  });

  it("erkennt einen Griff nur in seiner Naehe", () => {
    const at = normalizedToScreen(0.5, 0.5, zoomed);
    expect(isNearHandle(at, 0.5, 0.5, zoomed)).toBe(true);
    expect(isNearHandle({ x: at.x + 8, y: at.y }, 0.5, 0.5, zoomed)).toBe(true);
    expect(isNearHandle({ x: at.x + 40, y: at.y }, 0.5, 0.5, zoomed)).toBe(false);
  });

  it("haelt die Treffertoleranz in BILDSCHIRM-Pixeln fest, nicht in Bildpixeln", () => {
    // Derselbe Bildschirmabstand muss bei jedem Zoom gleich bewertet
    // werden — sonst waeren Griffe bei starkem Zoom unanfassbar.
    const a = normalizedToScreen(0.5, 0.5, fitted);
    const b = normalizedToScreen(0.5, 0.5, zoomed);
    expect(isNearHandle({ x: a.x + 10, y: a.y }, 0.5, 0.5, fitted)).toBe(true);
    expect(isNearHandle({ x: b.x + 10, y: b.y }, 0.5, 0.5, zoomed)).toBe(true);
  });
});
