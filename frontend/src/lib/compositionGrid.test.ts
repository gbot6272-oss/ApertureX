import { describe, expect, it } from "vitest";

import {
  diagonalMethodLines,
  goldenRatioLines,
  goldenSpiralPath,
  orientLine,
  orientationLabel,
  orientPoint,
  spiralSquares,
  thirdsLines,
  type GridOrientation,
} from "./compositionGrid";

describe("orientPoint", () => {
  it("lässt die Lage 0 unverändert", () => {
    expect(orientPoint({ x: 10, y: 20 }, 0)).toEqual({ x: 10, y: 20 });
  });

  it("dreht um 90 Grad gegen den Uhrzeigersinn des Bildschirms", () => {
    // Ecke links oben wandert nach rechts oben.
    expect(orientPoint({ x: 0, y: 0 }, 1)).toEqual({ x: 100, y: 0 });
  });

  it("bringt vier Vierteldrehungen zurück an den Anfang", () => {
    const start = { x: 17, y: 83 };
    let point = start;
    for (let i = 0; i < 4; i += 1) point = orientPoint(point, 1);
    expect(point.x).toBeCloseTo(start.x, 9);
    expect(point.y).toBeCloseTo(start.y, 9);
  });

  it("spiegelt an der senkrechten Achse", () => {
    expect(orientPoint({ x: 30, y: 40 }, 4)).toEqual({ x: 70, y: 40 });
  });

  it("bleibt für jede der acht Lagen im Quadrat", () => {
    const points = [
      { x: 0, y: 0 },
      { x: 100, y: 0 },
      { x: 0, y: 100 },
      { x: 100, y: 100 },
      { x: 38.2, y: 61.8 },
    ];
    for (let orientation = 0 as GridOrientation; orientation < 8; orientation += 1) {
      for (const point of points) {
        const moved = orientPoint(point, orientation as GridOrientation);
        expect(moved.x).toBeGreaterThanOrEqual(-1e-9);
        expect(moved.x).toBeLessThanOrEqual(100 + 1e-9);
        expect(moved.y).toBeGreaterThanOrEqual(-1e-9);
        expect(moved.y).toBeLessThanOrEqual(100 + 1e-9);
      }
    }
  });

  it("erhält beim Drehen die Länge einer Linie", () => {
    const line = { x1: 10, y1: 20, x2: 70, y2: 90 };
    const length = (l: typeof line) => Math.hypot(l.x2 - l.x1, l.y2 - l.y1);
    for (let orientation = 0 as GridOrientation; orientation < 8; orientation += 1) {
      expect(length(orientLine(line, orientation as GridOrientation))).toBeCloseTo(length(line), 9);
    }
  });
});

describe("feste Raster", () => {
  it("legt das Drittelraster auf exakte Drittel", () => {
    const lines = thirdsLines();
    expect(lines).toHaveLength(4);
    expect(lines[0]!.x1).toBeCloseTo(33.333333, 4);
    expect(lines[1]!.x1).toBeCloseTo(66.666666, 4);
  });

  it("legt den Goldenen Schnitt auf 38,2 und 61,8 Prozent", () => {
    const lines = goldenRatioLines();
    expect(lines[0]!.x1).toBeCloseTo(38.1966, 3);
    expect(lines[1]!.x1).toBeCloseTo(61.8034, 3);
  });

  it("zieht bei der Diagonalmethode sechs Linien, nicht zwei", () => {
    // Die alte Fassung zeichnete nur die zwei Ecke-zu-Ecke-Linien und
    // liess damit genau die Schnittpunkte weg, um die es geht.
    expect(diagonalMethodLines()).toHaveLength(6);
  });

  it("lässt bei der Diagonalmethode jede Linie an einem Rand beginnen und enden", () => {
    const onEdge = (v: number) => Math.abs(v) < 1e-9 || Math.abs(v - 100) < 1e-9;
    for (const line of diagonalMethodLines()) {
      expect(onEdge(line.x1) || onEdge(line.y1)).toBe(true);
      expect(onEdge(line.x2) || onEdge(line.y2)).toBe(true);
    }
  });
});

describe("Goldene Spirale", () => {
  it("erzeugt Stützrechtecke, die immer kleiner werden", () => {
    // Geprüft wird die FLÄCHE, nicht die Breite: die Schnittrichtung
    // wechselt mit jedem Schritt, ein senkrechter Schnitt lässt die
    // Breite also unangetastet. Ein erster Entwurf dieses Tests
    // verlangte monoton fallende Breite und schlug deshalb fehl,
    // obwohl die Konstruktion stimmte.
    const squares = spiralSquares(6);
    expect(squares.length).toBeGreaterThan(3);
    const area = (s: { w: number; h: number }) => s.w * s.h;
    for (let i = 1; i < squares.length; i += 1) {
      expect(area(squares[i]!)).toBeLessThan(area(squares[i - 1]!));
    }
  });

  it("hält jedes Stützquadrat innerhalb des Bildes", () => {
    for (const square of spiralSquares(7)) {
      expect(square.x).toBeGreaterThanOrEqual(-1e-6);
      expect(square.y).toBeGreaterThanOrEqual(-1e-6);
      expect(square.x + square.w).toBeLessThanOrEqual(100 + 1e-6);
      expect(square.y + square.h).toBeLessThanOrEqual(100 + 1e-6);
    }
  });

  it("liefert einen Pfad aus echten Bögen, nicht aus Rechtecken", () => {
    // Der eigentliche Punkt dieser Funktion: die alte Fassung zeichnete
    // ein Kästchengerüst, hier müssen Kreisbögen stehen.
    const path = goldenSpiralPath(0);
    expect(path.startsWith("M ")).toBe(true);
    expect((path.match(/A /g) ?? []).length).toBeGreaterThanOrEqual(4);
    expect(path).not.toContain("L ");
  });

  it("dreht sich mit der Lage tatsächlich mit", () => {
    expect(goldenSpiralPath(0)).not.toBe(goldenSpiralPath(1));
    expect(goldenSpiralPath(0)).not.toBe(goldenSpiralPath(4));
  });

  it("läuft gespiegelt andersherum", () => {
    // `sweep-flag` muss kippen, sonst zeigte die Spirale gespiegelt in
    // die falsche Richtung.
    expect(goldenSpiralPath(0)).toContain(" 0 0 1 ");
    expect(goldenSpiralPath(4)).toContain(" 0 0 0 ");
  });

  it("enthält keine unbrauchbaren Zahlen", () => {
    for (let orientation = 0 as GridOrientation; orientation < 8; orientation += 1) {
      const path = goldenSpiralPath(orientation as GridOrientation);
      expect(path).not.toMatch(/NaN|Infinity|undefined/);
    }
  });
});

describe("orientationLabel", () => {
  it("benennt Drehung und Spiegelung verständlich", () => {
    expect(orientationLabel(0)).toBe("0°");
    expect(orientationLabel(2)).toBe("180°");
    expect(orientationLabel(5)).toBe("90° gespiegelt");
  });
});
